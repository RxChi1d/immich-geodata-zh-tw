use std::collections::HashMap;
use std::path::Path;

use crate::wikidata::{
    BatchTranslateOptions, TranslationDataset, TranslationDatasetBuilder, TranslationItem,
    TranslationResult, WikidataCandidateMetadata, WikidataClientOptions, WikidataTranslator,
};

use super::handlers::korea_admin_components;
use super::types::{Feature, KoreaTranslations};

/// 南韓（大韓民國）的 Wikidata QID，作為 admin1 的 P131 驗證 parent。
const SOUTH_KOREA_QID: &str = "Q884";

const EXCLUDED_KEYWORDS: &[&str] = &[
    "의회",
    "議會",
    "council",
    "assembly",
    "委員會",
    "legislature",
    "廳",
    "government",
    "교육청",
    "도청",
    "군청",
    "구청",
    "시청",
];

pub(super) fn build_korea_wikidata_cache(
    features: &[Feature],
    cache_path: &Path,
) -> Result<KoreaTranslations, String> {
    let builder = TranslationDatasetBuilder::new("KR", SOUTH_KOREA_QID, "ko", "zh-tw")?;
    let components = features
        .iter()
        .map(korea_admin_components)
        .collect::<Vec<_>>();
    let admin1_dataset = builder.build_admin1_names(components.iter().map(|row| &row.sidonm))?;
    let options = WikidataClientOptions::new("ko", "zh-tw");
    let mut translator = WikidataTranslator::new(options, Some(cache_path.to_path_buf()), true)?;
    let admin1_results = translator.batch_translate(
        &admin1_dataset,
        BatchTranslateOptions {
            batch_size: 32,
            ..BatchTranslateOptions::default()
        },
    )?;
    let admin2_dataset = builder.build_admin2_pairs(
        components
            .iter()
            .filter(|row| row.sidonm != "세종특별자치시")
            .map(|row| (&row.sidonm, &row.sggnm)),
        true,
    )?;
    let parent_qids = admin2_parent_qids(&admin1_dataset, &admin1_results, &admin2_dataset);
    let candidate_filter =
        |_: &str, metadata: &WikidataCandidateMetadata| korea_candidate_allowed(metadata);
    let admin2_results = translator.batch_translate(
        &admin2_dataset,
        BatchTranslateOptions {
            batch_size: 32,
            parent_qids,
            candidate_filter: Some(&candidate_filter),
        },
    )?;
    Ok(korea_translations_from_results(
        &admin1_dataset,
        &admin1_results,
        &admin2_dataset,
        &admin2_results,
    ))
}

fn admin2_parent_qids(
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

fn korea_translations_from_results(
    admin1_dataset: &TranslationDataset,
    admin1_results: &HashMap<String, TranslationResult>,
    admin2_dataset: &TranslationDataset,
    admin2_results: &HashMap<String, TranslationResult>,
) -> KoreaTranslations {
    let mut translations = KoreaTranslations::default();
    for item in admin1_dataset.items() {
        if let Some(result) = admin1_results.get(&item.id) {
            translations
                .admin1_by_name
                .insert(item.original_name.clone(), korea_result_text(item, result));
        }
    }
    for item in admin2_dataset.items() {
        if let Some(result) = admin2_results.get(&item.id) {
            let translated = korea_result_text(item, result);
            let parent = item.parent_chain.last().cloned().unwrap_or_default();
            translations
                .admin2_by_parent
                .entry(parent)
                .or_default()
                .insert(item.original_name.clone(), translated.clone());
            translations
                .fallback_by_name
                .insert(item.original_name.clone(), translated);
        }
    }
    translations
}

fn korea_result_text(item: &TranslationItem, result: &TranslationResult) -> String {
    if result.translated.is_empty() {
        item.original_name.clone()
    } else {
        result.translated.clone()
    }
}

fn korea_candidate_allowed(metadata: &WikidataCandidateMetadata<'_>) -> bool {
    metadata.labels.values().all(|label| {
        let lower = label.to_ascii_lowercase();
        !EXCLUDED_KEYWORDS
            .iter()
            .any(|keyword| lower.contains(keyword))
    })
}
