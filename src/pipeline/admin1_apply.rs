//! 把 admin1 修正寫回 cities500 資料列，並在修正量異常時 fail-closed 中止。
//!
//! # 為什麼需要上限守衛
//!
//! `auto-update.yaml` 的步驟順序是「建置 → 發布 nightly → 提交檔案 → 開 PR」。
//! nightly 在任何人看到 PR 之前就已經出貨，所以人工 review 不是關卡。
//!
//! Reason: 唯一能在無人介入下擋住災難的是程式自己。最可怕的失敗不是改錯一兩筆，
//! 是整州映射學歪後一口氣搬走幾百個城市。MY 實測修正量為 8/740 = 1.08%，
//! 5% 是有數據根據又寬鬆的門檻。同類 fail-closed 守衛見 `cities500_load.rs`。

use std::collections::{BTreeSet, HashMap};

/// 修正量占該國已查點數的上限（1/20 = 5%），達到即中止。
const CAP_DENOMINATOR: usize = 20;

/// 待寫入修正的 cities500 行政區欄位。
#[derive(Debug, Clone, PartialEq)]
pub struct AdminRow {
    pub geoname_id: String,
    pub country_code: String,
    pub admin1: String,
    pub admin2: String,
}

/// 單筆修正的寫入結果。
#[derive(Debug, Clone, PartialEq)]
pub enum CorrectionOutcome {
    Applied,
    /// 候選指向的 geoname_id 不在待寫入資料裡。
    RowNotFound {
        geoname_id: String,
    },
}

/// 寫入結果彙總。
#[derive(Debug, Clone, Default)]
pub struct CorrectionSummary {
    pub applied: usize,
    pub admin2_cleared: usize,
    pub outcomes: Vec<CorrectionOutcome>,
}

/// 把 `(geoname_id, 新 admin1 代碼)` 逐筆寫回 `rows`。
///
/// `admin2_keys` 為 `admin2Codes.txt` 的全部鍵（`{國碼}.{admin1}.{admin2}`）。
/// `queried_points` 為該國已由 LocationIQ 查詢的座標數，用於計算上限。
///
/// 修正量達到 `queried_points` 的 5% 時回傳 `Err`，且不寫入任何一筆。
pub fn apply_corrections(
    rows: &mut [AdminRow],
    corrections: &[(String, String)],
    admin2_keys: &BTreeSet<String>,
    queried_points: usize,
) -> Result<CorrectionSummary, String> {
    // Reason: 先檢查再寫入。若邊寫邊檢查，中止時資料已經半殘，而呼叫端拿到
    // Err 不會知道有多少筆已經落地。
    if !corrections.is_empty() && corrections.len() * CAP_DENOMINATOR >= queried_points {
        return Err(cap_error_message(corrections, queried_points));
    }

    // Reason: 逐筆線性搜尋是 O(修正數 × 列數)；先建索引讓它退化成雜湊查詢。
    // cities500 全球二十多萬列，線性搜尋在多國情境會明顯拖慢 translate。
    let index: HashMap<String, usize> = rows
        .iter()
        .enumerate()
        .map(|(position, row)| (row.geoname_id.clone(), position))
        .collect();

    let mut summary = CorrectionSummary::default();
    for (geoname_id, new_admin1) in corrections {
        let Some(position) = index.get(geoname_id).copied() else {
            summary.outcomes.push(CorrectionOutcome::RowNotFound {
                geoname_id: geoname_id.clone(),
            });
            continue;
        };
        let row = &mut rows[position];
        // Reason: 新的 admin2 鍵存在，代表 `{國碼}.{新admin1}.{原admin2}` 是
        // 另一個縣。留著會讓這個城市無聲地指到錯的縣，比顯示不出縣名更糟。
        // 鍵不存在時原本就解析成 null，動它沒有意義。
        if !row.admin2.is_empty() {
            let new_key = format!("{}.{}.{}", row.country_code, new_admin1, row.admin2);
            if admin2_keys.contains(&new_key) {
                row.admin2.clear();
                summary.admin2_cleared += 1;
            }
        }
        row.admin1 = new_admin1.clone();
        summary.applied += 1;
        summary.outcomes.push(CorrectionOutcome::Applied);
    }
    Ok(summary)
}

/// 組出自帶完整候選表的中止訊息。
///
/// Reason: CI 上沒有本地資料可以重跑，錯誤訊息若只講數量，維護者無從判斷是
/// 整州學歪還是門檻訂太緊。把候選全部列進訊息，從 CI log 就能診斷。
fn cap_error_message(corrections: &[(String, String)], queried_points: usize) -> String {
    let listing = corrections
        .iter()
        .map(|(geoname_id, code)| format!("  {geoname_id} -> {code}"))
        .collect::<Vec<_>>()
        .join("\n");
    format!(
        "admin1 修正量達到已查點數的 5% 上限，已中止且未寫入任何修正：\
         corrections={} queried_points={queried_points}\n候選清單：\n{listing}",
        corrections.len()
    )
}
