//! 印尼（Indonesia）的 Wikidata 翻譯查詢表建構。
//!
//! 仿 `thailand_wikidata.rs` 結構：admin1（省）與 admin2（縣市）走標準
//! P131 鏈驗證，並以 instance-of（P31）類別過濾候選。
//!
//! 與 TH/KR 不同處：
//! - BIG 圖資的 WADMKK 同名單位需要消歧前綴（`Kabupaten ` / `Kota `）
//!   才能在 Wikidata 搜尋到正確實體；搜尋字串與最終查詢 key 因此分離。
//! - 雅加達特區的城區名稱含官方行政前綴（`Kota Adm.` / `Adm.`），需正規化
//!   為通用名稱後再搜尋。
//!
//! 查詢表以 BIG 原文（WADMPR / WADMKK）為 key，handler 端可直接用原文查表。

use std::collections::{HashMap, HashSet};

use crate::wikidata::{
    AdminLevel, BatchTranslateOptions, METADATA_OFFICIAL_ORIGINAL, TranslationDataset,
    TranslationItem, TranslationResult, WikidataCandidateMetadata, WikidataClientOptions,
    WikidataTranslator,
};

use super::types::{Feature, WikidataTranslations};
use std::path::Path;

/// 印尼（Indonesia）的 Wikidata QID，作為 admin1 的 P131 驗證 parent。
const INDONESIA_QID: &str = "Q252";
/// 印尼省（province of Indonesia）的 instance-of 類別。
const INDONESIA_PROVINCE_CLASS: &str = "Q5098";
/// 印尼縣（kabupaten / regency）的 instance-of 類別。
const INDONESIA_KABUPATEN_CLASS: &str = "Q3191695";
/// 印尼市（kota / city）的 instance-of 類別。
const INDONESIA_KOTA_CLASS: &str = "Q3199141";
/// 印尼行政市（administrative city）的 instance-of 類別。
///
/// Reason: 雅加達特區的 5 個城區（Kota Administrasi）屬此類別而非一般 kota；
/// 實驗證實缺此類別時雅加達 admin2 命中 0/6。
const INDONESIA_ADMIN_CITY_CLASS: &str = "Q4272761";
/// 印尼行政縣（administrative regency）的 instance-of 類別。
///
/// Reason: 千島群島（Kepulauan Seribu）屬雅加達特區下的行政縣（Kabupaten
/// Administrasi），類別與一般 kabupaten 不同；缺此類別會漏掉千島群島。
const INDONESIA_ADMIN_REGENCY_CLASS: &str = "Q11127777";

/// admin2 候選的 instance-of 允許類別集合（縣／市／行政市／行政縣）。
const INDONESIA_ADMIN2_CLASSES: &[&str] = &[
    INDONESIA_KABUPATEN_CLASS,
    INDONESIA_KOTA_CLASS,
    INDONESIA_ADMIN_CITY_CLASS,
    INDONESIA_ADMIN_REGENCY_CLASS,
];

/// 搜尋候選排除關鍵字（避免選舉區等非行政實體混入）。
///
/// Reason: 印尼 Wikidata 上「選區（daerah pemilihan / dapil）」常與行政區同名，
/// 含國會（DPR / DPD）與地方議會（DPR-D）選區；以小寫子字串比對候選 label
/// 排除。對照 KR 先例的 keyword 排除做法。另須注意「省名＋羅馬數字」型態的
/// 選區（如 `Jawa Barat V`）：既有比對機制要求候選 label 與搜尋字串完全相符
/// （見 wikidata translator 的 exact-match 判定），搜尋字串為純省名／縣市名時，
/// 此類帶羅馬數字後綴的選區 label 不會完全相等，因此自然被排除。
const EXCLUDED_KEYWORDS: &[&str] = &[
    "dapil",
    "daerah pemilihan",
    "electoral district",
    "dpr-d",
    "dpr",
    "dpd",
    "pemilihan",
];

/// 建構印尼 Wikidata 翻譯查詢表。
pub(super) fn build_indonesia_wikidata_cache(
    features: &[Feature],
    cache_path: &Path,
) -> Result<WikidataTranslations, String> {
    let admin1 = collect_admin1(features);
    let admin2 = collect_admin2(features);

    let mut options = WikidataClientOptions::new(SEARCH_LANG, "zh-tw");
    // Reason: BIG 來源無官方英文欄位，Wikidata 英文 label 不應覆蓋官方印尼文
    //         原文（會把 Aceh Tengah 寫成 Central Aceh）；缺中文時回退乾淨原文。
    //         比照 thailand_wikidata.rs，排除 en/source 後備。
    options.fallback_langs = vec!["zh-hant".to_string(), "zh".to_string()];
    let mut translator = WikidataTranslator::new(options, Some(cache_path.to_path_buf()), true)?;

    // admin1：以省搜尋字串建立 dataset，P31 過濾為「省」類別。
    let admin1_dataset = build_dataset(
        AdminLevel::Admin1,
        admin1.iter().map(|entry| {
            (
                entry.search_name.clone(),
                vec!["ID".to_string()],
                entry.raw_name.clone(),
            )
        }),
    )?;
    let admin1_filter = class_filter(&[INDONESIA_PROVINCE_CLASS]);
    let admin1_results = translator.batch_translate(
        &admin1_dataset,
        BatchTranslateOptions {
            batch_size: 32,
            candidate_filter: Some(&admin1_filter),
            ..BatchTranslateOptions::default()
        },
    )?;

    // search_name → QID，供 admin2 的 P131 parent 驗證使用。
    let admin1_qid_by_search: HashMap<String, String> = admin1
        .iter()
        .filter_map(|entry| {
            let id = build_item_id(AdminLevel::Admin1, &["ID".to_string()], &entry.search_name);
            admin1_results
                .get(&id)
                .and_then(|result| result.qid.clone())
                .map(|qid| (entry.search_name.clone(), qid))
        })
        .collect();

    // admin2：parent_chain 用省的 search_name，確保 ID 唯一且可對應 parent QID。
    let admin2_dataset = build_dataset(
        AdminLevel::Admin2,
        admin2.iter().map(|entry| {
            (
                entry.search_name.clone(),
                vec!["ID".to_string(), entry.parent_search_name.clone()],
                entry.raw_name.clone(),
            )
        }),
    )?;
    let mut parent_qids = HashMap::new();
    for entry in &admin2 {
        let id = build_item_id(
            AdminLevel::Admin2,
            &["ID".to_string(), entry.parent_search_name.clone()],
            &entry.search_name,
        );
        if let Some(qid) = admin1_qid_by_search.get(&entry.parent_search_name) {
            parent_qids.insert(id, qid.clone());
        }
    }
    let admin2_filter = class_filter(INDONESIA_ADMIN2_CLASSES);
    let admin2_results = translator.batch_translate(
        &admin2_dataset,
        BatchTranslateOptions {
            batch_size: 32,
            parent_qids,
            candidate_filter: Some(&admin2_filter),
        },
    )?;

    Ok(assemble_translations(
        &admin1,
        &admin1_results,
        &admin2,
        &admin2_results,
    ))
}

/// Wikidata 搜尋語言：印尼文（id）。
///
/// Reason: 實驗抽樣比對顯示 admin2 以 id 搜尋達 100% rank-1 命中，優於 en 的
/// 95.7%；印尼地名以印尼文 label 鑑別度最高。en 僅作為人工驗證後備，不在
/// 自動流程使用。
const SEARCH_LANG: &str = "id";

#[derive(Clone, Debug)]
struct Admin1Entry {
    /// BIG WADMPR 原文，作為查詢表 key。
    raw_name: String,
    /// 送入 Wikidata 的搜尋字串（雅加達等已正規化）。
    search_name: String,
}

#[derive(Clone, Debug)]
struct Admin2Entry {
    /// BIG WADMKK 原文，作為查詢表 key。
    raw_name: String,
    /// 送入 Wikidata 的搜尋字串（含 Kabupaten/Kota 前綴與雅加達正規化）。
    search_name: String,
    /// 所屬省的搜尋字串（與 admin1 entry 對齊）。
    parent_search_name: String,
    /// 所屬省的 BIG 原文，作為查詢表的 parent key。
    parent_raw_name: String,
}

fn collect_admin1(features: &[Feature]) -> Vec<Admin1Entry> {
    let mut seen = HashSet::new();
    let mut entries = Vec::new();
    for feature in features {
        let raw = attribute(feature, "WADMPR");
        if raw.is_empty() || !seen.insert(raw.to_string()) {
            continue;
        }
        entries.push(Admin1Entry {
            raw_name: raw.to_string(),
            search_name: normalize_province_search(raw),
        });
    }
    entries
}

fn collect_admin2(features: &[Feature]) -> Vec<Admin2Entry> {
    let mut seen = HashSet::new();
    let mut entries = Vec::new();
    for feature in features {
        let parent = attribute(feature, "WADMPR");
        let raw = attribute(feature, "WADMKK");
        if parent.is_empty() || raw.is_empty() {
            continue;
        }
        if !seen.insert((parent.to_string(), raw.to_string())) {
            continue;
        }
        entries.push(Admin2Entry {
            raw_name: raw.to_string(),
            search_name: kabupaten_kota_search(raw),
            parent_search_name: normalize_province_search(parent),
            parent_raw_name: parent.to_string(),
        });
    }
    entries
}

/// 雅加達特區城區搜尋正規化（去除官方行政前綴）。
///
/// Reason: BIG 圖資以官方全名（`Kota Adm. Jakarta Barat`）儲存雅加達五城區
/// 與千島群島，但 Wikidata 以通用名稱（`Jakarta Barat`）為主 label；不正規化
/// 會搜不到正確實體。
fn normalize_jakarta(name: &str) -> Option<&'static str> {
    match name {
        "Kota Adm. Jakarta Barat" => Some("Jakarta Barat"),
        "Kota Adm. Jakarta Timur" => Some("Jakarta Timur"),
        "Kota Adm. Jakarta Selatan" => Some("Jakarta Selatan"),
        "Kota Adm. Jakarta Utara" => Some("Jakarta Utara"),
        "Kota Adm. Jakarta Pusat" => Some("Jakarta Pusat"),
        "Adm. Kep. Seribu" => Some("Kepulauan Seribu"),
        _ => None,
    }
}

/// 省層級搜尋字串（目前省名不需前綴，僅保留正規化掛勾）。
fn normalize_province_search(name: &str) -> String {
    name.trim().to_string()
}

/// admin2（縣市）搜尋字串：套用雅加達正規化與 Kabupaten/Kota 前綴消歧。
///
/// Reason: BIG WADMKK 對 kabupaten 多半只存「Kabupaten」之後的地名，
/// 與同名 kota / 城市行政中心同名；加 `Kabupaten ` 前綴可在 Wikidata
/// 命中正確的縣級實體。以 `Kota ` 開頭者本身已含類型前綴，保持原樣。
fn kabupaten_kota_search(raw: &str) -> String {
    if let Some(normalized) = normalize_jakarta(raw) {
        return normalized.to_string();
    }
    if raw.starts_with("Kota ") {
        raw.to_string()
    } else {
        format!("Kabupaten {raw}")
    }
}

fn build_dataset(
    level: AdminLevel,
    rows: impl IntoIterator<Item = (String, Vec<String>, String)>,
) -> Result<TranslationDataset, String> {
    let mut items = Vec::new();
    for (search_name, parent_chain, raw_name) in rows {
        // Reason: 把 BIG 官方原文（WADMPR/WADMKK）寫入 metadata，讓翻譯失敗時
        //         source_fallback_result 回退到乾淨原文，而非帶 `Kabupaten `
        //         消歧前綴的 search_name。
        let metadata = HashMap::from([(METADATA_OFFICIAL_ORIGINAL.to_string(), raw_name.clone())]);
        items.push(TranslationItem::from_values(
            level,
            search_name,
            SEARCH_LANG,
            "zh-tw",
            parent_chain,
            metadata,
        )?);
    }
    TranslationDataset::new(items, level, INDONESIA_QID, SEARCH_LANG, "zh-tw", true)
}

fn build_item_id(level: AdminLevel, parent_chain: &[String], name: &str) -> String {
    crate::wikidata::build_translation_item_id(level, parent_chain, name)
}

fn class_filter(allowed_classes: &[&str]) -> impl Fn(&str, &WikidataCandidateMetadata<'_>) -> bool {
    let allowed: HashSet<String> = allowed_classes
        .iter()
        .map(|class| (*class).to_string())
        .collect();
    move |_name, metadata: &WikidataCandidateMetadata<'_>| {
        let label_excluded = metadata.labels.values().any(|label| {
            let lower = label.to_ascii_lowercase();
            EXCLUDED_KEYWORDS
                .iter()
                .any(|keyword| lower.contains(keyword))
        });
        if label_excluded {
            return false;
        }
        metadata
            .instance_of
            .iter()
            .any(|class| allowed.contains(class))
    }
}

/// 將搜尋字串結果重新以 BIG 原文（raw name）為 key 組成查詢表。
///
/// 此處只儲存「原始 Wikidata 譯名」（翻譯失敗時回退乾淨 BIG 原文）。s2t 簡轉繁
/// 與 admin1 補省字尾正規化延後到 handler 消費層（`indonesia.rs`）統一施作，
/// 讓 live 與 fixture 兩條路徑得到一致的最終形態。
fn assemble_translations(
    admin1: &[Admin1Entry],
    admin1_results: &HashMap<String, TranslationResult>,
    admin2: &[Admin2Entry],
    admin2_results: &HashMap<String, TranslationResult>,
) -> WikidataTranslations {
    let mut translations = WikidataTranslations::default();
    let mut fallback: HashMap<String, Option<String>> = HashMap::new();

    for entry in admin1 {
        let id = build_item_id(AdminLevel::Admin1, &["ID".to_string()], &entry.search_name);
        if let Some(result) = admin1_results.get(&id) {
            translations.admin1_by_name.insert(
                entry.raw_name.clone(),
                result_text(&entry.raw_name, &entry.search_name, result),
            );
        }
    }

    for entry in admin2 {
        let id = build_item_id(
            AdminLevel::Admin2,
            &["ID".to_string(), entry.parent_search_name.clone()],
            &entry.search_name,
        );
        if let Some(result) = admin2_results.get(&id) {
            let translated = result_text(&entry.raw_name, &entry.search_name, result);
            translations
                .admin2_by_parent
                .entry(entry.parent_raw_name.clone())
                .or_default()
                .insert(entry.raw_name.clone(), translated.clone());
            // 全域後備以 raw_name（WADMKK）為 key，剔除同名不同譯者。
            fallback
                .entry(entry.raw_name.clone())
                .and_modify(|existing| {
                    if existing.as_deref() != Some(translated.as_str()) {
                        *existing = None;
                    }
                })
                .or_insert(Some(translated));
        }
    }

    translations.fallback_by_name = fallback
        .into_iter()
        .filter_map(|(name, translated)| translated.map(|value| (name, value)))
        .collect();
    translations
}

/// 取得原始譯名：翻譯非空時用 Wikidata 譯名；翻譯為空時回退乾淨 BIG 原文。
///
/// Reason: 回退用 `raw_name`（WADMPR/WADMKK 原文）而非 search_name，避免把
/// 人工加上的 `Kabupaten ` 消歧前綴洩漏到輸出。`source == "original"` 表示
/// 翻譯流程已回退為原文（其值為 search_name），須改用乾淨 raw_name；
/// translated 等於 search_name 的字串防禦則涵蓋舊版 cache 殘留——此類
/// 譯名即使來自 Wikidata 標籤也無翻譯價值，回退原文與整體慣例一致。
fn result_text(raw_name: &str, search_name: &str, result: &TranslationResult) -> String {
    if result.translated.is_empty()
        || result.source == "original"
        || result.translated == search_name
    {
        raw_name.to_string()
    } else {
        result.translated.clone()
    }
}

fn attribute<'a>(feature: &'a Feature, key: &str) -> &'a str {
    feature.attributes.get(key).unwrap_or("")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn translated(text: &str) -> TranslationResult {
        TranslationResult {
            translated: text.to_string(),
            qid: Some("Q1".to_string()),
            source: "wikidata".to_string(),
            used_lang: "zh-hant".to_string(),
            parent_verified: true,
        }
    }

    fn empty_result() -> TranslationResult {
        TranslationResult::original("")
    }

    #[test]
    fn result_text_uses_translation_or_clean_raw_name() {
        // 翻譯非空時用 Wikidata 譯名（簡轉繁延後到消費層）。
        assert_eq!(
            result_text("Papua", "Papua", &translated("巴布亚省")),
            "巴布亚省"
        );
        // Q1：admin2 無中文標籤時回退乾淨 WADMKK 原文（無 `Kabupaten ` 前綴）。
        assert_eq!(
            result_text("Barito Timur", "Kabupaten Barito Timur", &empty_result()),
            "Barito Timur"
        );
        // translated 只回吐 search_name（帶消歧前綴）時，視為無翻譯回退原文。
        assert_eq!(
            result_text(
                "Sorong",
                "Kabupaten Sorong",
                &translated("Kabupaten Sorong")
            ),
            "Sorong"
        );
    }
}
