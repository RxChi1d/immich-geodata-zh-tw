use std::collections::{HashMap, HashSet};
use std::path::Path;

use super::indonesia::indonesia_feature_rows;
use super::indonesia_wikidata::build_indonesia_wikidata_cache;
use super::korea_wikidata::build_korea_wikidata_cache;
use super::label_sanitize::{
    is_mixed_script, is_valid_chinese_translation, strip_trailing_parenthetical,
};
use super::thailand_wikidata::build_thailand_wikidata_cache;
use super::types::{
    Country, ExtractContext, ExtractRow, Feature, FeatureGeometry, WikidataTranslations,
    wikidata_cache_path, wikidata_stub_source,
};
use super::wikidata_common::read_wikidata_stub;

impl Country {
    pub(super) fn load_context(
        self,
        source_path: &Path,
        wikidata_stub: Option<&Path>,
        features: &[Feature],
    ) -> Result<ExtractContext, String> {
        match self {
            Self::Taiwan | Self::Japan => Ok(ExtractContext::default()),
            Self::Korea => {
                let stub = wikidata_stub
                    .filter(|path| path.exists())
                    .map(Path::to_path_buf)
                    .or_else(|| wikidata_stub_source(source_path, "KR"));
                let korea_translations = if let Some(stub) = stub {
                    read_wikidata_stub(&stub, "KR")?
                } else {
                    build_korea_wikidata_cache(features, &wikidata_cache_path("KR"))?
                };
                Ok(ExtractContext {
                    korea_translations,
                    ..ExtractContext::default()
                })
            }
            Self::Thailand => {
                let stub = wikidata_stub
                    .filter(|path| path.exists())
                    .map(Path::to_path_buf)
                    .or_else(|| wikidata_stub_source(source_path, "TH"));
                let thailand_translations = if let Some(stub) = stub {
                    read_wikidata_stub(&stub, "TH")?
                } else {
                    build_thailand_wikidata_cache(features, &wikidata_cache_path("TH"))?
                };
                Ok(ExtractContext {
                    thailand_translations,
                    ..ExtractContext::default()
                })
            }
            Self::Indonesia => {
                let stub = wikidata_stub
                    .filter(|path| path.exists())
                    .map(Path::to_path_buf)
                    .or_else(|| wikidata_stub_source(source_path, "ID"));
                let indonesia_translations = if let Some(stub) = stub {
                    read_wikidata_stub(&stub, "ID")?
                } else {
                    build_indonesia_wikidata_cache(features, &wikidata_cache_path("ID"))?
                };
                Ok(ExtractContext {
                    indonesia_translations,
                    ..ExtractContext::default()
                })
            }
        }
    }

    pub(super) fn rows_from_features(
        self,
        features: &[Feature],
        context: &ExtractContext,
    ) -> Result<Vec<ExtractRow>, String> {
        match self {
            Self::Taiwan => features.iter().map(taiwan_feature_row).collect(),
            Self::Japan => japan_feature_rows(features),
            Self::Korea => features
                .iter()
                .map(|feature| korea_feature_row(feature, &context.korea_translations))
                .collect(),
            Self::Thailand => features
                .iter()
                .map(|feature| thailand_feature_row(feature, &context.thailand_translations))
                .collect(),
            Self::Indonesia => indonesia_feature_rows(features, &context.indonesia_translations),
        }
    }
}

fn taiwan_feature_row(feature: &Feature) -> Result<ExtractRow, String> {
    let (longitude, latitude) = point_geometry(&feature.geometry)?;
    Ok(ExtractRow::from_point(
        latitude,
        longitude,
        "臺灣",
        attribute(feature, "COUNTYNAME").to_string(),
        attribute(feature, "TOWNNAME").to_string(),
        attribute_or(feature, "VILLNAME", "None").to_string(),
        String::new(),
    ))
}

fn thailand_feature_row(
    feature: &Feature,
    translations: &WikidataTranslations,
) -> Result<ExtractRow, String> {
    let (longitude, latitude) = point_geometry(&feature.geometry)?;
    let admin1_name = attribute(feature, "adm1_name");
    let admin2_name = attribute(feature, "adm2_name");
    Ok(ExtractRow::from_point(
        latitude,
        longitude,
        "泰國",
        thailand_admin1(admin1_name, translations),
        thailand_admin2(admin1_name, admin2_name, translations),
        official_source_name(feature, "adm3_name", "adm3_name1"),
        String::new(),
    ))
}

fn japan_feature_rows(features: &[Feature]) -> Result<Vec<ExtractRow>, String> {
    let duplicate_gun_towns = duplicate_japan_gun_towns(features);
    features
        .iter()
        .filter(|feature| {
            clean_admin(attribute(feature, "N03_003")).is_some()
                || clean_admin(attribute(feature, "N03_004")).is_some()
                || clean_admin(attribute(feature, "N03_005")).is_some()
        })
        .map(|feature| japan_feature_row(feature, &duplicate_gun_towns))
        .collect()
}

fn japan_feature_row(
    feature: &Feature,
    duplicate_gun_towns: &HashMap<String, HashSet<String>>,
) -> Result<ExtractRow, String> {
    let (longitude, latitude) = point_geometry(&feature.geometry)?;
    let admin1 = attribute(feature, "N03_001");
    let n03_003 = clean_admin(attribute(feature, "N03_003"));
    let n03_004 = clean_admin(attribute(feature, "N03_004"));
    let n03_005 = clean_admin(attribute(feature, "N03_005"));
    let is_regular_shi = n03_003.is_none()
        && n03_004.as_ref().is_some_and(|value| value.ends_with('市'))
        && n03_005.is_none();
    let is_direct_town = n03_003.is_none()
        && n03_004.is_some()
        && !n03_004.as_ref().is_some_and(|value| value.ends_with('市'))
        && n03_005.is_none();
    let is_seirei_shi = n03_005.is_some();
    let is_gun = n03_003.as_ref().is_some_and(|value| value.ends_with('郡'));
    let admin2 = if is_regular_shi || is_direct_town || is_seirei_shi {
        n03_004.unwrap_or_default().to_string()
    } else if is_gun {
        let town = n03_004.unwrap_or_default();
        if duplicate_gun_towns
            .get(admin1)
            .is_some_and(|towns| towns.contains(town))
        {
            format!("{}{}", n03_003.unwrap_or_default(), town)
        } else {
            town.to_string()
        }
    } else {
        n03_003.unwrap_or_default().to_string()
    };
    Ok(ExtractRow::from_point(
        latitude,
        longitude,
        "日本",
        admin1.to_string(),
        admin2,
        if is_seirei_shi {
            n03_005.unwrap_or_default().to_string()
        } else {
            String::new()
        },
        String::new(),
    ))
}

fn duplicate_japan_gun_towns(features: &[Feature]) -> HashMap<String, HashSet<String>> {
    let mut unique_gun_towns = HashSet::<(String, String, String)>::new();
    for feature in features {
        let admin1 = attribute(feature, "N03_001");
        let Some(gun) = clean_admin(attribute(feature, "N03_003")) else {
            continue;
        };
        let Some(town) = clean_admin(attribute(feature, "N03_004")) else {
            continue;
        };
        if gun.ends_with('郡') {
            unique_gun_towns.insert((admin1.to_string(), gun.to_string(), town.to_string()));
        }
    }
    let mut counts = HashMap::<(String, String), usize>::new();
    for (admin1, _gun, town) in &unique_gun_towns {
        *counts.entry((admin1.clone(), town.clone())).or_default() += 1;
    }
    let mut duplicate_towns = HashMap::<String, HashSet<String>>::new();
    for ((admin1, town), count) in counts {
        if count > 1 {
            duplicate_towns.entry(admin1).or_default().insert(town);
        }
    }
    duplicate_towns
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

fn attribute_or<'a>(feature: &'a Feature, key: &str, default: &'a str) -> &'a str {
    feature.attributes.get(key).unwrap_or(default)
}

fn official_source_name(feature: &Feature, english_key: &str, thai_key: &str) -> String {
    let english = attribute(feature, english_key);
    if !english.is_empty() {
        return english.to_string();
    }
    attribute(feature, thai_key).to_string()
}

fn thailand_admin1(name: &str, translations: &WikidataTranslations) -> String {
    translations
        .admin1_by_name
        .get(name)
        .cloned()
        // Reason: TH 的官方英文為設計內的合法回退輸出，不可要求純中文；
        //         但中英夾雜（半翻譯髒資料）仍應視為無效，回退官方英文原文。
        .filter(|value| !is_mixed_script(value))
        .unwrap_or_else(|| name.to_string())
}

fn thailand_admin2(
    adm1_name: &str,
    adm2_name: &str,
    translations: &WikidataTranslations,
) -> String {
    translations
        .admin2_by_parent
        .get(adm1_name)
        .and_then(|by_name| by_name.get(adm2_name))
        .or_else(|| translations.fallback_by_name.get(adm2_name))
        .cloned()
        // Reason: TH 的官方英文為設計內的合法回退輸出，不可要求純中文；
        //         但中英夾雜（半翻譯髒資料）仍應視為無效，回退官方英文原文。
        .filter(|value| !is_mixed_script(value))
        .unwrap_or_else(|| adm2_name.to_string())
}

fn clean_admin(value: &str) -> Option<&str> {
    if value.is_empty() || value == "None" || value == "nan" {
        None
    } else {
        Some(value)
    }
}

fn korea_feature_row(
    feature: &Feature,
    translations: &WikidataTranslations,
) -> Result<ExtractRow, String> {
    let (longitude, latitude) = point_geometry(&feature.geometry)?;
    let components = korea_admin_components(feature);

    let mut admin2 = korea_admin2(&components.sidonm, &components.sggnm, translations);
    if components.sidonm == "광주광역시" {
        admin2 = strip_trailing_parenthetical(&admin2);
    }

    Ok(ExtractRow::from_point(
        latitude,
        longitude,
        "南韓",
        korea_admin1(&components.sidonm, translations),
        admin2,
        components.admin3,
        components.admin4,
    ))
}

#[derive(Clone, Debug)]
pub(super) struct KoreaAdminComponents {
    pub(super) sidonm: String,
    pub(super) sggnm: String,
    pub(super) admin3: String,
    pub(super) admin4: String,
}

pub(super) fn korea_admin_components(feature: &Feature) -> KoreaAdminComponents {
    let sidonm = attribute(feature, "sidonm").to_string();
    let mut sggnm = attribute(feature, "sggnm").to_string();
    let adm_nm = attribute(feature, "adm_nm");
    let mut admin3 = adm_nm
        .replace(&sidonm, "")
        .replace(&sggnm, "")
        .trim()
        .to_string();
    let mut admin4 = String::new();

    if is_sejong_admin2_normalization_target(&sidonm, &sggnm) {
        sggnm = admin3;
        admin3 = String::new();
    }

    if let Some((city, district)) = split_korea_city_district(&sggnm) {
        sggnm = city;
        admin4 = admin3;
        admin3 = district;
    }

    KoreaAdminComponents {
        sidonm,
        sggnm,
        admin3,
        admin4,
    }
}

fn korea_admin1(name: &str, translations: &WikidataTranslations) -> String {
    let built_in = match name {
        "서울특별시" => "首爾市",
        "부산광역시" => "釜山市",
        "대구광역시" => "大邱市",
        "인천광역시" => "仁川市",
        "광주광역시" => "光州市",
        "대전광역시" => "大田市",
        "울산광역시" => "蔚山市",
        "세종특별자치시" => "世宗市",
        "경기도" => "京畿道",
        "강원특별자치도" => "江原道",
        "충청북도" => "忠清北道",
        "충청남도" => "忠清南道",
        "전북특별자치도" => "全羅北道",
        "전라남도" => "全羅南道",
        "경상북도" => "慶尚北道",
        "경상남도" => "慶尚南道",
        "제주특별자치도" => "濟州道",
        _ => "",
    };
    if built_in.is_empty() {
        translations
            .admin1_by_name
            .get(name)
            .cloned()
            // Reason: KR 期望純中文輸出；非中文形態（純拉丁、中英夾雜）的
            //         「翻譯」一律視為無效（如 stale cache 殘留），回退韓文原文。
            .filter(|value| is_valid_chinese_translation(value))
            .unwrap_or_else(|| name.to_string())
    } else {
        built_in.to_string()
    }
}

fn korea_admin2(sidonm: &str, sggnm: &str, translations: &WikidataTranslations) -> String {
    if sidonm == "세종특별자치시" {
        return sejong_admin2(sggnm);
    }
    translations
        .admin2_by_parent
        .get(sidonm)
        .and_then(|by_name| by_name.get(sggnm))
        .or_else(|| translations.fallback_by_name.get(sggnm))
        .cloned()
        // Reason: KR 期望純中文輸出；非中文形態（純拉丁、中英夾雜）的
        //         「翻譯」一律視為無效（如 stale cache 殘留），回退韓文原文。
        .filter(|value| is_valid_chinese_translation(value))
        .unwrap_or_else(|| sggnm.to_string())
}

fn sejong_admin2(name: &str) -> String {
    match name {
        "보람동" => "寶藍洞",
        "대평동" => "大平洞",
        "다정동" => "多情洞",
        "도담동" => "陶潭洞",
        "고운동" => "高雲洞",
        "종촌동" => "鍾村洞",
        "새롬동" => "賽倫洞",
        "소담동" => "小潭洞",
        "어진동" => "汝珍洞",
        "반곡동" => "盤谷洞",
        "해밀동" => "海密洞",
        "한솔동" => "扞率洞",
        "합강동" => "合江洞",
        "나성동" => "羅城洞",
        "아름동" => "美麗洞",
        "조치원읍" => "鳥致院邑",
        "부강면" => "芙江面",
        "장군면" => "將軍面",
        "연서면" => "燕西面",
        "전의면" => "全義面",
        "전동면" => "全東面",
        "소정면" => "小井面",
        "연기면" => "燕岐面",
        "연동면" => "燕東面",
        "금남면" => "錦南面",
        _ => name,
    }
    .to_string()
}

fn is_sejong_admin2_normalization_target(sidonm: &str, sggnm: &str) -> bool {
    sidonm == "세종특별자치시"
        && !sggnm.ends_with('읍')
        && !sggnm.ends_with('면')
        && !sggnm.ends_with('동')
}

fn split_korea_city_district(name: &str) -> Option<(String, String)> {
    if !(name.ends_with('구') || name.ends_with('군')) {
        return None;
    }
    let city_end = name.find('시')? + '시'.len_utf8();
    if city_end >= name.len() {
        return None;
    }
    Some((name[..city_end].to_string(), name[city_end..].to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn translations_with_admin1(key: &str, value: &str) -> WikidataTranslations {
        let mut translations = WikidataTranslations::default();
        translations
            .admin1_by_name
            .insert(key.to_string(), value.to_string());
        translations
    }

    fn translations_with_admin2(parent: &str, key: &str, value: &str) -> WikidataTranslations {
        let mut translations = WikidataTranslations::default();
        translations.admin2_by_parent.insert(
            parent.to_string(),
            HashMap::from([(key.to_string(), value.to_string())]),
        );
        translations
    }

    #[test]
    fn korea_rejects_non_chinese_translation_and_falls_back_to_original() {
        // 正常：純中文翻譯直接採用。
        let ok = translations_with_admin2("경기도", "수원시", "水原市");
        assert_eq!(korea_admin2("경기도", "수원시", &ok), "水原市");
        // stale cache 殘留英文 label → 回退韓文原文。
        let stale = translations_with_admin2("경기도", "수원시", "Suwon");
        assert_eq!(korea_admin2("경기도", "수원시", &stale), "수원시");
        // 中英夾雜髒翻譯 → 回退韓文原文。
        // Reason: 現有 17 個 admin1 全為 built-in 對照，translations 查表
        //         僅在未知名稱（如未來行政區改制）時生效，故以非 built-in
        //         名稱驗證防禦。
        let mixed = translations_with_admin1("새특별자치도", "新特別Jachi道");
        assert_eq!(korea_admin1("새특별자치도", &mixed), "새특별자치도");
    }

    #[test]
    fn thailand_keeps_official_english_but_rejects_mixed_script() {
        // 純中文翻譯直接採用。
        let ok = translations_with_admin1("Bangkok", "曼谷");
        assert_eq!(thailand_admin1("Bangkok", &ok), "曼谷");
        // 官方英文為設計內的合法回退輸出，不可被純中文判定誤殺。
        let english = translations_with_admin2("Udon Thani", "Ban Dung", "Ban Dung");
        assert_eq!(
            thailand_admin2("Udon Thani", "Ban Dung", &english),
            "Ban Dung"
        );
        // 中英夾雜髒翻譯 → 回退官方英文原文。
        let mixed = translations_with_admin2("Udon Thani", "Ban Dung", "班Dung縣");
        assert_eq!(
            thailand_admin2("Udon Thani", "Ban Dung", &mixed),
            "Ban Dung"
        );
    }
}
