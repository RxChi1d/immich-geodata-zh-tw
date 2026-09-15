//! 印尼 production metadata 的時區端到端驗證。
//!
//! 以真實 `data/handler/id_geodata.csv` 完整跑 transform（cities500 列生成），
//! 驗證 per-province 時區解析在 production 資料上有效：
//! 1. 全部 104,470 列都能解析時區（解析失敗會直接回傳 Err，多時區國家
//!    不允許靜默回退）。
//! 2. 三個時區（WIB/WITA/WIT）皆有產出，且代表省份歸屬正確。
//!
//! Reason: 單元測試只能驗證對照表自身；此測試把「真實 extract 輸出 →
//! transform 時區欄位」整條路徑釘住，Wikidata 譯名漂移導致對照失效時
//! 會在 CI 直接失敗，而不是發版後才發現時區錯標。

use immich_geodata::pipeline::transform_cities_schema::{
    CoordinateFormat, build_city_rows_from_geodata,
};
use std::collections::HashMap;
use std::path::Path;

/// cities500 schema 中 timezone 欄位的索引（第 18 欄，0-based 17）。
const TIMEZONE_INDEX: usize = 17;
/// cities500 schema 中 name 欄位（印尼為 kecamatan 名，有譯名者為繁中）的索引。
const NAME_INDEX: usize = 1;

#[test]
fn production_id_geodata_resolves_all_timezones() {
    let input = Path::new("data/handler/id_geodata.csv");
    assert!(input.exists(), "data/handler/id_geodata.csv 應存在");

    // 解析失敗（省名未命中對照表）時 build 會回傳 Err——這就是要驗證的行為。
    let rows = build_city_rows_from_geodata(
        input,
        "ID",
        93_000_000,
        "2026-06-06",
        CoordinateFormat::Fixed,
    )
    .expect("真實 id_geodata.csv 的全部列都應成功解析時區");

    assert!(
        rows.len() > 100_000,
        "production 列數應為 desa 全量（實際 {}）",
        rows.len()
    );

    // 統計時區分布：三時區都必須出現。
    let mut by_timezone: HashMap<&str, usize> = HashMap::new();
    for row in &rows {
        *by_timezone.entry(row[TIMEZONE_INDEX].as_str()).or_default() += 1;
    }
    for timezone in ["Asia/Jakarta", "Asia/Makassar", "Asia/Jayapura"] {
        assert!(
            by_timezone.get(timezone).copied().unwrap_or(0) > 0,
            "時區 {timezone} 應有產出列（實際分布：{by_timezone:?}）"
        );
    }
    // WIB 涵蓋省份最多（爪哇＋蘇門答臘），列數應為三者之最。
    assert!(
        by_timezone["Asia/Jakarta"] > by_timezone["Asia/Makassar"]
            && by_timezone["Asia/Jakarta"] > by_timezone["Asia/Jayapura"],
        "WIB 應為最大宗（實際分布：{by_timezone:?}）"
    );

    // 代表省份歸屬抽查：以 kecamatan 名（name 欄）找各時區代表列。
    //
    // Reason: 三個名稱都經查證在 id_geodata.csv 中跨省唯一，所以可以斷言
    // 「同名的每一列」都是該時區——若日後 BIG 圖資出現同名 kecamatan，
    // 這個斷言會失敗，而不是靜默抽到別省的列。
    //
    // 取譯名而非 BIG 原文（Ubud / Gambir / Abepura），因為 admin_3 現在會經
    // data/vendor/indonesia/kecamatan_zh.csv 查表；這三筆都命中譯名表。
    let representative = [
        ("乌布", "Asia/Makassar"),     // 巴釐省（WITA），Ubud 的譯名
        ("甘密埔", "Asia/Jakarta"),    // 雅加達（WIB），Gambir 的譯名
        ("阿貝普拉", "Asia/Jayapura"), // 巴布亞省（WIT），Abepura 的譯名
    ];
    for (kecamatan, expected_timezone) in representative {
        let matched: Vec<&Vec<String>> = rows
            .iter()
            .filter(|row| row[NAME_INDEX] == kecamatan)
            .collect();
        assert!(
            !matched.is_empty(),
            "應存在 kecamatan 為「{kecamatan}」的列"
        );
        for row in matched {
            assert_eq!(
                row[TIMEZONE_INDEX], expected_timezone,
                "「{kecamatan}」的時區應為 {expected_timezone}"
            );
        }
    }
}
