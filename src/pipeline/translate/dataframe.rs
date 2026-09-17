//! Polars DataFrame 與 Rust 值之間的相互轉換。
//!
//! 自 `translate.rs` 拆出——metadata 與 alternate names 的載入、空表建構、
//! 欄位取值與查表建立，都是 schema 相關的樣板，與翻譯規則無關。

use std::collections::HashMap;
use std::fs::{self};
use std::path::{Path, PathBuf};

use polars::prelude::*;

use crate::pipeline::admin1_correct_stage::MetadataAdmin1;
use crate::pipeline::polars_table::read_string_rows;
use crate::pipeline::table::GEODATA_COLUMNS;

use super::rows::*;

/// 由檔名解析 LocationIQ metadata 的國碼。
///
/// production 的 LocationIQ 產物 `{CC}.csv`（ISO-3166-1 alpha-2）位於
/// `data/locationiq/`，與 handler extract 產物 `data/handler/{cc}_geodata.csv`
/// 分屬不同目錄。後者由 enhance 階段（`admin1_load` / `cities500_load`）以
/// 明確檔名消費，其內容早已寫入 cities500，不應在此重複載入。
///
/// Reason: 目錄雖已分離，檔名守衛仍保留——它擋住誤放或路徑指錯的檔案，並負責
/// 大小寫正規化（`--country-code us` 會產出 `us.csv`，而 cities500 國碼為大寫，
/// 不正規化會靜默失效）。歷史背景：兩種檔案曾共用 `meta_data/`，舊版直接把檔名
/// stem 當國碼，handler geodata 檔被當成國碼 `tw_geodata` 的 metadata 載入——
/// 25 萬列永不命中、不影響輸出，也沒有任何 log。
pub(super) fn locationiq_country_code(file_path: &Path) -> Option<String> {
    let stem = file_path.file_stem().and_then(|value| value.to_str())?;
    (stem.len() == 2 && stem.chars().all(|value| value.is_ascii_alphabetic()))
        .then(|| stem.to_ascii_uppercase())
}

pub(super) fn load_metadata_dataframe(path: &Path) -> Result<DataFrame, String> {
    let mut metadata = empty_metadata_dataframe()?;
    if !path.exists() {
        println!(
            "stage=translate metadata_dir_missing path={} metadata_files=0 metadata_rows=0",
            path.display()
        );
        return Ok(metadata);
    }

    // Reason: read_dir 順序由檔案系統決定，而 vstack 順序會經由 unique_stable
    // 的 KeepStrategy::First 影響保留的列。排序後載入順序、log 行與輸出才可重現。
    let mut files: Vec<PathBuf> = fs::read_dir(path)
        .map_err(|error| format!("無法讀取 metadata 目錄 {}：{error}", path.display()))?
        .map(|entry| {
            entry
                .map(|entry| entry.path())
                .map_err(|error| format!("無法讀取 metadata 項目：{error}"))
        })
        .collect::<Result<Vec<_>, String>>()?;
    files.sort();

    let mut loaded: Vec<String> = Vec::new();
    let mut skipped: Vec<String> = Vec::new();
    let mut duplicates: Vec<String> = Vec::new();
    for file_path in files {
        // Reason: 副檔名比對不分大小寫——LocationIQ 匯出檔若寫成 US.CSV，
        // 大小寫敏感的比對會讓它連 skip log 都沒有，正是本次要消除的靜默 no-op。
        if !file_path
            .extension()
            .and_then(|value| value.to_str())
            .is_some_and(|value| value.eq_ignore_ascii_case("csv"))
        {
            continue;
        }
        // Reason: 非 UTF-8 檔名以 lossy 轉換保留可辨識字元，不讓 skip log 出現空項目。
        let file_name = file_path
            .file_name()
            .map(|value| value.to_string_lossy().into_owned())
            .unwrap_or_else(|| file_path.display().to_string());
        let Some(country_code) = locationiq_country_code(&file_path) else {
            skipped.push(file_name);
            continue;
        };
        // Reason: 檔名正規化為大寫後，US.csv 與 us.csv 會落在同一個國碼上，
        // 兩者的列會在後續 unique_stable 互相遮蔽。排序後取字典序第一個檔案，
        // 其餘略過並記錄，避免同一棵檔案樹在不同機器上翻出不同結果。
        if loaded.contains(&country_code) {
            duplicates.push(file_name);
            continue;
        }
        let rows = read_string_rows(&file_path, b',', true, &GEODATA_COLUMNS)?;
        loaded.push(country_code.clone());
        let file_df = metadata_rows_to_dataframe(&country_code, &rows)?;
        metadata
            .vstack_mut(&file_df)
            .map_err(|error| format!("Polars metadata concat 失敗：{error}"))?;
    }
    let subset = [
        "country_code".to_string(),
        "latitude".to_string(),
        "longitude".to_string(),
    ];
    let metadata = metadata
        .unique_stable(Some(&subset), UniqueKeepStrategy::First, None)
        .map_err(|error| format!("Polars metadata unique 失敗：{error}"))?;
    // Reason: 列數取去重後的實際查表列數，與 metadata_lookup 的規模一致。
    println!(
        "stage=translate metadata_files={} metadata_rows={} countries=[{}]",
        loaded.len(),
        metadata.height(),
        loaded.join(",")
    );
    if !skipped.is_empty() {
        println!(
            "translate_metadata_skip reason=not_locationiq_metadata files=[{}]",
            skipped.join(",")
        );
    }
    if !duplicates.is_empty() {
        println!(
            "translate_metadata_skip reason=duplicate_country_code files=[{}]",
            duplicates.join(",")
        );
    }
    Ok(metadata)
}

pub(super) fn empty_metadata_dataframe() -> Result<DataFrame, String> {
    DataFrame::new(
        0,
        vec![
            Series::new("country_code".into(), Vec::<String>::new()).into(),
            Series::new("latitude".into(), Vec::<String>::new()).into(),
            Series::new("longitude".into(), Vec::<String>::new()).into(),
            Series::new("_meta_admin_1".into(), Vec::<String>::new()).into(),
            Series::new("_meta_admin_2".into(), Vec::<String>::new()).into(),
        ],
    )
    .map_err(|error| format!("無法建立空 metadata DataFrame：{error}"))
}

pub(super) fn metadata_rows_to_dataframe(
    country_code: &str,
    rows: &[Vec<String>],
) -> Result<DataFrame, String> {
    for row in rows {
        if row.len() != 7 {
            return Err(format!(
                "metadata 欄位數不符：expected=7 actual={}",
                row.len()
            ));
        }
    }
    DataFrame::new(
        rows.len(),
        vec![
            Series::new(
                "country_code".into(),
                vec![country_code.to_string(); rows.len()],
            )
            .into(),
            Series::new(
                "latitude".into(),
                rows.iter().map(|row| row[0].clone()).collect::<Vec<_>>(),
            )
            .into(),
            Series::new(
                "longitude".into(),
                rows.iter().map(|row| row[1].clone()).collect::<Vec<_>>(),
            )
            .into(),
            // Reason: admin_1（第 4 欄）供 admin1 修正器使用；在此之前它讀進來
            // 就被丟掉，付費查到的行政區資訊從未被任何程式讀取。
            Series::new(
                "_meta_admin_1".into(),
                rows.iter().map(|row| row[3].clone()).collect::<Vec<_>>(),
            )
            .into(),
            Series::new(
                "_meta_admin_2".into(),
                rows.iter().map(|row| row[4].clone()).collect::<Vec<_>>(),
            )
            .into(),
        ],
    )
    .map_err(|error| format!("無法建立 metadata DataFrame：{error}"))
}

pub(super) fn string_column_values(df: &DataFrame, name: &str) -> Result<Vec<String>, String> {
    let column = df
        .column(name)
        .and_then(|column| column.str())
        .map_err(|error| format!("Polars 欄位 {name} 錯誤：{error}"))?;
    Ok((0..df.height())
        .filter_map(|index| column.get(index).map(ToString::to_string))
        .collect())
}

pub(super) fn string_column<'a>(
    df: &'a DataFrame,
    name: &str,
) -> Result<&'a StringChunked, String> {
    df.column(name)
        .and_then(|column| column.str())
        .map_err(|error| format!("Polars 欄位 {name} 錯誤：{error}"))
}

pub(super) fn metadata_lookup_from_dataframe(df: &DataFrame) -> Result<MetadataLookup, String> {
    let country_codes = string_column(df, "country_code")?;
    let latitudes = string_column(df, "latitude")?;
    let longitudes = string_column(df, "longitude")?;
    let admin2_names = string_column(df, "_meta_admin_2")?;
    let mut lookup = MetadataLookup::with_capacity(df.height());
    for index in 0..df.height() {
        lookup.insert_first(
            country_codes.get(index).unwrap_or_default(),
            latitudes.get(index).unwrap_or_default(),
            longitudes.get(index).unwrap_or_default(),
            admin2_names.get(index).unwrap_or_default(),
        );
    }
    Ok(lookup)
}

/// 由 metadata DataFrame 建出 admin1 修正器的 `(國碼, 緯度, 經度) → 行政區名稱`。
pub(super) fn metadata_admin1_from_dataframe(df: &DataFrame) -> Result<MetadataAdmin1, String> {
    let country_codes = string_column(df, "country_code")?;
    let latitudes = string_column(df, "latitude")?;
    let longitudes = string_column(df, "longitude")?;
    let admin1_names = string_column(df, "_meta_admin_1")?;
    let mut lookup = MetadataAdmin1::with_capacity(df.height());
    for index in 0..df.height() {
        // Reason: 鍵與 `MetadataLookup::insert_first` 同構——同一座標重複出現時
        // 保留第一筆，兩張表才會對同一個點給出一致的答案。
        lookup
            .entry((
                country_codes.get(index).unwrap_or_default().to_string(),
                latitudes.get(index).unwrap_or_default().to_string(),
                longitudes.get(index).unwrap_or_default().to_string(),
            ))
            .or_insert_with(|| admin1_names.get(index).unwrap_or_default().to_string());
    }
    Ok(lookup)
}

pub(super) fn alternate_lookup_from_dataframe(df: &DataFrame) -> Result<AlternateLookup, String> {
    let geoname_ids = string_column(df, "geoname_id")?;
    let names = string_column(df, "name")?;
    let mut lookup = HashMap::with_capacity(df.height());
    for index in 0..df.height() {
        lookup
            .entry(geoname_ids.get(index).unwrap_or_default().to_string())
            .or_insert_with(|| names.get(index).unwrap_or_default().to_string());
    }
    Ok(lookup)
}
