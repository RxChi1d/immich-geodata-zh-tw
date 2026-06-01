use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use chrono::Utc;
use serde_json::{Map, Value, json};

use super::types::{TranslationItem, TranslationResult};

const VERSION: &str = "1.0";

#[derive(Debug, Clone)]
pub struct TranslationCacheStore {
    pub cache_path: Option<PathBuf>,
    metadata: Map<String, Value>,
    translations: Map<String, Value>,
    search: Map<String, Value>,
    labels: Map<String, Value>,
    p131: Map<String, Value>,
    instance_of: Map<String, Value>,
    indexes_by_name: Map<String, Value>,
    dirty: usize,
    last_flush: Instant,
}

impl TranslationCacheStore {
    pub fn load(
        source_lang: impl Into<String>,
        target_lang: impl Into<String>,
        cache_path: Option<PathBuf>,
    ) -> Result<Self, String> {
        let source_lang = source_lang.into();
        let target_lang = target_lang.into();
        let Some(path) = cache_path.as_deref() else {
            return Ok(Self::empty(source_lang, target_lang, None));
        };
        if !path.exists() {
            return Ok(Self::empty(
                source_lang,
                target_lang,
                Some(path.to_path_buf()),
            ));
        }

        let content = fs::read_to_string(path)
            .map_err(|error| format!("無法讀取 Wikidata cache {}：{error}", path.display()))?;
        let root: Value = serde_json::from_str(&content)
            .map_err(|error| format!("Wikidata cache JSON 解析失敗 {}：{error}", path.display()))?;
        if let Some(version) = root
            .get("metadata")
            .and_then(Value::as_object)
            .and_then(|metadata| metadata.get("version"))
            .and_then(Value::as_str)
            .filter(|version| *version != VERSION)
        {
            let backup = path.with_extension(format!("{}.bak", version));
            fs::copy(path, &backup).map_err(|error| {
                format!(
                    "Wikidata cache schema 不相容且備份失敗 {}：{error}",
                    backup.display()
                )
            })?;
            return Ok(Self::empty(
                source_lang,
                target_lang,
                Some(path.to_path_buf()),
            ));
        }

        let mut store = Self::empty(source_lang, target_lang, Some(path.to_path_buf()));
        store.metadata = object_at(&root, &["metadata"]).unwrap_or_else(|| store.metadata.clone());
        store.translations = object_at(&root, &["translations"]).unwrap_or_default();
        store.search = object_at(&root, &["cache", "search"]).unwrap_or_default();
        store.labels = object_at(&root, &["cache", "labels"]).unwrap_or_default();
        store.p131 = object_at(&root, &["cache", "p131"]).unwrap_or_default();
        store.instance_of = object_at(&root, &["cache", "instance_of"]).unwrap_or_default();
        store.indexes_by_name = object_at(&root, &["indexes", "by_name"]).unwrap_or_default();
        Ok(store)
    }

    pub fn get_translation(&self, item: &TranslationItem) -> Option<TranslationResult> {
        let entry = self.translations.get(&item.id)?.as_object()?;
        let translated = entry.get("translated")?.as_str()?.to_string();
        Some(TranslationResult {
            translated,
            qid: entry
                .get("qid")
                .and_then(Value::as_str)
                .map(ToString::to_string),
            source: string_field(entry, "source", "cache"),
            used_lang: string_field(entry, "used_lang", "unknown"),
            parent_verified: entry
                .get("parent_verified")
                .and_then(Value::as_bool)
                .unwrap_or(false),
        })
    }

    pub fn set_translation(
        &mut self,
        item: &TranslationItem,
        result: &TranslationResult,
        parent_qid: Option<&str>,
    ) -> Result<(), String> {
        self.translations.insert(
            item.id.clone(),
            json!({
                "original_name": item.original_name,
                "level": item.level.as_str(),
                "parent_chain": item.parent_chain,
                "parent_qid": parent_qid,
                "cached_at": Utc::now().to_rfc3339(),
                "translated": result.translated,
                "qid": result.qid,
                "source": result.source,
                "used_lang": result.used_lang,
                "parent_verified": result.parent_verified,
            }),
        );
        self.index_name(item);
        self.mark_dirty()
    }

    pub fn get_search_results(&self, item: &TranslationItem) -> Option<Vec<String>> {
        self.search.get(&item.id).and_then(string_array)
    }

    pub fn set_search_results(
        &mut self,
        item: &TranslationItem,
        qids: &[String],
    ) -> Result<(), String> {
        self.search.insert(item.id.clone(), json!(qids));
        self.mark_dirty()
    }

    pub fn get_labels(&self, qid: &str) -> Option<HashMap<String, String>> {
        value_to_string_map(self.labels.get(qid)?)
    }

    pub fn set_labels(
        &mut self,
        qid: &str,
        labels: &HashMap<String, String>,
    ) -> Result<(), String> {
        self.labels.insert(qid.to_string(), json!(labels));
        self.mark_dirty()
    }

    pub fn get_instance_of(&self, qid: &str) -> Option<Vec<String>> {
        self.instance_of.get(qid).and_then(string_array)
    }

    pub fn set_instance_of(&mut self, qid: &str, values: &[String]) -> Result<(), String> {
        self.instance_of.insert(qid.to_string(), json!(values));
        self.mark_dirty()
    }

    pub fn get_p131(&self, candidate_qid: &str, parent_qid: &str) -> Option<bool> {
        self.p131
            .get(&p131_key(candidate_qid, parent_qid))
            .and_then(Value::as_bool)
    }

    pub fn set_p131(
        &mut self,
        candidate_qid: &str,
        parent_qid: &str,
        value: bool,
    ) -> Result<(), String> {
        self.p131
            .insert(p131_key(candidate_qid, parent_qid), Value::Bool(value));
        self.mark_dirty()
    }

    pub fn flush_if_needed(
        &mut self,
        force: bool,
        max_dirty: usize,
        max_interval: Duration,
    ) -> Result<(), String> {
        if self.cache_path.is_none() {
            self.dirty = 0;
            self.last_flush = Instant::now();
            return Ok(());
        }
        if !force && self.dirty < max_dirty && self.last_flush.elapsed() < max_interval {
            return Ok(());
        }
        self.save()?;
        self.dirty = 0;
        self.last_flush = Instant::now();
        Ok(())
    }

    pub fn save(&self) -> Result<(), String> {
        let Some(path) = self.cache_path.as_deref() else {
            return Ok(());
        };
        let root = json!({
            "metadata": self.metadata,
            "translations": self.translations,
            "cache": {
                "search": self.search,
                "labels": self.labels,
                "p131": self.p131,
                "instance_of": self.instance_of,
            },
            "indexes": {
                "by_name": self.indexes_by_name,
            },
        });
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|error| {
                format!("無法建立 Wikidata cache 目錄 {}：{error}", parent.display())
            })?;
        }
        let tmp_path = path.with_extension(format!(
            "{}.tmp",
            path.extension()
                .and_then(|value| value.to_str())
                .unwrap_or("json")
        ));
        let content = serde_json::to_string_pretty(&root)
            .map_err(|error| format!("無法序列化 Wikidata cache：{error}"))?;
        fs::write(&tmp_path, content).map_err(|error| {
            format!(
                "無法寫入 Wikidata cache 暫存檔 {}：{error}",
                tmp_path.display()
            )
        })?;
        fs::rename(&tmp_path, path)
            .map_err(|error| format!("無法更新 Wikidata cache {}：{error}", path.display()))?;
        Ok(())
    }

    fn empty(source_lang: String, target_lang: String, cache_path: Option<PathBuf>) -> Self {
        Self {
            cache_path,
            metadata: Map::from_iter([
                ("version".to_string(), Value::String(VERSION.to_string())),
                ("source_lang".to_string(), Value::String(source_lang)),
                ("target_lang".to_string(), Value::String(target_lang)),
                (
                    "created_at".to_string(),
                    Value::String(Utc::now().to_rfc3339()),
                ),
                ("last_compacted_at".to_string(), Value::Null),
            ]),
            translations: Map::new(),
            search: Map::new(),
            labels: Map::new(),
            p131: Map::new(),
            instance_of: Map::new(),
            indexes_by_name: Map::new(),
            dirty: 0,
            last_flush: Instant::now(),
        }
    }

    fn mark_dirty(&mut self) -> Result<(), String> {
        self.dirty += 1;
        self.flush_if_needed(false, 20, Duration::from_secs(30))
    }

    fn index_name(&mut self, item: &TranslationItem) {
        let entry = self
            .indexes_by_name
            .entry(item.original_name.clone())
            .or_insert_with(|| Value::Array(Vec::new()));
        let Some(values) = entry.as_array_mut() else {
            return;
        };
        if !values.iter().any(|value| value.as_str() == Some(&item.id)) {
            values.push(Value::String(item.id.clone()));
        }
    }
}

fn object_at(root: &Value, path: &[&str]) -> Option<Map<String, Value>> {
    let mut current = root;
    for key in path {
        current = current.get(*key)?;
    }
    current.as_object().cloned()
}

fn string_field(entry: &Map<String, Value>, key: &str, default: &str) -> String {
    entry
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or(default)
        .to_string()
}

fn string_array(value: &Value) -> Option<Vec<String>> {
    Some(
        value
            .as_array()?
            .iter()
            .filter_map(Value::as_str)
            .map(ToString::to_string)
            .collect(),
    )
}

fn value_to_string_map(value: &Value) -> Option<HashMap<String, String>> {
    Some(
        value
            .as_object()?
            .iter()
            .filter_map(|(key, value)| value.as_str().map(|value| (key.clone(), value.to_string())))
            .collect(),
    )
}

fn p131_key(candidate_qid: &str, parent_qid: &str) -> String {
    format!("{candidate_qid}_{parent_qid}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wikidata::{AdminLevel, TranslationItem};

    #[test]
    fn cache_round_trips_context_aware_translation() {
        let mut store = TranslationCacheStore::load("ko", "zh-tw", None).unwrap();
        let item = TranslationItem::from_values(
            AdminLevel::Admin2,
            "중구",
            "ko",
            "zh-tw",
            vec!["KR".to_string(), "서울특별시".to_string()],
            HashMap::new(),
        )
        .unwrap();
        let result = TranslationResult {
            translated: "中區".to_string(),
            qid: Some("Q12993".to_string()),
            source: "wikidata".to_string(),
            used_lang: "zh-tw".to_string(),
            parent_verified: true,
        };

        store
            .set_translation(&item, &result, Some("Q8684"))
            .unwrap();

        assert_eq!(store.get_translation(&item), Some(result));
    }
}
