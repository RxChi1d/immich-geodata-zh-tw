use std::collections::HashMap;

use super::{
    AdminLevel, BatchTranslateOptions, METADATA_OFFICIAL_EN, METADATA_OFFICIAL_TH,
    TranslationDataset, TranslationItem, TranslationResult, WikidataApi, WikidataCandidateMetadata,
    WikidataClientOptions, WikidataTranslator,
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

#[test]
fn batch_translate_uses_official_metadata_when_search_has_no_match() {
    let item = thailand_item("Bangkok", "Bangkok", "กรุงเทพมหานคร");
    let dataset =
        TranslationDataset::new(vec![item.clone()], AdminLevel::Admin1, "en", "zh-tw", true)
            .unwrap();
    let mut translator =
        WikidataTranslator::with_client(thailand_options(), EmptySearchApi, None, false).unwrap();

    let results = translator
        .batch_translate(&dataset, BatchTranslateOptions::default())
        .unwrap();

    let result = results.get(&item.id).unwrap();
    assert_eq!(result.translated, "Bangkok");
    assert_eq!(result.source, "metadata");
    assert_eq!(result.used_lang, "official_en");
}

#[test]
fn batch_translate_ignores_wikidata_english_label_for_thailand_fallback_order() {
    let item = thailand_item("Bangkok", "Bangkok", "กรุงเทพมหานคร");
    let dataset =
        TranslationDataset::new(vec![item.clone()], AdminLevel::Admin1, "en", "zh-tw", true)
            .unwrap();
    let mut translator =
        WikidataTranslator::with_client(thailand_options(), EnglishOnlyApi, None, false).unwrap();

    let results = translator
        .batch_translate(&dataset, BatchTranslateOptions::default())
        .unwrap();

    let result = results.get(&item.id).unwrap();
    assert_eq!(result.translated, "Bangkok");
    assert_eq!(result.qid.as_deref(), Some("Q1861"));
    assert_eq!(result.source, "metadata");
    assert_eq!(result.used_lang, "official_en");
}

#[test]
fn batch_translate_rejects_candidates_when_parent_verification_fails() {
    let item = TranslationItem::from_values(
        AdminLevel::Admin2,
        "Fang",
        "en",
        "zh-tw",
        vec!["TH".to_string(), "Chiang Mai".to_string()],
        HashMap::from([
            (METADATA_OFFICIAL_EN.to_string(), "Fang".to_string()),
            (METADATA_OFFICIAL_TH.to_string(), "ฝาง".to_string()),
        ]),
    )
    .unwrap();
    let dataset =
        TranslationDataset::new(vec![item.clone()], AdminLevel::Admin2, "en", "zh-tw", true)
            .unwrap();
    let mut translator =
        WikidataTranslator::with_client(thailand_options(), UnverifiedParentApi, None, false)
            .unwrap();

    let results = translator
        .batch_translate(
            &dataset,
            BatchTranslateOptions {
                parent_qids: HashMap::from([(item.id.clone(), "Q233588".to_string())]),
                ..BatchTranslateOptions::default()
            },
        )
        .unwrap();

    let result = results.get(&item.id).unwrap();
    assert_eq!(result.translated, "Fang");
    assert_eq!(result.qid, None);
    assert_eq!(result.source, "metadata");
    assert_eq!(result.used_lang, "official_en");
    assert!(!result.parent_verified);
}

#[test]
fn batch_translate_ignores_unverified_cached_translation_when_parent_is_required() {
    let item = TranslationItem::from_values(
        AdminLevel::Admin2,
        "Fang",
        "en",
        "zh-tw",
        vec!["TH".to_string(), "Chiang Mai".to_string()],
        HashMap::from([
            (METADATA_OFFICIAL_EN.to_string(), "Fang".to_string()),
            (METADATA_OFFICIAL_TH.to_string(), "ฝาง".to_string()),
        ]),
    )
    .unwrap();
    let dataset =
        TranslationDataset::new(vec![item.clone()], AdminLevel::Admin2, "en", "zh-tw", true)
            .unwrap();
    let mut translator =
        WikidataTranslator::with_client(thailand_options(), UnverifiedParentApi, None, false)
            .unwrap();
    translator
        .cache_store
        .set_translation(
            &item,
            &TranslationResult {
                translated: "方".to_string(),
                qid: Some("Q18419656".to_string()),
                source: "wikidata".to_string(),
                used_lang: "zh-hant".to_string(),
                parent_verified: false,
            },
            Some("Q233588"),
        )
        .unwrap();

    let results = translator
        .batch_translate(
            &dataset,
            BatchTranslateOptions {
                parent_qids: HashMap::from([(item.id.clone(), "Q233588".to_string())]),
                ..BatchTranslateOptions::default()
            },
        )
        .unwrap();

    let result = results.get(&item.id).unwrap();
    assert_eq!(result.translated, "Fang");
    assert_eq!(result.qid, None);
    assert_eq!(
        translator.cache_store.get_translation(&item),
        Some(result.clone())
    );
}

fn thailand_item(original_name: &str, official_en: &str, official_th: &str) -> TranslationItem {
    TranslationItem::from_values(
        AdminLevel::Admin1,
        original_name,
        "en",
        "zh-tw",
        vec!["TH".to_string()],
        HashMap::from([
            (METADATA_OFFICIAL_EN.to_string(), official_en.to_string()),
            (METADATA_OFFICIAL_TH.to_string(), official_th.to_string()),
        ]),
    )
    .unwrap()
}

fn thailand_options() -> WikidataClientOptions {
    let mut options = WikidataClientOptions::new("en", "zh-tw");
    options.fallback_langs = vec!["zh-hant".to_string(), "zh".to_string()];
    options
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

    fn get_entities_json(&self, qids: &[String], props: &str, _: &str) -> Result<String, String> {
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

struct EmptySearchApi;

impl WikidataApi for EmptySearchApi {
    fn search_entities_json(&self, _: &str, _: usize) -> Result<String, String> {
        Ok(r#"{"search":[]}"#.to_string())
    }

    fn get_entities_json(&self, _: &[String], _: &str, _: &str) -> Result<String, String> {
        Ok(r#"{"entities":{}}"#.to_string())
    }

    fn ask_p131_json(&self, _: &str, _: &str) -> Result<String, String> {
        Ok(r#"{"boolean":false}"#.to_string())
    }

    fn zhwiki_convert_title_json(&self, _: &str) -> Result<String, String> {
        Ok(r#"{"query":{"pages":{"1":{"title":""}}}}"#.to_string())
    }
}

struct EnglishOnlyApi;

impl WikidataApi for EnglishOnlyApi {
    fn search_entities_json(&self, _: &str, _: usize) -> Result<String, String> {
        Ok(r#"{"search":[{"id":"Q1861"}]}"#.to_string())
    }

    fn get_entities_json(&self, _: &[String], props: &str, _: &str) -> Result<String, String> {
        if props == "claims" {
            return Ok(r#"{"entities":{"Q1861":{"claims":{"P31":[]}}}}"#.to_string());
        }
        Ok(r#"{"entities":{"Q1861":{"labels":{"en":{"value":"Bangkok"}}}}}"#.to_string())
    }

    fn ask_p131_json(&self, _: &str, _: &str) -> Result<String, String> {
        Ok(r#"{"boolean":false}"#.to_string())
    }

    fn zhwiki_convert_title_json(&self, _: &str) -> Result<String, String> {
        Ok(r#"{"query":{"pages":{"1":{"title":""}}}}"#.to_string())
    }
}

struct UnverifiedParentApi;

impl WikidataApi for UnverifiedParentApi {
    fn search_entities_json(&self, _: &str, _: usize) -> Result<String, String> {
        Ok(r#"{"search":[{"id":"Q18419656"}]}"#.to_string())
    }

    fn get_entities_json(&self, _: &[String], props: &str, _: &str) -> Result<String, String> {
        if props == "claims" {
            return Ok(r#"{"entities":{"Q18419656":{"claims":{"P31":[]}}}}"#.to_string());
        }
        Ok(r#"{"entities":{"Q18419656":{"labels":{"zh-hant":{"value":"方"}}}}}"#.to_string())
    }

    fn ask_p131_json(&self, _: &str, _: &str) -> Result<String, String> {
        Ok(r#"{"boolean":false}"#.to_string())
    }

    fn zhwiki_convert_title_json(&self, _: &str) -> Result<String, String> {
        Ok(r#"{"query":{"pages":{"1":{"title":"方"}}}}"#.to_string())
    }
}

struct PanicApi;

impl WikidataApi for PanicApi {
    fn search_entities_json(&self, _: &str, _: usize) -> Result<String, String> {
        panic!("全 cache hit 不應呼叫 search")
    }

    fn get_entities_json(&self, _: &[String], _: &str, _: &str) -> Result<String, String> {
        panic!("全 cache hit 不應呼叫 get_entities")
    }

    fn ask_p131_json(&self, _: &str, _: &str) -> Result<String, String> {
        panic!("全 cache hit 不應呼叫 P131")
    }

    fn zhwiki_convert_title_json(&self, _: &str) -> Result<String, String> {
        panic!("全 cache hit 不應呼叫 zhwiki")
    }
}
