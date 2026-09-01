use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::fs::{self, File};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

use opencc_rust::Converter;
use polars::prelude::*;

use crate::cli::RunOptions;
use crate::pipeline::fixtures::{Fixture, load_fixtures};
use crate::pipeline::naer_lookup::{NaerConfidence, NaerLookup, build_admin1_centroids};
use crate::pipeline::naer_stats::NaerStats;
use crate::pipeline::polars_table::{
    read_admin1_rows, read_alternate_name_rows_with_header, read_cities_rows, read_string_rows,
    write_alternate_name_rows_with_header,
};
use crate::pipeline::table::{GEODATA_COLUMNS, read_delimited, write_delimited};
use crate::pipeline::transform_cities_schema::sort_city_rows_for_golden;
use crate::pipeline::{admin1_load, cities500_load};
use crate::unicode_han::{includes_han, is_han_name};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProductionTranslateOptions {
    pub metadata_dir: PathBuf,
    pub data_dir: PathBuf,
    pub cities_file: PathBuf,
    pub admin1_file: PathBuf,
    pub alternate_name_file: PathBuf,
    pub naer_file: PathBuf,
    pub output_dir: PathBuf,
    pub profile: bool,
}

pub fn run(options: &RunOptions) -> Result<(), String> {
    admin1_load::run(options)?;
    cities500_load::run(options)?;

    let fixtures = load_fixtures(&options.fixtures_dir, options.fixture.as_deref())?;
    for fixture in fixtures {
        if !fixture.supports_stage("translate") {
            continue;
        }
        run_fixture(&fixture, options)?;
    }
    Ok(())
}

fn run_fixture(fixture: &Fixture, options: &RunOptions) -> Result<(), String> {
    let fixture_output = options.output_dir.join(&fixture.manifest.name);
    let metadata = load_metadata_dataframe(&fixture.root.join("meta_data"))?;
    let alternate_names =
        load_alternate_names_dataframe(&fixture.root.join("alternate_chinese_name.csv"))?;

    let cities_rows = read_cities_rows(
        &fixture_output
            .join("cities500_load")
            .join("cities500_optimized.txt"),
        b'\t',
    )?;
    let admin1_rows = read_admin1_rows(
        &fixture_output
            .join("admin1_load")
            .join("admin1CodesASCII_optimized.txt"),
    )?;
    let converter = OpenCcConverter::new(collect_translation_values(
        &metadata,
        &alternate_names,
        &cities_rows,
    ))?;
    let metadata_lookup = metadata_lookup_from_dataframe(&metadata)?;
    let alternate_lookup = alternate_lookup_from_dataframe(&alternate_names)?;
    let naer_lookup = NaerLookup::load(&fixture.root.join("naer_place_names.csv"))?;
    let admin1_centroids = build_admin1_centroids(&cities_rows);
    let mut naer_stats = NaerStats::default();
    let mut cities_rows = translate_cities_rows(
        cities_rows,
        &metadata_lookup,
        &alternate_lookup,
        &converter,
        &naer_lookup,
        &mut naer_stats,
    )?;
    sort_city_rows_for_golden(&mut cities_rows);

    let mut admin1_rows = translate_admin1_rows(
        admin1_rows,
        &alternate_lookup,
        &converter,
        &naer_lookup,
        &admin1_centroids,
        &mut naer_stats,
    )?;
    admin1_rows.sort_by(|left, right| left[0].cmp(&right[0]).then(left[3].cmp(&right[3])));

    let output_dir = fixture_output.join("translate");
    write_cities_rows_direct(&output_dir.join("cities500_translated.txt"), &cities_rows)?;
    write_admin1_rows_direct(
        &output_dir.join("admin1CodesASCII_translated.txt"),
        &admin1_rows,
    )?;
    println!("{}", naer_stats.log_line());
    println!(
        "stage=translate fixture={} cities_rows={} admin1_rows={}",
        fixture.manifest.name,
        cities_rows.len(),
        admin1_rows.len()
    );
    Ok(())
}

/// 執行 production translate 並回傳 NAER 統計，供品質 gate 與測試斷言。
pub fn run_production(options: &ProductionTranslateOptions) -> Result<NaerStats, String> {
    let mut profile = TranslateProfile::new(options.profile);
    let metadata = profile.time("load_metadata", || {
        load_metadata_dataframe(&options.metadata_dir)
    })?;
    let alternate_names = if options.alternate_name_file.exists() {
        profile.time("load_alternate_names", || {
            load_alternate_names_dataframe(&options.alternate_name_file)
        })?
    } else {
        profile.time("build_alternate_names", || {
            build_alternate_names_dataframe(&options.data_dir, &options.alternate_name_file)
        })?
    };

    let cities_rows = profile.time("read_cities", || {
        read_delimited(&options.cities_file, '\t', false)
    })?;
    let admin1_rows = profile.time("read_admin1", || {
        read_delimited(&options.admin1_file, '\t', false)
    })?;
    let converter = profile.time("build_opencc_converter", OpenCcConverter::new_lazy)?;
    let metadata_lookup = profile.time("build_metadata_lookup", || {
        metadata_lookup_from_dataframe(&metadata)
    })?;
    let alternate_lookup = profile.time("build_alternate_lookup", || {
        alternate_lookup_from_dataframe(&alternate_names)
    })?;
    let naer_lookup = profile.time("load_naer", || NaerLookup::load(&options.naer_file))?;
    // Reason: cities_rows 隨後被 translate_cities_rows by-value 消費並
    // shadow，admin1 質心索引必須在此之前以未翻譯列建立。
    let admin1_centroids = profile.time("build_admin1_centroids", || {
        Ok(build_admin1_centroids(&cities_rows))
    })?;
    let mut naer_stats = NaerStats::default();
    let cities_rows = profile.time("translate_cities", || {
        translate_cities_rows(
            cities_rows,
            &metadata_lookup,
            &alternate_lookup,
            &converter,
            &naer_lookup,
            &mut naer_stats,
        )
    })?;
    let admin1_rows = profile.time("translate_admin1", || {
        translate_admin1_rows(
            admin1_rows,
            &alternate_lookup,
            &converter,
            &naer_lookup,
            &admin1_centroids,
            &mut naer_stats,
        )
    })?;

    profile.time("write_cities", || {
        write_cities_rows_direct(
            &options.output_dir.join("cities500_translated.txt"),
            &cities_rows,
        )
    })?;
    profile.time("write_admin1", || {
        write_admin1_rows_direct(
            &options.output_dir.join("admin1CodesASCII_translated.txt"),
            &admin1_rows,
        )
    })?;
    println!("{}", naer_stats.log_line());
    println!(
        "stage=translate mode=production output={} cities_rows={} admin1_rows={}",
        options.output_dir.display(),
        cities_rows.len(),
        admin1_rows.len()
    );
    profile.print();
    Ok(naer_stats)
}

struct TranslateProfile {
    enabled: bool,
    started: Instant,
    timings: Vec<(&'static str, u128)>,
}

impl TranslateProfile {
    fn new(enabled: bool) -> Self {
        Self {
            enabled,
            started: Instant::now(),
            timings: Vec::new(),
        }
    }

    fn time<T, F>(&mut self, name: &'static str, action: F) -> Result<T, String>
    where
        F: FnOnce() -> Result<T, String>,
    {
        let started = Instant::now();
        let result = action();
        if self.enabled {
            self.timings.push((name, started.elapsed().as_millis()));
        }
        result
    }

    fn print(&self) {
        if !self.enabled {
            return;
        }
        let mut line = format!(
            "profile stage=translate.detail total_ms={}",
            self.started.elapsed().as_millis()
        );
        for (name, elapsed_ms) in &self.timings {
            line.push_str(&format!(" {name}_ms={elapsed_ms}"));
        }
        println!("{line}");
    }
}

fn write_cities_rows_direct(output: &Path, rows: &[Vec<String>]) -> Result<(), String> {
    write_delimited(output, '\t', None, rows)
}

fn write_admin1_rows_direct(output: &Path, rows: &[Vec<String>]) -> Result<(), String> {
    write_delimited(output, '\t', None, rows)
}

fn build_alternate_names_dataframe(data_dir: &Path, output: &Path) -> Result<DataFrame, String> {
    let rows = build_alternate_name_rows_from_data_dir(data_dir)?;
    write_alternate_name_rows_with_header(output, &rows)?;
    alternate_name_rows_to_dataframe(&rows)
}

fn build_alternate_name_rows_from_data_dir(data_dir: &Path) -> Result<Vec<Vec<String>>, String> {
    let source = data_dir.join("alternateNamesV2.txt");
    if !source.exists() {
        return Err(format!(
            "替代名稱檔案不存在：{}；請先執行 prepare 或指定 --alternate-name-file",
            source.display()
        ));
    }
    build_alternate_name_rows(&source)
}

fn build_alternate_name_rows(source: &Path) -> Result<Vec<Vec<String>>, String> {
    let priority = [
        "zh-Hant", "zh-TW", "zh-HK", "zh", "zh-Hans", "zh-CN", "zh-SG",
    ];
    let df = read_alternate_names_v2_dataframe(source)?;
    let mut filtered = filter_chinese_alternate_names(&df, &priority)?;
    let priority_values = alternate_name_priority_values(&filtered, &priority)?;
    filtered
        .with_column(UInt32Chunked::from_vec("priority".into(), priority_values).into_column())
        .map_err(|error| format!("無法建立 alternateNamesV2 priority 欄位：{error}"))?;

    let sorted = filtered
        .sort(
            ["priority"],
            SortMultipleOptions::new().with_maintain_order(true),
        )
        .map_err(|error| format!("Polars alternateNamesV2 priority sort 失敗：{error}"))?;
    let subset = ["geoname_id".to_string()];
    let selected = sorted
        .unique_stable(Some(&subset), UniqueKeepStrategy::First, None)
        .and_then(|df| df.select(["geoname_id", "name"]))
        .map_err(|error| format!("Polars alternateNamesV2 group/select 失敗：{error}"))?;
    alternate_name_dataframe_to_rows(&selected)
}

fn read_alternate_names_v2_dataframe(source: &Path) -> Result<DataFrame, String> {
    let file = File::open(source)
        .map_err(|error| format!("無法開啟 alternateNamesV2 {}：{error}", source.display()))?;
    let projection = Arc::new(vec![1_usize, 2, 3, 4]);
    let dtype_overwrite = Arc::new(vec![
        DataType::String,
        DataType::String,
        DataType::String,
        DataType::String,
    ]);
    let mut df = CsvReadOptions::default()
        .with_has_header(false)
        .with_projection(Some(projection))
        .with_dtype_overwrite(Some(dtype_overwrite))
        .map_parse_options(|parse_options| {
            parse_options
                .with_separator(b'\t')
                .with_missing_is_null(true)
                .with_null_values(Some(NullValues::AllColumnsSingle("\\N".into())))
        })
        .into_reader_with_file_handle(file)
        .finish()
        .map_err(|error| {
            format!(
                "Polars 無法讀取 alternateNamesV2 {}：{error}",
                source.display()
            )
        })?;
    df.set_column_names(&["geoname_id", "lang", "name", "is_preferred_name"])
        .map_err(|error| format!("Polars alternateNamesV2 欄位命名失敗：{error}"))?;
    Ok(df)
}

fn alternate_name_priority_values(df: &DataFrame, priority: &[&str]) -> Result<Vec<u32>, String> {
    let langs = df
        .column("lang")
        .and_then(|column| column.str())
        .map_err(|error| format!("Polars alternateNamesV2 lang 欄位錯誤：{error}"))?;
    let preferred = df
        .column("is_preferred_name")
        .map_err(|error| format!("Polars alternateNamesV2 is_preferred_name 欄位錯誤：{error}"))?;
    let fallback = u32::try_from(priority.len() + 1)
        .map_err(|error| format!("alternateNamesV2 priority 長度過大：{error}"))?;
    Ok((0..df.height())
        .map(|index| {
            if is_preferred_alternate_name(preferred, index) {
                0
            } else {
                langs
                    .get(index)
                    .and_then(|lang| {
                        priority
                            .iter()
                            .position(|candidate| *candidate == lang)
                            .and_then(|priority_index| u32::try_from(priority_index + 1).ok())
                    })
                    .unwrap_or(fallback)
            }
        })
        .collect())
}

fn is_preferred_alternate_name(column: &Column, index: usize) -> bool {
    match column.get(index) {
        Ok(AnyValue::Int64(1))
        | Ok(AnyValue::Int32(1))
        | Ok(AnyValue::UInt64(1))
        | Ok(AnyValue::UInt32(1))
        | Ok(AnyValue::String("1")) => true,
        Ok(AnyValue::StringOwned(value)) => value.as_str() == "1",
        _ => false,
    }
}

fn filter_chinese_alternate_names(df: &DataFrame, priority: &[&str]) -> Result<DataFrame, String> {
    let langs = df
        .column("lang")
        .and_then(|column| column.str())
        .map_err(|error| format!("Polars alternateNamesV2 lang 欄位錯誤：{error}"))?;
    let mask = BooleanChunked::from_iter_options(
        "is_chinese".into(),
        (0..df.height()).map(|index| langs.get(index).map(|lang| priority.contains(&lang))),
    );
    df.filter(&mask)
        .map_err(|error| format!("Polars alternateNamesV2 中文語言篩選失敗：{error}"))
}

fn alternate_name_dataframe_to_rows(df: &DataFrame) -> Result<Vec<Vec<String>>, String> {
    let geoname_ids = df
        .column("geoname_id")
        .and_then(|column| column.str())
        .map_err(|error| format!("Polars alternateNamesV2 geoname_id 欄位錯誤：{error}"))?;
    let names = df
        .column("name")
        .and_then(|column| column.str())
        .map_err(|error| format!("Polars alternateNamesV2 name 欄位錯誤：{error}"))?;
    let mut rows = (0..df.height())
        .map(|index| {
            vec![
                geoname_ids.get(index).unwrap_or_default().to_string(),
                names
                    .get(index)
                    .unwrap_or_default()
                    .replace("桃園縣", "桃園市"),
            ]
        })
        .collect::<Vec<Vec<String>>>();
    rows.sort_by(|left, right| left[0].cmp(&right[0]));
    Ok(rows)
}

/// 由檔名解析 LocationIQ metadata 的國碼。
///
/// `meta_data/` 同時放兩種語義不同、欄位卻相同的檔案：LocationIQ 逆地理查詢
/// 產物 `{CC}.csv`（ISO-3166-1 alpha-2）與各國 handler 的 extract 產物
/// `{cc}_geodata.csv`。translate 的查表只認前者；後者由 enhance 階段
/// （`admin1_load` / `cities500_load`）以明確檔名消費，其內容早已寫入
/// cities500，不應在此重複載入。
///
/// Reason: 舊版直接把檔名 stem 當國碼，handler geodata 檔自 2025-04 進入
/// `meta_data/` 後被當成國碼 `tw_geodata` 的 metadata 載入——25 萬列永不
/// 命中、不影響輸出，也沒有任何 log。大小寫一併正規化，因為
/// `--country-code us` 會產出 `us.csv`，而 cities500 國碼為大寫，
/// 不正規化同樣會靜默失效。
fn locationiq_country_code(file_path: &Path) -> Option<String> {
    let stem = file_path.file_stem().and_then(|value| value.to_str())?;
    (stem.len() == 2 && stem.chars().all(|value| value.is_ascii_alphabetic()))
        .then(|| stem.to_ascii_uppercase())
}

fn load_metadata_dataframe(path: &Path) -> Result<DataFrame, String> {
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

fn empty_metadata_dataframe() -> Result<DataFrame, String> {
    DataFrame::new(
        0,
        vec![
            Series::new("country_code".into(), Vec::<String>::new()).into(),
            Series::new("latitude".into(), Vec::<String>::new()).into(),
            Series::new("longitude".into(), Vec::<String>::new()).into(),
            Series::new("_meta_admin_2".into(), Vec::<String>::new()).into(),
        ],
    )
    .map_err(|error| format!("無法建立空 metadata DataFrame：{error}"))
}

fn metadata_rows_to_dataframe(
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
            Series::new(
                "_meta_admin_2".into(),
                rows.iter().map(|row| row[4].clone()).collect::<Vec<_>>(),
            )
            .into(),
        ],
    )
    .map_err(|error| format!("無法建立 metadata DataFrame：{error}"))
}

fn load_alternate_names_dataframe(path: &Path) -> Result<DataFrame, String> {
    if !path.exists() {
        return empty_alternate_names_dataframe();
    }
    alternate_name_rows_to_dataframe(&read_alternate_name_rows_with_header(path)?)
}

fn empty_alternate_names_dataframe() -> Result<DataFrame, String> {
    DataFrame::new(
        0,
        vec![
            Series::new("geoname_id".into(), Vec::<String>::new()).into(),
            Series::new("name".into(), Vec::<String>::new()).into(),
        ],
    )
    .map_err(|error| format!("無法建立空 alternate-name DataFrame：{error}"))
}

fn alternate_name_rows_to_dataframe(rows: &[Vec<String>]) -> Result<DataFrame, String> {
    DataFrame::new(
        rows.len(),
        vec![
            Series::new(
                "geoname_id".into(),
                rows.iter()
                    .map(|row| row.first().cloned().unwrap_or_default())
                    .collect::<Vec<_>>(),
            )
            .into(),
            Series::new(
                "name".into(),
                rows.iter()
                    .map(|row| row.get(1).cloned().unwrap_or_default())
                    .collect::<Vec<_>>(),
            )
            .into(),
        ],
    )
    .map_err(|error| format!("無法建立 alternate-name DataFrame：{error}"))
}

#[derive(Debug, Clone)]
struct OpenCcConverter {
    s2t_converter: Converter,
    t2s_converter: Converter,
    s2t: RefCell<HashMap<String, String>>,
    t2s: RefCell<HashMap<String, String>>,
}

impl OpenCcConverter {
    fn new_lazy() -> Result<Self, String> {
        Ok(Self {
            s2t_converter: native_converter("s2t")?,
            t2s_converter: native_converter("t2s")?,
            s2t: RefCell::new(HashMap::new()),
            t2s: RefCell::new(HashMap::new()),
        })
    }

    fn new(values: Vec<String>) -> Result<Self, String> {
        let mut unique: Vec<String> = values
            .into_iter()
            .filter(|value| !value.is_empty())
            .collect::<HashSet<_>>()
            .into_iter()
            .collect();
        unique.sort();
        let converter = Self::new_lazy()?;
        for value in unique {
            converter.s2t(&value);
            converter.t2s(&value);
        }
        Ok(converter)
    }

    fn s2t(&self, text: &str) -> String {
        if let Some(converted) = self.s2t.borrow().get(text) {
            return converted.clone();
        }
        let converted = self.s2t_converter.convert(text);
        self.s2t
            .borrow_mut()
            .insert(text.to_string(), converted.clone());
        converted
    }

    fn t2s(&self, text: &str) -> String {
        if let Some(converted) = self.t2s.borrow().get(text) {
            return converted.clone();
        }
        let converted = self.t2s_converter.convert(text);
        self.t2s
            .borrow_mut()
            .insert(text.to_string(), converted.clone());
        converted
    }
}

#[cfg(test)]
fn run_opencc(config: &str, values: &[String]) -> Result<HashMap<String, String>, String> {
    run_native_opencc(config, values)
}

#[cfg(test)]
fn run_native_opencc(config: &str, values: &[String]) -> Result<HashMap<String, String>, String> {
    if values.is_empty() {
        return Ok(HashMap::new());
    }

    let converter = native_converter(config)?;
    Ok(values
        .iter()
        .map(|value| (value.clone(), converter.convert(value)))
        .collect())
}

fn native_converter(config: &str) -> Result<Converter, String> {
    match config {
        "s2t" => opencc_rust::presets::cn2t::converter("cn", "t"),
        "t2s" => opencc_rust::presets::t2cn::converter("t", "cn"),
        other => {
            return Err(format!(
                "不支援的 OpenCC native config：{other}；目前僅支援 s2t/t2s"
            ));
        }
    }
    .map_err(|error| format!("無法建立 OpenCC native converter：{error}"))
}

fn collect_translation_values(
    metadata: &DataFrame,
    alternate_names: &DataFrame,
    city_rows: &[Vec<String>],
) -> Vec<String> {
    let mut values = Vec::new();
    extend_han_values(
        &mut values,
        string_column_values(metadata, "_meta_admin_2").unwrap_or_default(),
    );
    extend_han_values(
        &mut values,
        string_column_values(alternate_names, "name").unwrap_or_default(),
    );
    for row in city_rows {
        if let Some(alternatenames) = row.get(3) {
            extend_han_values(
                &mut values,
                alternatenames.split(',').map(ToString::to_string),
            );
        }
    }
    values
}

fn extend_han_values(values: &mut Vec<String>, candidates: impl IntoIterator<Item = String>) {
    values.extend(
        candidates
            .into_iter()
            .filter(|value| !value.is_empty() && includes_han(value)),
    );
}

fn string_column_values(df: &DataFrame, name: &str) -> Result<Vec<String>, String> {
    let column = df
        .column(name)
        .and_then(|column| column.str())
        .map_err(|error| format!("Polars 欄位 {name} 錯誤：{error}"))?;
    Ok((0..df.height())
        .filter_map(|index| column.get(index).map(ToString::to_string))
        .collect())
}

fn string_column<'a>(df: &'a DataFrame, name: &str) -> Result<&'a StringChunked, String> {
    df.column(name)
        .and_then(|column| column.str())
        .map_err(|error| format!("Polars 欄位 {name} 錯誤：{error}"))
}

type AlternateLookup = HashMap<String, String>;

struct MetadataLookup {
    countries: HashSet<String>,
    names_by_coordinate: HashMap<(String, String, String), String>,
}

impl MetadataLookup {
    fn with_capacity(capacity: usize) -> Self {
        Self {
            countries: HashSet::new(),
            names_by_coordinate: HashMap::with_capacity(capacity),
        }
    }

    fn insert_first(&mut self, country_code: &str, latitude: &str, longitude: &str, name: &str) {
        self.countries.insert(country_code.to_string());
        self.names_by_coordinate
            .entry((
                country_code.to_string(),
                latitude.to_string(),
                longitude.to_string(),
            ))
            .or_insert_with(|| name.to_string());
    }

    fn get_city_name(&self, city: &CityRow) -> Option<&str> {
        if !self.countries.contains(city.country_code()) {
            return None;
        }
        self.names_by_coordinate
            .get(&city.metadata_key())
            .map(String::as_str)
    }
}

struct CityRow {
    row: Vec<String>,
}

impl CityRow {
    fn try_from_row(row: Vec<String>) -> Result<Self, String> {
        if row.len() != 19 {
            return Err(format!(
                "cities500 欄位數不符：expected=19 actual={}",
                row.len()
            ));
        }
        Ok(Self { row })
    }

    fn geoname_id(&self) -> &str {
        &self.row[0]
    }

    fn name(&self) -> &str {
        &self.row[1]
    }

    fn asciiname(&self) -> &str {
        &self.row[2]
    }

    fn alternatenames(&self) -> &str {
        &self.row[3]
    }

    fn latitude(&self) -> &str {
        &self.row[4]
    }

    fn longitude(&self) -> &str {
        &self.row[5]
    }

    fn country_code(&self) -> &str {
        &self.row[8]
    }

    fn metadata_key(&self) -> (String, String, String) {
        (
            self.country_code().to_string(),
            self.latitude().to_string(),
            self.longitude().to_string(),
        )
    }

    fn apply_name(&mut self, name: String) {
        self.row[1] = name;
        self.row[2] = self.row[1].clone();
    }

    fn sync_asciiname(&mut self) {
        self.row[2] = self.row[1].clone();
    }

    fn into_row(self) -> Vec<String> {
        self.row
    }
}

struct Admin1Row {
    row: Vec<String>,
}

impl Admin1Row {
    fn try_from_row(row: Vec<String>) -> Result<Self, String> {
        if row.len() != 4 {
            return Err(format!(
                "admin1 欄位數不符：expected=4 actual={}",
                row.len()
            ));
        }
        Ok(Self { row })
    }

    fn geoname_id(&self) -> &str {
        &self.row[3]
    }

    fn code(&self) -> &str {
        &self.row[0]
    }

    fn name(&self) -> &str {
        &self.row[1]
    }

    fn asciiname(&self) -> &str {
        &self.row[2]
    }

    fn apply_name(&mut self, name: String) {
        self.row[1] = name;
        self.row[2] = self.row[1].clone();
    }

    fn sync_asciiname(&mut self) {
        self.row[2] = self.row[1].clone();
    }

    fn into_row(self) -> Vec<String> {
        self.row
    }
}

fn metadata_lookup_from_dataframe(df: &DataFrame) -> Result<MetadataLookup, String> {
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

fn alternate_lookup_from_dataframe(df: &DataFrame) -> Result<AlternateLookup, String> {
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

fn translate_cities_rows(
    rows: Vec<Vec<String>>,
    metadata: &MetadataLookup,
    alternate_names: &AlternateLookup,
    converter: &OpenCcConverter,
    naer: &NaerLookup,
    naer_stats: &mut NaerStats,
) -> Result<Vec<Vec<String>>, String> {
    let mut translated = Vec::with_capacity(rows.len());
    for row in rows {
        let mut city = CityRow::try_from_row(row)?;
        let mut naer_applied = false;
        let final_name = if city.country_code() == "TW" {
            Some(city.name().to_string())
        } else {
            let existing = metadata
                .get_city_name(&city)
                .filter(|value| !value.is_empty())
                .and_then(|name| translate_metadata_name(name, converter))
                .or_else(|| {
                    alternate_names
                        .get(city.geoname_id())
                        .filter(|value| !value.is_empty())
                        .map(|name| translate_alternate_name(name, converter))
                })
                .or_else(|| extract_chinese_name(city.alternatenames(), converter));
            let naer_match = match (
                city.latitude().parse::<f64>(),
                city.longitude().parse::<f64>(),
            ) {
                (Ok(latitude), Ok(longitude)) => naer.lookup_city(
                    city.name(),
                    city.asciiname(),
                    latitude,
                    longitude,
                    city.country_code(),
                    naer_stats,
                ),
                _ => None,
            };
            match naer_match {
                Some(matched) if matched.confidence == NaerConfidence::High => {
                    if existing.is_some() {
                        naer_stats.city_override += 1;
                    } else {
                        naer_stats.city_fill += 1;
                    }
                    // Reason: 距離分布僅統計「被採用」的匹配；demote 保留
                    // 既有譯名時不記錄，避免污染品質報告的採用距離語意。
                    naer_stats.record_city_distance(matched.distance_km);
                    naer_applied = true;
                    Some(matched.name_zh)
                }
                Some(matched) => {
                    if existing.is_none() {
                        naer_stats.city_fill += 1;
                        naer_stats.record_city_distance(matched.distance_km);
                        naer_applied = true;
                        Some(matched.name_zh)
                    } else {
                        naer_stats.city_demoted_kept_existing += 1;
                        existing
                    }
                }
                None => existing,
            }
        };

        if let Some(name) = final_name {
            if naer_applied {
                // Reason: NAER 為官方審譯結果，原樣使用、不經 '裏'→'里' 後處理。
                city.apply_name(name);
            } else {
                city.apply_name(name.replacen('裏', "里", 1));
            }
        } else {
            city.sync_asciiname();
        }
        if !city.name().is_empty() {
            translated.push(city.into_row());
        }
    }
    Ok(translated)
}

fn translate_admin1_rows(
    rows: Vec<Vec<String>>,
    alternate_names: &AlternateLookup,
    converter: &OpenCcConverter,
    naer: &NaerLookup,
    admin1_centroids: &HashMap<String, (f64, f64)>,
    naer_stats: &mut NaerStats,
) -> Result<Vec<Vec<String>>, String> {
    let mut translated = Vec::with_capacity(rows.len());
    for row in rows {
        let mut admin1 = Admin1Row::try_from_row(row)?;
        if let Some(name) = alternate_names
            .get(admin1.geoname_id())
            .filter(|value| !value.is_empty())
        {
            let translated_name = if is_simplified_chinese(name, converter) {
                converter.s2t(name)
            } else {
                name.clone()
            };
            admin1.apply_name(translated_name);
        } else if let Some(name) = naer.lookup_admin1(
            admin1.name(),
            admin1.asciiname(),
            admin1.code(),
            admin1_centroids.get(admin1.code()).copied(),
            naer_stats,
        ) {
            // Reason: admin1 第一版僅補洞——只在既有來源無中文名時使用
            // NAER，覆寫待品質報告量化錯配率後再評估。
            naer_stats.admin1_fill += 1;
            admin1.apply_name(name);
        } else {
            admin1.sync_asciiname();
        }
        translated.push(admin1.into_row());
    }
    Ok(translated)
}

#[cfg(test)]
fn translate_cities(
    rows: &mut [Vec<String>],
    metadata: &HashMap<(String, String, String), String>,
    alternate_names: &HashMap<String, String>,
    converter: &OpenCcConverter,
) {
    for row in rows {
        let final_name = if row[8] == "TW" {
            Some(row[1].clone())
        } else {
            metadata
                .get(&(row[8].clone(), row[4].clone(), row[5].clone()))
                .and_then(|name| translate_metadata_name(name, converter))
                .or_else(|| {
                    alternate_names
                        .get(&row[0])
                        .filter(|value| !value.is_empty())
                        .map(|name| translate_alternate_name(name, converter))
                })
                .or_else(|| extract_chinese_name(&row[3], converter))
        };

        if let Some(name) = final_name {
            let name = name.replacen('裏', "里", 1);
            row[1] = name.clone();
            row[2] = name;
        }
        row[2] = row[1].clone();
    }
}

#[cfg(test)]
fn translate_admin1(
    rows: &mut [Vec<String>],
    alternate_names: &HashMap<String, String>,
    converter: &OpenCcConverter,
) {
    for row in rows {
        if let Some(name) = alternate_names
            .get(&row[3])
            .filter(|value| !value.is_empty())
        {
            let translated = if is_simplified_chinese(name, converter) {
                converter.s2t(name)
            } else {
                name.clone()
            };
            row[1] = translated.clone();
            row[2] = translated;
        }
        row[2] = row[1].clone();
    }
}

fn translate_metadata_name(name: &str, converter: &OpenCcConverter) -> Option<String> {
    if !is_chinese_name(name) {
        None
    } else if is_simplified_chinese(name, converter) {
        Some(converter.s2t(name))
    } else {
        Some(name.to_string())
    }
}

fn translate_alternate_name(name: &str, converter: &OpenCcConverter) -> String {
    if is_traditional_chinese(name, converter) {
        name.to_string()
    } else {
        converter.s2t(name)
    }
}

fn extract_chinese_name(alternate_names: &str, converter: &OpenCcConverter) -> Option<String> {
    let mut simplified_candidate = None;
    let mut generic_candidate = None;

    for name in alternate_names.split(',') {
        if is_traditional_chinese(name, converter) {
            return Some(name.to_string());
        }
        if is_simplified_chinese(name, converter) && simplified_candidate.is_none() {
            simplified_candidate = Some(name.to_string());
        } else if includes_han(name) && generic_candidate.is_none() {
            generic_candidate = Some(name.to_string());
        }
    }

    simplified_candidate
        .map(|name| converter.s2t(&name))
        .or(generic_candidate)
}

fn is_simplified_chinese(text: &str, converter: &OpenCcConverter) -> bool {
    is_chinese_name(text) && text == converter.t2s(text)
}

fn is_traditional_chinese(text: &str, converter: &OpenCcConverter) -> bool {
    is_chinese_name(text) && text == converter.s2t(text)
}

fn is_chinese_name(text: &str) -> bool {
    is_han_name(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn translate_city_prefers_metadata_over_alternate_name() {
        let mut rows = vec![city_row("4000", "Redwood Preferred", "US")];
        let mut metadata = HashMap::new();
        metadata.insert(
            (
                "US".to_string(),
                "37.00000000".to_string(),
                "-122.00000000".to_string(),
            ),
            "紅木市".to_string(),
        );
        let mut alternate_names = HashMap::new();
        alternate_names.insert("4000".to_string(), "紅木備用".to_string());
        let converter = converter_for(["紅木市", "紅木備用"]);

        translate_cities(&mut rows, &metadata, &alternate_names, &converter);

        assert_eq!(rows[0][1], "紅木市");
        assert_eq!(rows[0][2], "紅木市");
    }

    #[test]
    fn translate_city_converts_simplified_metadata_with_opencc() {
        let mut rows = vec![city_row("3040131", "Massana", "AD")];
        let mut metadata = HashMap::new();
        metadata.insert(
            (
                "AD".to_string(),
                "37.00000000".to_string(),
                "-122.00000000".to_string(),
            ),
            "马萨纳".to_string(),
        );
        let converter = converter_for(["马萨纳"]);

        translate_cities(&mut rows, &metadata, &HashMap::new(), &converter);

        assert_eq!(rows[0][1], "馬薩納");
        assert_eq!(rows[0][2], "馬薩納");
    }

    #[test]
    fn translate_city_matches_single_li_replacement() {
        let mut rows = vec![city_row("2767466", "Ried", "AT")];
        let mut alternate_names = HashMap::new();
        alternate_names.insert("2767466".to_string(), "裏德馬克地區裏德".to_string());
        let converter = converter_for(["裏德馬克地區裏德"]);

        translate_cities(&mut rows, &HashMap::new(), &alternate_names, &converter);

        assert_eq!(rows[0][1], "里德馬克地區裏德");
        assert_eq!(rows[0][2], "里德馬克地區裏德");
    }

    #[test]
    fn extract_chinese_name_prefers_traditional_then_simplified() {
        let converter = converter_for(["汉字", "漢字"]);

        assert_eq!(
            extract_chinese_name("汉字,漢字", &converter),
            Some("漢字".to_string())
        );
        assert_eq!(
            extract_chinese_name("汉字,Latin", &converter),
            Some("漢字".to_string())
        );
    }

    #[test]
    fn han_detection_covers_extension_blocks_without_ascii_fallback() {
        assert!(is_chinese_name("𠀀-臺"));
        assert!(includes_han("A𠀀B"));
        assert!(includes_han("unmu・aru・kaiwain"));
        assert!(includes_han("Frîn·ne-d'léz-Bujnal"));
        assert!(!is_chinese_name("São Tomé"));
    }

    #[test]
    fn translate_city_preserves_diacritic_name_when_no_chinese_candidate() {
        let mut rows = vec![city_row("5000", "São Tomé", "ST")];
        rows[0][2] = "Sao Tome".to_string();
        rows[0][3] = "Sao Tome,San Tome".to_string();
        let converter = converter_for(["Sao Tome", "San Tome"]);

        translate_cities(&mut rows, &HashMap::new(), &HashMap::new(), &converter);

        assert_eq!(rows[0][1], "São Tomé");
        assert_eq!(rows[0][2], "São Tomé");
    }

    #[test]
    fn translate_admin1_converts_simplified_alternate_name() {
        let mut rows = vec![vec![
            "AD.04".to_string(),
            "La Massana".to_string(),
            "La Massana".to_string(),
            "3040131".to_string(),
        ]];
        let mut alternate_names = HashMap::new();
        alternate_names.insert("3040131".to_string(), "马萨纳".to_string());
        let converter = converter_for(["马萨纳"]);

        translate_admin1(&mut rows, &alternate_names, &converter);

        assert_eq!(rows[0][1], "馬薩納");
        assert_eq!(rows[0][2], "馬薩納");
    }

    #[test]
    fn translate_admin1_preserves_diacritics_in_asciiname_like_reference() {
        let mut rows = vec![vec![
            "AL.41".to_string(),
            "Dibër County".to_string(),
            "Diber County".to_string(),
            "865731".to_string(),
        ]];
        let converter = converter_for(["Dibër County"]);

        translate_admin1(&mut rows, &HashMap::new(), &converter);

        assert_eq!(rows[0][1], "Dibër County");
        assert_eq!(rows[0][2], "Dibër County");
    }

    #[test]
    fn native_opencc_keeps_rust_dictionary_variant_regressions() {
        let samples = [
            ("竹溪城关镇", "竹溪城關鎮", "竹溪城关镇"),
            ("浚县城关镇", "浚縣城關鎮", "浚县城关镇"),
            ("兰溪", "蘭溪", "兰溪"),
            ("慈溪", "慈溪", "慈溪"),
            ("辰溪县", "辰溪縣", "辰溪县"),
            ("栗溪", "慄溪", "栗溪"),
            ("木栗", "木慄", "木栗"),
            ("浮梁", "浮樑", "浮梁"),
            ("绥棱", "綏棱", "绥棱"),
            ("穆棱", "穆棱", "穆棱"),
        ];
        let values: Vec<String> = samples
            .iter()
            .map(|(input, _s2t, _t2s)| input.to_string())
            .collect();
        let s2t = run_native_opencc("s2t", &values).unwrap();
        let t2s = run_native_opencc("t2s", &values).unwrap();

        for (input, expected_s2t, expected_t2s) in samples {
            assert_eq!(s2t[input], expected_s2t);
            assert_eq!(t2s[input], expected_t2s);
        }
    }

    #[test]
    fn native_opencc_matches_reference_spike_cases() {
        let samples = [
            ("马萨纳", "馬薩納", "马萨纳"),
            ("裏德馬克地區裏德", "裏德馬克地區裏德", "里德马克地区里德"),
            ("里仁官庄", "里仁官莊", "里仁官庄"),
            (
                "圣胡利娅-德洛里亚",
                "聖胡利婭-德洛里亞",
                "圣胡利娅-德洛里亚",
            ),
            (
                "萊塞斯卡爾德－恩戈爾達",
                "萊塞斯卡爾德－恩戈爾達",
                "莱塞斯卡尔德－恩戈尔达",
            ),
            ("OpenCC", "OpenCC", "OpenCC"),
            ("São Tomé", "São Tomé", "São Tomé"),
            ("混合Mixed中文", "混合Mixed中文", "混合Mixed中文"),
            ("𠀀-臺", "𠀀-臺", "𠀀-台"),
        ];
        let values: Vec<String> = samples
            .iter()
            .map(|(input, _s2t, _t2s)| input.to_string())
            .collect();
        let s2t = run_opencc("s2t", &values).unwrap();
        let t2s = run_opencc("t2s", &values).unwrap();

        for (input, expected_s2t, expected_t2s) in samples {
            assert_eq!(s2t[input], expected_s2t);
            assert_eq!(t2s[input], expected_t2s);
        }
    }

    #[test]
    fn alternate_names_builder_handles_quoted_tsv_like_polars() {
        let path = std::env::temp_dir().join(format!(
            "alternate_names_quoted_{}_{}.txt",
            std::process::id(),
            "polars"
        ));
        fs::write(&path, "1\t4000\tzh\t\"臺\t北\"\t0\n").unwrap();

        let rows = build_alternate_name_rows(&path).unwrap();

        assert_eq!(rows, vec![vec!["4000".to_string(), "臺\t北".to_string()]]);
        let _ = fs::remove_file(path);
    }

    fn converter_for(values: impl IntoIterator<Item = &'static str>) -> OpenCcConverter {
        OpenCcConverter::new(values.into_iter().map(ToString::to_string).collect()).unwrap()
    }

    fn city_row(geoname_id: &str, name: &str, country_code: &str) -> Vec<String> {
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
