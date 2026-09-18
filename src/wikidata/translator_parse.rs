//! Wikidata／Wikipedia API 回應的解析，與不依賴 client 的純函式輔助。
//!
//! 自 `translator.rs` 拆出——回應的 JSON 形狀與翻譯流程的決策邏輯是兩件事，
//! 混在同一檔會讓 `WikidataTranslator` 的主流程被 JSON 走訪淹沒。

use std::collections::{HashMap, HashSet};

use serde_json::Value;

use super::types::TranslationResult;
use super::{BatchTranslateOptions, TranslationDataset, TranslationItem};

#[derive(Debug)]
pub(super) struct SearchData {
    pub(super) item: TranslationItem,
    pub(super) qids: Vec<String>,
}

/// 解析 item 的 P131 驗證 parent：明確指定的 parent QID（admin2 對
/// admin1）優先，否則回退到 dataset 的國家 QID（admin1 對國家）。
///
/// Reason: 這是 translator 的標準規則——每個 item 至少對「已知最特定
/// 的上層」驗證行政隸屬，新增國家時無需（也無法）另行選擇。
pub(super) fn resolve_parent_qid<'a>(
    options: &'a BatchTranslateOptions<'_>,
    dataset: &'a TranslationDataset,
    item: &TranslationItem,
) -> &'a str {
    options
        .parent_qids
        .get(&item.id)
        .or_else(|| options.parent_qids.get(&item.original_name))
        .map(String::as_str)
        .unwrap_or(dataset.country_qid.as_str())
}

pub fn dedupe_keep_order(values: impl IntoIterator<Item = String>) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut deduped = Vec::new();
    for value in values {
        if seen.insert(value.clone()) {
            deduped.push(value);
        }
    }
    deduped
}

pub(super) fn wikidata_result(
    translated: &str,
    source: &str,
    used_lang: &str,
) -> TranslationResult {
    TranslationResult {
        translated: translated.to_string(),
        qid: None,
        source: source.to_string(),
        used_lang: used_lang.to_string(),
        parent_verified: false,
    }
}

pub(super) fn parse_search_qids(body: &str) -> Result<Vec<String>, String> {
    let root: Value = serde_json::from_str(body)
        .map_err(|error| format!("Wikidata search JSON 解析失敗：{error}"))?;
    Ok(root
        .get("search")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|entry| entry.get("id").and_then(Value::as_str))
        .map(ToString::to_string)
        .collect())
}

pub(super) fn parse_entity_labels(
    body: &str,
) -> Result<HashMap<String, HashMap<String, String>>, String> {
    let root: Value = serde_json::from_str(body)
        .map_err(|error| format!("Wikidata labels JSON 解析失敗：{error}"))?;
    let mut results = HashMap::new();
    let Some(entities) = root.get("entities").and_then(Value::as_object) else {
        return Ok(results);
    };
    for (qid, entity) in entities {
        let mut labels = HashMap::new();
        if let Some(label_map) = entity.get("labels").and_then(Value::as_object) {
            for (lang, label) in label_map {
                if let Some(value) = label.get("value").and_then(Value::as_str) {
                    labels.insert(lang.clone(), value.to_string());
                }
            }
        }
        // Reason: sitelink 標題與 label 一起存進同一張表，沿用既有的 labels
        //         快取與失效機制，不必為了「條目標題」另開一層快取。
        //         zhwiki 供既有的中文標題轉換後備使用；kowiki 供 KR handler
        //         取韓國行政區的漢字表記（Wikidata 沒有結構化漢字欄位）。
        for wiki in ["zhwiki", "kowiki"] {
            if let Some(title) = entity
                .get("sitelinks")
                .and_then(|value| value.get(wiki))
                .and_then(|value| value.get("title"))
                .and_then(Value::as_str)
            {
                labels.insert(wiki.to_string(), title.to_string());
            }
        }
        results.insert(qid.clone(), labels);
    }
    Ok(results)
}

pub(super) fn parse_entity_instance_of(body: &str) -> Result<HashMap<String, Vec<String>>, String> {
    let root: Value = serde_json::from_str(body)
        .map_err(|error| format!("Wikidata P31 JSON 解析失敗：{error}"))?;
    let mut results = HashMap::new();
    let Some(entities) = root.get("entities").and_then(Value::as_object) else {
        return Ok(results);
    };
    for (qid, entity) in entities {
        let mut values = Vec::new();
        if let Some(claims) = entity
            .get("claims")
            .and_then(|claims| claims.get("P31"))
            .and_then(Value::as_array)
        {
            for claim in claims {
                if let Some(id) = claim
                    .get("mainsnak")
                    .and_then(|value| value.get("datavalue"))
                    .and_then(|value| value.get("value"))
                    .and_then(|value| value.get("id"))
                    .and_then(Value::as_str)
                {
                    values.push(id.to_string());
                }
            }
        }
        results.insert(qid.clone(), values);
    }
    Ok(results)
}

pub(super) fn parse_p131_answer(body: &str) -> Result<bool, String> {
    let root: Value = serde_json::from_str(body)
        .map_err(|error| format!("Wikidata P131 JSON 解析失敗：{error}"))?;
    root.get("boolean")
        .and_then(Value::as_bool)
        .ok_or_else(|| "Wikidata P131 回應缺少 boolean".to_string())
}

/// 解析韓文維基 extracts 回應，回傳「請求時使用的標題 → 條目開頭文字」。
///
/// Reason: MediaWiki 會把請求標題正規化（`normalized`）或跟隨重新導向
/// （`redirects`）後才回傳 page，回應中的 `title` 未必等於我們送出的標題。
/// 呼叫端是以「送出的標題」為 key 查回結果，因此必須把這兩層映射反推回去，
/// 否則被重新導向的條目會被誤判為查無資料。
pub(super) fn parse_kowiki_extracts(body: &str) -> Result<HashMap<String, String>, String> {
    let root: Value = serde_json::from_str(body)
        .map_err(|error| format!("韓文維基 extracts JSON 解析失敗：{error}"))?;
    let Some(query) = root.get("query") else {
        return Ok(HashMap::new());
    };
    // 回傳標題 → 原始請求標題（可能經過 normalize 再 redirect 兩段）。
    let mut origin = HashMap::<String, String>::new();
    for key in ["normalized", "redirects"] {
        for entry in query
            .get(key)
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            let (Some(from), Some(to)) = (
                entry.get("from").and_then(Value::as_str),
                entry.get("to").and_then(Value::as_str),
            ) else {
                continue;
            };
            let source = origin
                .get(from)
                .cloned()
                .unwrap_or_else(|| from.to_string());
            origin.insert(to.to_string(), source);
        }
    }
    let mut extracts = HashMap::new();
    for page in query
        .get("pages")
        .and_then(Value::as_object)
        .into_iter()
        .flatten()
        .map(|(_, page)| page)
    {
        let (Some(title), Some(extract)) = (
            page.get("title").and_then(Value::as_str),
            page.get("extract").and_then(Value::as_str),
        ) else {
            continue;
        };
        if let Some(requested) = origin.get(title) {
            extracts.insert(requested.clone(), extract.to_string());
        }
        extracts.insert(title.to_string(), extract.to_string());
    }
    Ok(extracts)
}

pub(super) fn parse_zhwiki_converted_title(body: &str) -> Result<String, String> {
    let root: Value = serde_json::from_str(body)
        .map_err(|error| format!("中文維基轉換 JSON 解析失敗：{error}"))?;
    if let Some(converted) = root
        .get("query")
        .and_then(|query| query.get("converted"))
        .and_then(Value::as_array)
        .and_then(|values| values.first())
        .and_then(|value| value.get("to"))
        .and_then(Value::as_str)
    {
        return Ok(converted.to_string());
    }
    root.get("query")
        .and_then(|query| query.get("pages"))
        .and_then(Value::as_object)
        .and_then(|pages| pages.values().next())
        .and_then(|page| page.get("title"))
        .and_then(Value::as_str)
        .map(ToString::to_string)
        .ok_or_else(|| "中文維基轉換回應缺少 title".to_string())
}
