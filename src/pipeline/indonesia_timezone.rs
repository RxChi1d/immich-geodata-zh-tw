//! 印尼 38 省的 per-province 時區對照。
//!
//! 印尼橫跨三個時區：
//! - WIB（Waktu Indonesia Barat）= `Asia/Jakarta`（UTC+7）
//! - WITA（Waktu Indonesia Tengah）= `Asia/Makassar`（UTC+8）
//! - WIT（Waktu Indonesia Timur）= `Asia/Jayapura`（UTC+9）
//!
//! ## 解析時機與 key 選擇
//!
//! 時區在 transform 階段（cities500 schema）解析。此時 `admin_1` 已是 ID
//! handler 輸出的「最終省名」（Wikidata 繁中譯名經 s2t 簡轉繁 + 補省字尾
//! 正規化後的形態，如 `中爪哇省`、`巴釐省`、`巴布亞省`）。
//!
//! ## 時區歸屬的權威來源：WADMPR 原文
//!
//! 時區歸屬以 **BIG WADMPR 原文拼寫**（如 `DKI Jakarta`、`Bali`、`Papua`）
//! 為唯一權威 key（`WIB/WITA/WIT_PROVINCES`）——原文穩定、語言無關，不受
//! 翻譯形態與未來 Wikidata label 漂移影響。
//!
//! transform 端只拿得到最終繁中省名（canonical CSV schema 不含 WADMPR 欄位），
//! 故另附一份「最終繁中省名 → WADMPR 原文」對照（`PROVINCE_ZH_TW`）作為查詢
//! 入口。此處的繁中 key **必須與 ID handler 的最終 admin1 輸出逐字一致**；
//! `indonesia_normalize::normalize_admin1_suffix` 決定最終形態，對應測試
//! （`handler_admin1_outputs_resolve_timezone`）斷言 38 省最終形態全部命中，
//! 避免兩處漂移。

/// WIB（Asia/Jakarta）省份——BIG WADMPR 原文拼寫，共 18 省。
const WIB_PROVINCES: &[&str] = &[
    "Aceh",
    "Sumatera Utara",
    "Sumatera Barat",
    "Riau",
    "Kepulauan Riau",
    "Jambi",
    "Sumatera Selatan",
    "Kepulauan Bangka Belitung",
    "Bengkulu",
    "Lampung",
    "DKI Jakarta",
    "Banten",
    "Jawa Barat",
    "Jawa Tengah",
    "Daerah Istimewa Yogyakarta",
    "Jawa Timur",
    "Kalimantan Barat",
    "Kalimantan Tengah",
];

/// WITA（Asia/Makassar）省份——BIG WADMPR 原文拼寫，共 12 省。
const WITA_PROVINCES: &[&str] = &[
    "Kalimantan Selatan",
    "Kalimantan Timur",
    "Kalimantan Utara",
    "Bali",
    "Nusa Tenggara Barat",
    "Nusa Tenggara Timur",
    "Sulawesi Utara",
    "Sulawesi Tengah",
    "Sulawesi Selatan",
    "Sulawesi Tenggara",
    "Gorontalo",
    "Sulawesi Barat",
];

/// WIT（Asia/Jayapura）省份——BIG WADMPR 原文拼寫，共 8 省。
const WIT_PROVINCES: &[&str] = &[
    "Maluku",
    "Maluku Utara",
    "Papua",
    "Papua Barat",
    "Papua Selatan",
    "Papua Tengah",
    "Papua Pegunungan",
    "Papua Barat Daya",
];

/// ID handler 最終 admin1 省名 → BIG WADMPR 原文對照。
///
/// 左欄為 `indonesia_wikidata` 經「Wikidata 繁中譯名 → s2t 簡轉繁 → 補省字尾
/// 正規化」後的最終 admin1 輸出（transform 階段拿到的實際值）；右欄為對應的
/// WADMPR 原文，用以查 `WIB/WITA/WIT_PROVINCES` 取得時區。
///
/// 左欄必須與 handler 最終輸出逐字一致（含「省」字尾、繁簡用字與標點），由
/// `handler_admin1_outputs_resolve_timezone` 測試保護。
const PROVINCE_ZH_TW: &[(&str, &str)] = &[
    ("亞齊特區", "Aceh"),
    ("北蘇門答臘省", "Sumatera Utara"),
    ("西蘇門答臘省", "Sumatera Barat"),
    ("廖內省", "Riau"),
    ("廖內羣島省", "Kepulauan Riau"),
    ("占碑省", "Jambi"),
    ("南蘇門答臘省", "Sumatera Selatan"),
    ("邦加-勿里洞省", "Kepulauan Bangka Belitung"),
    ("明古魯省", "Bengkulu"),
    ("楠榜省", "Lampung"),
    ("雅加達", "DKI Jakarta"),
    ("萬丹省", "Banten"),
    ("西爪哇省", "Jawa Barat"),
    ("中爪哇省", "Jawa Tengah"),
    ("日惹特區", "Daerah Istimewa Yogyakarta"),
    ("東爪哇省", "Jawa Timur"),
    ("西加里曼丹省", "Kalimantan Barat"),
    ("中加里曼丹省", "Kalimantan Tengah"),
    ("南加里曼丹省", "Kalimantan Selatan"),
    ("東加里曼丹省", "Kalimantan Timur"),
    ("北加里曼丹省", "Kalimantan Utara"),
    ("巴釐省", "Bali"),
    ("西努沙登加拉省", "Nusa Tenggara Barat"),
    ("東努沙登加拉省", "Nusa Tenggara Timur"),
    ("北蘇拉威西省", "Sulawesi Utara"),
    ("中蘇拉威西省", "Sulawesi Tengah"),
    ("南蘇拉威西省", "Sulawesi Selatan"),
    ("東南蘇拉威西省", "Sulawesi Tenggara"),
    ("哥倫打洛省", "Gorontalo"),
    ("西蘇拉威西省", "Sulawesi Barat"),
    ("馬魯古省", "Maluku"),
    ("北馬魯古省", "Maluku Utara"),
    ("巴布亞省", "Papua"),
    ("西巴布亞省", "Papua Barat"),
    ("南巴布亞省", "Papua Selatan"),
    ("中巴布亞省", "Papua Tengah"),
    ("高地巴布亞省", "Papua Pegunungan"),
    ("西南巴布亞省", "Papua Barat Daya"),
];

/// 依省名解析時區，支援 BIG WADMPR 原文與 handler 最終繁中省名兩種 key。
///
/// transform 端傳入的是 handler 最終繁中省名；WADMPR 原文 key 額外保留，作為
/// 穩定、語言無關的後備入口。回傳 `None` 時由呼叫端
/// （`CountryProfile::timezone_for_admin1`）回報錯誤、使 release 失敗。
pub fn timezone_for_province(province: &str) -> Option<&'static str> {
    let key = province.trim();
    // WADMPR 原文 key（穩定、語言無關）優先。
    if let Some(timezone) = timezone_by_original(key) {
        return Some(timezone);
    }
    // 最終繁中省名 → WADMPR 原文 → 時區。
    let original = PROVINCE_ZH_TW
        .iter()
        .find(|(zh_tw, _)| *zh_tw == key)
        .map(|(_, original)| *original)?;
    timezone_by_original(original)
}

fn timezone_by_original(original: &str) -> Option<&'static str> {
    if WIB_PROVINCES.contains(&original) {
        Some("Asia/Jakarta")
    } else if WITA_PROVINCES.contains(&original) {
        Some("Asia/Makassar")
    } else if WIT_PROVINCES.contains(&original) {
        Some("Asia/Jayapura")
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn province_count_matches_38() {
        assert_eq!(
            WIB_PROVINCES.len() + WITA_PROVINCES.len() + WIT_PROVINCES.len(),
            38
        );
    }

    #[test]
    fn each_group_has_expected_size() {
        assert_eq!(WIB_PROVINCES.len(), 18);
        assert_eq!(WITA_PROVINCES.len(), 12);
        assert_eq!(WIT_PROVINCES.len(), 8);
    }

    #[test]
    fn zh_tw_table_has_38_entries() {
        assert_eq!(PROVINCE_ZH_TW.len(), 38);
    }

    #[test]
    fn resolves_original_spelling() {
        // 全 38 省以 WADMPR 原文解析，皆得非預設（None）時區。
        for original in WIB_PROVINCES {
            assert_eq!(
                timezone_for_province(original),
                Some("Asia/Jakarta"),
                "WIB 省以原文未解析：{original}"
            );
        }
        for original in WITA_PROVINCES {
            assert_eq!(
                timezone_for_province(original),
                Some("Asia/Makassar"),
                "WITA 省以原文未解析：{original}"
            );
        }
        for original in WIT_PROVINCES {
            assert_eq!(
                timezone_for_province(original),
                Some("Asia/Jayapura"),
                "WIT 省以原文未解析：{original}"
            );
        }
    }

    /// 核心回歸防線：以 ID handler 實際輸出的最終省名（s2t + 補省）逐一驗證
    /// 時區解析，確保 transform 端拿到的繁中省名全部命中非預設時區。
    #[test]
    fn handler_admin1_outputs_resolve_timezone() {
        // (handler 最終 admin1 省名, 期望時區)
        let cases: &[(&str, &str)] = &[
            // WIB（含特區／首都／補省案例）
            ("亞齊特區", "Asia/Jakarta"),
            ("北蘇門答臘省", "Asia/Jakarta"),  // 補「省」案例
            ("廖內羣島省", "Asia/Jakarta"),    // 「羣」異體字案例
            ("邦加-勿里洞省", "Asia/Jakarta"), // 半形連字號案例
            ("雅加達", "Asia/Jakarta"),        // 首都特區不加「省」
            ("日惹特區", "Asia/Jakarta"),
            ("中爪哇省", "Asia/Jakarta"),
            ("中加里曼丹省", "Asia/Jakarta"), // 補「省」案例
            // WITA
            ("巴釐省", "Asia/Makassar"), // 「巴釐」而非「峇里」用字案例
            ("東加里曼丹省", "Asia/Makassar"),
            ("北加里曼丹省", "Asia/Makassar"),
            ("哥倫打洛省", "Asia/Makassar"), // 補「省」案例
            ("西蘇拉威西省", "Asia/Makassar"),
            // WIT
            ("巴布亞省", "Asia/Jayapura"), // s2t 簡轉繁（亚→亞）案例
            ("西巴布亞省", "Asia/Jayapura"),
            ("高地巴布亞省", "Asia/Jayapura"),
            ("馬魯古省", "Asia/Jayapura"),
        ];
        for (name, expected) in cases {
            assert_eq!(
                timezone_for_province(name),
                Some(*expected),
                "handler 最終省名未解析或時區錯誤：{name}"
            );
        }
        // 全 38 筆繁中 key 皆解析出非預設時區。
        for (zh_tw, _) in PROVINCE_ZH_TW {
            assert!(
                timezone_for_province(zh_tw).is_some(),
                "最終繁中省名未解析：{zh_tw}"
            );
        }
    }

    #[test]
    fn unknown_province_returns_none() {
        assert_eq!(timezone_for_province("Nowhere"), None);
        // 舊式無字尾繁中名（非 handler 最終形態）不再命中——時區改以最終形態
        // 與原文為準。
        assert_eq!(timezone_for_province("峇里"), None);
    }

    /// 加里曼丹（婆羅洲）跨時區邊界：西／中→WIB；南／東／北→WITA。
    #[test]
    fn kalimantan_timezone_boundary() {
        assert_eq!(
            timezone_for_province("西加里曼丹省"),
            Some("Asia/Jakarta"),
            "西加里曼丹應為 WIB"
        );
        assert_eq!(
            timezone_for_province("中加里曼丹省"),
            Some("Asia/Jakarta"),
            "中加里曼丹應為 WIB"
        );
        assert_eq!(
            timezone_for_province("南加里曼丹省"),
            Some("Asia/Makassar"),
            "南加里曼丹應為 WITA"
        );
        assert_eq!(
            timezone_for_province("東加里曼丹省"),
            Some("Asia/Makassar"),
            "東加里曼丹應為 WITA"
        );
        assert_eq!(
            timezone_for_province("北加里曼丹省"),
            Some("Asia/Makassar"),
            "北加里曼丹應為 WITA"
        );
    }
}
