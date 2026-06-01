use std::collections::HashMap;

use super::{
    AdminLevel, BatchTranslateOptions, TranslationDataset, TranslationItem, TranslationResult,
    WikidataApi, WikidataCandidateMetadata, WikidataClientOptions, WikidataTranslator,
};

#[test]
fn batch_translate_filters_candidates_verifies_parent_and_writes_cache() {
    let item = TranslationItem::from_values(
        AdminLevel::Admin2,
        "중구",
        "ko",
        "zh-tw",
        vec!["KR".to_string(), "서울특별시".to_string()],
        HashMap::new(),
    )
    .unwrap();
    let dataset =
        TranslationDataset::new(vec![item.clone()], AdminLevel::Admin2, "ko", "zh-tw", true)
            .unwrap();
    let mut translator = test_translator();
    let filter = |_: &str, metadata: &WikidataCandidateMetadata| {
        !metadata
            .labels
            .values()
            .any(|label| label.to_ascii_lowercase().contains("council"))
    };
    let results = translator
        .batch_translate(
            &dataset,
            BatchTranslateOptions {
                batch_size: 32,
                parent_qids: HashMap::from([(item.id.clone(), "Q8684".to_string())]),
                candidate_filter: Some(&filter),
            },
        )
        .unwrap();

    let result = results.get(&item.id).unwrap();
    assert_eq!(result.translated, "中區");
    assert_eq!(result.qid.as_deref(), Some("Q2"));
    assert!(result.parent_verified);
    assert_eq!(
        translator.cache_store.get_translation(&item),
        Some(result.clone())
    );
}

#[test]
fn parse_zhwiki_converted_title_prefers_converted_value() {
    let body = r#"{"query":{"converted":[{"from":"重庆市","to":"重慶市"}]}}"#;

    assert_eq!(
        super::translator::parse_zhwiki_converted_title(body).unwrap(),
        "重慶市"
    );
}

#[test]
fn batch_translate_returns_cached_results_without_network_calls() {
    let item = TranslationItem::from_values(
        AdminLevel::Admin1,
        "서울특별시",
        "ko",
        "zh-tw",
        vec!["KR".to_string()],
        HashMap::new(),
    )
    .unwrap();
    let dataset =
        TranslationDataset::new(vec![item.clone()], AdminLevel::Admin1, "ko", "zh-tw", true)
            .unwrap();
    let mut translator = WikidataTranslator::with_client(
        WikidataClientOptions::new("ko", "zh-tw"),
        PanicApi,
        None,
        false,
    )
    .unwrap();
    let cached = TranslationResult {
        translated: "首爾市".to_string(),
        qid: Some("Q8684".to_string()),
        source: "cache".to_string(),
        used_lang: "zh-tw".to_string(),
        parent_verified: false,
    };
    translator
        .cache_store
        .set_translation(&item, &cached, None)
        .unwrap();

    let results = translator
        .batch_translate(&dataset, BatchTranslateOptions::default())
        .unwrap();

    assert_eq!(results.get(&item.id), Some(&cached));
}

fn test_translator() -> WikidataTranslator<MockApi> {
    WikidataTranslator::with_client(
        WikidataClientOptions::new("ko", "zh-tw"),
        MockApi,
        None,
        false,
    )
    .unwrap()
}

struct MockApi;

impl WikidataApi for MockApi {
    fn search_entities_json(&self, _: &str, _: usize) -> Result<String, String> {
        Ok(r#"{"search":[{"id":"Q1"},{"id":"Q2"}]}"#.to_string())
    }

    fn get_entities_json(
        &self,
        qids: &[String],
        props: &str,
        _: &[String],
    ) -> Result<String, String> {
        if props == "claims" {
            return Ok(format!(
                r#"{{"entities":{{{}}}}}"#,
                qids.iter()
                    .map(|qid| format!(r#""{qid}":{{"claims":{{"P31":[]}}}}"#))
                    .collect::<Vec<_>>()
                    .join(",")
            ));
        }
        Ok(r#"{"entities":{
            "Q1":{"labels":{"en":{"value":"Jung District Council"}}},
            "Q2":{"labels":{"zh-tw":{"value":"中區"},"en":{"value":"Jung District"}}}
        }}"#
        .to_string())
    }

    fn ask_p131_json(&self, candidate_qid: &str, _: &str) -> Result<String, String> {
        Ok(format!(r#"{{"boolean":{}}}"#, candidate_qid == "Q2"))
    }

    fn zhwiki_convert_title_json(&self, _: &str) -> Result<String, String> {
        Ok(r#"{"query":{"pages":{"1":{"title":"中區"}}}}"#.to_string())
    }
}

struct PanicApi;

impl WikidataApi for PanicApi {
    fn search_entities_json(&self, _: &str, _: usize) -> Result<String, String> {
        panic!("全 cache hit 不應呼叫 search")
    }

    fn get_entities_json(&self, _: &[String], _: &str, _: &[String]) -> Result<String, String> {
        panic!("全 cache hit 不應呼叫 get_entities")
    }

    fn ask_p131_json(&self, _: &str, _: &str) -> Result<String, String> {
        panic!("全 cache hit 不應呼叫 P131")
    }

    fn zhwiki_convert_title_json(&self, _: &str) -> Result<String, String> {
        panic!("全 cache hit 不應呼叫 zhwiki")
    }
}
