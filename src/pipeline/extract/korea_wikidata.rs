use std::path::Path;

use crate::wikidata::{
    BatchTranslateOptions, TranslationDatasetBuilder, WikidataCandidateMetadata,
    WikidataClientOptions, WikidataTranslator,
};

use super::handlers::korea_admin_components;
use super::types::{Feature, WikidataTranslations};
use super::wikidata_common::{admin2_parent_qids, translations_from_results};

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
) -> Result<WikidataTranslations, String> {
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
    Ok(translations_from_results(
        &admin1_dataset,
        &admin1_results,
        &admin2_dataset,
        &admin2_results,
    ))
}

fn korea_candidate_allowed(metadata: &WikidataCandidateMetadata<'_>) -> bool {
    metadata.labels.values().all(|label| {
        let lower = label.to_ascii_lowercase();
        !EXCLUDED_KEYWORDS
            .iter()
            .any(|keyword| lower.contains(keyword))
    })
}
