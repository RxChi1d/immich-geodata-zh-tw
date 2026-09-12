//! admin1 修正器的輸出：可 diff 的修正紀錄檔與四類統計 log。
//!
//! # 檔案與 log 的分工
//!
//! `{CC}_admin1_fixes.csv` 收**列**——每一筆「任一可判定來源與原值不符」的點，
//! 含採納與拒絕。log 收**計數**——即使完全沒有分歧列，映射健康度仍需要被看見。
//!
//! Reason: 這個檔案是唯寫的診斷紀錄，程式永遠不讀回。一旦讀回去，它就變成一份
//! 人工核准清單，把 LocationIQ 這條自動路線變成半手動——handoff §3 明確排除。
//! 它也擋不住壞資料：`auto-update.yaml` 在開 PR 之前就發布了 nightly。真正的
//! 煞車是 `admin1_apply` 的 5% 上限；這裡提供的是「事後知道它改了什麼」。

use std::path::{Path, PathBuf};

use crate::pipeline::admin1_correct::{Candidate, LearnedMapping, RejectReason, Verdict};
use crate::pipeline::table::write_delimited;

const HEADER: &[&str] = &[
    "geoname_id",
    "name",
    "latitude",
    "longitude",
    "original_admin1",
    "natural_earth_admin1",
    "locationiq_admin1_name",
    "locationiq_admin1",
    "boundary_km",
    "verdict",
    "reasons",
];

/// 某國修正紀錄檔的路徑：`{metadata_dir}/{CC}_admin1_fixes.csv`。
///
/// Reason: 檔名 stem 不是兩個字母，`translate` 的 metadata 載入器會把它歸入
/// skip 清單而非當成 LocationIQ 查詢結果解析。同時 `data/locationiq/*` 這個
/// auto-commit glob 匹配得到，兩個 workflow 都不需要改。
pub fn fixes_csv_path(metadata_dir: &Path, country_code: &str) -> PathBuf {
    metadata_dir.join(format!("{}_admin1_fixes.csv", country_code.to_uppercase()))
}

/// 寫出修正紀錄檔。`candidates` 須已依 `geoname_id` 排序。
pub fn write_fixes_csv(path: &Path, candidates: &[Candidate]) -> Result<(), String> {
    let rows: Vec<Vec<String>> = candidates.iter().map(candidate_row).collect();
    write_delimited(path, ',', Some(HEADER), &rows)
}

fn candidate_row(candidate: &Candidate) -> Vec<String> {
    vec![
        candidate.geoname_id.clone(),
        candidate.name.clone(),
        // Reason: 固定小數位讓同一個座標在每次執行都印成同一個字串。依賴
        // f64 的預設 Display 會讓平台差異滲進檔案，破壞跨週 diff。
        format!("{:.5}", candidate.latitude),
        format!("{:.5}", candidate.longitude),
        format!("{}.{}", candidate.country_code, candidate.original_admin1),
        candidate.natural_earth_code.clone().unwrap_or_default(),
        candidate.locationiq_admin1.clone().unwrap_or_default(),
        candidate.locationiq_code.clone().unwrap_or_default(),
        candidate
            .boundary_km
            .map(|km| format!("{km:.3}"))
            .unwrap_or_default(),
        match candidate.verdict {
            Verdict::Accepted => "accepted".to_string(),
            Verdict::Rejected => "rejected".to_string(),
        },
        candidate
            .reasons
            .iter()
            .map(reason_slug)
            .collect::<Vec<_>>()
            .join(";"),
    ]
}

fn reason_slug(reason: &RejectReason) -> String {
    match reason {
        RejectReason::OriginalCodeUnknown { code } => format!("original_code_unknown({code})"),
        RejectReason::NoLocationiqAdmin1 => "no_locationiq_admin1".to_string(),
        RejectReason::NoTrustedMapping => "no_trusted_mapping".to_string(),
        RejectReason::NaturalEarthNoHit => "natural_earth_no_hit".to_string(),
        RejectReason::NaturalEarthMultipleHit { gn_ids } => {
            let ids = gn_ids
                .iter()
                .map(i64::to_string)
                .collect::<Vec<_>>()
                .join("|");
            format!("natural_earth_multiple_hit({ids})")
        }
        RejectReason::SourcesDoNotAgree {
            natural_earth,
            locationiq,
        } => format!("sources_do_not_agree({natural_earth}->{locationiq})"),
        RejectReason::NaturalEarthForeignCountry { code } => {
            format!("natural_earth_foreign_country({code})")
        }
    }
}

/// 產生四類統計的 log 行。
///
/// 四類缺一不可（handoff §3）：無可信映射、NE 無法判定、來源不一致但未修正、
/// 原代碼不在 `admin1CodesASCII`。
///
/// Reason: 第 3 類是無聲失敗偵測器——某州整片學錯時，它不會產生任何修正，只會
/// 讓這個計數暴增。若只報「修正了幾筆」，整州失效看起來會跟「本週無事」一樣。
pub fn report_lines(
    country_code: &str,
    candidates: &[Candidate],
    mapping: &LearnedMapping,
    queried_points: usize,
) -> Vec<String> {
    let mut accepted = 0usize;
    let mut no_trusted_mapping = 0usize;
    let mut no_locationiq = 0usize;
    let mut natural_earth_unusable = 0usize;
    let mut sources_do_not_agree = 0usize;
    let mut original_code_unknown = 0usize;

    for candidate in candidates {
        if candidate.verdict == Verdict::Accepted {
            accepted += 1;
            continue;
        }
        // Reason: 只計主因。一筆候選可能同時觸發多個原因，全部都計會讓各類
        // 加總超過候選數，看起來像資料錯亂。完整原因留在 CSV 裡。
        match candidate.reasons.first() {
            Some(RejectReason::OriginalCodeUnknown { .. }) => original_code_unknown += 1,
            Some(RejectReason::NoLocationiqAdmin1) => no_locationiq += 1,
            Some(RejectReason::NoTrustedMapping) => no_trusted_mapping += 1,
            Some(RejectReason::NaturalEarthNoHit)
            | Some(RejectReason::NaturalEarthMultipleHit { .. })
            | Some(RejectReason::NaturalEarthForeignCountry { .. }) => natural_earth_unusable += 1,
            Some(RejectReason::SourcesDoNotAgree { .. }) => sources_do_not_agree += 1,
            None => {}
        }
    }

    let untrusted_names = mapping
        .stats()
        .filter(|stats| stats.rejection.is_some())
        .count();

    let mut lines = vec![format!(
        "stage=translate admin1_correct country={country_code} queried_points={queried_points} \
         candidates={} accepted={accepted} no_trusted_mapping={no_trusted_mapping} \
         no_locationiq_admin1={no_locationiq} natural_earth_unusable={natural_earth_unusable} \
         sources_do_not_agree={sources_do_not_agree} original_code_unknown={original_code_unknown} \
         trusted_mappings={} untrusted_mappings={untrusted_names}",
        candidates.len(),
        mapping.trusted_len(),
    )];

    // Reason: 逐名稱列出未建立映射的原因。整州失效時，光看總數無法分辨是
    // 「樣本不足」（該州本來就小）還是「雜訊過高」（映射可能學歪了）。
    for stats in mapping.stats() {
        if let Some(rejection) = &stats.rejection {
            lines.push(format!(
                "stage=translate admin1_correct_mapping country={country_code} name={} \
                 resolved={} unresolved={} rejection={rejection:?}",
                stats.name, stats.resolved, stats.unresolved
            ));
        }
    }
    lines
}
