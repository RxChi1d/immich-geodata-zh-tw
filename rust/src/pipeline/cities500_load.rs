use std::path::{Path, PathBuf};

use crate::cli::RunOptions;
use crate::pipeline::fixtures::{Fixture, load_fixtures};
use crate::pipeline::geodata::{admin1_mapping, normalize_admin_fields, read_geodata};
use crate::pipeline::polars_cities::merge_extra_rows as merge_extra_city_rows;
use crate::pipeline::polars_table::read_cities_rows;
use crate::pipeline::table::write_delimited;
use crate::pipeline::transform_cities_schema::{
    CoordinateFormat, build_city_rows_from_geodata, country_profile, sort_city_rows_for_golden,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProductionCities500Options {
    pub input: PathBuf,
    pub output: PathBuf,
    pub extra_files: Vec<PathBuf>,
    pub metadata_dir: PathBuf,
    pub handler_countries: Vec<String>,
    pub min_population: u32,
    pub current_max_id: i64,
    pub modification_date: String,
}

pub fn run(options: &RunOptions) -> Result<(), String> {
    let fixtures = load_fixtures(&options.fixtures_dir, options.fixture.as_deref())?;
    for fixture in fixtures {
        if !fixture.supports_stage("cities500_load") {
            continue;
        }
        run_fixture(&fixture, options)?;
    }
    Ok(())
}

fn run_fixture(fixture: &Fixture, options: &RunOptions) -> Result<(), String> {
    let mut rows = read_cities_rows(&fixture.root.join("geoname").join("cities500.txt"), b'\t')?;
    rows = merge_extra_data(fixture, rows)?;

    let mut max_id = calculate_admin1_max_id(fixture)?;
    for country in &fixture.manifest.countries {
        let input = fixture
            .root
            .join("geodata")
            .join(format!("{}_geodata.csv", country.to_lowercase()));
        replace_country_cities(
            country,
            &input,
            &fixture.manifest.modification_date,
            CoordinateFormat::Fixed,
            &mut rows,
            &mut max_id,
        )?;
    }

    sort_city_rows_for_golden(&mut rows);
    let output = options
        .output_dir
        .join(&fixture.manifest.name)
        .join("cities500_load")
        .join("cities500_optimized.txt");
    write_cities_rows_direct(&output, &rows)?;
    println!(
        "stage=cities500_load fixture={} rows={} max_geoname_id={}",
        fixture.manifest.name,
        rows.len(),
        max_id
    );
    Ok(())
}

pub fn run_production(options: &ProductionCities500Options) -> Result<i64, String> {
    let mut rows = read_cities_rows(&options.input, b'\t')?;
    rows = merge_extra_rows(rows, &options.extra_files, options.min_population)?;

    let mut max_id = options.current_max_id;
    for country in &options.handler_countries {
        let input = options
            .metadata_dir
            .join(format!("{}_geodata.csv", country.to_lowercase()));
        if !input.exists() {
            println!(
                "cities500_load_skip country={} reason=missing_geodata path={}",
                country,
                input.display()
            );
            continue;
        }
        replace_country_cities(
            country,
            &input,
            &options.modification_date,
            CoordinateFormat::Compact,
            &mut rows,
            &mut max_id,
        )?;
    }

    write_cities_rows_direct(&options.output, &rows)?;
    println!(
        "stage=cities500_load mode=production output={} rows={} max_geoname_id={}",
        options.output.display(),
        rows.len(),
        max_id
    );
    Ok(max_id)
}

fn write_cities_rows_direct(output: &Path, rows: &[Vec<String>]) -> Result<(), String> {
    write_delimited(output, '\t', None, rows)
}

fn merge_extra_data(
    fixture: &Fixture,
    base_rows: Vec<Vec<String>>,
) -> Result<Vec<Vec<String>>, String> {
    let extra_files: Vec<PathBuf> = fixture
        .manifest
        .extra_files
        .iter()
        .map(|extra_file| fixture.root.join(extra_file))
        .collect();
    merge_extra_rows(base_rows, &extra_files, fixture.manifest.min_population)
}

fn merge_extra_rows(
    base_rows: Vec<Vec<String>>,
    extra_files: &[PathBuf],
    min_population: u32,
) -> Result<Vec<Vec<String>>, String> {
    let mut extra_rows = Vec::new();
    for path in extra_files {
        if !path.exists() {
            continue;
        }
        extra_rows.extend(read_cities_rows(path, b'\t')?);
    }
    merge_extra_city_rows(base_rows, extra_rows, min_population)
}

fn calculate_admin1_max_id(fixture: &Fixture) -> Result<i64, String> {
    let mut max_id = fixture.manifest.base_geoname_id - 1;
    for country in &fixture.manifest.countries {
        country_profile(country)?;
        let input = fixture
            .root
            .join("geodata")
            .join(format!("{}_geodata.csv", country.to_lowercase()));
        let mut records = read_geodata(&input)?;
        normalize_admin_fields(&mut records);
        max_id += admin1_mapping(&records, country).len() as i64;
    }
    Ok(max_id)
}

fn replace_country_cities(
    country: &str,
    input: &Path,
    modification_date: &str,
    coordinate_format: CoordinateFormat,
    rows: &mut Vec<Vec<String>>,
    max_id: &mut i64,
) -> Result<(), String> {
    country_profile(country)?;
    let base_id = *max_id + 1;
    let converted_rows = build_city_rows_from_geodata(
        input,
        country,
        base_id,
        modification_date,
        coordinate_format,
    )?;
    let converted_len = converted_rows.len();
    let mut output_rows = converted_rows;
    for row in rows.iter() {
        let row_country = row
            .get(8)
            .ok_or_else(|| format!("cities500 欄位數不足，無法讀取 country_code：{row:?}"))?;
        if row_country != country {
            output_rows.push(row.clone());
        }
    }
    *rows = output_rows;
    *max_id = base_id + converted_len as i64 - 1;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deduplicate_keeps_max_population_and_lowest_geoname_id() {
        let rows = vec![city_row("5000", "1000"), city_row("4000", "1000")];

        let deduped = crate::pipeline::polars_cities::deduplicate_by_coordinate(rows).unwrap();

        assert_eq!(deduped.len(), 1);
        assert_eq!(deduped[0][0], "4000");
    }

    #[test]
    fn replace_country_cities_keeps_legacy_vstack_order() {
        let fixture = load_fixtures(
            std::path::Path::new("../fixtures/parity"),
            Some("tw_minimal"),
        )
        .unwrap()
        .remove(0);
        let mut rows = vec![city_row_with_country("3039154", "El Tarter", "AD")];
        let mut max_id = 91_999_999;
        let input = fixture.root.join("geodata").join("tw_geodata.csv");

        replace_country_cities(
            "TW",
            &input,
            &fixture.manifest.modification_date,
            CoordinateFormat::Fixed,
            &mut rows,
            &mut max_id,
        )
        .unwrap();

        assert_eq!(rows[0][8], "TW");
        assert_eq!(rows.last().unwrap()[8], "AD");
    }

    fn city_row(geoname_id: &str, population: &str) -> Vec<String> {
        let mut row = city_row_with_country(geoname_id, "Name", "US");
        row[14] = population.to_string();
        row
    }

    fn city_row_with_country(geoname_id: &str, name: &str, country_code: &str) -> Vec<String> {
        vec![
            geoname_id.to_string(),
            name.to_string(),
            name.to_string(),
            String::new(),
            "37.00000000".to_string(),
            "-122.00000000".to_string(),
            "P".to_string(),
            "PPL".to_string(),
            country_code.to_string(),
            String::new(),
            "CA".to_string(),
            String::new(),
            String::new(),
            String::new(),
            "1000".to_string(),
            String::new(),
            String::new(),
            "America/Los_Angeles".to_string(),
            "2024-01-01".to_string(),
        ]
    }
}
