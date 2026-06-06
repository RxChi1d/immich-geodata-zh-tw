use crate::cli::RunOptions;
use crate::pipeline::fixtures::{Fixture, load_fixtures};
use crate::pipeline::geodata::{
    admin1_mapping, normalize_admin_fields, read_geodata, sort_for_cities,
};
use crate::pipeline::polars_table::write_cities_rows;
use crate::pipeline::table::format_coordinate;
use std::collections::HashMap;
use std::path::Path;

pub fn run(options: &RunOptions) -> Result<(), String> {
    let fixtures = load_fixtures(&options.fixtures_dir, options.fixture.as_deref())?;
    for fixture in fixtures {
        if !fixture.supports_stage("transform_cities_schema") {
            continue;
        }
        run_fixture(&fixture, options)?;
    }
    Ok(())
}

fn run_fixture(fixture: &Fixture, options: &RunOptions) -> Result<(), String> {
    for country in &fixture.manifest.countries {
        run_country(fixture, options, country)?;
    }
    Ok(())
}

fn run_country(fixture: &Fixture, options: &RunOptions, country: &str) -> Result<(), String> {
    let mut rows = build_country_city_rows(fixture, country, fixture.manifest.base_geoname_id)?;
    sort_city_rows_for_golden(&mut rows);
    let output = options
        .output_dir
        .join(&fixture.manifest.name)
        .join("transform_cities_schema")
        .join(format!("{country}.csv"));
    write_cities_rows(&output, b',', true, &rows)?;
    println!(
        "stage=transform_cities_schema fixture={} country={} rows={}",
        fixture.manifest.name,
        country,
        rows.len()
    );
    Ok(())
}

pub fn build_country_city_rows(
    fixture: &Fixture,
    country_code: &str,
    base_geoname_id: i64,
) -> Result<Vec<Vec<String>>, String> {
    let input = fixture
        .root
        .join("geodata")
        .join(format!("{}_geodata.csv", country_code.to_lowercase()));
    build_city_rows_from_geodata(
        &input,
        country_code,
        base_geoname_id,
        &fixture.manifest.modification_date,
        CoordinateFormat::Fixed,
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoordinateFormat {
    Fixed,
    Compact,
}

pub fn build_city_rows_from_geodata(
    input: &Path,
    country_code: &str,
    base_geoname_id: i64,
    modification_date: &str,
    coordinate_format: CoordinateFormat,
) -> Result<Vec<Vec<String>>, String> {
    let raw_records = read_geodata(input)?;
    let mapping = admin1_mapping(&raw_records, country_code);
    let admin1_code_by_name: HashMap<String, String> = mapping
        .into_iter()
        .map(|(name, code)| {
            let short_code = code.split('.').next_back().unwrap_or_default().to_string();
            (name, short_code)
        })
        .collect();
    let mut records = raw_records;
    normalize_admin_fields(&mut records);
    sort_for_cities(&mut records);

    let profile = country_profile(country_code)?;
    let mut rows = Vec::new();
    for (index, record) in records.iter().enumerate() {
        let admin1_code = admin1_code_by_name
            .get(&record.admin_1)
            .cloned()
            .unwrap_or_default();
        // Reason: 多時區國家（如印尼）的時區依 admin1 解析。此處 record.admin_1
        //         已是 handler 最終省名（s2t + 補省正規化後）；indonesia_timezone
        //         以「最終省名 → WADMPR 原文 → 時區」解析，原文為權威 key。
        let timezone = profile.timezone_for_admin1(&record.admin_1)?;
        rows.push(vec![
            (base_geoname_id + index as i64).to_string(),
            record.admin_2.clone(),
            record.admin_2.clone(),
            String::new(),
            format_city_coordinate(&record.latitude, coordinate_format)?,
            format_city_coordinate(&record.longitude, coordinate_format)?,
            "A".to_string(),
            "ADM2".to_string(),
            country_code.to_string(),
            String::new(),
            admin1_code,
            String::new(),
            String::new(),
            String::new(),
            "0".to_string(),
            String::new(),
            String::new(),
            timezone.to_string(),
            modification_date.to_string(),
        ]);
    }
    Ok(rows)
}

fn format_city_coordinate(
    value: &str,
    coordinate_format: CoordinateFormat,
) -> Result<String, String> {
    let fixed = format_coordinate(value)?;
    if coordinate_format == CoordinateFormat::Fixed {
        return Ok(fixed);
    }

    let Some((integer, decimal)) = fixed.split_once('.') else {
        return Ok(fixed);
    };
    let trimmed = decimal.trim_end_matches('0');
    if trimmed.is_empty() {
        Ok(format!("{integer}.0"))
    } else {
        Ok(format!("{integer}.{trimmed}"))
    }
}

// Reason: 不衍生 PartialEq/Eq——欄位含函式指標，指標相等性無實質意義
//         （編譯器警告），且 CountryProfile 不需要比較。
#[derive(Debug, Clone, Copy)]
pub struct CountryProfile {
    pub country_name: &'static str,
    /// 國家預設時區（單一時區國家）。
    ///
    /// 多時區國家（印尼）以 `timezone_for_admin1` 依省解析；解析失敗
    /// 時回傳錯誤（不靜默回退）。
    pub timezone: &'static str,
    /// 多時區國家的 per-province 時區解析函式（key 為繁中省名）。
    timezone_resolver: Option<fn(&str) -> Option<&'static str>>,
}

impl CountryProfile {
    /// 依 admin1（繁中省名）解析時區。
    ///
    /// 單一時區國家直接回傳預設時區；多時區國家（設有 resolver）解析
    /// 失敗時回傳錯誤而非靜默回退。
    ///
    /// Reason: 多時區國家的省名未命中對照表（如 Wikidata 譯名漂移）若
    /// 靜默回退預設時區，WITA/WIT 省份會被錯標為 WIB 而無人察覺；
    /// 讓 release 直接失敗才能在發版前暴露問題。
    pub fn timezone_for_admin1(&self, admin1: &str) -> Result<&'static str, String> {
        match self.timezone_resolver {
            Some(resolver) => resolver(admin1).ok_or_else(|| {
                format!(
                    "無法解析 admin1「{admin1}」的時區：省名未命中時區對照表\
                     （可能為 Wikidata 譯名漂移），請校準 indonesia_timezone 對照表"
                )
            }),
            None => Ok(self.timezone),
        }
    }
}

pub fn country_profile(country_code: &str) -> Result<CountryProfile, String> {
    match country_code {
        "TW" => Ok(CountryProfile {
            country_name: "臺灣",
            timezone: "Asia/Taipei",
            timezone_resolver: None,
        }),
        "JP" => Ok(CountryProfile {
            country_name: "日本",
            timezone: "Asia/Tokyo",
            timezone_resolver: None,
        }),
        "KR" => Ok(CountryProfile {
            country_name: "南韓",
            timezone: "Asia/Seoul",
            timezone_resolver: None,
        }),
        "TH" => Ok(CountryProfile {
            country_name: "泰國",
            timezone: "Asia/Bangkok",
            timezone_resolver: None,
        }),
        "ID" => Ok(CountryProfile {
            country_name: "印尼",
            // Reason: 印尼跨 WIB/WITA/WIT 三時區，per-province 解析見
            //         indonesia_timezone；後備預設取最多省份的 WIB。
            timezone: "Asia/Jakarta",
            timezone_resolver: Some(crate::pipeline::indonesia_timezone::timezone_for_province),
        }),
        other => Err(format!("transform_cities_schema 尚未支援國家：{other}")),
    }
}

pub fn sort_city_rows_for_golden(rows: &mut [Vec<String>]) {
    rows.sort_by(|left, right| {
        left[8]
            .cmp(&right[8])
            .then(left[10].cmp(&right[10]))
            .then(left[1].cmp(&right[1]))
            .then(left[0].cmp(&right[0]))
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compact_coordinate_format_matches_polars_float_output() {
        assert_eq!(
            format_city_coordinate("120.79553010", CoordinateFormat::Compact).unwrap(),
            "120.7955301"
        );
        assert_eq!(
            format_city_coordinate("23.86220960", CoordinateFormat::Compact).unwrap(),
            "23.8622096"
        );
        assert_eq!(
            format_city_coordinate("24.00000000", CoordinateFormat::Compact).unwrap(),
            "24.0"
        );
    }

    #[test]
    fn fixed_coordinate_format_keeps_golden_precision() {
        assert_eq!(
            format_city_coordinate("23.86220960", CoordinateFormat::Fixed).unwrap(),
            "23.86220960"
        );
    }
}
