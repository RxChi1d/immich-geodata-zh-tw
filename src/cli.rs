use std::fs;
use std::path::{Path, PathBuf};

use crate::pipeline::{self, Stage};

mod args;
mod stages;

use args::*;
use stages::*;

const HELP: &str = "\
immich-geodata

USAGE:
  immich-geodata help
  immich-geodata list-stages
  immich-geodata run-stage --stage <stage> [--fixture <name>] [--fixtures-dir <path>] [--output-dir <path>]
  immich-geodata full-pipeline [--fixture <name>] [--fixtures-dir <path>] [--output-dir <path>]
  immich-geodata prepare [--country-code <cc...>] [--data-folder <path>] [--update]
  immich-geodata <cleanup|prepare|extract|enhance|locationiq|translate|prune|pack|release|naer-prepare> [--dry-run|--fixture-mode|--profile] [--threads <N|-1>] [options]
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
        "cleanup" | "prepare" | "extract" | "enhance" | "locationiq" | "translate" | "prune"
        | "pack" | "release" => run_production_command(command, &args[2..]),
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
    pass_prune: bool,
    pass_pack: bool,
    prune_threads: i32,
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
    /// `address_fields.json` 的位置。
    ///
    /// Reason: 刻意不跟著 `--locationiq-folder` 走。那個旗標是用來搬移 **CSV 產物**
    /// （相容舊的 `meta_data` 路徑），但欄位設定是 git 追蹤的**輸入**，隨著 repo
    /// 走而非隨著輸出目錄走。綁在一起的話，照 `docs/*/development.md` 把 CSV 移到
    /// 自訂目錄的使用者會因為那裡沒有設定檔而整個跑不動。
    locationiq_address_fields: PathBuf,
    batch_size: u32,
    qps: u32,
    api_key: Option<String>,
    allow_partial_locationiq: bool,
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
            pass_prune: false,
            pass_pack: false,
            prune_threads: 0,
            country_codes: vec!["TW".to_string()],
            data_folder: PathBuf::from("./geoname_data"),
            output_folder: PathBuf::from("./output"),
            shapefile: None,
            extract_country: None,
            extract_output: None,
            cities_file: None,
            output_file: None,
            alternate_name_file: None,
            metadata_folder: PathBuf::from("./data/handler"),
            // Reason: LocationIQ 產物與 handler extract 產物欄位同為 GEODATA_COLUMNS
            // 但生命週期相反——前者是重建需消耗付費 quota 的查詢結果，後者是 CLAUDE.md
            // 資料保護規則禁止重新產生的 canonical metadata。兩者曾共用 meta_data/，只靠檔名
            // 大小寫區分，清理者無從判斷哪些檔案可動，因此以目錄分隔。刻意不跟隨
            // --metadata-folder：搬移 handler metadata 時不應連帶移動付費查詢結果。
            locationiq_folder: PathBuf::from("./data/locationiq"),
            locationiq_address_fields: PathBuf::from("./data/locationiq/address_fields.json"),
            batch_size: 100,
            // Reason: LocationIQ 免費方案同時有 2 req/s 與 60 req/min 兩條限制，
            // 兩者互相矛盾——照 2 req/s 打滿是 120 req/min，必然撞上分鐘上限。
            // 實際生效的是比較嚴的那條，因此節流要以 60 req/min 為準，也就是
            // qps=1（1020 ms 間隔 ≈ 58.8 req/min，留 2% 邊際）。付費方案可用
            // --locationiq-qps 調高。
            qps: 1,
            api_key: std::env::var("LOCATIONIQ_API_KEY").ok(),
            allow_partial_locationiq: false,
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
        "prune" => run_profiled_stage(options, "prune", || run_prune_production(options)),
        "pack" => run_profiled_stage(options, "pack", || run_pack_production(options)),
        "release" => run_release_production(options),
        other => Err(format!("未知 production 命令：{other}")),
    }
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
            ("prune", options.pass_prune),
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
    /// handler 合成 ID 的起點必須高於所有會併入 cities500 的來源。
    ///
    /// Reason: `extra_data/{CC}.txt` 的 ID 由上游 GeoNames 配發並持續成長，
    /// 漏掉它就會與 handler 合成 ID 撞號，且沒有守衛會攔下。
    #[test]
    fn global_max_geoname_id_includes_non_handler_extra_data() {
        let dir = tempfile::tempdir().unwrap();
        let data = dir.path().to_path_buf();
        std::fs::create_dir_all(data.join("extra_data")).unwrap();
        std::fs::write(
            data.join("cities500.txt"),
            "100\tA\tA\t\t1.0\t2.0\tP\tPPL\tMY\t\t\t\t\t\t0\t\t\tAsia/Kuala_Lumpur\t2026-01-01\n",
        )
        .unwrap();
        std::fs::write(
            data.join("extra_data").join("MY.txt"),
            "999\tB\tB\t\t1.0\t2.0\tP\tPPL\tMY\t\t\t\t\t\t0\t\t\tAsia/Kuala_Lumpur\t2026-01-01\n",
        )
        .unwrap();

        let options = super::ProductionOptions {
            data_folder: data,
            country_codes: vec!["MY".to_string()],
            ..Default::default()
        };

        assert_eq!(
            super::calculate_global_max_geoname_id(&options).unwrap(),
            999,
            "extra_data 的 ID 必須納入，否則 handler 合成 ID 會從 100 起而撞號"
        );
    }

    /// `--cities-file` 覆寫來源時，上限計算必須跟著改讀同一份檔案。
    #[test]
    fn global_max_geoname_id_follows_cities_file_override() {
        let dir = tempfile::tempdir().unwrap();
        let data = dir.path().to_path_buf();
        std::fs::write(data.join("cities500.txt"), "100\tA\tA\n").unwrap();
        let custom = data.join("custom_cities.txt");
        std::fs::write(&custom, "555\tB\tB\n").unwrap();

        let options = super::ProductionOptions {
            data_folder: data,
            cities_file: Some(custom),
            ..Default::default()
        };

        assert_eq!(
            super::calculate_global_max_geoname_id(&options).unwrap(),
            555,
            "上限必須取自 enhance 實際讀取的 cities file"
        );
    }

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

    /// `address_fields.json` 不跟著 `--locationiq-folder` 走。
    ///
    /// Reason: 那個旗標搬的是 CSV 產物，而欄位設定是 git 追蹤的輸入。兩者綁在
    /// 一起時，照 `docs/*/development.md` 把 CSV 移到自訂目錄的使用者會因為那裡
    /// 沒有設定檔而完全跑不動。
    #[test]
    fn address_fields_path_does_not_follow_locationiq_folder() {
        let options = parse_production_options(&[
            "--locationiq-folder".to_string(),
            "./meta_data".to_string(),
        ])
        .unwrap();

        assert_eq!(
            locationiq_address_fields_path(&options),
            PathBuf::from("./data/locationiq/address_fields.json")
        );
    }

    /// `--locationiq-address-fields` 覆寫預設路徑，且不受 `--locationiq-folder` 影響。
    #[test]
    fn address_fields_path_can_be_overridden_by_flag() {
        let options = parse_production_options(&[
            "--locationiq-folder".to_string(),
            "./meta_data".to_string(),
            "--locationiq-address-fields".to_string(),
            "/opt/geodata/address_fields.json".to_string(),
        ])
        .unwrap();

        assert_eq!(
            locationiq_address_fields_path(&options),
            PathBuf::from("/opt/geodata/address_fields.json")
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
