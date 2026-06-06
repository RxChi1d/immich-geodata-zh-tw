//! NAER《外國地名譯名》離線前處理：原始 CSV → vendored 6 欄清理檔。
//! 僅於資料更新時手動執行，不在 release path。

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use regex::Regex;

/// 度分制座標 regex：緯度（度+可選分+NS）緊接經度（度+可選分+EW）。
fn coordinate_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"^(\d{1,2})°(\d{1,2})?′?([NS])(\d{1,3})°(\d{1,2})?′?([EW])$").unwrap()
    })
}

fn tag_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"<[^>]+>").unwrap())
}

/// 解析 NAER 經緯坐標欄位（含 HTML entities、標籤、引號變體等髒格式）。
/// 回傳 (latitude, longitude)；不可解析回 None。
pub fn parse_naer_coordinate(raw: &str) -> Option<(f64, f64)> {
    let unescaped = raw
        .replace("&deg;", "°")
        .replace("&prime;", "′")
        .replace("&Prime;", "″")
        .replace("&nbsp;", " ")
        .replace("&amp;", "&");
    let no_tags = tag_regex().replace_all(&unescaped, "");
    let normalized: String = no_tags
        .chars()
        .map(|c| match c {
            // Reason: 各種引號與符號變體統一替換為標準分號符號 ′（U+2032），
            // 確保 regex 能以單一符號比對。
            '\u{2019}' | '\'' | '\u{00B4}' | '`' | '\u{2032}' => '\u{2032}',
            other => other,
        })
        .filter(|c| !c.is_whitespace())
        .collect();
    let captures = coordinate_regex().captures(&normalized)?;
    let degree: f64 = captures[1].parse().ok()?;
    let minute: f64 = captures
        .get(2)
        .map_or(0.0, |m| m.as_str().parse().unwrap_or(0.0));
    // Reason: 分值 >= 60 在角度制中無意義，且 regex 允許 \d{1,2} 故 59 以上仍可通過
    // 比對；不驗證會導致靜默換算成合法範圍內的錯誤座標（例如 10°99′ → 11.65°）。
    if minute >= 60.0 {
        return None;
    }
    let mut latitude = degree + minute / 60.0;
    if &captures[3] == "S" {
        latitude = -latitude;
    }
    let degree: f64 = captures[4].parse().ok()?;
    let minute: f64 = captures
        .get(5)
        .map_or(0.0, |m| m.as_str().parse().unwrap_or(0.0));
    // Reason: 同上，經度分值亦須驗證。
    if minute >= 60.0 {
        return None;
    }
    let mut longitude = degree + minute / 60.0;
    if &captures[6] == "W" {
        longitude = -longitude;
    }
    if !(-90.0..=90.0).contains(&latitude) || !(-180.0..=180.0).contains(&longitude) {
        return None;
    }
    if !latitude.is_finite() || !longitude.is_finite() {
        return None;
    }
    Some((latitude, longitude))
}

/// 英文地物標記（token 完全比對，小寫）。
const EN_FEATURE_MARKERS: &[&str] = &[
    "r.",
    "l.",
    "i.",
    "is.",
    "mt.",
    "mts.",
    "pt.",
    "c.",
    "bay",
    "lake",
    "river",
    "island",
    "islands",
    "cape",
    "peak",
    "mount",
    "mountain",
    "mountains",
    "strait",
    "gulf",
    "desert",
    "plateau",
    "falls",
    "glacier",
    "reef",
    "lagoon",
    "channel",
    "sound",
    "depression",
    "basin",
    "valley",
    "hills",
    "inlet",
    "harbour",
    "harbor",
    "sea",
];

/// 中文地形字尾（單字尾與雙字尾）。
const ZH_FEATURE_SINGLE_SUFFIXES: &[char] = &[
    '河', '灣', '島', '嶼', '山', '湖', '角', '峽', '礁', '岬', '灘', '泉', '海',
];
const ZH_FEATURE_MULTI_SUFFIXES: &[&str] = &[
    "沙漠", "高原", "盆地", "平原", "瀑布", "冰川", "海峽", "群島", "運河", "水庫", "冰穹",
];

/// 自然地物啟發式：英文 token 含地物標記，或中文譯名以地形字尾結尾。
/// 僅作降權依據（中信心、僅補洞），不刪列。
pub fn detect_feature_hint(name_en: &str, name_zh: &str) -> bool {
    let en_tokens = name_en
        .to_lowercase()
        .replace(',', " ")
        .split_whitespace()
        .map(str::to_string)
        .collect::<Vec<_>>();
    if en_tokens
        .iter()
        .any(|token| EN_FEATURE_MARKERS.contains(&token.as_str()))
    {
        return true;
    }
    if ZH_FEATURE_MULTI_SUFFIXES
        .iter()
        .any(|suffix| name_zh.ends_with(suffix))
    {
        return true;
    }
    name_zh
        .chars()
        .last()
        .is_some_and(|last| ZH_FEATURE_SINGLE_SUFFIXES.contains(&last))
}

/// 剝離中文譯名的括號類註記（圓括號、全形括號、方括號）。
/// 括號剝離規則與匹配 key 正規化共用單一實作，避免兩側分歧。
pub fn clean_zh_name(raw: &str) -> String {
    crate::pipeline::naer_lookup::strip_bracket_annotations(raw)
        .trim()
        .to_string()
}

pub struct NaerPrepareOptions {
    pub input: PathBuf,
    pub output: PathBuf,
    /// i18n-iso-countries 的 zh-tw 國名檔，預設 `i18n-iso-countries/langs/zh-tw.json`。
    pub country_names_file: PathBuf,
}

#[derive(Debug, Default)]
pub struct NaerPrepareReport {
    pub total: usize,
    pub written: usize,
    pub coordinate_failures: usize,
    pub coordinate_empty: usize,
    /// 名稱不可用（正規化／清理後為空，或含逗號）而略過的列數，
    /// 與座標解析失敗分開計數以維持 coord_failed 統計語意精確。
    pub name_failures: usize,
    pub feature_hints: usize,
    pub unmapped_countries: Vec<String>,
    pub conflicts: usize,
    /// 解析成功但座標為 (0.0, 0.0) 的可疑列數（列仍保留）。
    pub suspicious_zero_coordinates: usize,
}

/// NAER 用語與 zh-tw.json 的國名差異 alias 表。
/// 註：依 vendored 生成時的報告逐步補充高頻未對應國名。
const COUNTRY_ALIASES: &[(&str, &str)] = &[
    ("韓國", "KR"),   // zh-tw.json 使用「南韓」
    ("俄羅斯", "RU"), // 防 zh-tw.json 用語為「俄羅斯聯邦」
    ("美國", "US"),
    ("英國", "GB"),
    // 以下為 vendored 生成報告判讀後補入的高頻單一國名（>40 筆），
    // zh-tw.json 用語與 NAER 不一致；ISO 3166-1 alpha-2 碼經人工查證。
    ("巴紐", "PG"),           // 巴布亞紐幾內亞；zh-tw.json 用語為「巴布亞紐幾內亞」
    ("波赫", "BA"),           // 波士尼亞與赫塞哥維納；zh-tw.json 用語為「波士尼亞及赫塞哥維納」
    ("阿聯", "AE"),           // 阿拉伯聯合大公國；zh-tw.json 用語為「阿拉伯聯合大公國」
    ("千里達及托巴哥", "TT"), // 千里達及托巴哥；zh-tw.json 用語為「千里達」
    ("馬紹爾群島", "MH"),     // 馬紹爾群島；zh-tw.json 用語為「馬紹爾群島共和國」
    // Reason: NAER 大括號為消歧註記而非多國邊界，以下兩者皆為單一主權國家，
    // 其用語與 zh-tw.json 不一致，需補 alias 才能正確對應 country_code。
    ("剛果{金夏沙}", "CD"), // 剛果民主共和國；zh-tw.json 用語為「剛果民主共和國」
    ("剛果{布拉薩}", "CG"), // 剛果共和國；zh-tw.json 用語為「剛果」
];

fn load_country_mapping(path: &Path) -> Result<HashMap<String, String>, String> {
    let content = std::fs::read_to_string(path)
        .map_err(|error| format!("無法讀取國名對應檔 {}：{error}", path.display()))?;
    let json: serde_json::Value = serde_json::from_str(&content)
        .map_err(|error| format!("國名對應檔 {} JSON 解析失敗：{error}", path.display()))?;
    let countries = json
        .get("countries")
        .and_then(|value| value.as_object())
        .ok_or_else(|| format!("國名對應檔 {} 缺少 countries 物件", path.display()))?;
    let mut mapping = HashMap::with_capacity(countries.len() + COUNTRY_ALIASES.len());
    for (code, name) in countries {
        if let Some(name) = name.as_str() {
            mapping.insert(name.to_string(), code.clone());
        }
    }
    for (alias, code) in COUNTRY_ALIASES {
        mapping.insert((*alias).to_string(), (*code).to_string());
    }
    Ok(mapping)
}

/// 以 polars 讀取原始 NAER CSV（處理引號欄位與 BOM），欄序固定：
/// 序號、英文名稱、中文名稱、所在國、經緯坐標、來源網站。
fn read_raw_rows(input: &Path) -> Result<Vec<(String, String, String, String)>, String> {
    use std::fs::File;
    use std::sync::Arc;

    use polars::prelude::*;

    let file = File::open(input)
        .map_err(|error| format!("無法開啟 NAER 原始檔 {}：{error}", input.display()))?;
    let projection = Arc::new(vec![1_usize, 2, 3, 4]);
    let dtype_overwrite = Arc::new(vec![
        DataType::String,
        DataType::String,
        DataType::String,
        DataType::String,
    ]);
    let df = CsvReadOptions::default()
        .with_has_header(true)
        .with_projection(Some(projection))
        .with_dtype_overwrite(Some(dtype_overwrite))
        .map_parse_options(|parse_options| parse_options.with_separator(b','))
        .into_reader_with_file_handle(file)
        .finish()
        .map_err(|error| format!("Polars 無法讀取 NAER 原始檔 {}：{error}", input.display()))?;
    let column = |index: usize| -> Result<Vec<String>, String> {
        let series = df
            .columns()
            .get(index)
            .ok_or_else(|| format!("NAER 原始檔 {} 欄位不足", input.display()))?;
        Ok(series
            .str()
            .map_err(|error| format!("NAER 原始檔欄位型別錯誤：{error}"))?
            .into_iter()
            .map(|value| value.unwrap_or_default().to_string())
            .collect())
    };
    let names_en = column(0)?;
    let names_zh = column(1)?;
    let countries = column(2)?;
    let coordinates = column(3)?;
    Ok(names_en
        .into_iter()
        .zip(names_zh)
        .zip(countries)
        .zip(coordinates)
        .map(|(((en, zh), country), coordinate)| (en, zh, country, coordinate))
        .collect())
}

pub fn run(options: &NaerPrepareOptions) -> Result<NaerPrepareReport, String> {
    use crate::pipeline::naer_lookup::normalize_lookup_name;
    use crate::pipeline::table::write_delimited;

    let mapping = load_country_mapping(&options.country_names_file)?;
    let raw_rows = read_raw_rows(&options.input)?;
    let mut report = NaerPrepareReport {
        total: raw_rows.len(),
        ..Default::default()
    };
    let mut unmapped: Vec<String> = Vec::new();
    let mut rows: Vec<Vec<String>> = Vec::with_capacity(raw_rows.len());
    for (name_en, name_zh_raw, country, coordinate) in &raw_rows {
        let trimmed_coordinate = coordinate.trim();
        if trimmed_coordinate.is_empty() {
            report.coordinate_empty += 1;
            continue;
        }
        let Some((latitude, longitude)) = parse_naer_coordinate(trimmed_coordinate) else {
            report.coordinate_failures += 1;
            continue;
        };
        let name_norm = normalize_lookup_name(name_en);
        let name_zh = clean_zh_name(name_zh_raw);
        if name_norm.is_empty() || name_zh.is_empty() {
            report.name_failures += 1;
            continue;
        }
        // vendored 檔為逗號分隔且 runtime 以簡單 split 讀取，
        // 欄位含逗號的列直接略過並記入報告（實測極罕見）。
        // Reason: 只檢查 name_zh——name_norm 經 normalize_lookup_name 已
        // 截取逗號前段，不可能含逗號。
        if name_zh.contains(',') {
            report.name_failures += 1;
            continue;
        }
        let country_code = match mapping.get(country.trim()) {
            Some(code) => code.clone(),
            None => {
                if !country.trim().is_empty() && !unmapped.contains(&country.trim().to_string()) {
                    unmapped.push(country.trim().to_string());
                }
                String::new()
            }
        };
        // Reason: (0,0) 落於幾內亞灣 Null Island，幾乎不可能為真實地名座標，
        // 多為來源缺漏佔位；計數供品質報告審視，但不刪列（仍可能極少數合法）。
        if latitude == 0.0 && longitude == 0.0 {
            report.suspicious_zero_coordinates += 1;
        }
        let feature_hint = detect_feature_hint(name_en, &name_zh);
        if feature_hint {
            report.feature_hints += 1;
        }
        rows.push(vec![
            name_norm,
            name_zh,
            country_code,
            format!("{latitude:.6}"),
            format!("{longitude:.6}"),
            feature_hint.to_string(),
        ]);
    }
    // 穩定排序：git diff 可審查。
    rows.sort();
    // 衝突偵測：同 (name_norm, country_code) 分組，組內任兩列譯名不同且距離 <5km。
    // Reason: 原 windows(2) 只看排序後相鄰列；同 key 列可能因 name_zh 不同而被
    // 其他列（如其他國家的同 name_norm）隔開，相鄰比對會漏報，故改為分組兩兩比對。
    use crate::pipeline::naer_lookup::distance_km;
    let coords =
        |r: &Vec<String>| -> Option<(f64, f64)> { Some((r[3].parse().ok()?, r[4].parse().ok()?)) };
    let mut groups: HashMap<(String, String), Vec<usize>> = HashMap::new();
    for (index, row) in rows.iter().enumerate() {
        groups
            .entry((row[0].clone(), row[2].clone()))
            .or_default()
            .push(index);
    }
    for indices in groups.values() {
        // Reason: conflicts 以「組」為單位計數——同組 N 列互衝突應報 1 組
        // 而非 C(N,2) 對，避免品質報告高估衝突嚴重性；逐對 eprintln 仍
        // 保留供人工審查細節。
        let mut group_conflicted = false;
        for (offset, &i) in indices.iter().enumerate() {
            for &j in &indices[offset + 1..] {
                let (a, b) = (&rows[i], &rows[j]);
                if a[1] == b[1] {
                    continue;
                }
                if let (Some((lat_a, lon_a)), Some((lat_b, lon_b))) = (coords(a), coords(b))
                    && distance_km(lat_a, lon_a, lat_b, lon_b) < 5.0
                {
                    group_conflicted = true;
                    eprintln!(
                        "naer-prepare 衝突：{} ({}) → {} / {}",
                        a[0], a[2], a[1], b[1]
                    );
                }
            }
        }
        if group_conflicted {
            report.conflicts += 1;
        }
    }
    report.written = rows.len();
    report.unmapped_countries = unmapped;
    write_delimited(
        &options.output,
        ',',
        Some(&[
            "name_norm",
            "name_zh",
            "country_code",
            "latitude",
            "longitude",
            "feature_hint",
        ]),
        &rows,
    )?;
    println!(
        "naer-prepare total={} written={} coord_failed={} coord_empty={} name_failed={} feature_hints={} unmapped_countries={} conflicts={} suspicious_zero={}",
        report.total,
        report.written,
        report.coordinate_failures,
        report.coordinate_empty,
        report.name_failures,
        report.feature_hints,
        report.unmapped_countries.len(),
        report.conflicts,
        report.suspicious_zero_coordinates
    );
    if !report.unmapped_countries.is_empty() {
        println!(
            "naer-prepare 未對應國名：{}",
            report.unmapped_countries.join("、")
        );
    }
    Ok(report)
}
