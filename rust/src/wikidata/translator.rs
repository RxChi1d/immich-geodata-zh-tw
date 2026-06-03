use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::time::Duration;

use opencc_rust::Converter;
use serde_json::Value;

use super::cache::TranslationCacheStore;
use super::client::{WikidataApi, WikidataClientOptions, WikidataHttpClient};
use super::types::{
    METADATA_OFFICIAL_EN, METADATA_OFFICIAL_TH, TranslationDataset, TranslationItem,
    TranslationResult,
};

const SEARCH_LIMIT: usize = 7;
const ENTITY_BATCH_SIZE: usize = 50;
const CLAIM_LANGUAGES: &str = "en";

pub struct BatchTranslateOptions<'a> {
    pub batch_size: usize,
    pub parent_qids: HashMap<String, String>,
    pub candidate_filter: Option<&'a CandidateFilter>,
}

pub type CandidateFilter = dyn for<'a> Fn(&str, &WikidataCandidateMetadata<'a>) -> bool;

impl Default for BatchTranslateOptions<'_> {
    fn default() -> Self {
        Self {
            batch_size: 20,
            parent_qids: HashMap::new(),
            candidate_filter: None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WikidataCandidateMetadata<'a> {
    pub qid: &'a str,
    pub labels: &'a HashMap<String, String>,
    pub instance_of: &'a [String],
}

pub struct WikidataTranslator<C: WikidataApi> {
    options: WikidataClientOptions,
    label_languages: String,
    client: C,
    pub cache_store: TranslationCacheStore,
    opencc: Option<Converter>,
}

impl WikidataTranslator<WikidataHttpClient> {
    pub fn new(
        options: WikidataClientOptions,
        cache_path: Option<PathBuf>,
        use_opencc: bool,
    ) -> Result<Self, String> {
        let client = WikidataHttpClient::new(options.clone())?;
        Self::with_client(options, client, cache_path, use_opencc)
    }
}

impl<C: WikidataApi> WikidataTranslator<C> {
    pub fn with_client(
        options: WikidataClientOptions,
        client: C,
        cache_path: Option<PathBuf>,
        use_opencc: bool,
    ) -> Result<Self, String> {
        let label_languages = dedupe_keep_order(
            [options.target_lang.clone()]
                .into_iter()
                .chain(options.fallback_langs.iter().cloned()),
        )
        .join("|");
        let opencc = if use_opencc {
            Some(
                opencc_rust::presets::cn2t::converter("cn", "t")
                    .map_err(|error| format!("無法建立 OpenCC native converter：{error}"))?,
            )
        } else {
            None
        };
        let cache_store = TranslationCacheStore::load(
            options.source_lang.clone(),
            options.target_lang.clone(),
            cache_path,
        )?;
        Ok(Self {
            options,
            label_languages,
            client,
            cache_store,
            opencc,
        })
    }

    pub fn batch_translate(
        &mut self,
        dataset: &TranslationDataset,
        options: BatchTranslateOptions<'_>,
    ) -> Result<HashMap<String, TranslationResult>, String> {
        if dataset.is_empty() {
            return Ok(HashMap::new());
        }
        let batch_size = options.batch_size.max(1);
        let mut results = HashMap::with_capacity(dataset.len());
        let mut pending = Vec::<SearchData>::with_capacity(dataset.len());
        let mut cache_hits = 0_usize;

        for batch in dataset.items().chunks(batch_size) {
            for item in batch {
                if let Some(result) = self.cache_store.get_translation(item) {
                    let parent_qid = options
                        .parent_qids
                        .get(&item.id)
                        .or_else(|| options.parent_qids.get(&item.original_name));
                    if parent_qid.is_none() || result.qid.is_none() || result.parent_verified {
                        cache_hits += 1;
                        results.insert(item.id.clone(), result);
                        continue;
                    }
                }
                let qids = self.search_wikidata(item);
                pending.push(SearchData {
                    item: item.clone(),
                    qids,
                });
            }
        }

        println!(
            "stage=wikidata phase=search level={} total={} cache_hits={}",
            dataset.level.as_str(),
            dataset.len(),
            cache_hits
        );

        let prefetched_labels = if let Some(candidate_filter) = options.candidate_filter {
            Some(self.apply_candidate_filter(&mut pending, candidate_filter)?)
        } else {
            None
        };

        let all_labels = if let Some(labels) = prefetched_labels {
            labels
        } else {
            let all_qids =
                dedupe_keep_order(pending.iter().flat_map(|data| data.qids.iter().cloned()));
            self.batch_get_labels(&all_qids)?
        };
        let mut fallback_count = 0_usize;

        for data in pending {
            let parent_qid = options
                .parent_qids
                .get(&data.item.id)
                .or_else(|| options.parent_qids.get(&data.item.original_name))
                .map(String::as_str);
            let Some(selected_qid) = self.select_qid(&data.qids, parent_qid)? else {
                let result = self.source_fallback_result(&data.item);
                self.cache_store
                    .set_translation(&data.item, &result, parent_qid)?;
                fallback_count += 1;
                results.insert(data.item.id, result);
                continue;
            };

            let mut result = self.select_best_label(all_labels.get(&selected_qid), &data.item);
            result.qid = Some(selected_qid.clone());
            if let Some(parent_qid) = parent_qid {
                result.parent_verified = self.verify_p131(&selected_qid, parent_qid)?;
            }
            self.cache_store
                .set_translation(&data.item, &result, parent_qid)?;
            results.insert(data.item.id, result);
        }

        self.cache_store
            .flush_if_needed(true, 0, Duration::from_secs(0))?;
        println!(
            "stage=wikidata phase=translate level={} total={} fallback={}",
            dataset.level.as_str(),
            dataset.len(),
            fallback_count
        );
        Ok(results)
    }

    fn search_wikidata(&mut self, item: &TranslationItem) -> Vec<String> {
        if let Some(qids) = self.cache_store.get_search_results(item) {
            return qids;
        }
        let qids = self
            .client
            .search_entities_json(&item.original_name, SEARCH_LIMIT)
            .ok()
            .and_then(|json| parse_search_qids(&json).ok())
            .unwrap_or_default();
        let _ = self.cache_store.set_search_results(item, &qids);
        qids
    }

    fn apply_candidate_filter(
        &mut self,
        search_results: &mut [SearchData],
        candidate_filter: &CandidateFilter,
    ) -> Result<HashMap<String, HashMap<String, String>>, String> {
        let qids = dedupe_keep_order(
            search_results
                .iter()
                .flat_map(|data| data.qids.iter().cloned()),
        );
        let labels = self.batch_get_labels(&qids)?;
        let instance_of = self.batch_get_instance_of(&qids)?;
        for data in search_results {
            let empty_labels = HashMap::new();
            let empty_instance_of = Vec::new();
            data.qids.retain(|qid| {
                let metadata = WikidataCandidateMetadata {
                    qid,
                    labels: labels.get(qid).unwrap_or(&empty_labels),
                    instance_of: instance_of
                        .get(qid)
                        .map(Vec::as_slice)
                        .unwrap_or(empty_instance_of.as_slice()),
                };
                candidate_filter(&data.item.original_name, &metadata)
            });
        }
        Ok(labels)
    }

    fn batch_get_labels(
        &mut self,
        qids: &[String],
    ) -> Result<HashMap<String, HashMap<String, String>>, String> {
        let unique_qids = dedupe_keep_order(qids.iter().cloned());
        let mut results = HashMap::with_capacity(unique_qids.len());
        let mut uncached = Vec::new();
        for qid in &unique_qids {
            if let Some(labels) = self.cache_store.get_labels(qid) {
                results.insert(qid.clone(), labels);
            } else {
                uncached.push(qid.clone());
            }
        }
        for batch in uncached.chunks(ENTITY_BATCH_SIZE) {
            let Ok(body) =
                self.client
                    .get_entities_json(batch, "labels|sitelinks", &self.label_languages)
            else {
                continue;
            };
            for (qid, labels) in parse_entity_labels(&body)? {
                self.cache_store.set_labels(&qid, &labels)?;
                results.insert(qid, labels);
            }
        }
        Ok(results)
    }

    fn batch_get_instance_of(
        &mut self,
        qids: &[String],
    ) -> Result<HashMap<String, Vec<String>>, String> {
        let unique_qids = dedupe_keep_order(qids.iter().cloned());
        let mut results = HashMap::with_capacity(unique_qids.len());
        let mut uncached = Vec::new();
        for qid in &unique_qids {
            if let Some(values) = self.cache_store.get_instance_of(qid) {
                results.insert(qid.clone(), values);
            } else {
                uncached.push(qid.clone());
            }
        }
        for batch in uncached.chunks(ENTITY_BATCH_SIZE) {
            let Ok(body) = self
                .client
                .get_entities_json(batch, "claims", CLAIM_LANGUAGES)
            else {
                continue;
            };
            for (qid, values) in parse_entity_instance_of(&body)? {
                self.cache_store.set_instance_of(&qid, &values)?;
                results.insert(qid, values);
            }
        }
        Ok(results)
    }

    fn select_qid(
        &mut self,
        qids: &[String],
        parent_qid: Option<&str>,
    ) -> Result<Option<String>, String> {
        let Some(parent_qid) = parent_qid else {
            return Ok(qids.first().cloned());
        };
        for qid in qids {
            if self.verify_p131(qid, parent_qid)? {
                return Ok(Some(qid.clone()));
            }
        }
        Ok(None)
    }

    fn verify_p131(&mut self, candidate_qid: &str, parent_qid: &str) -> Result<bool, String> {
        if let Some(value) = self.cache_store.get_p131(candidate_qid, parent_qid) {
            return Ok(value);
        }
        let value = self
            .client
            .ask_p131_json(candidate_qid, parent_qid)
            .ok()
            .and_then(|json| parse_p131_answer(&json).ok())
            .unwrap_or(false);
        self.cache_store
            .set_p131(candidate_qid, parent_qid, value)?;
        Ok(value)
    }

    fn select_best_label(
        &self,
        labels: Option<&HashMap<String, String>>,
        item: &TranslationItem,
    ) -> TranslationResult {
        let Some(labels) = labels else {
            return self.source_fallback_result(item);
        };
        if let Some(label) = labels.get(&self.options.target_lang) {
            return wikidata_result(label, "wikidata", &self.options.target_lang);
        }
        for lang in &self.options.fallback_langs {
            if let Some(label) = labels.get(lang) {
                if lang == "zh" {
                    if let Some(converter) = &self.opencc {
                        return wikidata_result(&converter.convert(label), "opencc", "zh→zh-tw");
                    }
                    return wikidata_result(label, "wikidata-zh", "zh");
                }
                return wikidata_result(label, &format!("wikidata-{lang}"), lang);
            }
        }
        if let Some(title) = labels.get("zhwiki") {
            if let Ok(body) = self.client.zhwiki_convert_title_json(title)
                && let Ok(converted) = parse_zhwiki_converted_title(&body)
            {
                return wikidata_result(&converted, "zhwiki-convert", "zhwiki");
            }
            return wikidata_result(title, "zhwiki", "zhwiki");
        }
        self.source_fallback_result(item)
    }

    fn source_fallback_result(&self, item: &TranslationItem) -> TranslationResult {
        for (key, used_lang) in [
            (METADATA_OFFICIAL_EN, "official_en"),
            (METADATA_OFFICIAL_TH, "official_th"),
        ] {
            if let Some(value) = item.metadata.get(key) {
                let value = value.trim();
                if !value.is_empty() {
                    return wikidata_result(value, "metadata", used_lang);
                }
            }
        }
        TranslationResult::original(&item.original_name)
    }
}

#[derive(Debug)]
struct SearchData {
    item: TranslationItem,
    qids: Vec<String>,
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

fn wikidata_result(translated: &str, source: &str, used_lang: &str) -> TranslationResult {
    TranslationResult {
        translated: translated.to_string(),
        qid: None,
        source: source.to_string(),
        used_lang: used_lang.to_string(),
        parent_verified: false,
    }
}

fn parse_search_qids(body: &str) -> Result<Vec<String>, String> {
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

fn parse_entity_labels(body: &str) -> Result<HashMap<String, HashMap<String, String>>, String> {
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
        if let Some(title) = entity
            .get("sitelinks")
            .and_then(|value| value.get("zhwiki"))
            .and_then(|value| value.get("title"))
            .and_then(Value::as_str)
        {
            labels.insert("zhwiki".to_string(), title.to_string());
        }
        results.insert(qid.clone(), labels);
    }
    Ok(results)
}

fn parse_entity_instance_of(body: &str) -> Result<HashMap<String, Vec<String>>, String> {
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

fn parse_p131_answer(body: &str) -> Result<bool, String> {
    let root: Value = serde_json::from_str(body)
        .map_err(|error| format!("Wikidata P131 JSON 解析失敗：{error}"))?;
    root.get("boolean")
        .and_then(Value::as_bool)
        .ok_or_else(|| "Wikidata P131 回應缺少 boolean".to_string())
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
