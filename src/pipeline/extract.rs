use crate::cli::RunOptions;
use crate::pipeline::fixtures::{Fixture, load_fixtures};
use crate::pipeline::polars_table::read_geodata_rows_with_header;
use crate::pipeline::table::{GEODATA_COLUMNS, format_coordinate};
use std::fs;
use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use std::time::Instant;

mod geometry;
mod handlers;
mod indonesia;
mod indonesia_normalize;
mod indonesia_wikidata;
mod korea_wikidata;
mod label_sanitize;
mod sources;
mod thailand_wikidata;
mod types;
mod wikidata_common;

/// 所有擁有 extract handler 的國家代碼，由 `Country` enum 導出。
pub fn handler_country_codes() -> Vec<&'static str> {
    types::Country::ALL
        .iter()
        .map(|country| country.code())
        .collect()
}

use geometry::{apply_country_centroids, split_multipolygon_parts};
use sources::{read_geojson_features_from_content, read_shapefile};
use types::{Country, ExtractRow};

pub fn run(options: &RunOptions) -> Result<(), String> {
    let fixtures = load_fixtures(&options.fixtures_dir, options.fixture.as_deref())?;
    for fixture in fixtures {
        if !fixture.supports_stage("extract") {
            continue;
        }
        run_fixture(&fixture, options)?;
    }
    Ok(())
}

pub fn run_production(country: &str, input: &Path, output: &Path) -> Result<(), String> {
    run_production_with_profile(country, input, output, false)
}

pub fn run_production_with_profile(
    country: &str,
    input: &Path,
    output: &Path,
    profile_enabled: bool,
) -> Result<(), String> {
    let total_start = Instant::now();
    let mut profile = ExtractProfile::new(profile_enabled);
    let country = Country::parse(country)?;
    let suffix = input
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    let mut rows = match suffix.as_str() {
        "csv" => profile.time("read_normalized", || read_normalized_rows_from_path(input))?,
        "geojson" | "json" => {
            read_geospatial_rows_from_path_profiled(country, input, None, &mut profile)?
        }
        "shp" => read_shapefile_rows_from_path_profiled(country, input, None, &mut profile)?,
        other => {
            return Err(format!(
                "不支援的 extract 輸入格式：.{other}，請提供 .csv、.shp、.geojson 或 .json"
            ));
        }
    };
    profile.time("sort", || sort_extract_rows(&mut rows))?;
    profile.time("round", || round_extract_coordinates(&mut rows))?;
    profile.time("write", || write_extract_rows(output, &rows))?;
    profile.print(country, total_start.elapsed().as_millis());
    println!(
        "stage=extract mode=production country={} input={} output={} rows={}",
        country.code(),
        input.display(),
        output.display(),
        rows.len()
    );
    Ok(())
}

struct ExtractProfile {
    enabled: bool,
    entries: Vec<(&'static str, u128)>,
}

impl ExtractProfile {
    fn new(enabled: bool) -> Self {
        Self {
            enabled,
            entries: Vec::new(),
        }
    }

    fn time<T>(
        &mut self,
        name: &'static str,
        operation: impl FnOnce() -> Result<T, String>,
    ) -> Result<T, String> {
        if !self.enabled {
            return operation();
        }
        let start = Instant::now();
        let result = operation()?;
        self.entries.push((name, start.elapsed().as_millis()));
        Ok(result)
    }

    fn print(&self, country: Country, total_ms: u128) {
        if !self.enabled {
            return;
        }
        let details = self
            .entries
            .iter()
            .map(|(name, elapsed_ms)| format!("{name}_ms={elapsed_ms}"))
            .collect::<Vec<_>>()
            .join(" ");
        println!(
            "profile stage=extract.detail country={} total_ms={} {}",
            country.code(),
            total_ms,
            details
        );
    }
}

fn run_fixture(fixture: &Fixture, options: &RunOptions) -> Result<(), String> {
    for country_code in &fixture.manifest.countries {
        let country = Country::parse(country_code)?;
        let mut rows = if has_geospatial_source(fixture, country.code()) {
            read_geospatial_rows(fixture, country)?
        } else {
            read_normalized_rows(fixture, country.code())?
        };
        sort_extract_rows(&mut rows)?;
        round_extract_coordinates(&mut rows)?;

        let output = options
            .output_dir
            .join(&fixture.manifest.name)
            .join("extract")
            .join(format!("{}.csv", country.code()));
        write_extract_rows(&output, &rows)?;
        println!(
            "stage=extract fixture={} country={} rows={}",
            fixture.manifest.name,
            country.code(),
            rows.len()
        );
    }
    Ok(())
}

fn read_normalized_rows(fixture: &Fixture, country: &str) -> Result<Vec<ExtractRow>, String> {
    let input = fixture
        .root
        .join("geodata")
        .join(format!("{}_geodata.csv", country.to_lowercase()));
    read_normalized_rows_from_path(&input)
}

fn read_normalized_rows_from_path(input: &Path) -> Result<Vec<ExtractRow>, String> {
    let rows = read_geodata_rows_with_header(input)?;
    let mut extract_rows = Vec::with_capacity(rows.len());
    for (row_index, row) in rows.into_iter().enumerate() {
        if row.len() != GEODATA_COLUMNS.len() {
            return Err(format!(
                "geodata 欄位數不符：expected={} actual={} path={}",
                GEODATA_COLUMNS.len(),
                row.len(),
                input.display()
            ));
        }
        extract_rows.push(extract_row_from_fields(row, row_index)?);
    }
    Ok(extract_rows)
}

fn extract_row_from_fields(row: Vec<String>, row_index: usize) -> Result<ExtractRow, String> {
    let mut fields = row.into_iter();
    let latitude = fields.next().unwrap_or_default();
    let longitude = fields.next().unwrap_or_default();
    let latitude_key = coordinate_sort_key_checked(&latitude, "latitude", row_index)?;
    let longitude_key = coordinate_sort_key_checked(&longitude, "longitude", row_index)?;
    Ok(ExtractRow {
        latitude,
        longitude,
        latitude_key,
        longitude_key,
        country: fields.next().unwrap_or_default(),
        admin_1: fields.next().unwrap_or_default(),
        admin_2: fields.next().unwrap_or_default(),
        admin_3: fields.next().unwrap_or_default(),
        admin_4: fields.next().unwrap_or_default(),
    })
}

fn sort_extract_rows(rows: &mut [ExtractRow]) -> Result<(), String> {
    rows.sort_by(|left, right| {
        left.country
            .cmp(&right.country)
            .then(left.admin_1.cmp(&right.admin_1))
            .then(left.admin_2.cmp(&right.admin_2))
            .then(left.admin_3.cmp(&right.admin_3))
            .then(left.admin_4.cmp(&right.admin_4))
            .then(left.latitude_key.total_cmp(&right.latitude_key))
            .then(left.longitude_key.total_cmp(&right.longitude_key))
    });
    Ok(())
}

fn coordinate_sort_key_checked(value: &str, field: &str, row_index: usize) -> Result<f64, String> {
    value.parse::<f64>().map_err(|error| {
        format!("extract 第 {row_index} 列 {field} 座標格式錯誤 {value:?}：{error}")
    })
}

fn round_extract_coordinates(rows: &mut [ExtractRow]) -> Result<(), String> {
    for row in rows {
        row.latitude = format_coordinate(&row.latitude)?;
        row.longitude = format_coordinate(&row.longitude)?;
    }
    Ok(())
}

fn write_extract_rows(output: &Path, rows: &[ExtractRow]) -> Result<(), String> {
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("無法建立 extract 輸出目錄 {}：{error}", parent.display()))?;
    }
    let file = File::create(output)
        .map_err(|error| format!("無法建立 extract CSV {}：{error}", output.display()))?;
    let mut writer = BufWriter::new(file);
    write_csv_record(&mut writer, &GEODATA_COLUMNS)
        .map_err(|error| format!("無法寫入 extract CSV {}：{error}", output.display()))?;
    for row in rows {
        let latitude = compact_coordinate(&row.latitude)?;
        let longitude = compact_coordinate(&row.longitude)?;
        write_csv_record(
            &mut writer,
            &[
                latitude.as_str(),
                longitude.as_str(),
                row.country.as_str(),
                row.admin_1.as_str(),
                row.admin_2.as_str(),
                row.admin_3.as_str(),
                row.admin_4.as_str(),
            ],
        )
        .map_err(|error| format!("無法寫入 extract CSV {}：{error}", output.display()))?;
    }
    writer
        .flush()
        .map_err(|error| format!("無法寫入 extract CSV {}：{error}", output.display()))
}

fn compact_coordinate(value: &str) -> Result<String, String> {
    let fixed = format_coordinate(value)?;
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

fn write_csv_record(writer: &mut BufWriter<File>, fields: &[&str]) -> std::io::Result<()> {
    for (index, field) in fields.iter().enumerate() {
        if index > 0 {
            writer.write_all(b",")?;
        }
        write_csv_field(writer, field)?;
    }
    writer.write_all(b"\n")
}

fn write_csv_field(writer: &mut BufWriter<File>, value: &str) -> std::io::Result<()> {
    if !value.contains(',') && !value.contains('"') && !value.contains('\n') {
        return writer.write_all(value.as_bytes());
    }
    writer.write_all(b"\"")?;
    for char in value.chars() {
        if char == '"' {
            writer.write_all(b"\"\"")?;
        } else {
            write_char(writer, char)?;
        }
    }
    writer.write_all(b"\"")
}

fn write_char(writer: &mut BufWriter<File>, value: char) -> std::io::Result<()> {
    let mut buffer = [0_u8; 4];
    writer.write_all(value.encode_utf8(&mut buffer).as_bytes())
}

fn has_geospatial_source(fixture: &Fixture, country: &str) -> bool {
    geospatial_source_path(fixture, country).is_some()
}

fn geospatial_source_path(fixture: &Fixture, country: &str) -> Option<PathBuf> {
    let source_dir = fixture.root.join("extract_sources");
    [format!("{country}.geojson"), format!("{country}.shp")]
        .into_iter()
        .map(|filename| source_dir.join(filename))
        .find(|path| path.exists())
}

fn read_geospatial_rows(fixture: &Fixture, country: Country) -> Result<Vec<ExtractRow>, String> {
    let source_path = geospatial_source_path(fixture, country.code())
        .ok_or_else(|| format!("找不到 {} geospatial extract fixture。", country.code()))?;
    let suffix = source_path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    let wikidata_stub = fixture
        .root
        .join("extract_sources")
        .join(format!("{}_wikidata_stub.json", country.code()));
    match suffix.as_str() {
        "geojson" | "json" => {
            read_geospatial_rows_from_path(country, &source_path, Some(&wikidata_stub))
        }
        "shp" => read_shapefile_rows_from_path(country, &source_path, Some(&wikidata_stub)),
        other => Err(format!(
            "不支援的 fixture extract 輸入格式：.{other}，請提供 .shp 或 .geojson"
        )),
    }
}

fn read_geospatial_rows_from_path(
    country: Country,
    source_path: &Path,
    wikidata_stub: Option<&Path>,
) -> Result<Vec<ExtractRow>, String> {
    let mut profile = ExtractProfile::new(false);
    read_geospatial_rows_from_path_profiled(country, source_path, wikidata_stub, &mut profile)
}

fn read_geospatial_rows_from_path_profiled(
    country: Country,
    source_path: &Path,
    wikidata_stub: Option<&Path>,
    profile: &mut ExtractProfile,
) -> Result<Vec<ExtractRow>, String> {
    let content = profile.time("read_geojson_file", || {
        fs::read_to_string(source_path)
            .map_err(|error| format!("無法讀取 GeoJSON {}：{error}", source_path.display()))
    })?;
    let mut features = profile.time("parse_geojson", || {
        read_geojson_features_from_content(country, &content, source_path)
    })?;
    let context = profile.time("load_context", || {
        country.load_context(source_path, wikidata_stub, &features)
    })?;
    let source_crs = features.first().and_then(|feature| feature.crs.clone());
    if country.splits_multipolygon_parts() {
        features = profile.time("split_parts", || Ok(split_multipolygon_parts(features)))?;
    }
    profile.time("centroid", || {
        apply_country_centroids(country, &mut features, source_crs.as_deref())
    })?;
    profile.time("build_rows", || {
        country.rows_from_features(&features, &context)
    })
}

fn read_shapefile_rows_from_path(
    country: Country,
    source_path: &Path,
    wikidata_stub: Option<&Path>,
) -> Result<Vec<ExtractRow>, String> {
    let mut profile = ExtractProfile::new(false);
    read_shapefile_rows_from_path_profiled(country, source_path, wikidata_stub, &mut profile)
}

fn read_shapefile_rows_from_path_profiled(
    country: Country,
    source_path: &Path,
    wikidata_stub: Option<&Path>,
    profile: &mut ExtractProfile,
) -> Result<Vec<ExtractRow>, String> {
    let mut features = profile.time("read_shapefile", || read_shapefile(country, source_path))?;
    let context = profile.time("load_context", || {
        country.load_context(source_path, wikidata_stub, &features)
    })?;
    let source_crs = features.first().and_then(|feature| feature.crs.clone());
    if country.splits_multipolygon_parts() {
        features = profile.time("split_parts", || Ok(split_multipolygon_parts(features)))?;
    }
    profile.time("centroid", || {
        apply_country_centroids(country, &mut features, source_crs.as_deref())
    })?;
    profile.time("build_rows", || {
        country.rows_from_features(&features, &context)
    })
}
