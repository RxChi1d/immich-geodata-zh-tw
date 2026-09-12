//! 真實資料 gate：以 production 路徑跑完整條 translate，驗證 admin1 修正器
//! 在 Polars metadata 表、cities500 欄位與檔案輸出之間的接線確實通了。
//!
//! 預設 `#[ignore]`——需要 NE admin-1 圖資、admin2Codes 與 cities500 等大型
//! 上游資料，不放進 repo。以環境變數指向已備妥的目錄後手動執行：
//!
//! ```text
//! ADMIN1_REAL_DATA_DIR=<dir> cargo test --release --test admin1_correct_real_data -- --ignored --nocapture
//! ```
//!
//! 目錄需含 `geoname_data/`、`out/`（cities500_optimized.txt、
//! admin1CodesASCII_optimized.txt、alternate_chinese_name.csv）與 `locationiq/`。

use std::path::PathBuf;

use immich_geodata::pipeline::translate::{self, ProductionTranslateOptions};

#[test]
#[ignore = "需要大型上游資料，以 ADMIN1_REAL_DATA_DIR 指定後手動執行"]
fn real_malaysia_run_applies_eight_verified_corrections() {
    let Ok(root) = std::env::var("ADMIN1_REAL_DATA_DIR") else {
        panic!("請設定 ADMIN1_REAL_DATA_DIR");
    };
    let root = PathBuf::from(root);

    translate::run_production(&ProductionTranslateOptions {
        metadata_dir: root.join("locationiq"),
        data_dir: root.join("geoname_data"),
        cities_file: root.join("out/cities500_optimized.txt"),
        admin1_file: root.join("out/admin1CodesASCII_optimized.txt"),
        alternate_name_file: root.join("out/alternate_chinese_name.csv"),
        naer_file: PathBuf::from("data/vendor/naer/naer_place_names.csv"),
        output_dir: root.join("out"),
        profile: false,
    })
    .expect("translate 應成功");

    let translated =
        std::fs::read_to_string(root.join("out/cities500_translated.txt")).expect("讀取輸出");
    let admin1_by_id: std::collections::HashMap<&str, &str> = translated
        .lines()
        .filter_map(|line| {
            let fields: Vec<&str> = line.split('\t').collect();
            Some((*fields.first()?, *fields.get(10)?))
        })
        .collect();

    // 8 筆已逐筆對照官方與 OSM 資料查核為真實上游錯誤的修正。
    for (geoname_id, expected, label) in [
        ("13118198", "12", "SS2"),
        ("13118277", "12", "Bandar Utama"),
        ("13118370", "12", "Mutiara Damansara"),
        ("1735162", "14", "Setapak"),
        ("1735168", "12", "Ampang"),
        ("1744366", "02", "Serdang"),
        ("1769612", "12", "Kampong Dungun"),
        ("1777077", "14", "Cheras"),
    ] {
        assert_eq!(
            admin1_by_id.get(geoname_id),
            Some(&expected),
            "{label} 的 admin1 應被修正為 {expected}"
        );
    }

    // NE 獨排眾議的案例必須保留原值。
    for (geoname_id, expected, label) in [
        ("13100287", "01", "Mukim Tioman"),
        ("1734949", "06", "Kampung Tekek"),
        ("1734399", "07", "Selama"),
    ] {
        assert_eq!(
            admin1_by_id.get(geoname_id),
            Some(&expected),
            "{label} 應保留原值 {expected}"
        );
    }

    let fixes = std::fs::read_to_string(root.join("locationiq/MY_admin1_fixes.csv"))
        .expect("應寫出 MY_admin1_fixes.csv");
    let rows = fixes.lines().count() - 1;
    assert_eq!(rows, 19, "候選列數應為 19，實際 {rows}");
    assert_eq!(
        fixes.matches(",accepted,").count(),
        8,
        "採納列數應為 8：\n{fixes}"
    );
}
