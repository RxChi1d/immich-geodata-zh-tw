//! KR/TH 等 Wikidata 翻譯國家共用的查詢表建構與 stub 讀取。
//!
//! Reason: 這些邏輯與國家無關（僅差在國碼字首），集中一份避免
//! 各國 handler 複製貼上後分歧。

use std::collections::HashMap;
use std::fs;
use std::path::Path;

use serde_json::Value;

use crate::wikidata::{TranslationDataset, TranslationItem, TranslationResult};

use super::types::WikidataTranslations;

/// 從 admin1 翻譯結果建立「admin1 名稱 → QID」對照，供 admin2 P131 驗證。
pub(super) fn admin2_parent_qids(
    admin1_dataset: &TranslationDataset,
    admin1_results: &HashMap<String, TranslationResult>,
    admin2_dataset: &TranslationDataset,
) -> HashMap<String, String> {
    let admin1_qids = admin1_dataset
        .items()
        .iter()
        .filter_map(|item| {
            admin1_results
                .get(&item.id)
                .and_then(|result| result.qid.clone())
                .map(|qid| (item.original_name.as_str(), qid))
        })
        .collect::<HashMap<_, _>>();
    let mut parent_qids = HashMap::new();
    for item in admin2_dataset.items() {
        let Some(parent_name) = item.parent_chain.get(1).map(String::as_str) else {
            continue;
        };
        let Some(parent_qid) = admin1_qids.get(parent_name) else {
            continue;
        };
        parent_qids.insert(item.id.clone(), parent_qid.clone());
    }
    parent_qids
}

/// 將 admin1/admin2 翻譯結果整理為 extract 查詢表。
pub(super) fn translations_from_results(
    admin1_dataset: &TranslationDataset,
    admin1_results: &HashMap<String, TranslationResult>,
    admin2_dataset: &TranslationDataset,
    admin2_results: &HashMap<String, TranslationResult>,
) -> WikidataTranslations {
    let mut translations = WikidataTranslations::default();
    let mut fallback = FallbackBuilder::default();
    for item in admin1_dataset.items() {
        if let Some(result) = admin1_results.get(&item.id) {
            translations
                .admin1_by_name
                .insert(item.original_name.clone(), result_text(item, result));
        }
    }
    for item in admin2_dataset.items() {
        if let Some(result) = admin2_results.get(&item.id) {
            let translated = result_text(item, result);
            let parent = item.parent_chain.last().cloned().unwrap_or_default();
            translations
                .admin2_by_parent
                .entry(parent)
                .or_default()
                .insert(item.original_name.clone(), translated.clone());
            fallback.insert(item.original_name.clone(), translated);
        }
    }
    translations.fallback_by_name = fallback.into_unambiguous();
    translations
}

fn result_text(item: &TranslationItem, result: &TranslationResult) -> String {
    if result.translated.is_empty() {
        item.original_name.clone()
    } else {
        result.translated.clone()
    }
}

/// 讀取 `{CC}_wikidata_stub.json`（或同 schema 的快取檔）為查詢表。
pub(super) fn read_wikidata_stub(
    path: &Path,
    country_code: &str,
) -> Result<WikidataTranslations, String> {
    let content = fs::read_to_string(path).map_err(|error| {
        format!(
            "無法讀取 {country_code} Wikidata stub {}：{error}",
            path.display()
        )
    })?;
    let root: Value = serde_json::from_str(&content).map_err(|error| {
        format!(
            "{country_code} Wikidata JSON 解析失敗 {}：{error}",
            path.display()
        )
    })?;
    let entries = root
        .get("translations")
        .and_then(Value::as_object)
        .ok_or_else(|| {
            format!(
                "{country_code} Wikidata stub/cache 缺少 translations：{}",
                path.display()
            )
        })?;
    let admin1_prefix = format!("admin_1/{country_code}/");
    let admin2_prefix = format!("admin_2/{country_code}/");
    let mut translations = WikidataTranslations::default();
    let mut fallback = FallbackBuilder::default();
    for (key, value) in entries {
        let Some(translated) = stub_translation_value(value) else {
            continue;
        };
        if let Some(name) = key.strip_prefix(&admin1_prefix) {
            translations
                .admin1_by_name
                .insert(name.to_string(), translated);
        } else if let Some(rest) = key.strip_prefix(&admin2_prefix) {
            let mut parts = rest.splitn(2, '/');
            if let (Some(parent), Some(name)) = (parts.next(), parts.next()) {
                translations
                    .admin2_by_parent
                    .entry(parent.to_string())
                    .or_default()
                    .insert(name.to_string(), translated.clone());
                fallback.insert(name.to_string(), translated);
            }
        } else {
            fallback.insert(key.to_string(), translated.clone());
            translations
                .admin1_by_name
                .insert(key.to_string(), translated);
        }
    }
    translations.fallback_by_name = fallback.into_unambiguous();
    Ok(translations)
}

fn stub_translation_value(value: &Value) -> Option<String> {
    match value {
        Value::String(value) => Some(value.clone()),
        Value::Object(object) => object
            .get("translated")
            .and_then(Value::as_str)
            .map(ToString::to_string),
        _ => None,
    }
}

/// 收集名稱 → 翻譯的全域後備表，剔除「同名但不同譯」的歧義名稱。
///
/// Reason: 跨上層行政區同名的單位（如多個府都有同名縣）若譯名不同，
/// 以名稱為 key 的全域後備會 last-wins 回傳錯誤翻譯；寧可不後備也
/// 不能拿到別的行政區的譯名。譯名一致的同名單位（如南韓多個 중구
/// 都是中區）則安全保留。
#[derive(Default)]
struct FallbackBuilder {
    entries: HashMap<String, Option<String>>,
}

impl FallbackBuilder {
    fn insert(&mut self, name: String, translated: String) {
        self.entries
            .entry(name)
            .and_modify(|existing| {
                if existing.as_deref() != Some(translated.as_str()) {
                    *existing = None;
                }
            })
            .or_insert(Some(translated));
    }

    fn into_unambiguous(self) -> HashMap<String, String> {
        self.entries
            .into_iter()
            .filter_map(|(name, translated)| translated.map(|value| (name, value)))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fallback_builder_drops_ambiguous_names() {
        let mut builder = FallbackBuilder::default();
        builder.insert("중구".to_string(), "中區".to_string());
        builder.insert("중구".to_string(), "中區".to_string());
        builder.insert("Mueang".to_string(), "甲府".to_string());
        builder.insert("Mueang".to_string(), "乙府".to_string());

        let fallback = builder.into_unambiguous();

        // 同名同譯保留；同名不同譯剔除。
        assert_eq!(fallback.get("중구").map(String::as_str), Some("中區"));
        assert!(!fallback.contains_key("Mueang"));
    }
}
