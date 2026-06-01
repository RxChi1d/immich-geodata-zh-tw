use crate::cli::RunOptions;
use crate::pipeline::fixtures::{Fixture, load_fixtures};
use crate::pipeline::geodata::{admin1_mapping, normalize_admin_fields, read_geodata};
use crate::pipeline::polars_table::{read_admin1_rows, write_string_rows};
use crate::pipeline::table::write_delimited;
use crate::pipeline::transform_cities_schema::country_profile;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProductionAdmin1Options {
    pub input: PathBuf,
    pub output: PathBuf,
    pub metadata_dir: PathBuf,
    pub handler_countries: Vec<String>,
    pub base_geoname_id: i64,
}

pub fn run(options: &RunOptions) -> Result<(), String> {
    let fixtures = load_fixtures(&options.fixtures_dir, options.fixture.as_deref())?;
    for fixture in fixtures {
        if !fixture.supports_stage("admin1_load") {
            continue;
        }
        run_fixture(&fixture, options)?;
    }
    Ok(())
}

fn run_fixture(fixture: &Fixture, options: &RunOptions) -> Result<(), String> {
    let mut rows = read_admin1_rows(&fixture.root.join("geoname").join("admin1CodesASCII.txt"))?;
    let mut max_id = fixture.manifest.base_geoname_id - 1;

    for country in &fixture.manifest.countries {
        let input = fixture
            .root
            .join("geodata")
            .join(format!("{}_geodata.csv", country.to_lowercase()));
        replace_country_admin1(country, &input, &mut rows, &mut max_id)?;
    }

    rows.sort_by(|left, right| left[0].cmp(&right[0]).then(left[3].cmp(&right[3])));
    let output = options
        .output_dir
        .join(&fixture.manifest.name)
        .join("admin1_load")
        .join("admin1CodesASCII_optimized.txt");
    write_admin1_rows_direct(&output, &rows)?;
    println!(
        "stage=admin1_load fixture={} rows={} max_geoname_id={}",
        fixture.manifest.name,
        rows.len(),
        max_id
    );
    Ok(())
}

pub fn run_production(options: &ProductionAdmin1Options) -> Result<i64, String> {
    let mut rows = read_admin1_rows(&options.input)?;
    let mut max_id = options.base_geoname_id - 1;
    for country in &options.handler_countries {
        let input = options
            .metadata_dir
            .join(format!("{}_geodata.csv", country.to_lowercase()));
        if !input.exists() {
            println!(
                "admin1_load_skip country={} reason=missing_geodata path={}",
                country,
                input.display()
            );
            continue;
        }
        replace_country_admin1(country, &input, &mut rows, &mut max_id)?;
        if country == "TW" {
            write_tw_admin1_map(&input, &options.output)?;
        }
    }

    write_admin1_rows_direct(&options.output, &rows)?;
    println!(
        "stage=admin1_load mode=production output={} rows={} max_geoname_id={}",
        options.output.display(),
        rows.len(),
        max_id
    );
    Ok(max_id)
}

fn write_admin1_rows_direct(output: &Path, rows: &[Vec<String>]) -> Result<(), String> {
    write_delimited(output, '\t', None, rows)
}

fn write_tw_admin1_map(input: &Path, admin1_output: &Path) -> Result<(), String> {
    let Some(output_dir) = admin1_output.parent() else {
        return Ok(());
    };
    let mut records = read_geodata(input)?;
    normalize_admin_fields(&mut records);
    let mapping = admin1_mapping(&records, "TW");
    let rows: Vec<Vec<String>> = mapping
        .into_iter()
        .map(|(name, code)| vec![code, name])
        .collect();
    write_string_rows(
        &output_dir.join("tw_admin1_map.csv"),
        b',',
        true,
        &["new_id", "name"],
        &rows,
    )
}

fn replace_country_admin1(
    country: &str,
    input: &Path,
    rows: &mut Vec<Vec<String>>,
    max_id: &mut i64,
) -> Result<(), String> {
    country_profile(country)?;
    let mut records = read_geodata(input)?;
    normalize_admin_fields(&mut records);
    let mapping = admin1_mapping(&records, country);
    let base_id = *max_id + 1;

    let mut new_rows = Vec::new();
    for (index, (name, code)) in mapping.into_iter().enumerate() {
        new_rows.push(vec![
            code,
            name.clone(),
            name,
            (base_id + index as i64).to_string(),
        ]);
    }
    let converted_len = new_rows.len();
    let prefix = format!("{country}.");
    let mut output_rows = new_rows;
    for row in rows.iter() {
        let id = row
            .first()
            .ok_or_else(|| format!("admin1 欄位數不足，無法讀取 id：{row:?}"))?;
        if !id.starts_with(&prefix) {
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
    fn replace_taiwan_removes_old_rows_and_prepends_new_rows() {
        let fixture = load_fixtures(
            std::path::Path::new("../fixtures/parity"),
            Some("tw_minimal"),
        )
        .unwrap()
        .remove(0);
        let mut rows = vec![
            vec![
                "TW.OLD".to_string(),
                "Old Taiwan".to_string(),
                "Old Taiwan".to_string(),
                "100".to_string(),
            ],
            vec![
                "US.CA".to_string(),
                "California".to_string(),
                "California".to_string(),
                "5332921".to_string(),
            ],
        ];
        let mut max_id = 91_999_999;

        let input = fixture.root.join("geodata").join("tw_geodata.csv");
        replace_country_admin1("TW", &input, &mut rows, &mut max_id).unwrap();

        assert_eq!(max_id, 92_000_001);
        assert_eq!(rows[0][0], "TW.1");
        assert_eq!(rows[1][0], "TW.2");
        assert_eq!(rows[2][0], "US.CA");
    }

    #[test]
    fn production_replacement_keeps_legacy_vstack_order() {
        let fixture = load_fixtures(
            std::path::Path::new("../fixtures/parity"),
            Some("tw_minimal"),
        )
        .unwrap()
        .remove(0);
        let mut rows = vec![
            vec![
                "AD.02".to_string(),
                "Canillo".to_string(),
                "Canillo".to_string(),
                "3041203".to_string(),
            ],
            vec![
                "TW.OLD".to_string(),
                "Old Taiwan".to_string(),
                "Old Taiwan".to_string(),
                "100".to_string(),
            ],
        ];
        let mut max_id = 91_999_999;

        let input = fixture.root.join("geodata").join("tw_geodata.csv");
        replace_country_admin1("TW", &input, &mut rows, &mut max_id).unwrap();

        assert!(rows[0][0].starts_with("TW."));
        assert_eq!(rows.last().unwrap()[0], "AD.02");
    }
}
