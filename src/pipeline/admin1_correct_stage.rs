//! admin1 修正器在 translate 階段的編排：載入輸入、逐國執行、寫出紀錄與 log。
//!
//! 只作用於非 handler 國家——handler 國家（TW/JP/KR/TH/ID）的行政區由官方圖資
//! 直接寫入 cities500，不經 LocationIQ metadata，自然也不會進入這裡。

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fs;
use std::path::Path;

use crate::pipeline::admin1_apply::{AdminRow, CorrectionOutcome, apply_corrections};
use crate::pipeline::admin1_correct::{
    CityPoint, PointSample, Verdict, evaluate_points, learn_admin1_mapping,
};
use crate::pipeline::admin1_report::{fixes_csv_path, report_lines, write_fixes_csv};
use crate::pipeline::ne_admin1::NeAdmin1Index;

/// cities500 的欄位索引。
const COLUMN_GEONAME_ID: usize = 0;
const COLUMN_NAME: usize = 1;
const COLUMN_LATITUDE: usize = 4;
const COLUMN_LONGITUDE: usize = 5;
const COLUMN_COUNTRY_CODE: usize = 8;
const COLUMN_ADMIN1: usize = 10;
const COLUMN_ADMIN2: usize = 11;

/// LocationIQ metadata 的 `(國碼, 緯度字串, 經度字串) → 一級行政區名稱`。
///
/// Reason: 鍵沿用 `MetadataLookup` 既有的字串三元組，與 cities500 的欄位逐字
/// 比對。改用浮點數比較會在格式化差異上無聲失配，產生零個候選卻不報錯。
pub type MetadataAdmin1 = HashMap<(String, String, String), String>;

/// 執行 admin1 修正，就地改寫 `cities_rows`。
///
/// `natural_earth_file` 不存在時略過並記錄，不視為錯誤——NE admin-1 圖資由
/// prepare 下載，尚未下載時 translate 仍應能跑完其餘工作。
pub fn run(
    cities_rows: &mut [Vec<String>],
    admin1_rows: &[Vec<String>],
    metadata_admin1: &MetadataAdmin1,
    natural_earth_file: &Path,
    admin2_codes_file: &Path,
    fixes_dir: &Path,
) -> Result<(), String> {
    if metadata_admin1.is_empty() {
        println!("stage=translate admin1_correct skipped=no_metadata");
        return Ok(());
    }
    if !natural_earth_file.exists() {
        println!(
            "stage=translate admin1_correct skipped=natural_earth_missing path={}",
            natural_earth_file.display()
        );
        return Ok(());
    }

    let index = NeAdmin1Index::load(natural_earth_file)?;
    let (gn_id_to_code, known_admin1_codes) = admin1_tables(admin1_rows);
    let admin2_keys = load_admin2_keys(admin2_codes_file)?;

    for country_code in countries_in(metadata_admin1) {
        run_country(
            &country_code,
            cities_rows,
            &index,
            &gn_id_to_code,
            &known_admin1_codes,
            &admin2_keys,
            metadata_admin1,
            fixes_dir,
        )?;
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn run_country(
    country_code: &str,
    cities_rows: &mut [Vec<String>],
    index: &NeAdmin1Index,
    gn_id_to_code: &BTreeMap<i64, String>,
    known_admin1_codes: &BTreeMap<String, String>,
    admin2_keys: &BTreeSet<String>,
    metadata_admin1: &MetadataAdmin1,
    fixes_dir: &Path,
) -> Result<(), String> {
    let samples = samples_for(country_code, metadata_admin1);
    let mapping = learn_admin1_mapping(&samples, index, gn_id_to_code);

    // Reason: 一併要求欄位寬度足以容納 admin2。`admin_row` 與回寫端都以索引直接
    // 取用第 10／11 欄，只檢查國碼（第 8 欄）的話，欄位不足的列會讓 translate
    // 以 index out of bounds panic，而不是留下可讀的錯誤。
    let positions: Vec<usize> = cities_rows
        .iter()
        .enumerate()
        .filter(|(_, row)| {
            row.get(COLUMN_COUNTRY_CODE).map(String::as_str) == Some(country_code)
                && row.len() > COLUMN_ADMIN2
        })
        .map(|(position, _)| position)
        .collect();
    let points: Vec<CityPoint> = positions
        .iter()
        .filter_map(|position| city_point(&cities_rows[*position], metadata_admin1))
        .collect();

    let candidates = evaluate_points(&points, index, &mapping, gn_id_to_code, known_admin1_codes);
    for line in report_lines(country_code, &candidates, &mapping, samples.len()) {
        println!("{line}");
    }
    write_fixes_csv(&fixes_csv_path(fixes_dir, country_code), &candidates)?;

    let corrections: Vec<(String, String)> = candidates
        .iter()
        .filter(|candidate| candidate.verdict == Verdict::Accepted)
        .filter_map(|candidate| {
            candidate
                .corrected_admin1
                .clone()
                .map(|code| (candidate.geoname_id.clone(), code))
        })
        .collect();
    if corrections.is_empty() {
        return Ok(());
    }

    let mut admin_rows: Vec<AdminRow> = positions
        .iter()
        .map(|position| admin_row(&cities_rows[*position]))
        .collect();
    let summary = apply_corrections(&mut admin_rows, &corrections, admin2_keys, samples.len())?;
    for outcome in &summary.outcomes {
        if let CorrectionOutcome::RowNotFound { geoname_id } = outcome {
            // Reason: 候選一律由 cities_rows 產生，找不到代表內部不一致。
            // 不靜默吞掉——「修正數」與實際寫入數不符時，報表就不能當診斷依據。
            println!(
                "stage=translate admin1_correct_row_missing country={country_code} \
                 geoname_id={geoname_id}"
            );
        }
    }
    for (position, admin_row) in positions.iter().zip(admin_rows.iter()) {
        cities_rows[*position][COLUMN_ADMIN1] = admin_row.admin1.clone();
        cities_rows[*position][COLUMN_ADMIN2] = admin_row.admin2.clone();
    }
    println!(
        "stage=translate admin1_correct_applied country={country_code} applied={} \
         admin2_cleared={}",
        summary.applied, summary.admin2_cleared
    );
    Ok(())
}

fn countries_in(metadata_admin1: &MetadataAdmin1) -> Vec<String> {
    // Reason: BTreeSet 讓國家處理順序固定，log 行序與檔案寫出順序才可重現。
    metadata_admin1
        .keys()
        .map(|(country_code, _, _)| country_code.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn samples_for(country_code: &str, metadata_admin1: &MetadataAdmin1) -> Vec<PointSample> {
    let mut samples: Vec<PointSample> = metadata_admin1
        .iter()
        .filter(|((code, _, _), name)| code == country_code && !name.is_empty())
        .filter_map(|((_, latitude, longitude), name)| {
            Some(PointSample {
                latitude: latitude.parse().ok()?,
                longitude: longitude.parse().ok()?,
                locationiq_admin1: name.clone(),
            })
        })
        .collect();
    // Reason: HashMap 迭代順序不固定。學習本身對順序不敏感（已有測試），但
    // `samples.len()` 之外的任何後續變更都可能引入順序相依，先排序更保險。
    samples.sort_by(|left, right| {
        left.locationiq_admin1
            .cmp(&right.locationiq_admin1)
            .then(left.latitude.total_cmp(&right.latitude))
            .then(left.longitude.total_cmp(&right.longitude))
    });
    samples
}

fn city_point(row: &[String], metadata_admin1: &MetadataAdmin1) -> Option<CityPoint> {
    let country_code = row.get(COLUMN_COUNTRY_CODE)?.clone();
    let latitude_text = row.get(COLUMN_LATITUDE)?.clone();
    let longitude_text = row.get(COLUMN_LONGITUDE)?.clone();
    let key = (
        country_code.clone(),
        latitude_text.clone(),
        longitude_text.clone(),
    );
    Some(CityPoint {
        geoname_id: row.get(COLUMN_GEONAME_ID)?.clone(),
        name: row.get(COLUMN_NAME)?.clone(),
        latitude: latitude_text.parse().ok()?,
        longitude: longitude_text.parse().ok()?,
        country_code,
        original_admin1: row.get(COLUMN_ADMIN1)?.clone(),
        locationiq_admin1: metadata_admin1
            .get(&key)
            .filter(|name| !name.is_empty())
            .cloned(),
    })
}

fn admin_row(row: &[String]) -> AdminRow {
    AdminRow {
        geoname_id: row[COLUMN_GEONAME_ID].clone(),
        country_code: row[COLUMN_COUNTRY_CODE].clone(),
        admin1: row[COLUMN_ADMIN1].clone(),
        admin2: row[COLUMN_ADMIN2].clone(),
    }
}

/// 從 `admin1CodesASCII` 列建出 `gn_id → 代碼` 與 `代碼 → 名稱` 兩張表。
fn admin1_tables(admin1_rows: &[Vec<String>]) -> (BTreeMap<i64, String>, BTreeMap<String, String>) {
    let mut gn_id_to_code = BTreeMap::new();
    let mut known = BTreeMap::new();
    for row in admin1_rows {
        let (Some(code), Some(name)) = (row.first(), row.get(1)) else {
            continue;
        };
        known.insert(code.clone(), name.clone());
        if let Some(Ok(gn_id)) = row.get(3).map(|value| value.parse::<i64>()) {
            gn_id_to_code.insert(gn_id, code.clone());
        }
    }
    (gn_id_to_code, known)
}

fn load_admin2_keys(path: &Path) -> Result<BTreeSet<String>, String> {
    if !path.exists() {
        // Reason: 檔案缺席時把鍵集當空的，admin2 一律保留。保留是安全的一側:
        // 錯誤地清空會讓縣名消失，錯誤地保留只是維持現狀。
        println!(
            "stage=translate admin1_correct admin2_codes_missing path={}",
            path.display()
        );
        return Ok(BTreeSet::new());
    }
    let content = fs::read_to_string(path)
        .map_err(|error| format!("無法讀取 {}：{error}", path.display()))?;
    Ok(content
        .lines()
        .filter_map(|line| line.split('\t').next())
        .filter(|key| !key.is_empty())
        .map(str::to_string)
        .collect())
}
