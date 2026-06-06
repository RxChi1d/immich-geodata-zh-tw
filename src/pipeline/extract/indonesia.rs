//! 印尼（Indonesia）BIG desa 圖資的 feature → ExtractRow 轉換。
//!
//! 行政層級對應（BIG 欄位 → 輸出欄位）：
//! - WADMPR（省）  → admin_1（Wikidata 繁中翻譯）
//! - WADMKK（縣市）→ admin_2（Wikidata 繁中翻譯）
//! - WADMKC（郡）  → admin_3（沿用印尼文原文）
//! - WADMKD（村）  → admin_4（沿用印尼文原文）
//!
//! 比照 TH/KR：admin_1/admin_2 為翻譯後繁中，admin_3 以下沿用原文。

use super::indonesia_normalize::{
    fix_simplified_chars, is_valid_chinese_translation, normalize_admin1_suffix,
    strip_trailing_parenthetical,
};
use super::types::{ExtractRow, Feature, FeatureGeometry, WikidataTranslations};

/// 將印尼 feature 集合轉為 ExtractRow，並過濾未定義行政區的列。
///
/// admin1/admin2 的最終形態在此消費層統一施作，確保 live（Wikidata 即時查詢）
/// 與 fixture（stub）兩條來源路徑得到一致輸出：
/// - 安全字元級簡轉繁（白名單；修正 Wikidata label 殘留簡體字，如
///   「巴布亚」→「巴布亞」），不過度轉換已正確的繁體專名。
/// - admin1 額外補「省」字尾正規化（特區／首都／已含字尾者不動）。
pub(super) fn indonesia_feature_rows(
    features: &[Feature],
    translations: &WikidataTranslations,
) -> Result<Vec<ExtractRow>, String> {
    features
        .iter()
        // Reason: WADMPR 或 WADMKK 空白者為「Area tidak terdefinisi」
        //         （未定義行政區），無法對應省/縣市，直接跳過。
        .filter(|feature| {
            !attribute(feature, "WADMPR").trim().is_empty()
                && !attribute(feature, "WADMKK").trim().is_empty()
        })
        .map(|feature| indonesia_feature_row(feature, translations))
        .collect()
}

fn indonesia_feature_row(
    feature: &Feature,
    translations: &WikidataTranslations,
) -> Result<ExtractRow, String> {
    let (longitude, latitude) = point_geometry(&feature.geometry)?;
    let wadmpr = attribute(feature, "WADMPR");
    let wadmkk = attribute(feature, "WADMKK");
    Ok(ExtractRow::from_point(
        latitude,
        longitude,
        "印尼",
        indonesia_admin1(wadmpr, translations),
        indonesia_admin2(wadmpr, wadmkk, translations),
        attribute(feature, "WADMKC").to_string(),
        attribute(feature, "WADMKD").to_string(),
    ))
}

fn indonesia_admin1(wadmpr: &str, translations: &WikidataTranslations) -> String {
    let base = translations
        .admin1_by_name
        .get(wadmpr)
        .cloned()
        // Reason: 非中文形態（純拉丁、中英夾雜）的「翻譯」一律視為無效，
        //         回退原文；涵蓋英文 label、stale cache 殘留與髒資料。
        .filter(|name| is_valid_chinese_translation(name))
        .unwrap_or_else(|| wadmpr.to_string());
    // 最終形態：去消歧括號、安全簡轉繁後補省字尾正規化。
    normalize_admin1_suffix(&fix_simplified_chars(&strip_trailing_parenthetical(&base)))
}

fn indonesia_admin2(wadmpr: &str, wadmkk: &str, translations: &WikidataTranslations) -> String {
    let base = translations
        .admin2_by_parent
        .get(wadmpr)
        .and_then(|by_name| by_name.get(wadmkk))
        .or_else(|| translations.fallback_by_name.get(wadmkk))
        .cloned()
        // Reason: 非中文形態（純拉丁如「East Barito」、中英夾雜如「西Kutai區」）
        //         一律視為無效翻譯，回退 BIG 原文；涵蓋 stale cache 殘留。
        .filter(|name| is_valid_chinese_translation(name))
        // admin2 無對應翻譯時沿用 BIG 原文（fallback 原文）。
        .unwrap_or_else(|| wadmkk.to_string());
    // 最終形態：去消歧括號後安全簡轉繁（修正殘留簡體字，不過度轉換正確繁體）。
    fix_simplified_chars(&strip_trailing_parenthetical(&base))
}

fn point_geometry(geometry: &FeatureGeometry) -> Result<(f64, f64), String> {
    match geometry {
        FeatureGeometry::Point(point) => Ok(*point),
        _ => Err("預期 centroid 後的 Point geometry".to_string()),
    }
}

fn attribute<'a>(feature: &'a Feature, key: &str) -> &'a str {
    feature.attributes.get(key).unwrap_or("")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pipeline::extract::types::{Country, FeatureAttributes};
    use std::collections::HashMap;

    /// 以 Point geometry 建立印尼 Feature（模擬 centroid 計算後的狀態）。
    fn make_feature(wadmpr: &str, wadmkk: &str, wadmkc: &str, wadmkd: &str) -> Feature {
        let mut attrs = FeatureAttributes::empty(Country::Indonesia);
        attrs.set("WADMPR", wadmpr.to_string());
        attrs.set("WADMKK", wadmkk.to_string());
        attrs.set("WADMKC", wadmkc.to_string());
        attrs.set("WADMKD", wadmkd.to_string());
        Feature {
            // Reason: Point 模擬 centroid 計算後狀態；座標使用雅加達市中心附近。
            geometry: FeatureGeometry::Point((106.82, -6.17)),
            attributes: attrs,
            crs: Some("EPSG:4326".to_string()),
        }
    }

    fn stub_translations() -> WikidataTranslations {
        let mut t = WikidataTranslations::default();
        t.admin1_by_name
            .insert("DKI Jakarta".to_string(), "雅加達".to_string());
        t.admin1_by_name
            .insert("Jawa Barat".to_string(), "西爪哇".to_string());
        // Reason: Papua 的 Wikidata zh-hant label 殘留簡體「亚」，用於驗證
        //         消費層 s2t 後轉繁為「巴布亞省」。
        t.admin1_by_name
            .insert("Papua".to_string(), "巴布亚省".to_string());
        let mut dki_admin2 = HashMap::new();
        dki_admin2.insert(
            "Kota Adm. Jakarta Pusat".to_string(),
            "中雅加達".to_string(),
        );
        dki_admin2.insert("Adm. Kep. Seribu".to_string(), "千島群島".to_string());
        t.admin2_by_parent
            .insert("DKI Jakarta".to_string(), dki_admin2);
        let mut jabar_admin2 = HashMap::new();
        jabar_admin2.insert("Bandung".to_string(), "萬隆縣".to_string());
        jabar_admin2.insert("Kota Bandung".to_string(), "萬隆市".to_string());
        t.admin2_by_parent
            .insert("Jawa Barat".to_string(), jabar_admin2);
        t
    }

    // ---- 正常情境 --------------------------------------------------------

    #[test]
    fn translated_admin1_and_admin2_are_rendered() {
        let features = vec![make_feature(
            "DKI Jakarta",
            "Kota Adm. Jakarta Pusat",
            "Gambir",
            "Gambir",
        )];
        let translations = stub_translations();
        let rows = indonesia_feature_rows(&features, &translations).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].admin_1, "雅加達");
        assert_eq!(rows[0].admin_2, "中雅加達");
    }

    #[test]
    fn jakarta_archipelago_admin_prefix_translates_correctly() {
        // 邊界：雅加達千島群島以官方前綴「Adm. Kep. Seribu」儲存，
        //       確認正規化後翻譯查詢命中「千島群島」而非回退原文。
        let features = vec![make_feature(
            "DKI Jakarta",
            "Adm. Kep. Seribu",
            "Kepulauan Seribu Utara",
            "Pulau Kelapa",
        )];
        let translations = stub_translations();
        let rows = indonesia_feature_rows(&features, &translations).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].admin_1, "雅加達");
        assert_eq!(rows[0].admin_2, "千島群島");
        assert_eq!(rows[0].admin_3, "Kepulauan Seribu Utara");
    }

    #[test]
    fn kota_kabupaten_same_name_pair_resolved_by_parent_scope() {
        // 正常：同名的 Kota Bandung（萬隆市）與 Bandung（萬隆縣）
        //       在同一省（Jawa Barat）下，以 parent-scoped 查詢區分。
        let features = vec![
            make_feature("Jawa Barat", "Kota Bandung", "Coblong", "Lebak Siliwangi"),
            make_feature("Jawa Barat", "Bandung", "Cicendo", "Pasirkaliki"),
        ];
        let translations = stub_translations();
        let rows = indonesia_feature_rows(&features, &translations).unwrap();
        assert_eq!(rows.len(), 2);
        let kota_row = rows.iter().find(|r| r.admin_3 == "Coblong").unwrap();
        let kab_row = rows.iter().find(|r| r.admin_3 == "Cicendo").unwrap();
        assert_eq!(kota_row.admin_2, "萬隆市");
        assert_eq!(kab_row.admin_2, "萬隆縣");
        // admin1 補省正規化：西爪哇 → 西爪哇省。
        assert_eq!(kota_row.admin_1, "西爪哇省");
        assert_eq!(kab_row.admin_1, "西爪哇省");
    }

    #[test]
    fn admin1_s2t_and_suffix_normalization_applied() {
        // Q2+Q3：Papua 簡體 label「巴布亚省」→ s2t 轉繁「巴布亞省」（已含字尾不補）；
        //        DKI Jakarta「雅加達」首都特區不補「省」。
        let features = vec![
            make_feature("Papua", "Kota Jayapura", "Jayapura Utara", "Gurabesi"),
            make_feature("DKI Jakarta", "Kota Adm. Jakarta Pusat", "Gambir", "Gambir"),
        ];
        let translations = stub_translations();
        let rows = indonesia_feature_rows(&features, &translations).unwrap();
        let papua = rows.iter().find(|r| r.admin_3 == "Jayapura Utara").unwrap();
        let jakarta = rows.iter().find(|r| r.admin_3 == "Gambir").unwrap();
        assert_eq!(papua.admin_1, "巴布亞省");
        assert_eq!(jakarta.admin_1, "雅加達");
    }

    // ---- 邊界情境 --------------------------------------------------------

    #[test]
    fn admin3_and_admin4_preserve_original_indonesian_text() {
        // admin_3 / admin_4 沿用 BIG 原文，不走 Wikidata 翻譯。
        let features = vec![make_feature(
            "DKI Jakarta",
            "Kota Adm. Jakarta Pusat",
            "Gambir",
            "Petojo Utara",
        )];
        let translations = stub_translations();
        let rows = indonesia_feature_rows(&features, &translations).unwrap();
        assert_eq!(rows[0].admin_3, "Gambir");
        assert_eq!(rows[0].admin_4, "Petojo Utara");
    }

    // ---- 失敗情境 --------------------------------------------------------

    #[test]
    fn missing_admin2_stub_fallbacks_to_original_text() {
        // 失敗（fallback）：stub 中不含「Bandung Barat」的翻譯，
        //                   handler 應回退 BIG 原文而非 panic 或回傳空字串。
        let features = vec![make_feature(
            "Jawa Barat",
            "Bandung Barat",
            "Lembang",
            "Jayagiri",
        )];
        let translations = stub_translations();
        let rows = indonesia_feature_rows(&features, &translations).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(
            rows[0].admin_2, "Bandung Barat",
            "admin2 缺翻譯時應回退 BIG 原文，而非空字串或錯誤"
        );
    }

    #[test]
    fn blank_wadmpr_rows_are_filtered_out() {
        // 失敗（過濾）：WADMPR 空白者為「Area tidak terdefinisi」，
        //               必須被過濾，不可輸出任何列。
        let features = vec![
            make_feature(" ", " ", " ", " "),
            make_feature("", "", "", ""),
            make_feature("DKI Jakarta", "Kota Adm. Jakarta Pusat", "Gambir", "Gambir"),
        ];
        let translations = stub_translations();
        let rows = indonesia_feature_rows(&features, &translations).unwrap();
        // 只有最後一個合法 feature 應輸出。
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].admin_1, "雅加達");
    }
}
