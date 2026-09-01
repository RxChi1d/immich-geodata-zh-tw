use std::fs;
use std::path::{Path, PathBuf};
use std::time::Instant;

use crate::pipeline::prepare::ProductionPrepareOptions;
use crate::pipeline::{self, Stage};
use crate::pipeline::{
    admin1_load, cities500_load, extract, locationiq, naer_prepare, pack, translate,
};

const HELP: &str = "\
immich-geodata

USAGE:
  immich-geodata help
  immich-geodata list-stages
  immich-geodata run-stage --stage <stage> [--fixture <name>] [--fixtures-dir <path>] [--output-dir <path>]
  immich-geodata full-pipeline [--fixture <name>] [--fixtures-dir <path>] [--output-dir <path>]
  immich-geodata prepare [--country-code <cc...>] [--data-folder <path>] [--update]
  immich-geodata <cleanup|prepare|extract|enhance|locationiq|translate|pack|release|naer-prepare> [--dry-run|--fixture-mode|--profile] [options]
  immich-geodata naer-prepare --input <原始CSV> [--output <vendored_path>] [--country-names <json_path>]
";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunOptions {
    pub fixture: Option<String>,
    pub fixtures_dir: PathBuf,
    pub output_dir: PathBuf,
}

impl Default for RunOptions {
    fn default() -> Self {
        Self {
            fixture: None,
            fixtures_dir: PathBuf::from("fixtures"),
            output_dir: PathBuf::from("target/stage-output"),
        }
    }
}

pub fn run(args: Vec<String>) -> Result<(), String> {
    let command = args.get(1).map(String::as_str).unwrap_or("help");
    match command {
        "help" | "--help" | "-h" => {
            print!("{HELP}");
            Ok(())
        }
        "list-stages" => {
            for stage in Stage::all() {
                println!("{}", stage.as_str());
            }
            Ok(())
        }
        "run-stage" => {
            let (stage, options) = parse_run_stage_args(&args[2..])?;
            pipeline::run_stage(stage, &options)
        }
        "full-pipeline" => {
            let options = parse_options(&args[2..])?;
            pipeline::run_full_pipeline(&options)
        }
        "cleanup" | "prepare" | "extract" | "enhance" | "locationiq" | "translate" | "pack"
        | "release" => run_production_command(command, &args[2..]),
        "naer-prepare" => run_naer_prepare_command(&args[2..]),
        other => Err(format!("未知命令：{other}\n\n{HELP}")),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ProductionOptions {
    dry_run: bool,
    fixture_mode: bool,
    profile: bool,
    update_prepare: bool,
    overwrite: bool,
    pass_cleanup: bool,
    pass_prepare: bool,
    pass_enhance: bool,
    pass_locationiq: bool,
    pass_translate: bool,
    pass_pack: bool,
    country_codes: Vec<String>,
    data_folder: PathBuf,
    output_folder: PathBuf,
    shapefile: Option<PathBuf>,
    extract_country: Option<String>,
    extract_output: Option<PathBuf>,
    cities_file: Option<PathBuf>,
    output_file: Option<PathBuf>,
    alternate_name_file: Option<PathBuf>,
    metadata_folder: PathBuf,
    locationiq_folder: PathBuf,
    min_population: u32,
    batch_size: u32,
    qps: u32,
    api_key: Option<String>,
}

impl Default for ProductionOptions {
    fn default() -> Self {
        Self {
            dry_run: false,
            fixture_mode: false,
            profile: false,
            update_prepare: false,
            overwrite: false,
            pass_cleanup: false,
            pass_prepare: false,
            pass_enhance: false,
            pass_locationiq: false,
            pass_translate: false,
            pass_pack: false,
            country_codes: vec!["TW".to_string()],
            data_folder: PathBuf::from("./geoname_data"),
            output_folder: PathBuf::from("./output"),
            shapefile: None,
            extract_country: None,
            extract_output: None,
            cities_file: None,
            output_file: None,
            alternate_name_file: None,
            metadata_folder: PathBuf::from("./meta_data"),
            // Reason: LocationIQ 產物與 handler extract 產物欄位同為 GEODATA_COLUMNS
            // 但生命週期相反——前者是重建需消耗付費 quota 的查詢結果，後者是 CLAUDE.md
            // 資料保護規則禁止重新產生的 canonical metadata。共用 meta_data/ 時只靠檔名
            // 大小寫區分，清理者無從判斷哪些檔案可動，因此以目錄分隔。刻意不跟隨
            // --metadata-folder：搬移 handler metadata 時不應連帶移動付費查詢結果。
            locationiq_folder: PathBuf::from("./data/locationiq"),
            min_population: 100,
            batch_size: 100,
            qps: 2,
            api_key: std::env::var("LOCATIONIQ_API_KEY").ok(),
        }
    }
}

fn run_production_command(command: &str, args: &[String]) -> Result<(), String> {
    let mut options = parse_production_options(args)?;
    filter_country_codes_without_handler(command, &mut options);
    if !options.dry_run && !options.fixture_mode {
        validate_production_contract(command, &options)?;
        run_real_production_command(command, &options)?;
        print_production_plan(command, &options);
        return Ok(());
    }
    validate_production_contract(command, &options)?;
    if options.fixture_mode && !options.dry_run {
        run_fixture_production(command, &options)?;
    }
    print_production_plan(command, &options);
    Ok(())
}

fn run_real_production_command(command: &str, options: &ProductionOptions) -> Result<(), String> {
    match command {
        "cleanup" => run_profiled_stage(options, "cleanup", || run_cleanup_production(options)),
        "prepare" => run_profiled_stage(options, "prepare", || run_prepare_production(options)),
        "extract" => run_profiled_stage(options, "extract", || run_extract_production(options)),
        "enhance" => {
            run_profiled_stage(options, "enhance", || run_enhance_production(options)).map(|_| ())
        }
        "locationiq" => {
            run_profiled_stage(options, "locationiq", || run_locationiq_production(options))
        }
        "translate" => {
            run_profiled_stage(options, "translate", || run_translate_production(options))
        }
        "pack" => run_profiled_stage(options, "pack", || run_pack_production(options)),
        "release" => run_release_production(options),
        other => Err(format!("未知 production 命令：{other}")),
    }
}

fn run_profiled_stage<T>(
    options: &ProductionOptions,
    stage: &str,
    operation: impl FnOnce() -> Result<T, String>,
) -> Result<T, String> {
    let start = Instant::now();
    let result = operation()?;
    if options.profile {
        println!(
            "profile stage={stage} elapsed_ms={}",
            start.elapsed().as_millis()
        );
    }
    Ok(result)
}

fn run_cleanup_production(options: &ProductionOptions) -> Result<(), String> {
    if options.output_folder.exists() {
        fs::remove_dir_all(&options.output_folder).map_err(|error| {
            format!(
                "無法清理 output folder {}：{error}",
                options.output_folder.display()
            )
        })?;
    }
    fs::create_dir_all(&options.output_folder).map_err(|error| {
        format!(
            "無法建立 output folder {}：{error}",
            options.output_folder.display()
        )
    })?;
    println!(
        "stage=cleanup mode=production output={}",
        options.output_folder.display()
    );
    Ok(())
}

fn run_prepare_production(options: &ProductionOptions) -> Result<(), String> {
    let prepare_options = ProductionPrepareOptions::new(
        options.data_folder.clone(),
        options.country_codes.clone(),
        options.update_prepare,
    );
    pipeline::prepare::run_production(&prepare_options)
}

fn run_extract_production(options: &ProductionOptions) -> Result<(), String> {
    let country = options
        .extract_country
        .as_deref()
        .ok_or_else(|| "extract 必須提供 --country".to_string())?;
    let input = options
        .shapefile
        .as_deref()
        .ok_or_else(|| "extract 必須提供 --shapefile".to_string())?;
    let output = options.extract_output.clone().unwrap_or_else(|| {
        options
            .metadata_folder
            .join(format!("{}_geodata.csv", country.to_lowercase()))
    });
    extract::run_production_with_profile(country, input, &output, options.profile)
}

fn run_enhance_production(options: &ProductionOptions) -> Result<i64, String> {
    let handler_countries = handler_countries_with_metadata(&options.metadata_folder);
    let base_max = calculate_global_max_geoname_id(options)?;
    let admin1_max = admin1_load::run_production(&admin1_load::ProductionAdmin1Options {
        input: options.data_folder.join("admin1CodesASCII.txt"),
        output: options.output_folder.join("admin1CodesASCII_optimized.txt"),
        metadata_dir: options.metadata_folder.clone(),
        handler_countries: handler_countries.clone(),
        base_geoname_id: base_max + 1,
    })?;
    let extra_files = non_handler_country_codes(&options.country_codes)
        .into_iter()
        .map(|country| {
            options
                .data_folder
                .join("extra_data")
                .join(format!("{country}.txt"))
        })
        .collect();
    cities500_load::run_production(&cities500_load::ProductionCities500Options {
        input: options
            .cities_file
            .clone()
            .unwrap_or_else(|| options.data_folder.join("cities500.txt")),
        output: options
            .output_file
            .clone()
            .unwrap_or_else(|| options.output_folder.join("cities500_optimized.txt")),
        extra_files,
        metadata_dir: options.metadata_folder.clone(),
        handler_countries,
        min_population: options.min_population,
        current_max_id: admin1_max,
        modification_date: current_date_iso()?,
    })
}

fn run_locationiq_production(options: &ProductionOptions) -> Result<(), String> {
    // Reason: 國家清單為空代表沒有任何需要 LocationIQ 的國家，整個階段不會發出請求。
    // 守衛必須放在取用 api_key 之前，否則單獨執行 locationiq 命令時會因為缺 key 而
    // 失敗，即使實際上一次 API 都不會呼叫。
    if options.country_codes.is_empty() {
        println!("stage=locationiq mode=production status=skip reason=no_non_handler_country");
        return Ok(());
    }
    let api_key = options
        .api_key
        .clone()
        .ok_or_else(|| "locationiq 需要 --locationiq-api-key 或 LOCATIONIQ_API_KEY".to_string())?;
    fs::create_dir_all(&options.locationiq_folder).map_err(|error| {
        format!(
            "無法建立 LocationIQ 資料目錄 {}：{error}",
            options.locationiq_folder.display()
        )
    })?;
    for country in &options.country_codes {
        let output_file = locationiq_output_path(options, country);
        locationiq::run_production(&locationiq::ProductionLocationiqOptions {
            cities_file: options.output_folder.join("cities500_optimized.txt"),
            output_file,
            country_code: country.clone(),
            batch_size: options.batch_size as usize,
            qps: options.qps,
            api_key: api_key.clone(),
            overwrite: options.overwrite,
        })?;
    }
    Ok(())
}

/// 某國 LocationIQ 產物的輸出路徑。
///
/// Reason: 抽成具名函式讓「寫入端與 translate 讀取端同源」這件事可被測試直接
/// 斷言。若哪天有人把 locationiq 改回寫入 `metadata_folder`，必須同時改動這裡
/// 與它的測試才能通過，不會像先前只比較欄位值那樣悄悄溜過去。
fn locationiq_output_path(options: &ProductionOptions, country: &str) -> PathBuf {
    options.locationiq_folder.join(format!("{country}.csv"))
}

fn run_translate_production(options: &ProductionOptions) -> Result<(), String> {
    // Reason: 統計已由 run_production 內部 log，CLI 不需回傳值。
    translate::run_production(&translate::ProductionTranslateOptions {
        // Reason: translate 的查表只認 LocationIQ 產物；handler 的
        // {cc}_geodata.csv 由 enhance 以明確檔名消費，不參與此處。
        metadata_dir: options.locationiq_folder.clone(),
        data_dir: options.data_folder.clone(),
        cities_file: options.output_folder.join("cities500_optimized.txt"),
        admin1_file: options.output_folder.join("admin1CodesASCII_optimized.txt"),
        alternate_name_file: options
            .alternate_name_file
            .clone()
            .unwrap_or_else(|| options.output_folder.join("alternate_chinese_name.csv")),
        naer_file: PathBuf::from("naer/naer_place_names.csv"),
        output_dir: options.output_folder.clone(),
        profile: options.profile,
    })
    .map(|_| ())
}

fn run_naer_prepare_command(args: &[String]) -> Result<(), String> {
    let mut input: Option<PathBuf> = None;
    let mut output = PathBuf::from("naer/naer_place_names.csv");
    let mut country_names_file = PathBuf::from("i18n-iso-countries/langs/zh-tw.json");
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--input" => {
                index += 1;
                input = Some(PathBuf::from(args.get(index).ok_or("--input 需要參數")?));
            }
            "--output" => {
                index += 1;
                output = PathBuf::from(args.get(index).ok_or("--output 需要參數")?);
            }
            "--country-names" => {
                index += 1;
                country_names_file =
                    PathBuf::from(args.get(index).ok_or("--country-names 需要參數")?);
            }
            other => return Err(format!("naer-prepare 不支援參數：{other}")),
        }
        index += 1;
    }
    let input = input.ok_or("naer-prepare 需要 --input <原始CSV>")?;
    naer_prepare::run(&naer_prepare::NaerPrepareOptions {
        input,
        output,
        country_names_file,
    })
    .map(|_| ())
}

fn run_pack_production(options: &ProductionOptions) -> Result<(), String> {
    pack::run_production(&pack::ProductionPackOptions {
        output_dir: options.output_folder.clone(),
        data_dir: options.data_folder.clone(),
        project_dir: PathBuf::from("."),
        release_date: current_date_iso()?,
        profile: options.profile,
    })
}

fn run_release_production(options: &ProductionOptions) -> Result<(), String> {
    if !options.pass_cleanup {
        run_profiled_stage(options, "cleanup", || run_cleanup_production(options))?;
    }
    if !options.pass_prepare {
        run_profiled_stage(options, "prepare", || run_prepare_production(options))?;
    }
    if !options.pass_enhance {
        run_profiled_stage(options, "enhance", || run_enhance_production(options))?;
    }
    if !options.pass_locationiq {
        // Reason: country_codes 進到這裡前已由 filter_country_codes_without_handler 濾掉
        // handler 國家，不需要（也不應該）再過濾一次，否則同一條規則會有兩份實作。
        run_profiled_stage(options, "locationiq", || run_locationiq_production(options))?;
    }
    if !options.pass_translate {
        run_profiled_stage(options, "translate", || run_translate_production(options))?;
    }
    if !options.pass_pack {
        run_profiled_stage(options, "pack", || run_pack_production(options))?;
    }
    Ok(())
}

fn parse_production_options(args: &[String]) -> Result<ProductionOptions, String> {
    let mut options = ProductionOptions::default();
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--dry-run" => {
                options.dry_run = true;
                index += 1;
            }
            "--fixture-mode" => {
                options.fixture_mode = true;
                index += 1;
            }
            "--profile" => {
                options.profile = true;
                index += 1;
            }
            "--update" | "--update-prepare" => {
                options.update_prepare = true;
                index += 1;
            }
            "--overwrite" => {
                options.overwrite = true;
                index += 1;
            }
            "--pass-cleanup" => {
                options.pass_cleanup = true;
                index += 1;
            }
            "--pass-prepare" => {
                options.pass_prepare = true;
                index += 1;
            }
            "--pass-enhance" => {
                options.pass_enhance = true;
                index += 1;
            }
            "--pass-locationiq" => {
                options.pass_locationiq = true;
                index += 1;
            }
            "--pass-translate" => {
                options.pass_translate = true;
                index += 1;
            }
            "--pass-pack" => {
                options.pass_pack = true;
                index += 1;
            }
            "--country-code" => {
                let (values, next) = collect_values(args, index + 1)?;
                options.country_codes = values;
                index = next;
            }
            "--country" => {
                options.extract_country = Some(required_value(args, index, "--country")?);
                index += 2;
            }
            "--data-folder" | "--source-folder" => {
                options.data_folder =
                    PathBuf::from(required_value(args, index, args[index].as_str())?);
                index += 2;
            }
            "--metadata-folder" => {
                options.metadata_folder =
                    PathBuf::from(required_value(args, index, "--metadata-folder")?);
                index += 2;
            }
            // Reason: LocationIQ 產物自 data/locationiq 起與 handler metadata 分家後，
            // --metadata-folder 不再搬動它。保留獨立旗標，既有自訂路徑的使用者才有辦法
            // 指回舊位置續查，不會因為升級而重跑一遍付費查詢。
            "--locationiq-folder" => {
                options.locationiq_folder =
                    PathBuf::from(required_value(args, index, "--locationiq-folder")?);
                index += 2;
            }
            "--output-folder" => {
                options.output_folder =
                    PathBuf::from(required_value(args, index, "--output-folder")?);
                index += 2;
            }
            "--shapefile" | "-s" => {
                options.shapefile = Some(PathBuf::from(required_value(
                    args,
                    index,
                    args[index].as_str(),
                )?));
                index += 2;
            }
            "--output" | "-o" => {
                options.extract_output = Some(PathBuf::from(required_value(
                    args,
                    index,
                    args[index].as_str(),
                )?));
                index += 2;
            }
            "--cities-file" => {
                options.cities_file =
                    Some(PathBuf::from(required_value(args, index, "--cities-file")?));
                index += 2;
            }
            "--output-file" => {
                options.output_file =
                    Some(PathBuf::from(required_value(args, index, "--output-file")?));
                index += 2;
            }
            "--alternate-name-file" => {
                options.alternate_name_file = Some(PathBuf::from(required_value(
                    args,
                    index,
                    "--alternate-name-file",
                )?));
                index += 2;
            }
            "--batch-size" => {
                options.batch_size = parse_u32_arg(args, index, "--batch-size")?;
                index += 2;
            }
            "--min-population" => {
                options.min_population = parse_u32_arg(args, index, "--min-population")?;
                index += 2;
            }
            "--locationiq-qps" => {
                options.qps = parse_u32_arg(args, index, "--locationiq-qps")?;
                index += 2;
            }
            "--locationiq-api-key" => {
                options.api_key = Some(required_value(args, index, "--locationiq-api-key")?);
                index += 2;
            }
            other => return Err(format!("未知 production 參數：{other}")),
        }
    }
    Ok(options)
}

fn required_value(args: &[String], index: usize, flag: &str) -> Result<String, String> {
    args.get(index + 1)
        .filter(|value| !value.starts_with('-'))
        .cloned()
        .ok_or_else(|| format!("{flag} 需要值"))
}

fn collect_values(args: &[String], mut index: usize) -> Result<(Vec<String>, usize), String> {
    let mut values = Vec::new();
    while let Some(value) = args.get(index) {
        if value.starts_with('-') {
            break;
        }
        values.push(value.to_string());
        index += 1;
    }
    if values.is_empty() {
        Err("--country-code 需要至少一個國家代碼".to_string())
    } else {
        Ok((values, index))
    }
}

fn parse_u32_arg(args: &[String], index: usize, flag: &str) -> Result<u32, String> {
    required_value(args, index, flag)?
        .parse()
        .map_err(|error| format!("{flag} 數值格式錯誤：{error}"))
}

fn validate_production_contract(command: &str, options: &ProductionOptions) -> Result<(), String> {
    if command == "extract" && (options.extract_country.is_none() || options.shapefile.is_none()) {
        return Err("extract 必須提供 --country 與 --shapefile".to_string());
    }
    if matches!(command, "locationiq" | "release")
        && !options.pass_locationiq
        && options.api_key.is_none()
        && !options.fixture_mode
        // Reason: country_codes 已由 filter_country_codes_without_handler 濾掉 handler 國家；
        // 清單為空時 locationiq 階段只會印出 no_non_handler_country 並跳過，不會呼叫 API，
        // 此時要求 API key 會讓純 handler 國家的 release 無謂失敗。
        && !options.country_codes.is_empty()
    {
        return Err(
            "locationiq/release 需要 --locationiq-api-key 或 LOCATIONIQ_API_KEY".to_string(),
        );
    }
    Ok(())
}

fn calculate_global_max_geoname_id(options: &ProductionOptions) -> Result<i64, String> {
    let mut max_id = 0_i64;
    for path in [
        options.data_folder.join("cities500.txt"),
        options.data_folder.join("admin1CodesASCII.txt"),
    ] {
        if !path.exists() {
            continue;
        }
        let content = fs::read_to_string(&path)
            .map_err(|error| format!("無法讀取 geoname id 來源 {}：{error}", path.display()))?;
        for line in content.lines().filter(|line| !line.is_empty()) {
            let fields: Vec<&str> = line.split('\t').collect();
            let candidate = if fields.len() == 4 {
                fields[3]
            } else {
                fields[0]
            };
            if let Ok(value) = candidate.parse::<i64>() {
                max_id = max_id.max(value);
            }
        }
    }
    if max_id == 0 {
        Ok(91_999_999)
    } else {
        Ok(max_id)
    }
}

fn handler_countries() -> Vec<String> {
    // Reason: 由 extract 的 Country enum 單一事實來源導出，
    // 新增國家時不會發生 CLI 清單與 handler 路由不同步。
    extract::handler_country_codes()
        .into_iter()
        .map(ToString::to_string)
        .collect()
}

fn is_handler_country(country: &str) -> bool {
    let normalized = country.to_ascii_uppercase();
    handler_countries().contains(&normalized)
}

fn handler_countries_with_metadata(metadata_folder: &Path) -> Vec<String> {
    handler_countries()
        .into_iter()
        .filter(|country| {
            metadata_folder
                .join(format!("{}_geodata.csv", country.to_lowercase()))
                .exists()
        })
        .collect()
}

fn non_handler_country_codes(country_codes: &[String]) -> Vec<String> {
    country_codes
        .iter()
        .filter(|country| !is_handler_country(country))
        .cloned()
        .collect()
}

fn filter_country_codes_without_handler(command: &str, options: &mut ProductionOptions) {
    if !matches!(command, "prepare" | "enhance" | "locationiq" | "release") {
        return;
    }

    let mut filtered = Vec::new();
    for country in &options.country_codes {
        if is_handler_country(country) {
            println!("country_handler_skip country={country} reason=handler");
        } else {
            filtered.push(country.clone());
        }
    }
    options.country_codes = filtered;
}

fn current_date_iso() -> Result<String, String> {
    Ok(chrono::Local::now().format("%F").to_string())
}

/// fixture-mode 煙測使用的 pack-only fixture；run_stage 會以同名子目錄輸出產物。
const RELEASE_SMOKE_FIXTURE: &str = "release_archive";

fn run_fixture_production(command: &str, options: &ProductionOptions) -> Result<(), String> {
    if !matches!(command, "pack" | "release") || options.pass_pack {
        return Ok(());
    }

    let run_options = RunOptions {
        fixture: Some(RELEASE_SMOKE_FIXTURE.to_string()),
        output_dir: options.output_folder.clone(),
        ..RunOptions::default()
    };
    pipeline::run_stage(Stage::Pack, &run_options)?;
    copy_fixture_release_artifacts(&options.output_folder)
}

fn copy_fixture_release_artifacts(output_folder: &Path) -> Result<(), String> {
    let pack_output = output_folder.join(RELEASE_SMOKE_FIXTURE).join("pack");
    for file_name in ["release.zip", "release.tar.gz"] {
        fs::copy(pack_output.join(file_name), output_folder.join(file_name))
            .map_err(|error| format!("無法複製 fixture release artifact {file_name}：{error}"))?;
    }
    Ok(())
}

fn print_production_plan(command: &str, options: &ProductionOptions) {
    println!(
        "command={command} dry_run={} fixture_mode={}",
        options.dry_run, options.fixture_mode
    );
    println!("data_folder={}", options.data_folder.display());
    println!("output_folder={}", options.output_folder.display());
    println!("country_code={}", options.country_codes.join(","));
    if matches!(command, "locationiq" | "release") && !options.pass_locationiq {
        println!(
            "locationiq=qps:{} batch_size:{} api_key_configured={}",
            options.qps,
            options.batch_size,
            options.api_key.is_some()
        );
    }
    if command == "release" {
        let steps = [
            ("cleanup", options.pass_cleanup),
            ("prepare", options.pass_prepare),
            ("enhance", options.pass_enhance),
            ("locationiq", options.pass_locationiq),
            ("translate", options.pass_translate),
            ("pack", options.pass_pack),
        ];
        for (step, skipped) in steps {
            println!(
                "step={step} status={}",
                if skipped { "skip" } else { "run" }
            );
        }
    }
}

fn parse_run_stage_args(args: &[String]) -> Result<(Stage, RunOptions), String> {
    let mut stage: Option<Stage> = None;
    let mut passthrough = Vec::new();
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--stage" => {
                let value = args
                    .get(index + 1)
                    .ok_or_else(|| "--stage 需要 stage 名稱".to_string())?;
                stage = Some(value.parse()?);
                index += 2;
            }
            value => {
                passthrough.push(value.to_string());
                index += 1;
            }
        }
    }

    let stage = stage.ok_or_else(|| "run-stage 必須提供 --stage".to_string())?;
    Ok((stage, parse_options(&passthrough)?))
}

fn parse_options(args: &[String]) -> Result<RunOptions, String> {
    let mut options = RunOptions::default();
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--fixture" => {
                options.fixture = Some(
                    args.get(index + 1)
                        .ok_or_else(|| "--fixture 需要 fixture 名稱".to_string())?
                        .to_string(),
                );
                index += 2;
            }
            "--fixtures-dir" => {
                options.fixtures_dir = PathBuf::from(
                    args.get(index + 1)
                        .ok_or_else(|| "--fixtures-dir 需要路徑".to_string())?,
                );
                index += 2;
            }
            "--output-dir" => {
                options.output_dir = PathBuf::from(
                    args.get(index + 1)
                        .ok_or_else(|| "--output-dir 需要路徑".to_string())?,
                );
                index += 2;
            }
            other => return Err(format!("未知參數：{other}")),
        }
    }
    Ok(options)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_default_options() {
        let options = parse_options(&[]).unwrap();

        assert_eq!(options.fixtures_dir, PathBuf::from("fixtures"));
        assert_eq!(options.output_dir, PathBuf::from("target/stage-output"));
        assert_eq!(options.fixture, None);
    }

    #[test]
    fn parse_run_stage_requires_stage() {
        let error = parse_run_stage_args(&[]).unwrap_err();

        assert!(error.contains("--stage"));
    }

    #[test]
    fn production_release_fixture_mode_accepts_api_free_execution() {
        let options = parse_production_options(&[
            "--fixture-mode".to_string(),
            "--pass-locationiq".to_string(),
            "--country-code".to_string(),
            "KR".to_string(),
            "TH".to_string(),
        ])
        .unwrap();

        assert!(options.fixture_mode);
        assert_eq!(options.country_codes, vec!["KR", "TH"]);
        assert!(validate_production_contract("release", &options).is_ok());
    }

    #[test]
    fn production_prepare_non_dry_run_uses_data_folder_and_country_codes() {
        let options = parse_production_options(&[
            "--country-code".to_string(),
            "TW".to_string(),
            "JP".to_string(),
            "--data-folder".to_string(),
            "/tmp/geoname-data".to_string(),
            "--update".to_string(),
        ])
        .unwrap();

        assert!(!options.dry_run);
        assert!(options.update_prepare);
        assert_eq!(options.country_codes, vec!["TW", "JP"]);
        assert_eq!(options.data_folder, PathBuf::from("/tmp/geoname-data"));
        assert!(validate_production_contract("prepare", &options).is_ok());
    }

    /// LocationIQ 產物與 handler metadata 分屬不同生命週期，`--metadata-folder`
    /// 只能搬動後者。若哪天讓 LocationIQ 目錄跟隨該旗標，搬移或清理 handler
    /// metadata 的操作會連帶動到付費 quota 換來的查詢結果，本測試守住這個分界。
    #[test]
    fn locationiq_folder_is_independent_of_metadata_folder() {
        let options = parse_production_options(&[
            "--metadata-folder".to_string(),
            "/tmp/custom-meta".to_string(),
        ])
        .unwrap();

        assert_eq!(options.metadata_folder, PathBuf::from("/tmp/custom-meta"));
        assert_eq!(
            options.locationiq_folder,
            PathBuf::from("./data/locationiq")
        );
    }

    /// 寫入端實際採用的路徑必須落在 LocationIQ 目錄，而非 handler metadata 目錄。
    /// 只比較欄位值不足以擋住「把 locationiq 改回寫入 metadata_folder」的回歸。
    #[test]
    fn locationiq_output_path_follows_locationiq_folder() {
        let options = parse_production_options(&[
            "--metadata-folder".to_string(),
            "/tmp/custom-meta".to_string(),
            "--locationiq-folder".to_string(),
            "/tmp/custom-locationiq".to_string(),
        ])
        .unwrap();

        assert_eq!(
            locationiq_output_path(&options, "US"),
            PathBuf::from("/tmp/custom-locationiq/US.csv")
        );
    }

    /// 既有使用者以自訂路徑存放查詢進度時，必須能指回舊位置續查，
    /// 否則升級後同一條指令會從零重查、重複消耗付費額度。
    #[test]
    fn locationiq_folder_flag_allows_pointing_back_to_legacy_path() {
        let options = parse_production_options(&[
            "--locationiq-folder".to_string(),
            "./meta_data".to_string(),
        ])
        .unwrap();

        assert_eq!(options.locationiq_folder, PathBuf::from("./meta_data"));
        assert_eq!(
            locationiq_output_path(&options, "US"),
            PathBuf::from("./meta_data/US.csv")
        );
    }

    #[test]
    fn production_profile_flag_is_opt_in() {
        let options = parse_production_options(&["--profile".to_string()]).unwrap();

        assert!(options.profile);
    }

    #[test]
    fn production_filters_handler_country_codes_like_legacy_entrypoint() {
        let mut options = parse_production_options(&[
            "--country-code".to_string(),
            "KR".to_string(),
            "US".to_string(),
            "tw".to_string(),
        ])
        .unwrap();

        filter_country_codes_without_handler("release", &mut options);

        assert_eq!(options.country_codes, vec!["US"]);
    }

    #[test]
    fn production_release_without_non_handler_country_skips_api_key_requirement() {
        let mut options =
            parse_production_options(&["--country-code".to_string(), "TW".to_string()]).unwrap();
        // Reason: 預設值會讀取 LOCATIONIQ_API_KEY 環境變數，測試需固定為未提供 key。
        options.api_key = None;
        filter_country_codes_without_handler("release", &mut options);

        assert!(options.country_codes.is_empty());
        assert!(validate_production_contract("release", &options).is_ok());
    }

    #[test]
    fn production_release_with_non_handler_country_still_requires_api_key() {
        let mut options = parse_production_options(&[
            "--country-code".to_string(),
            "TW".to_string(),
            "US".to_string(),
        ])
        .unwrap();
        options.api_key = None;
        filter_country_codes_without_handler("release", &mut options);

        assert_eq!(options.country_codes, vec!["US"]);
        let error = validate_production_contract("release", &options).unwrap_err();
        assert!(error.contains("--locationiq-api-key"));
    }

    #[test]
    fn production_locationiq_command_without_non_handler_country_skips_without_api_key() {
        let mut options =
            parse_production_options(&["--country-code".to_string(), "JP".to_string()]).unwrap();
        options.api_key = None;
        filter_country_codes_without_handler("locationiq", &mut options);

        assert!(options.country_codes.is_empty());
        assert!(validate_production_contract("locationiq", &options).is_ok());
        // Reason: 光通過 pre-flight 檢查不夠，階段本身也必須在沒有 key 時安全跳過。
        assert!(run_locationiq_production(&options).is_ok());
    }

    #[test]
    fn production_extract_requires_country_and_input() {
        let error = run_production_command("extract", &["--dry-run".to_string()]).unwrap_err();

        assert!(error.contains("--country"));
    }

    #[test]
    fn naer_prepare_command_requires_input() {
        let error = run(vec!["immich-geodata".into(), "naer-prepare".into()]).unwrap_err();
        assert!(error.contains("--input"));
    }
}
