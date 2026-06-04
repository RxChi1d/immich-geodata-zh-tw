use std::collections::{HashMap, HashSet};
use std::path::Path;

use crate::wikidata::{
    AdminLevel, BatchTranslateOptions, METADATA_OFFICIAL_EN, METADATA_OFFICIAL_TH,
    TranslationDataset, TranslationItem, TranslationResult, WikidataApi, WikidataCandidateMetadata,
    WikidataClientOptions, WikidataTranslator,
};

use super::types::{Feature, ThailandTranslations};

/// 泰國（Thailand）的 Wikidata QID，作為 admin1 的 P131 驗證 parent。
const THAILAND_QID: &str = "Q869";
/// 泰國府（province of Thailand）的 instance-of 類別。
const THAILAND_PROVINCE_CLASS: &str = "Q50198";
/// 泰國縣（amphoe）的 instance-of 類別。
const THAILAND_AMPHOE_CLASS: &str = "Q475061";
/// 曼谷轄區（khet）的 instance-of 類別；曼谷的 admin2 不是 amphoe。
const BANGKOK_KHET_CLASS: &str = "Q15634531";

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

    // Reason: admin1 一律以 P131 鏈驗證隸屬泰國，避免英文同名歧義
    //（如 Nan vs 南特）盲選第一個搜尋結果。
    let admin1_results = translator.batch_translate(
        &admin1_dataset,
        BatchTranslateOptions {
            batch_size: 32,
            parent_qids: uniform_parent_qids(&admin1_dataset, THAILAND_QID),
            ..BatchTranslateOptions::default()
        },
    )?;
    let admin1_results = retranslate_failures_with_thai_names(
        &mut translator,
        &admin1_dataset,
        admin1_results,
        |item| HashMap::from([(item.id.clone(), THAILAND_QID.to_string())]),
        &[THAILAND_PROVINCE_CLASS],
    )?;

    let parent_qids = admin2_parent_qids(&admin1_dataset, &admin1_results, &admin2_dataset);
    let admin2_results = translator.batch_translate(
        &admin2_dataset,
        BatchTranslateOptions {
            batch_size: 32,
            parent_qids: parent_qids.clone(),
            ..BatchTranslateOptions::default()
        },
    )?;
    let admin2_results = retranslate_failures_with_thai_names(
        &mut translator,
        &admin2_dataset,
        admin2_results,
        |item| {
            parent_qids
                .get(&item.id)
                .map(|qid| HashMap::from([(item.id.clone(), qid.clone())]))
                .unwrap_or_default()
        },
        &[THAILAND_AMPHOE_CLASS, BANGKOK_KHET_CLASS],
    )?;
    Ok(thailand_translations_from_results(
        &admin1_dataset,
        &admin1_results,
        &admin2_dataset,
        &admin2_results,
    ))
}

/// 為 dataset 中所有 item 指定同一個 parent QID。
fn uniform_parent_qids(dataset: &TranslationDataset, parent_qid: &str) -> HashMap<String, String> {
    dataset
        .items()
        .iter()
        .map(|item| (item.id.clone(), parent_qid.to_string()))
        .collect()
}

/// 對第一輪（英文搜尋）P131 驗證失敗的 item，以 COD-AB 官方泰文名稱
/// 進行第二輪搜尋，並以 instance-of 類別過濾候選後重新驗證。
///
/// Reason: 英文同名歧義（如 Nan、Tak）會讓正確實體擠不進搜尋結果；
/// 改用泰文官方名稱搜尋已實證可將正確實體排上第一名。instance-of
/// 過濾則避免泰文搜尋雜訊（同名建築、村落）通過 P131 鏈驗證。
fn retranslate_failures_with_thai_names<C: WikidataApi>(
    translator: &mut WikidataTranslator<C>,
    dataset: &TranslationDataset,
    mut results: HashMap<String, TranslationResult>,
    parent_qids_for: impl Fn(&TranslationItem) -> HashMap<String, String>,
    allowed_classes: &[&str],
) -> Result<HashMap<String, TranslationResult>, String> {
    let mut fallback_pairs = Vec::new();
    for item in dataset.items() {
        let failed = results
            .get(&item.id)
            .is_none_or(|result| !result.parent_verified);
        if !failed {
            continue;
        }
        let Some(thai_name) = item
            .metadata
            .get(METADATA_OFFICIAL_TH)
            .filter(|name| !name.is_empty())
        else {
            continue;
        };
        // 泰文 item 的 id 以泰文名稱構成，與英文搜尋快取自然區隔。
        let thai_item = TranslationItem::from_values(
            item.level,
            thai_name,
            "th",
            "zh-tw",
            item.parent_chain.clone(),
            item.metadata.clone(),
        )?;
        fallback_pairs.push((item.clone(), thai_item));
    }
    if fallback_pairs.is_empty() {
        return Ok(results);
    }

    let allowed: HashSet<String> = allowed_classes
        .iter()
        .map(|class| (*class).to_string())
        .collect();
    let candidate_filter = move |_: &str, metadata: &WikidataCandidateMetadata<'_>| {
        metadata
            .instance_of
            .iter()
            .any(|class| allowed.contains(class))
    };

    let mut parent_qids = HashMap::new();
    for (en_item, thai_item) in &fallback_pairs {
        for (_, qid) in parent_qids_for(en_item) {
            parent_qids.insert(thai_item.id.clone(), qid);
        }
    }
    let thai_dataset = TranslationDataset::new(
        fallback_pairs
            .iter()
            .map(|(_, thai_item)| thai_item.clone())
            .collect(),
        dataset.level,
        "th",
        "zh-tw",
        true,
    )?;
    let thai_results = translator.batch_translate(
        &thai_dataset,
        BatchTranslateOptions {
            batch_size: 32,
            parent_qids,
            candidate_filter: Some(&candidate_filter),
        },
    )?;

    let mut recovered = 0_usize;
    for (en_item, thai_item) in &fallback_pairs {
        if let Some(thai_result) = thai_results.get(&thai_item.id)
            && thai_result.parent_verified
        {
            results.insert(en_item.id.clone(), thai_result.clone());
            recovered += 1;
        }
    }
    println!(
        "stage=wikidata phase=thai_fallback level={} attempted={} recovered={}",
        dataset.level.as_str(),
        fallback_pairs.len(),
        recovered
    );
    Ok(results)
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
