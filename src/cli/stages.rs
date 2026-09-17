//! 各 production 階段的執行函式，與它們共用的路徑／國家清單輔助。
//!
//! 自 `cli.rs` 拆出——「每個階段要餵什麼參數給 pipeline」與「命令列字串怎麼
//! 解析成 ProductionOptions」是兩件獨立的事。

use std::fs;
use std::path::{Path, PathBuf};
use std::time::Instant;

use crate::cli::{ProductionOptions, current_date_iso};
use crate::pipeline::prepare::ProductionPrepareOptions;
use crate::pipeline::{self};
use crate::pipeline::{
    admin1_load, cities500_load, extract, locationiq, naer_prepare, pack, translate,
};

pub(super) fn run_profiled_stage<T>(
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

pub(super) fn run_cleanup_production(options: &ProductionOptions) -> Result<(), String> {
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

pub(super) fn run_prepare_production(options: &ProductionOptions) -> Result<(), String> {
    let prepare_options = ProductionPrepareOptions::new(
        options.data_folder.clone(),
        options.country_codes.clone(),
        options.update_prepare,
    );
    pipeline::prepare::run_production(&prepare_options)
}

pub(super) fn run_extract_production(options: &ProductionOptions) -> Result<(), String> {
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

pub(super) fn run_enhance_production(options: &ProductionOptions) -> Result<i64, String> {
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
        input: resolved_cities_file(options),
        output: options
            .output_file
            .clone()
            .unwrap_or_else(|| options.output_folder.join("cities500_optimized.txt")),
        extra_files,
        metadata_dir: options.metadata_folder.clone(),
        handler_countries,
        admin1_file: options.data_folder.join("admin1CodesASCII.txt"),
        current_max_id: admin1_max,
        modification_date: current_date_iso()?,
    })
}

pub(super) fn run_locationiq_production(options: &ProductionOptions) -> Result<(), String> {
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
        let outcome = locationiq::run_production(&locationiq::ProductionLocationiqOptions {
            cities_file: options.output_folder.join("cities500_optimized.txt"),
            output_file,
            address_fields_file: locationiq_address_fields_path(options),
            country_code: country.clone(),
            batch_size: options.batch_size as usize,
            qps: options.qps,
            api_key: api_key.clone(),
            overwrite: options.overwrite,
            allow_partial: options.allow_partial_locationiq,
        })?;
        // Reason: 每日額度是整把金鑰共用的，不是各國分開計。前一國撞到額度上限後，
        // 後面每一國都只會重跑一輪重試退避再停下，白花時間也讓日誌難讀。
        if outcome == locationiq::LocationiqOutcome::RateLimited {
            println!("stage=locationiq stop=rate_limited skipped_remaining_countries=true");
            break;
        }
    }
    Ok(())
}

/// `address_fields.json` 的實際取用路徑。
///
/// Reason: 與 `locationiq_output_path` 同樣的理由抽成具名函式——只斷言預設值的
/// 測試擋不住「呼叫端改成 join 到別的目錄」這種改動。設定是 git 追蹤的輸入，
/// 不得跟著 `--locationiq-folder` 走，這一點必須可被測試直接斷言。
pub(super) fn locationiq_address_fields_path(options: &ProductionOptions) -> PathBuf {
    options.locationiq_address_fields.clone()
}

/// 某國 LocationIQ 產物的輸出路徑。
///
/// Reason: 抽成具名函式讓「寫入端與 translate 讀取端同源」這件事可被測試直接
/// 斷言。若哪天有人把 locationiq 改回寫入 `metadata_folder`，必須同時改動這裡
/// 與它的測試才能通過，不會像先前只比較欄位值那樣悄悄溜過去。
pub(super) fn locationiq_output_path(options: &ProductionOptions, country: &str) -> PathBuf {
    options.locationiq_folder.join(format!("{country}.csv"))
}

pub(super) fn run_translate_production(options: &ProductionOptions) -> Result<(), String> {
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
        naer_file: PathBuf::from("data/vendor/naer/naer_place_names.csv"),
        output_dir: options.output_folder.clone(),
        profile: options.profile,
    })
    .map(|_| ())
}

/// 剪枝：translate 之後、pack 之前，就地改寫 `cities500_translated.txt`。
///
/// Reason: 刪除的是「刪了也不改變任何 Immich 反向地理編碼答案」的點，
/// 455 萬探測點對真實 PostgreSQL 驗證過零標籤改變、零新增空結果。
pub(super) fn run_prune_production(options: &ProductionOptions) -> Result<(), String> {
    use crate::pipeline::prune::{multipass, stage};

    // Reason: 直接取 translate 的產物路徑，不再先試 `output/output/`。
    // production 的 translate 寫的是 `output_folder` 根目錄（見
    // `translate::run_production` 的 `output_dir`），巢狀路徑永遠不存在；
    // 真的存在時只會是上一輪的殘留檔，剪枝會改寫它而 pack 仍打包未剪枝的本體，
    // 整條流程不會有任何錯誤訊息。
    let cities_file = options.output_folder.join("cities500_translated.txt");
    // Reason: label 必須用 **release 實際打包的** admin1 表。pack 複製的是
    // `admin1CodesASCII_translated.txt`（見 `pack::run_production`），
    // 而 `_optimized` 是翻譯前的版本。兩者的「代碼 → 名稱」映射不同時，
    // 剪枝證明所依據的標籤分割就不是 Immich 顯示的那一個。
    let admin1_file = options
        .output_folder
        .join("admin1CodesASCII_translated.txt");
    if !cities_file.exists() {
        return Err(format!(
            "剪枝：找不到 translate 產物 {}",
            cities_file.display()
        ));
    }
    if !admin1_file.exists() {
        return Err(format!("剪枝：找不到 admin1 表 {}", admin1_file.display()));
    }

    let report = stage::run(&stage::PruneOptions {
        cities_file,
        admin1_file,
        output_file: None,
        config: multipass::Config {
            threads: options.prune_threads,
            ..Default::default()
        },
    })?;
    println!(
        "prune: {} → {} 列（刪 {}，{:.1}%），{} 趟，{:.0}s（{} 執行緒）",
        report.rows_out + report.deleted,
        report.rows_out,
        report.deleted,
        100.0 * report.deleted as f64 / (report.rows_out + report.deleted) as f64,
        report.passes,
        report.seconds,
        multipass::resolve_threads(options.prune_threads)
    );
    Ok(())
}

pub(super) fn run_naer_prepare_command(args: &[String]) -> Result<(), String> {
    let mut input: Option<PathBuf> = None;
    let mut output = PathBuf::from("data/vendor/naer/naer_place_names.csv");
    let mut country_names_file = PathBuf::from("data/vendor/i18n-iso-countries/langs/zh-tw.json");
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

pub(super) fn run_pack_production(options: &ProductionOptions) -> Result<(), String> {
    pack::run_production(&pack::ProductionPackOptions {
        output_dir: options.output_folder.clone(),
        data_dir: options.data_folder.clone(),
        project_dir: PathBuf::from("."),
        release_date: current_date_iso()?,
        profile: options.profile,
    })
}

pub(super) fn run_release_production(options: &ProductionOptions) -> Result<(), String> {
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
    if !options.pass_prune {
        run_profiled_stage(options, "prune", || run_prune_production(options))?;
    }
    if !options.pass_pack {
        run_profiled_stage(options, "pack", || run_pack_production(options))?;
    }
    Ok(())
}

/// enhance 實際讀取的 base cities 檔案。
///
/// Reason: `--cities-file` 可覆寫來源，而 handler 合成 ID 的起點取自這份檔案的
/// 最大值。若上限計算與實際輸入取自不同路徑，指定自訂 cities file 時就會撞號。
pub(super) fn resolved_cities_file(options: &ProductionOptions) -> PathBuf {
    options
        .cities_file
        .clone()
        .unwrap_or_else(|| options.data_folder.join("cities500.txt"))
}

/// 掃描所有會進入 cities500 的來源，取得既有 geoname_id 的最大值。
///
/// handler 合成 ID 自此值 +1 起配置，因此凡是最終會被寫進 cities500 的檔案都必須
/// 納入掃描範圍。
///
/// Reason: `extra_data/{CC}.txt`（非 handler 國家的 GeoNames 完整 dump）由
/// `merge_extra_rows` 併入 cities500，但它的 ID 由上游 GeoNames 配發，會隨新地點
/// 加入而持續成長。漏掉這些檔案時，只要某筆 extra 列的 ID 超過 `cities500.txt`
/// 的最大值，就會落進 handler 合成 ID 的區間而重複——目前沒有重複 ID 守衛，
/// 且每週自動更新正是會逐步觸發這個情況的路徑。
pub(super) fn calculate_global_max_geoname_id(options: &ProductionOptions) -> Result<i64, String> {
    let mut max_id = 0_i64;
    let extra_data_dir = options.data_folder.join("extra_data");
    let paths = [
        resolved_cities_file(options),
        options.data_folder.join("admin1CodesASCII.txt"),
    ]
    .into_iter()
    .chain(
        non_handler_country_codes(&options.country_codes)
            .into_iter()
            .map(|country| extra_data_dir.join(format!("{country}.txt"))),
    );
    for path in paths {
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

pub(super) fn handler_countries() -> Vec<String> {
    // Reason: 由 extract 的 Country enum 單一事實來源導出，
    // 新增國家時不會發生 CLI 清單與 handler 路由不同步。
    extract::handler_country_codes()
        .into_iter()
        .map(ToString::to_string)
        .collect()
}

pub(super) fn is_handler_country(country: &str) -> bool {
    let normalized = country.to_ascii_uppercase();
    handler_countries().contains(&normalized)
}

pub(super) fn handler_countries_with_metadata(metadata_folder: &Path) -> Vec<String> {
    handler_countries()
        .into_iter()
        .filter(|country| {
            metadata_folder
                .join(format!("{}_geodata.csv", country.to_lowercase()))
                .exists()
        })
        .collect()
}

pub(super) fn non_handler_country_codes(country_codes: &[String]) -> Vec<String> {
    country_codes
        .iter()
        .filter(|country| !is_handler_country(country))
        .cloned()
        .collect()
}

pub(super) fn filter_country_codes_without_handler(command: &str, options: &mut ProductionOptions) {
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
