mod cache;
mod client;
mod translator;
mod types;

#[cfg(test)]
mod translator_tests;

pub use cache::TranslationCacheStore;
pub use client::{
    WDACT_URL, WDQS_URL, WikidataApi, WikidataClientOptions, WikidataHttpClient, ZHWIKI_URL,
    get_entities_url, search_entities_url, wdqs_url, zhwiki_convert_title_url,
};
pub use translator::{
    BatchTranslateOptions, WikidataCandidateMetadata, WikidataTranslator, dedupe_keep_order,
};
pub use types::{
    AdminLevel, METADATA_OFFICIAL_EN, METADATA_OFFICIAL_TH, TranslationDataset,
    TranslationDatasetBuilder, TranslationItem, TranslationResult, build_translation_item_id,
};
