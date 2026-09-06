use std::collections::HashSet;
use std::path::{Path, PathBuf};

use crate::cli::RunOptions;
use crate::pipeline::fixtures::{Fixture, load_fixtures};
use crate::pipeline::geodata::{admin1_mapping, normalize_admin_fields, read_geodata};
use crate::pipeline::polars_cities::merge_extra_rows as merge_extra_city_rows;
use crate::pipeline::polars_table::{read_admin1_rows, read_cities_rows};
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
    pub admin1_file: PathBuf,
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
    ensure_admin1_resolvable(
        &rows,
        &fixture.root.join("geoname").join("admin1CodesASCII.txt"),
    )?;

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
    rows = merge_extra_rows(rows, &options.extra_files)?;
    ensure_admin1_resolvable(&rows, &options.admin1_file)?;

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
    merge_extra_rows(base_rows, &extra_files)
}

fn merge_extra_rows(
    base_rows: Vec<Vec<String>>,
    extra_files: &[PathBuf],
) -> Result<Vec<Vec<String>>, String> {
    let mut extra_rows = Vec::new();
    for path in extra_files {
        if !path.exists() {
            continue;
        }
        extra_rows.extend(read_cities_rows(path, b'\t')?);
    }
    merge_extra_city_rows(base_rows, extra_rows)
}

/// 確認每一列 A 類（行政區）點的 `<country_code>.<admin1_code>` 都能在
/// `admin1CodesASCII.txt` 找到對應。
///
/// Reason: A 類點是類別白名單才開始收的新來源，cities500 原本 100% 是 P 類。admin1
/// 解析不到時 Immich 只會顯示空的 state，是靜默的資料缺損，因此在此 fail closed。
/// 檔案只在真的有 A 類列時才讀取——沒有 A 類列的 fixture 不必準備 admin1 檔案。
///
/// Reason: 驗的是尚未置換的 raw `admin1CodesASCII.txt`，而非 `admin1_load` 產出的
/// `_optimized` 版。可行的前提是「handler 置換的國家」與「extra 併入的國家」互斥——
/// `admin1_load` 只置換 handler 國家的列，`extra_files` 只由非 handler 國家組成，兩個
/// 集合不相交，所以 A 類列要對的 admin1 一定還是 raw 檔裡的原始列。這個不變式由
/// `cli.rs` 的國家分流維持，不是本函式自己保證的；分流一旦改變，此處要改驗 optimized。
fn ensure_admin1_resolvable(rows: &[Vec<String>], admin1_file: &Path) -> Result<(), String> {
    let mut admin_rows = rows.iter().filter(|row| row[6] == "A").peekable();
    if admin_rows.peek().is_none() {
        return Ok(());
    }
    let admin1_codes: HashSet<String> = read_admin1_rows(admin1_file)?
        .into_iter()
        .map(|row| row[0].clone())
        .collect();
    for row in admin_rows {
        let key = format!("{}.{}", row[8], row[10]);
        if row[10].is_empty() || !admin1_codes.contains(&key) {
            return Err(format!(
                "A 類行政區點的 admin1 無法解析：geoname_id={} name={} key={key} source={}",
                row[0],
                row[1],
                admin1_file.display()
            ));
        }
    }
    Ok(())
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
    fn replace_country_cities_keeps_legacy_vstack_order() {
        let fixture = load_fixtures(
            &Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures"),
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

    #[test]
    fn ensure_admin1_resolvable_accepts_resolvable_admin_rows() {
        let dir = tempfile::tempdir().unwrap();
        let admin1_file = write_admin1_file(dir.path(), "MY.01\tJohor\tJohor\t1732750\n");
        let rows = vec![admin_row("101", "MY", "01")];

        ensure_admin1_resolvable(&rows, &admin1_file).unwrap();
    }

    #[test]
    fn ensure_admin1_resolvable_fails_closed_on_unknown_admin1() {
        let dir = tempfile::tempdir().unwrap();
        let admin1_file = write_admin1_file(dir.path(), "MY.01\tJohor\tJohor\t1732750\n");
        let rows = vec![admin_row("101", "MY", "99")];

        let error = ensure_admin1_resolvable(&rows, &admin1_file).unwrap_err();

        assert!(error.contains("MY.99"), "{error}");
    }

    #[test]
    fn ensure_admin1_resolvable_fails_closed_on_empty_admin1() {
        let dir = tempfile::tempdir().unwrap();
        let admin1_file = write_admin1_file(dir.path(), "MY.01\tJohor\tJohor\t1732750\n");
        let rows = vec![admin_row("101", "MY", "")];

        let error = ensure_admin1_resolvable(&rows, &admin1_file).unwrap_err();

        assert!(error.contains("geoname_id=101"), "{error}");
    }

    #[test]
    fn ensure_admin1_resolvable_skips_reading_when_no_admin_rows() {
        let rows = vec![city_row_with_country("3039154", "El Tarter", "AD")];

        ensure_admin1_resolvable(&rows, Path::new("/nonexistent/admin1CodesASCII.txt")).unwrap();
    }

    fn write_admin1_file(dir: &Path, content: &str) -> PathBuf {
        let path = dir.join("admin1CodesASCII.txt");
        std::fs::write(&path, content).unwrap();
        path
    }

    fn admin_row(geoname_id: &str, country_code: &str, admin1_code: &str) -> Vec<String> {
        let mut row = city_row_with_country(geoname_id, "Admin", country_code);
        row[6] = "A".to_string();
        row[7] = "ADM2".to_string();
        row[10] = admin1_code.to_string();
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
