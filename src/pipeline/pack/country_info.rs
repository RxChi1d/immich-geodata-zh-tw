//! 產生 release 的 `geodata/countryInfo.txt`。
//!
//! Immich v3.3.0 起以 GeoNames 的 `countryInfo.txt` 取代 `i18n-iso-countries`
//! 解析國名，只讀第 1（alpha-2）、2（alpha-3）、5（Country）欄。這裡以
//! `en.json` 的繁中國名取代第 5 欄，其餘內容逐位元組保留。

use std::collections::HashMap;
use std::fs;
use std::path::Path;

/// GeoNames 檔頭註解標明已不存在的舊代碼，沒有對應譯名屬預期，維持英文原樣。
const LEGACY_CODES: [&str; 2] = ["AN", "CS"];

/// 以 `names_json`（i18n-iso-countries 的 `en.json`）的譯名取代 `base` 第 5 欄，
/// 寫入 `output`，回傳被取代的列數。
///
/// Reason: 底檔出現 `en.json` 沒有的現行代碼時直接失敗，否則 GeoNames 新增國家後
/// 該國會悄悄退回英文，使用者看到的是混雜語言而不是錯誤。
pub fn write_country_info(base: &Path, names_json: &Path, output: &Path) -> Result<usize, String> {
    let names = read_names(names_json)?;
    let base_text = fs::read_to_string(base)
        .map_err(|error| format!("無法讀取 countryInfo 底檔 {}：{error}", base.display()))?;
    let (text, replaced) = localize(&base_text, &names)?;
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("無法建立 {}：{error}", parent.display()))?;
    }
    fs::write(output, text)
        .map_err(|error| format!("無法寫入 countryInfo {}：{error}", output.display()))?;
    Ok(replaced)
}

fn read_names(path: &Path) -> Result<HashMap<String, String>, String> {
    let raw = fs::read_to_string(path)
        .map_err(|error| format!("無法讀取國名檔 {}：{error}", path.display()))?;
    let json: serde_json::Value = serde_json::from_str(&raw)
        .map_err(|error| format!("國名檔 {} 不是合法 JSON：{error}", path.display()))?;
    let countries = json
        .get("countries")
        .and_then(|value| value.as_object())
        .ok_or_else(|| format!("國名檔 {} 缺少 countries 物件", path.display()))?;
    let mut names = HashMap::new();
    for (code, value) in countries {
        // i18n-iso-countries 允許以陣列列出別名，第一個為主要名稱。
        let name = match value {
            serde_json::Value::String(name) => Some(name.as_str()),
            serde_json::Value::Array(items) => items.first().and_then(|item| item.as_str()),
            _ => None,
        };
        if let Some(name) = name.filter(|name| !name.is_empty()) {
            names.insert(code.clone(), name.to_string());
        }
    }
    Ok(names)
}

fn localize(base: &str, names: &HashMap<String, String>) -> Result<(String, usize), String> {
    let mut out = String::with_capacity(base.len());
    let mut replaced = 0;
    let mut missing = Vec::new();
    for line in base.split_inclusive('\n') {
        let body = line.trim_end_matches(['\r', '\n']);
        let ending = &line[body.len()..];
        let mut fields: Vec<&str> = body.split('\t').collect();
        if body.starts_with('#') || fields.len() < 5 {
            out.push_str(line);
            continue;
        }
        match names.get(fields[0]) {
            Some(name) => {
                fields[4] = name;
                replaced += 1;
            }
            None if LEGACY_CODES.contains(&fields[0]) => {}
            None => missing.push(fields[0].to_string()),
        }
        out.push_str(&fields.join("\t"));
        out.push_str(ending);
    }
    if !missing.is_empty() {
        return Err(format!(
            "en.json 缺少 countryInfo 的現行國碼 {}，請先補上譯名",
            missing.join("、")
        ));
    }
    Ok((out, replaced))
}

#[cfg(test)]
mod tests {
    use super::*;

    const BASE: &str = "# header\r\n#\r\nTW\tTWN\t158\tTW\tTaiwan\tTaipei\t1\r\nAN\tANT\t530\tNT\tNetherlands Antilles\tX\t1\r\n";

    fn names(pairs: &[(&str, &str)]) -> HashMap<String, String> {
        pairs
            .iter()
            .map(|(code, name)| (code.to_string(), name.to_string()))
            .collect()
    }

    #[test]
    fn replaces_only_country_column_and_keeps_crlf() {
        let (text, replaced) = localize(BASE, &names(&[("TW", "臺灣")])).unwrap();
        assert_eq!(replaced, 1);
        assert_eq!(
            text,
            "# header\r\n#\r\nTW\tTWN\t158\tTW\t臺灣\tTaipei\t1\r\nAN\tANT\t530\tNT\tNetherlands Antilles\tX\t1\r\n"
        );
    }

    #[test]
    fn missing_current_code_fails() {
        let error = localize(BASE, &names(&[])).unwrap_err();
        assert!(error.contains("TW"), "{error}");
        assert!(!error.contains("AN"), "{error}");
    }

    #[test]
    fn write_reads_array_alias_and_rejects_bad_json() {
        let dir = std::env::temp_dir().join(format!("country_info_test_{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let base = dir.join("base.txt");
        let json = dir.join("en.json");
        let out = dir.join("out").join("countryInfo.txt");
        fs::write(&base, BASE).unwrap();
        fs::write(&json, r#"{"countries":{"TW":["臺灣","台灣"]}}"#).unwrap();
        assert_eq!(write_country_info(&base, &json, &out).unwrap(), 1);
        assert!(fs::read_to_string(&out).unwrap().contains("\t臺灣\t"));
        fs::write(&json, "not json").unwrap();
        assert!(write_country_info(&base, &json, &out).is_err());
        fs::remove_dir_all(&dir).unwrap();
    }

    /// 以實際 vendored 資料守住「en.json 涵蓋 GeoNames 現行國碼」，更新底檔時 CI 會提醒補譯。
    #[test]
    fn vendored_data_covers_all_current_codes() {
        let vendor = Path::new(env!("CARGO_MANIFEST_DIR")).join("data/vendor");
        let out =
            std::env::temp_dir().join(format!("country_info_real_{}.txt", std::process::id()));
        let replaced = write_country_info(
            &vendor.join("geonames/countryInfo.txt"),
            &vendor.join("i18n-iso-countries/langs/en.json"),
            &out,
        )
        .unwrap();
        let text = fs::read_to_string(&out).unwrap();
        fs::remove_file(&out).unwrap();
        assert!(replaced >= 250, "replaced={replaced}");
        assert!(text.contains("TW\tTWN\t158\tTW\t臺灣\t"));
        assert!(text.contains("\r\n"));
    }
}
