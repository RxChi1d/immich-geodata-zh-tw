//! production 子命令的參數解析與契約檢查。
//!
//! 自 `cli.rs` 拆出。這裡是唯一把命令列字串轉成 ProductionOptions 的地方，
//! 集中後才看得出哪些旗標有解析、哪些只寫在 HELP 裡。

use std::path::PathBuf;

use crate::cli::ProductionOptions;

pub(super) fn parse_production_options(args: &[String]) -> Result<ProductionOptions, String> {
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
            "--threads" => {
                let value = args
                    .get(index + 1)
                    .ok_or_else(|| "--threads 需要一個數值".to_string())?;
                options.prune_threads = value
                    .parse()
                    .map_err(|_| format!("--threads 必須是整數，收到：{value}"))?;
                index += 2;
            }
            "--pass-prune" => {
                options.pass_prune = true;
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
            "--locationiq-qps" => {
                options.qps = parse_u32_arg(args, index, "--locationiq-qps")?;
                index += 2;
            }
            "--locationiq-api-key" => {
                options.api_key = Some(required_value(args, index, "--locationiq-api-key")?);
                index += 2;
            }
            // Reason: 預設是相對路徑 ./data/locationiq/address_fields.json，
            // 在 repo 根目錄以外執行就找不到。release binary 以獨立 tarball 發布，
            // 沒有這個旗標的話，其他輸入路徑都可覆寫、唯獨欄位設定不行。
            "--locationiq-address-fields" => {
                options.locationiq_address_fields =
                    PathBuf::from(required_value(args, index, "--locationiq-address-fields")?);
                index += 2;
            }
            "--locationiq-allow-partial" => {
                options.allow_partial_locationiq = true;
                index += 1;
            }
            other => return Err(format!("未知 production 參數：{other}")),
        }
    }
    Ok(options)
}

pub(super) fn required_value(args: &[String], index: usize, flag: &str) -> Result<String, String> {
    args.get(index + 1)
        .filter(|value| !value.starts_with('-'))
        .cloned()
        .ok_or_else(|| format!("{flag} 需要值"))
}

pub(super) fn collect_values(
    args: &[String],
    mut index: usize,
) -> Result<(Vec<String>, usize), String> {
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

pub(super) fn parse_u32_arg(args: &[String], index: usize, flag: &str) -> Result<u32, String> {
    required_value(args, index, flag)?
        .parse()
        .map_err(|error| format!("{flag} 數值格式錯誤：{error}"))
}

pub(super) fn validate_production_contract(
    command: &str,
    options: &ProductionOptions,
) -> Result<(), String> {
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
