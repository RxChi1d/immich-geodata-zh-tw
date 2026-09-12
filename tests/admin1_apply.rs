use std::collections::BTreeSet;

use immich_geodata::pipeline::admin1_apply::{
    AdminRow, CorrectionOutcome, CorrectionSummary, apply_corrections,
};

fn row(geoname_id: &str, admin1: &str, admin2: &str) -> AdminRow {
    AdminRow {
        geoname_id: geoname_id.to_string(),
        country_code: "MY".to_string(),
        admin1: admin1.to_string(),
        admin2: admin2.to_string(),
    }
}

fn admin2_keys(keys: &[&str]) -> BTreeSet<String> {
    keys.iter().map(|key| key.to_string()).collect()
}

fn corrections(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
    pairs
        .iter()
        .map(|(id, code)| (id.to_string(), code.to_string()))
        .collect()
}

fn apply(
    rows: &mut [AdminRow],
    pairs: &[(&str, &str)],
    keys: &[&str],
    queried: usize,
) -> Result<CorrectionSummary, String> {
    apply_corrections(rows, &corrections(pairs), &admin2_keys(keys), queried)
}

#[test]
fn admin1_code_is_replaced() {
    let mut rows = vec![row("1", "14", "")];
    let summary = apply(&mut rows, &[("1", "12")], &[], 100).expect("應成功");
    assert_eq!(rows[0].admin1, "12");
    assert_eq!(summary.applied, 1);
    assert_eq!(summary.outcomes[0], CorrectionOutcome::Applied);
}

#[test]
fn admin2_is_cleared_when_new_key_exists() {
    // Reason: 新鍵存在代表 `MY.12.A01` 是另一個縣，留著會讓這個城市無聲地
    // 指到錯的縣——比顯示不出縣名更糟。
    let mut rows = vec![row("1", "14", "A01")];
    apply(&mut rows, &[("1", "12")], &["MY.12.A01"], 100).expect("應成功");
    assert_eq!(rows[0].admin1, "12");
    assert_eq!(rows[0].admin2, "");
}

#[test]
fn admin2_is_kept_when_new_key_does_not_exist() {
    // Reason: 新鍵不存在時，admin2 本來就解析成 null，動它沒有意義。
    let mut rows = vec![row("1", "14", "A01")];
    apply(&mut rows, &[("1", "12")], &["MY.12.B99"], 100).expect("應成功");
    assert_eq!(rows[0].admin1, "12");
    assert_eq!(rows[0].admin2, "A01");
}

#[test]
fn rows_without_corrections_are_untouched() {
    let mut rows = vec![row("1", "14", "A01"), row("2", "07", "B02")];
    apply(&mut rows, &[("1", "12")], &[], 100).expect("應成功");
    assert_eq!(rows[1].admin1, "07");
    assert_eq!(rows[1].admin2, "B02");
}

#[test]
fn correction_for_missing_row_is_reported_not_silently_dropped() {
    // Reason: 候選與待寫入資料不同步是嚴重的內部不一致。靜默忽略會讓
    // 「修正數」與實際寫入數不符，報表就不能當診斷依據了。
    let mut rows = vec![row("1", "14", "")];
    let summary = apply(&mut rows, &[("999", "12")], &[], 100).expect("應成功");
    assert_eq!(summary.applied, 0);
    assert_eq!(
        summary.outcomes[0],
        CorrectionOutcome::RowNotFound {
            geoname_id: "999".to_string()
        }
    );
}

#[test]
fn corrections_within_cap_are_applied() {
    // 4 筆／100 點 = 4%，低於 5% 上限。
    let mut rows: Vec<AdminRow> = (0..10)
        .map(|index| row(&index.to_string(), "14", ""))
        .collect();
    let pairs: Vec<(&str, &str)> = vec![("0", "12"), ("1", "12"), ("2", "12"), ("3", "12")];
    assert!(apply(&mut rows, &pairs, &[], 100).is_ok());
}

#[test]
fn corrections_exceeding_cap_abort_with_full_candidate_table() {
    // 6 筆／100 點 = 6%，超過 5% 上限。
    let mut rows: Vec<AdminRow> = (0..10)
        .map(|index| row(&index.to_string(), "14", ""))
        .collect();
    let pairs: Vec<(&str, &str)> = (0..6)
        .map(|index| (["0", "1", "2", "3", "4", "5"][index], "12"))
        .collect();
    let error = apply(&mut rows, &pairs, &[], 100).expect_err("超過上限應中止");
    assert!(error.contains("5%"), "錯誤訊息應說明門檻：{error}");
    // Reason: CI 上沒有本地資料可重跑，錯誤訊息必須自帶完整候選表才能診斷。
    for id in ["0", "1", "2", "3", "4", "5"] {
        assert!(error.contains(id), "錯誤訊息應列出候選 {id}：{error}");
    }
    assert_eq!(rows[0].admin1, "14", "中止時不得寫入任何修正");
}

#[test]
fn cap_boundary_at_exactly_five_percent_aborts() {
    // Reason: 門檻寫成「超過 5% 即中止」，5% 本身就要擋——與映射雜訊門檻
    // 採同一側的嚴格解讀，避免兩處規則對邊界值有不同語意。
    let mut rows: Vec<AdminRow> = (0..10)
        .map(|index| row(&index.to_string(), "14", ""))
        .collect();
    let pairs: Vec<(&str, &str)> = vec![
        ("0", "12"),
        ("1", "12"),
        ("2", "12"),
        ("3", "12"),
        ("4", "12"),
    ];
    assert!(apply(&mut rows, &pairs, &[], 100).is_err());
}

#[test]
fn zero_queried_points_never_divides_by_zero() {
    let mut rows = vec![row("1", "14", "")];
    let result = apply(&mut rows, &[], &[], 0);
    assert!(result.is_ok(), "沒有修正時不應因分母為零而失敗");
}
