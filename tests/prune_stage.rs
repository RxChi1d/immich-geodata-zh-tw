//! 剪枝階段的整合測試：用小型 fixture 驗證刪除、寫出與交叉檢查。

use std::fs;
use std::path::{Path, PathBuf};

use immich_geodata::pipeline::prune::{multipass, stage};

/// 產生一份最小 cities500：`side`×`side` 的同 label 密集網格，間距 `step_deg`。
///
/// Reason: 剪枝要成立必須通過 T1 覆蓋——`Rp`（p 周圍 43.5 km）內每個位置都要有
/// 保留點在 25 km 內。稀疏的點群（例如相隔數百公里的幾個小群）一個點都刪不掉，
/// 那是正確行為而不是 bug。所以 fixture 必須是**夠大且夠密**的網格：
/// 間距遠小於 25 km，且邊長要讓中心點四周有 43.5 + 25 km 的餘裕。
fn write_grid(label: &str, side: usize, step_deg: f64, lat0: f64, lon0: f64, gid0: i64) -> String {
    let mut out = String::new();
    let mut gid = gid0;
    for i in 0..side {
        for j in 0..side {
            let lat = lat0 + i as f64 * step_deg;
            let lon = lon0 + j as f64 * step_deg;
            // GeoNames 欄位：0=id 1=name 3=alt 4=lat 5=lon 7=fcode 8=cc 10=admin1 18=moddate
            out.push_str(&format!(
                "{gid}\t{label}\t{label}\t\t{lat:.6}\t{lon:.6}\tP\tPPL\tJP\t\t01\t\t\t\t0\t\t0\tAsia/Tokyo\t2026-01-01\n"
            ));
            gid += 1;
        }
    }
    out
}

/// 兩塊互相遠離的同 label 網格，各自內部可刪、彼此不互相影響。
fn write_fixture(dir: &Path, side: usize) -> (PathBuf, PathBuf, usize) {
    let cities = dir.join("cities500.txt");
    let admin1 = dir.join("admin1.txt");
    fs::write(&admin1, "JP.01\tTokyo\tTokyo\t1\n").expect("寫 admin1 失敗");

    // 0.05° ≈ 5.5 km，遠小於 25 km。
    let mut out = write_grid("Alpha", side, 0.05, 34.0, 135.0, 1_000_000);
    out.push_str(&write_grid("Beta", side, 0.05, 34.0, 150.0, 2_000_000));
    fs::write(&cities, &out).expect("寫 cities 失敗");
    (cities, admin1, side * side * 2)
}

fn cfg() -> multipass::Config {
    multipass::Config {
        // fixture 是 JP，沿用預設國別限制即可；預算調小讓測試跑得快。
        budget: 256,
        max_pass: 5,
        min_delete: 1,
        ..Default::default()
    }
}

#[test]
fn prunes_duplicate_clusters_and_keeps_one_per_label() {
    let dir = tempfile::tempdir().expect("暫存目錄");
    let base = dir.path().to_path_buf();
    let (cities, admin1, total) = write_fixture(&base, 25);
    let out = base.join("pruned.txt");

    let report = stage::run(&stage::PruneOptions {
        cities_file: cities.clone(),
        admin1_file: admin1,
        output_file: Some(out.clone()),
        config: cfg(),
    })
    .expect("剪枝失敗");

    assert_eq!(report.rows_in, total, "fixture 列數應為 {total}");
    assert!(report.deleted > 0, "同名擠在一起的點應該要能刪掉一些");
    assert_eq!(
        report.rows_out + report.deleted,
        total,
        "寫出 {} + 刪除 {} 必須等於原本的 {total} 列",
        report.rows_out,
        report.deleted
    );

    // 每個 label 至少要留一個點——剪枝不得讓任何地名整個消失。
    let kept = fs::read_to_string(&out).expect("讀剪枝檔失敗");
    for label in ["Alpha", "Beta"] {
        assert!(kept.contains(label), "label {label} 不應整個消失");
    }
}

#[test]
fn writes_in_place_when_no_output_given() {
    let dir = tempfile::tempdir().expect("暫存目錄");
    let base = dir.path().to_path_buf();
    let (cities, admin1, _) = write_fixture(&base, 25);
    let before = fs::read_to_string(&cities).expect("讀原檔失敗");

    let report = stage::run(&stage::PruneOptions {
        cities_file: cities.clone(),
        admin1_file: admin1,
        output_file: None,
        config: cfg(),
    })
    .expect("剪枝失敗");

    let after = fs::read_to_string(&cities).expect("讀覆寫後的檔失敗");
    assert_ne!(before, after, "未指定 output_file 時應就地覆寫");
    assert_eq!(
        after.lines().count(),
        report.rows_out,
        "覆寫後的列數必須等於回報的 rows_out"
    );
    // Reason: 就地覆寫是先寫暫存檔再 rename，暫存檔不可殘留。
    assert!(
        !cities.with_extension("prune.tmp").exists(),
        "暫存檔應已被 rename 掉"
    );
}

#[test]
fn missing_input_reports_a_useful_error() {
    let dir = tempfile::tempdir().expect("暫存目錄");
    let base = dir.path().to_path_buf();
    let (_, admin1, _) = write_fixture(&base, 6);
    let err = stage::run(&stage::PruneOptions {
        cities_file: base.join("does_not_exist.txt"),
        admin1_file: admin1,
        output_file: None,
        config: cfg(),
    })
    .expect_err("不存在的輸入應該要失敗");
    assert!(err.contains("載入"), "錯誤訊息應指出載入失敗，實際：{err}");
}
