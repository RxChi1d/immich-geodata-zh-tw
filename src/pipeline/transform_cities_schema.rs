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
            profile.timezone.to_string(),
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CountryProfile {
    pub country_name: &'static str,
    pub timezone: &'static str,
}

pub fn country_profile(country_code: &str) -> Result<CountryProfile, String> {
    match country_code {
        "TW" => Ok(CountryProfile {
            country_name: "臺灣",
            timezone: "Asia/Taipei",
        }),
        "JP" => Ok(CountryProfile {
            country_name: "日本",
            timezone: "Asia/Tokyo",
        }),
        "KR" => Ok(CountryProfile {
            country_name: "南韓",
            timezone: "Asia/Seoul",
        }),
        "TH" => Ok(CountryProfile {
            country_name: "泰國",
            timezone: "Asia/Bangkok",
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
