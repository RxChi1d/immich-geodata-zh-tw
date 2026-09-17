use std::path::{Path, PathBuf};
use std::time::Instant;

use polars::prelude::*;

use crate::cli::RunOptions;
use crate::pipeline::admin1_correct_stage::{self};
use crate::pipeline::fixtures::{Fixture, load_fixtures};
use crate::pipeline::naer_lookup::{NaerLookup, build_admin1_centroids};
use crate::pipeline::naer_stats::NaerStats;
use crate::pipeline::polars_table::{read_admin1_rows, read_cities_rows};
use crate::pipeline::prepare_download::NATURAL_EARTH_ADMIN1_FILE;
use crate::pipeline::table::{read_delimited, write_delimited};
use crate::pipeline::transform_cities_schema::sort_city_rows_for_golden;
use crate::pipeline::{admin1_load, cities500_load};
use crate::unicode_han::includes_han;

mod alternate_names;
mod dataframe;
mod opencc;
mod rows;

use alternate_names::*;
use dataframe::*;
use opencc::*;
use rows::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProductionTranslateOptions {
    /// LocationIQ 逆地理查詢產物的目錄（production 為 `data/locationiq/`）。
    /// 不是 handler 的 `data/handler/`——後者由 enhance 階段消費。
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

    // Reason: 必須排在 build_admin1_centroids 之前。質心是依 admin1 分群算出來的，
    // 若先算質心再改 admin1，被搬走的城市仍會計入原本那一州的質心，NAER 譯名
    // 比對就會用到與輸出不一致的座標。
    let mut cities_rows = cities_rows;
    profile.time("admin1_correct", || {
        let metadata_admin1 = metadata_admin1_from_dataframe(&metadata)?;
        admin1_correct_stage::run(
            &mut cities_rows,
            &admin1_rows,
            &metadata_admin1,
            &options.data_dir.join(NATURAL_EARTH_ADMIN1_FILE),
            &options.data_dir.join("admin2Codes.txt"),
            &options.metadata_dir,
        )
    })?;

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

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::fs;

    use super::*;

    /// metadata 是補位而非覆蓋：GeoNames 中文別名存在時優先採用。
    ///
    /// Reason: metadata 來自 Nominatim 的 `city`／`county`，聚落標記稀疏處會退回
    /// 轄區，讓它覆蓋既有譯名會把城市名塌成上一層（蕉賴 → 吉隆坡）。
    #[test]
    fn translate_city_prefers_alternate_name_over_metadata() {
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

        assert_eq!(rows[0][1], "紅木備用");
        assert_eq!(rows[0][2], "紅木備用");
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
