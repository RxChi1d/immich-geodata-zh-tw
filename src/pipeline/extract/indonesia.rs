//! 印尼（Indonesia）BIG desa 圖資的 feature → ExtractRow 轉換。
//!
//! 行政層級對應（BIG 欄位 → 輸出欄位）：
//! - WADMPR（省）  → admin_1（Wikidata 繁中翻譯）
//! - WADMKK（縣市）→ admin_2（Wikidata 繁中翻譯）
//! - WADMKC（郡）  → admin_3（vendored 對照表譯名，查無則回退原文）
//! - WADMKD（村）  → admin_4（沿用印尼文原文）
//!
//! admin_1／admin_2 走 Wikidata translator；admin_3 是 Immich 顯示的城市名
//! （見 docs/zh-tw/city-level-criteria.md），但 Wikidata 對該層的中文覆蓋不足
//! 3%，改以 data/vendor/indonesia/kecamatan_zh.csv 補位。admin_4 不翻譯。

use super::indonesia_kecamatan::KecamatanNames;
use super::indonesia_normalize::{fix_simplified_chars, normalize_admin1_suffix};
use super::label_sanitize::{is_valid_chinese_translation, strip_trailing_parenthetical};
use super::types::{ExtractRow, Feature, FeatureGeometry, WikidataTranslations};
use std::collections::HashMap;

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
    kecamatan_names: &KecamatanNames,
) -> Result<Vec<ExtractRow>, String> {
    let admitted: Vec<&Feature> = features
        .iter()
        // Reason: WADMPR 或 WADMKK 空白者為「Area tidak terdefinisi」
        //         （未定義行政區），無法對應省/縣市，直接跳過。
        .filter(|feature| {
            !attribute(feature, "WADMPR").trim().is_empty()
                && !attribute(feature, "WADMKK").trim().is_empty()
        })
        .collect();
    let admin3_by_unit = resolve_admin3_names(&admitted, kecamatan_names)?;
    admitted
        .into_iter()
        .map(|feature| indonesia_feature_row(feature, translations, &admin3_by_unit))
        .collect()
}

/// 逐「行政單位」決定 admin_3 的最終名稱，key 為 (WADMKK, WADMKC)。
///
/// Reason: 早期版本逐列查表，而查表要比對座標。大型 kecamatan 的邊緣 desa 會
/// 落在距離門檻外，導致同一個郡的一部分列顯示中文、另一部分顯示印尼文——實測
/// 45 個單位出現這種分裂（凱馬納縣的 Kaimana：341 列「開馬納」、181 列
/// 「Kaimana」），在 Immich 上就是同一個郡變成兩個地方，違反
/// docs/zh-tw/city-level-criteria.md 的條件 3（層級一致）。
///
/// 改為：同一單位只判定一次，只要該單位**任一**代表點落在門檻內就整組採用譯名。
fn resolve_admin3_names(
    features: &[&Feature],
    kecamatan_names: &KecamatanNames,
) -> Result<HashMap<(String, String), String>, String> {
    let mut resolved: HashMap<(String, String), String> = HashMap::new();
    for feature in features {
        let wadmkc = attribute(feature, "WADMKC");
        let unit = (attribute(feature, "WADMKK").to_string(), wadmkc.to_string());
        if resolved.contains_key(&unit) {
            continue;
        }
        let (longitude, latitude) = point_geometry(&feature.geometry)?;
        // Reason: 查表失敗不寫入 resolved，所以同一單位的下一個代表點會再試一次；
        // 只要任一點落在門檻內就採用，不需要第二輪掃描。
        if let Some(name) = kecamatan_names.lookup(wadmkc, latitude, longitude) {
            resolved.insert(unit, indonesia_admin3(Some(name), wadmkc));
        }
    }
    Ok(resolved)
}

fn indonesia_feature_row(
    feature: &Feature,
    translations: &WikidataTranslations,
    admin3_by_unit: &HashMap<(String, String), String>,
) -> Result<ExtractRow, String> {
    let (longitude, latitude) = point_geometry(&feature.geometry)?;
    let wadmpr = attribute(feature, "WADMPR");
    let wadmkk = attribute(feature, "WADMKK");
    let wadmkc = attribute(feature, "WADMKC");
    let admin_3 = admin3_by_unit
        .get(&(wadmkk.to_string(), wadmkc.to_string()))
        .cloned()
        .unwrap_or_else(|| indonesia_admin3(None, wadmkc));
    Ok(ExtractRow::from_point(
        latitude,
        longitude,
        "印尼",
        indonesia_admin1(wadmpr, translations),
        indonesia_admin2(wadmpr, wadmkk, translations),
        admin_3,
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

/// kecamatan（郡）的繁中譯名，即 Immich 顯示的城市名。
///
/// Reason: 這一層的中文覆蓋不足 3%，Wikidata 的 admin1／admin2 translator 拿不到，
/// 改以 vendored 對照表補位（來源與收錄規則見 data/vendor/indonesia/README.md）。
/// 查無譯名時回退 BIG 原文——多數 kecamatan 本來就沒有中文名，那是預期結果而非缺陷。
fn indonesia_admin3(translated: Option<&str>, wadmkc: &str) -> String {
    match translated
        // Reason: 與 admin1／admin2 同一道防線——非中文形態的「翻譯」視為無效。
        .filter(|name| is_valid_chinese_translation(name))
    {
        // 譯名只做安全簡轉繁，與 admin1／admin2 一致。
        Some(name) => fix_simplified_chars(name),
        // Reason: 回退的 BIG 原文**不做**去括號正規化。admin1／admin2 去括號是為了
        // 清掉 Wikidata label 的消歧後綴，但 BIG 的 WADMKC 括號本身就是官方消歧
        // 資訊——`Misool (Misool Utara)` 與 `Misool (Misool Selatan)` 是不同的郡，
        // 去掉會讓兩者變成同一個名字。實測 70 列受影響。
        None => wadmkc.trim().to_string(),
    }
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
        let rows =
            indonesia_feature_rows(&features, &translations, &KecamatanNames::default()).unwrap();
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
        let rows =
            indonesia_feature_rows(&features, &translations, &KecamatanNames::default()).unwrap();
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
        let rows =
            indonesia_feature_rows(&features, &translations, &KecamatanNames::default()).unwrap();
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
        let rows =
            indonesia_feature_rows(&features, &translations, &KecamatanNames::default()).unwrap();
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
        let rows =
            indonesia_feature_rows(&features, &translations, &KecamatanNames::default()).unwrap();
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
        let rows =
            indonesia_feature_rows(&features, &translations, &KecamatanNames::default()).unwrap();
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
        let rows =
            indonesia_feature_rows(&features, &translations, &KecamatanNames::default()).unwrap();
        // 只有最後一個合法 feature 應輸出。
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].admin_1, "雅加達");
    }

    fn stub_kecamatan_names() -> KecamatanNames {
        // 座標與 make_feature 的雅加達市中心一致，讓距離驗證通過。
        let dir = Box::leak(Box::new(tempfile::tempdir().unwrap()));
        let path = dir.path().join("kecamatan_zh.csv");
        std::fs::write(
            &path,
            "name,name_zh,latitude,longitude,source\n\
             Gambir,甘碧,-6.170000,106.820000,wikidata\n\
             Menteng,面田,-6.170000,106.820000,osm\n",
        )
        .unwrap();
        KecamatanNames::load(&path).unwrap()
    }

    /// admin_3 有譯名時採用，沒有時回退 BIG 原文。
    ///
    /// Reason: admin_3 是 Immich 顯示的城市名，多數 kecamatan 沒有中文名，
    /// 回退原文是預期行為；若改成留空，Immich 會顯示不出城市。
    #[test]
    fn admin3_uses_translation_and_falls_back_to_original() {
        let features = vec![
            make_feature("DKI Jakarta", "Kota Adm. Jakarta Pusat", "Gambir", "Gambir"),
            make_feature("DKI Jakarta", "Kota Adm. Jakarta Pusat", "Cempaka", "X"),
        ];
        let rows = indonesia_feature_rows(&features, &stub_translations(), &stub_kecamatan_names())
            .unwrap();

        assert_eq!(rows[0].admin_3, "甘碧", "表中有譯名時應採用");
        assert_eq!(
            rows[1].admin_3, "Cempaka",
            "表中無譯名時應回退 BIG 原文，而非留空"
        );
    }

    /// 空表（檔案缺失）時每一列都回退原文，不得 panic 或留空。
    #[test]
    fn admin3_without_table_keeps_original_names() {
        let features = vec![make_feature(
            "DKI Jakarta",
            "Kota Adm. Jakarta Pusat",
            "Gambir",
            "Gambir",
        )];
        let rows =
            indonesia_feature_rows(&features, &stub_translations(), &KecamatanNames::default())
                .unwrap();

        assert_eq!(rows[0].admin_3, "Gambir");
    }

    /// 以 Point geometry 建立印尼 Feature，座標可指定。
    fn make_feature_at(
        wadmkk: &str,
        wadmkc: &str,
        wadmkd: &str,
        longitude: f64,
        latitude: f64,
    ) -> Feature {
        let mut attrs = FeatureAttributes::empty(Country::Indonesia);
        attrs.set("WADMPR", "DKI Jakarta".to_string());
        attrs.set("WADMKK", wadmkk.to_string());
        attrs.set("WADMKC", wadmkc.to_string());
        attrs.set("WADMKD", wadmkd.to_string());
        Feature {
            geometry: FeatureGeometry::Point((longitude, latitude)),
            attributes: attrs,
            crs: Some("EPSG:4326".to_string()),
        }
    }

    /// 同一個 kecamatan 的所有列必須是同一個名字，即使部分代表點在距離門檻外。
    ///
    /// Reason: 早期版本逐列查表，大型 kecamatan 的邊緣 desa 落在 15 km 門檻外而
    /// 保留印尼文，同一個郡因此在 Immich 上變成兩個地方（實測 45 個單位分裂）。
    #[test]
    fn admin3_is_uniform_within_one_kecamatan() {
        let features = vec![
            // 近中心——查表會命中
            make_feature_at("Kota Adm. Jakarta Pusat", "Gambir", "A", 106.82, -6.17),
            // 距離 100 km 以上——單獨查表必定落空
            make_feature_at("Kota Adm. Jakarta Pusat", "Gambir", "B", 108.00, -6.17),
        ];
        let rows = indonesia_feature_rows(&features, &stub_translations(), &stub_kecamatan_names())
            .unwrap();

        assert_eq!(rows.len(), 2);
        assert_eq!(
            rows[0].admin_3, rows[1].admin_3,
            "同一 kecamatan 的列不得出現兩種名字"
        );
        assert_eq!(rows[0].admin_3, "甘碧");
    }

    /// 回退的 BIG 原文要保留消歧括號。
    ///
    /// Reason: `Misool (Misool Utara)` 與 `Misool (Misool Selatan)` 是不同的郡，
    /// 套用 admin1／admin2 的去括號正規化會讓兩者變成同一個名字。
    #[test]
    fn admin3_fallback_keeps_disambiguating_parenthetical() {
        let features = vec![
            make_feature_at("Raja Ampat", "Misool (Misool Utara)", "A", 130.0, -2.0),
            make_feature_at("Raja Ampat", "Misool (Misool Selatan)", "B", 130.1, -2.1),
        ];
        let rows = indonesia_feature_rows(&features, &stub_translations(), &stub_kecamatan_names())
            .unwrap();

        assert_eq!(rows[0].admin_3, "Misool (Misool Utara)");
        assert_eq!(rows[1].admin_3, "Misool (Misool Selatan)");
        assert_ne!(
            rows[0].admin_3, rows[1].admin_3,
            "去括號會讓兩個不同的郡變成同一個名字"
        );
    }

    /// admin_4（desa）不翻譯——它不進 cities500，翻了只是增加誤配風險。
    #[test]
    fn admin4_is_never_translated() {
        let features = vec![make_feature(
            "DKI Jakarta",
            "Kota Adm. Jakarta Pusat",
            "Gambir",
            "Menteng",
        )];
        let rows = indonesia_feature_rows(&features, &stub_translations(), &stub_kecamatan_names())
            .unwrap();

        assert_eq!(rows[0].admin_3, "甘碧");
        assert_eq!(
            rows[0].admin_4, "Menteng",
            "表中雖有 Menteng 的譯名，admin_4 仍須保持原文"
        );
    }
}
