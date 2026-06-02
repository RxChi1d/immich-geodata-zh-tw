use std::collections::{HashMap, HashSet};
use std::path::Path;

use crate::wikidata::{
    AdminLevel, BatchTranslateOptions, METADATA_OFFICIAL_EN, METADATA_OFFICIAL_TH,
    TranslationDataset, TranslationItem, TranslationResult, WikidataClientOptions,
    WikidataTranslator,
};

use super::types::{Feature, ThailandTranslations};

pub(super) fn build_thailand_wikidata_cache(
    features: &[Feature],
    cache_path: &Path,
) -> Result<ThailandTranslations, String> {
    let admin1_dataset = thailand_admin1_dataset(features)?;
    let admin2_dataset = thailand_admin2_dataset(features)?;
    let mut options = WikidataClientOptions::new("en", "zh-tw");
    // Reason: TH COD-AB 已提供官方英文與泰文，Wikidata fallback 不應以
    //         Wikidata 英文/泰文覆蓋官方來源；中文與 zhwiki 之後才回官方名稱。
    options.fallback_langs = vec!["zh-hant".to_string(), "zh".to_string()];
    let mut translator = WikidataTranslator::new(options, Some(cache_path.to_path_buf()), true)?;

    let admin1_results = translator.batch_translate(
        &admin1_dataset,
        BatchTranslateOptions {
            batch_size: 32,
            ..BatchTranslateOptions::default()
        },
    )?;
    let parent_qids = admin2_parent_qids(&admin1_dataset, &admin1_results, &admin2_dataset);
    let admin2_results = translator.batch_translate(
        &admin2_dataset,
        BatchTranslateOptions {
            batch_size: 32,
            parent_qids,
            ..BatchTranslateOptions::default()
        },
    )?;
    Ok(thailand_translations_from_results(
        &admin1_dataset,
        &admin1_results,
        &admin2_dataset,
        &admin2_results,
    ))
}

fn thailand_admin1_dataset(features: &[Feature]) -> Result<TranslationDataset, String> {
    let mut seen = HashSet::new();
    let mut items = Vec::new();
    for feature in features {
        let name = attribute(feature, "adm1_name");
        if name.is_empty() || !seen.insert(name.to_string()) {
            continue;
        }
        items.push(thailand_item(
            AdminLevel::Admin1,
            name,
            vec!["TH".to_string()],
            attribute(feature, "adm1_name"),
            attribute(feature, "adm1_name1"),
        )?);
    }
    TranslationDataset::new(items, AdminLevel::Admin1, "en", "zh-tw", true)
}

fn thailand_admin2_dataset(features: &[Feature]) -> Result<TranslationDataset, String> {
    let mut seen = HashSet::new();
    let mut items = Vec::new();
    for feature in features {
        let parent = attribute(feature, "adm1_name");
        let name = attribute(feature, "adm2_name");
        if parent.is_empty() || name.is_empty() {
            continue;
        }
        let dedupe_key = (parent.to_string(), name.to_string());
        if !seen.insert(dedupe_key) {
            continue;
        }
        items.push(thailand_item(
            AdminLevel::Admin2,
            name,
            vec!["TH".to_string(), parent.to_string()],
            attribute(feature, "adm2_name"),
            attribute(feature, "adm2_name1"),
        )?);
    }
    TranslationDataset::new(items, AdminLevel::Admin2, "en", "zh-tw", true)
}

fn thailand_item(
    level: AdminLevel,
    original_name: &str,
    parent_chain: Vec<String>,
    official_en: &str,
    official_th: &str,
) -> Result<TranslationItem, String> {
    TranslationItem::from_values(
        level,
        original_name,
        "en",
        "zh-tw",
        parent_chain,
        HashMap::from([
            (METADATA_OFFICIAL_EN.to_string(), official_en.to_string()),
            (METADATA_OFFICIAL_TH.to_string(), official_th.to_string()),
        ]),
    )
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

fn thailand_translations_from_results(
    admin1_dataset: &TranslationDataset,
    admin1_results: &HashMap<String, TranslationResult>,
    admin2_dataset: &TranslationDataset,
    admin2_results: &HashMap<String, TranslationResult>,
) -> ThailandTranslations {
    let mut translations = ThailandTranslations::default();
    for item in admin1_dataset.items() {
        if let Some(result) = admin1_results.get(&item.id) {
            translations.admin1_by_name.insert(
                item.original_name.clone(),
                thailand_result_text(item, result),
            );
        }
    }
    for item in admin2_dataset.items() {
        if let Some(result) = admin2_results.get(&item.id) {
            let translated = thailand_result_text(item, result);
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

fn thailand_result_text(item: &TranslationItem, result: &TranslationResult) -> String {
    if result.translated.is_empty() {
        item.original_name.clone()
    } else {
        result.translated.clone()
    }
}

fn attribute<'a>(feature: &'a Feature, key: &str) -> &'a str {
    feature.attributes.get(key).unwrap_or("")
}
