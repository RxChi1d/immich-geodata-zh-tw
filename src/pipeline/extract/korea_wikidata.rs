use std::collections::{BTreeSet, HashMap};
use std::path::Path;

use regex::Regex;

use crate::wikidata::{
    BatchTranslateOptions, TranslationDataset, TranslationDatasetBuilder, TranslationResult,
    WikidataApi, WikidataCandidateMetadata, WikidataClientOptions, WikidataTranslator,
};

use super::handlers::korea_admin_components;
use super::types::{Feature, WikidataTranslations};
use super::wikidata_common::translations_from_results;

/// 南韓（大韓民國）的 Wikidata QID，作為 admin1 的 P131 驗證 parent。
const SOUTH_KOREA_QID: &str = "Q884";

/// 快取中存放韓國漢字表記的 key（與 label 同表，沿用既有快取失效機制）。
const HANJA_LABEL_KEY: &str = "kohanja";

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
    let candidate_filter = korea_candidate_allowed;
    // Reason: admin1 也必須套用候選過濾。2026-07-01 改制後搜尋
    //         「전남광주통합특별시」的候選中含同名的「…교육청」（教育廳），
    //         而該機關的 P131 鏈同樣可回溯到大韓民國、能通過驗證，於是 admin1
    //         取到機關實體，再讓 27 個下級行政區的 parent QID 全部指錯。
    let admin1_results = translator.batch_translate(
        &admin1_dataset,
        BatchTranslateOptions {
            batch_size: 32,
            candidate_filter: Some(&candidate_filter),
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
    let parent_qids = korea_admin2_parent_qids(&admin1_dataset, &admin1_results, &admin2_dataset)?;
    let mut admin2_results = translator.batch_translate(
        &admin2_dataset,
        BatchTranslateOptions {
            batch_size: 32,
            parent_qids: parent_qids.clone(),
            candidate_filter: Some(&candidate_filter),
        },
    )?;
    apply_korean_hanja(
        &mut translator,
        &admin2_dataset,
        &mut admin2_results,
        &parent_qids,
    )?;
    // Reason: batch_translate 內部的 flush 發生在漢字覆寫之前，此處補一次寫入，
    //         否則本次查到的漢字不會落盤，下次重跑仍會重新連線韓文維基。
    translator.cache_store.save()?;
    Ok(translations_from_results(
        &admin1_dataset,
        &admin1_results,
        &admin2_dataset,
        &admin2_results,
    ))
}

/// 候選必須自報的韓文名稱與查詢名稱完全相同。
///
/// Reason: 取代原本的機關關鍵字黑名單。黑名單有兩個致命問題——(1) 它檢查
/// 候選的「所有語言 label」，任一語言含關鍵字就整個剔除，實測把正確的
/// 관악구（zh-hant label 被機器人誤填成「冠嶽區廳」）與 송파구（「鬆坡區廳」）
/// 一併殺掉，於是改選到區內的洞（신림동）與地鐵站（잠실역）；(2) 需要人工
/// 維護且永遠列不完（교육감、선거관리위원회、시내버스 都不在名單上）。
/// 改用韓文 label 全等比對後，機關、職位、選區、車站全部自然落選，且對現行
/// 245 筆實測零誤判。
fn korea_candidate_allowed(original_name: &str, metadata: &WikidataCandidateMetadata<'_>) -> bool {
    metadata
        .labels
        .get("ko")
        .is_some_and(|label| label == original_name)
}

/// 建立 admin2 的 P131 驗證 parent QID 表；任何 admin1 解析不出 QID 即失敗。
///
/// Reason: 共用層的 `resolve_parent_qid` 找不到明確 parent 時會退回國家
/// QID，對 admin2 而言等於把「這個 동구 是不是統合市底下的」放寬成「這個
/// 동구 在不在韓國」——釜山東區、大邱東區全都會通過。2026-07-01 改制時就是
/// 這條靜默降級把單一 admin1 查錯放大成 393 + 27 筆錯誤輸出。此處改為缺一
/// 即報錯，讓下次改制當場停住而不是安靜出貨。
fn korea_admin2_parent_qids(
    admin1_dataset: &TranslationDataset,
    admin1_results: &HashMap<String, TranslationResult>,
    admin2_dataset: &TranslationDataset,
) -> Result<HashMap<String, String>, String> {
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
    let mut missing = BTreeSet::new();
    for item in admin2_dataset.items() {
        let Some(parent_name) = item.parent_chain.get(1).map(String::as_str) else {
            missing.insert(item.original_name.clone());
            continue;
        };
        match admin1_qids.get(parent_name) {
            Some(parent_qid) => {
                parent_qids.insert(item.id.clone(), parent_qid.clone());
            }
            None => {
                missing.insert(parent_name.to_string());
            }
        }
    }
    if !missing.is_empty() {
        return Err(format!(
            "KR admin1 未能解析出 Wikidata QID，無法驗證下級行政區歸屬：{}。\
             請確認該行政區在 Wikidata 上的韓文 label 與圖資一致，\
             且具備可回溯至 {SOUTH_KOREA_QID} 的 P131 敘述。",
            missing.into_iter().collect::<Vec<_>>().join("、")
        ));
    }
    Ok(parent_qids)
}

/// 以韓文維基條目的漢字表記覆寫 admin2 譯名。
///
/// Reason: 韓國行政區名本來就是漢字詞，漢字表記是「原文」而非「翻譯」，
/// 與日本 handler 直接沿用圖資中的日文漢字同一原則。Wikidata 的中文 label
/// 則是二手產物，實測有三類系統性錯誤：機器人從中文維基 infobox 誤抓機關名
/// （冠嶽區廳）、簡繁轉換選錯字（咸平郡→鹹平郡、冠岳區→冠嶽區）、行政層級
/// 變更後未更新（여주시→驪州郡、검단구→黔丹面）。改以韓文維基漢字為準可
/// 一次消除這三類，且不需維護任何人工對照表。
///
/// 取不到漢字時保留原本的 Wikidata 譯名，不使結果變差。
fn apply_korean_hanja<C: WikidataApi>(
    translator: &mut WikidataTranslator<C>,
    dataset: &TranslationDataset,
    results: &mut HashMap<String, TranslationResult>,
    parent_qids: &HashMap<String, String>,
) -> Result<(), String> {
    // 先蒐集需要查詢的條目標題；已快取漢字者不再連線。
    let mut cached = HashMap::<String, String>::new();
    let mut title_by_qid = HashMap::<String, String>::new();
    let mut titles = Vec::new();
    for item in dataset.items() {
        let Some(qid) = results.get(&item.id).and_then(|result| result.qid.clone()) else {
            continue;
        };
        let Some(labels) = translator.cache_store.get_labels(&qid) else {
            continue;
        };
        if let Some(hanja) = labels.get(HANJA_LABEL_KEY) {
            cached.insert(qid, hanja.clone());
            continue;
        }
        if let Some(title) = labels.get("kowiki") {
            title_by_qid.insert(qid, title.clone());
            titles.push(title.clone());
        }
    }
    let extracts = translator.fetch_kowiki_extracts(&titles)?;
    let pattern = Regex::new(r"\(\s*(?:한자\s*[:：]\s*)?(\p{Han}{2,12})")
        .map_err(|error| format!("韓文維基漢字比對式建立失敗：{error}"))?;

    let mut resolved = HashMap::<String, String>::new();
    let mut applied = 0_usize;
    for item in dataset.items() {
        let Some(qid) = results.get(&item.id).and_then(|result| result.qid.clone()) else {
            continue;
        };
        let hanja = match cached.get(&qid).or_else(|| resolved.get(&qid)) {
            Some(hanja) => hanja.clone(),
            None => {
                let Some(extract) = title_by_qid.get(&qid).and_then(|title| extracts.get(title))
                else {
                    continue;
                };
                let Some(hanja) = hanja_from_extract(&pattern, extract, &item.original_name) else {
                    continue;
                };
                // 寫回 labels 快取，重跑時不必再連線韓文維基。
                let mut labels = translator.cache_store.get_labels(&qid).unwrap_or_default();
                labels.insert(HANJA_LABEL_KEY.to_string(), hanja.clone());
                translator.cache_store.set_labels(&qid, &labels)?;
                resolved.insert(qid.clone(), hanja.clone());
                hanja
            }
        };
        if let Some(result) = results.get_mut(&item.id) {
            result.translated = hanja;
            result.source = "kowiki-hanja".to_string();
            result.used_lang = "hanja".to_string();
            // Reason: 覆寫後一併寫回快取，否則快取記錄的仍是覆寫前的 Wikidata
            //         譯名，與實際輸出不一致（如快取寫「清州市」、輸出為
            //         「淸州市」），會誤導日後靠快取稽核譯名的人。
            translator.cache_store.set_translation(
                item,
                result,
                parent_qids.get(&item.id).map(String::as_str),
            )?;
            applied += 1;
        }
    }
    println!(
        "stage=wikidata phase=hanja level=admin_2 total={} applied={applied}",
        dataset.len()
    );
    Ok(())
}

/// 從條目開頭文字抽出漢字，並補齊行政層級後綴。
///
/// Reason: 條目首句格式為「<한글>(<漢字>)…」，但有三種變體必須容忍——
/// 開頭可能先出現 infobox 殘留文字（대구 중구）、括號內可能帶「한자:」前綴
/// （연제구），以及條目以不含層級的名稱起首（목포시的首句是
/// 「목포(木浦, Mokpo)」）。最後一種會少掉層級後綴，依韓文名尾字補回。
fn hanja_from_extract(pattern: &Regex, extract: &str, korean_name: &str) -> Option<String> {
    // 優先比對緊接在查詢名稱之後的括號，避免抓到句中其他漢字註記。
    let hanja = extract
        .find(korean_name)
        .and_then(|start| {
            let rest = &extract[start + korean_name.len()..];
            pattern
                .captures(rest)
                .filter(|caps| caps.get(0).is_some_and(|matched| matched.start() == 0))
        })
        .or_else(|| pattern.captures(extract))
        .map(|caps| caps[1].to_string())?;
    Some(ensure_level_suffix(&hanja, korean_name))
}

/// 韓文行政層級尾字與漢字後綴的對應；漢字缺後綴時補上。
fn ensure_level_suffix(hanja: &str, korean_name: &str) -> String {
    let suffix = match korean_name.chars().last() {
        Some('시') => "市",
        Some('군') => "郡",
        Some('구') => "區",
        _ => return hanja.to_string(),
    };
    if hanja.ends_with(suffix) {
        hanja.to_string()
    } else {
        format!("{hanja}{suffix}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn pattern() -> Regex {
        Regex::new(r"\(\s*(?:한자\s*[:：]\s*)?(\p{Han}{2,12})").unwrap()
    }

    fn metadata<'a>(labels: &'a HashMap<String, String>) -> WikidataCandidateMetadata<'a> {
        WikidataCandidateMetadata {
            qid: "Q1",
            labels,
            instance_of: &[],
        }
    }

    #[test]
    fn candidate_requires_exact_korean_label() {
        // 正解：韓文 label 與查詢名稱全等。
        let mut labels = HashMap::new();
        labels.insert("ko".to_string(), "전남광주통합특별시".to_string());
        assert!(korea_candidate_allowed(
            "전남광주통합특별시",
            &metadata(&labels)
        ));

        // 真實誤選案例：教育廳、地鐵站、區內的洞，韓文 label 都不等於查詢名稱。
        for wrong in ["전남광주통합특별시교육청", "잠실역", "신림동"] {
            let mut labels = HashMap::new();
            labels.insert("ko".to_string(), wrong.to_string());
            assert!(!korea_candidate_allowed("송파구", &metadata(&labels)));
        }

        // 舊黑名單會因為 zh-hant label 含「廳」而誤殺正確候選；新規則不受影響。
        let mut labels = HashMap::new();
        labels.insert("ko".to_string(), "관악구".to_string());
        labels.insert("zh-hant".to_string(), "冠嶽區廳".to_string());
        assert!(korea_candidate_allowed("관악구", &metadata(&labels)));

        // 完全沒有韓文 label 的候選一律不採用。
        let labels = HashMap::new();
        assert!(!korea_candidate_allowed("관악구", &metadata(&labels)));
    }

    #[test]
    fn hanja_extraction_handles_real_article_shapes() {
        let pattern = pattern();
        // 一般形態。
        assert_eq!(
            hanja_from_extract(&pattern, "함평군(咸平郡)은 대한민국 …", "함평군").as_deref(),
            Some("咸平郡")
        );
        // 括號內帶「한자:」前綴（연제구）。
        assert_eq!(
            hanja_from_extract(
                &pattern,
                "연제구(한자:蓮堤區, 경상어:옌제구)는 부산광역시 …",
                "연제구"
            )
            .as_deref(),
            Some("蓮堤區")
        );
        // 開頭有 infobox 殘留文字（대구 중구）。
        assert_eq!(
            hanja_from_extract(
                &pattern,
                "인구밀도\n10,927.90명\n중구(中區)는 대구 …",
                "중구"
            )
            .as_deref(),
            Some("中區")
        );
        // 條目以不含層級的名稱起首，需補回層級後綴（목포시）。
        assert_eq!(
            hanja_from_extract(&pattern, "목포(木浦, Mokpo)는 대한민국 …", "목포시").as_deref(),
            Some("木浦市")
        );
        // 抽不到漢字時回傳 None，由呼叫端保留原譯名。
        assert_eq!(
            hanja_from_extract(&pattern, "서울은 대한민국의 수도이다.", "서울시"),
            None
        );
    }

    #[test]
    fn level_suffix_is_only_appended_when_missing() {
        assert_eq!(ensure_level_suffix("木浦", "목포시"), "木浦市");
        assert_eq!(ensure_level_suffix("木浦市", "목포시"), "木浦市");
        assert_eq!(ensure_level_suffix("咸平郡", "함평군"), "咸平郡");
        assert_eq!(ensure_level_suffix("冠岳區", "관악구"), "冠岳區");
        // 尾字不是行政層級時不動（世宗的洞／邑／面走另一條路徑）。
        assert_eq!(ensure_level_suffix("寶藍洞", "보람동"), "寶藍洞");
    }

    #[test]
    fn missing_admin1_qid_fails_instead_of_falling_back_to_country() {
        let builder = TranslationDatasetBuilder::new("KR", SOUTH_KOREA_QID, "ko", "zh-tw").unwrap();
        let admin1 = builder
            .build_admin1_names(["전남광주통합특별시".to_string()].iter())
            .unwrap();
        let admin2 = builder
            .build_admin2_pairs(
                [("전남광주통합특별시".to_string(), "목포시".to_string())]
                    .iter()
                    .map(|(parent, name)| (parent, name)),
                true,
            )
            .unwrap();

        // admin1 查不到 QID：必須報錯，不可讓 admin2 退回以國家 QID 驗證。
        let empty = HashMap::new();
        let error = korea_admin2_parent_qids(&admin1, &empty, &admin2).unwrap_err();
        assert!(error.contains("전남광주통합특별시"));

        // admin1 有 QID：每個 admin2 都取得明確 parent。
        let mut results = HashMap::new();
        results.insert(
            admin1.items()[0].id.clone(),
            TranslationResult {
                translated: "全南光州市".to_string(),
                qid: Some("Q138870299".to_string()),
                source: "wikidata".to_string(),
                used_lang: "zh-tw".to_string(),
                parent_verified: true,
            },
        );
        let parent_qids = korea_admin2_parent_qids(&admin1, &results, &admin2).unwrap();
        assert_eq!(parent_qids.len(), admin2.len());
        assert!(parent_qids.values().all(|qid| qid == "Q138870299"));
    }
}
