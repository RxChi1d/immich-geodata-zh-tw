use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;

use serde_json::Value;

use super::types::{
    Country, ExtractContext, ExtractRow, Feature, FeatureGeometry, KoreaTranslations,
    korea_translation_source,
};

impl Country {
    pub(super) fn load_context(
        self,
        source_path: &Path,
        korea_stub: Option<&Path>,
    ) -> Result<ExtractContext, String> {
        match self {
            Self::Taiwan | Self::Japan => Ok(ExtractContext::default()),
            Self::Korea => {
                let stub = korea_stub
                    .filter(|path| path.exists())
                    .map(Path::to_path_buf)
                    .or_else(|| korea_translation_source(source_path))
                    .ok_or_else(|| {
                        "KR extract 需要 Wikidata stub/cache，避免 production gate 呼叫即時網路"
                            .to_string()
                    })?;
                Ok(ExtractContext {
                    korea_translations: read_korea_stub(&stub)?,
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

fn clean_admin(value: &str) -> Option<&str> {
    if value.is_empty() || value == "None" || value == "nan" {
        None
    } else {
        Some(value)
    }
}

fn korea_feature_row(
    feature: &Feature,
    translations: &KoreaTranslations,
) -> Result<ExtractRow, String> {
    let (longitude, latitude) = point_geometry(&feature.geometry)?;
    let sidonm = attribute(feature, "sidonm");
    let mut sggnm = attribute(feature, "sggnm").to_string();
    let adm_nm = attribute(feature, "adm_nm");
    let mut admin3 = adm_nm
        .replace(sidonm, "")
        .replace(&sggnm, "")
        .trim()
        .to_string();
    let mut admin4 = String::new();

    if is_sejong_admin2_normalization_target(sidonm, &sggnm) {
        sggnm = admin3;
        admin3 = String::new();
    }

    if let Some((city, district)) = split_korea_city_district(&sggnm) {
        sggnm = city;
        admin4 = admin3;
        admin3 = district;
    }

    let mut admin2 = korea_admin2(sidonm, &sggnm, translations);
    if sidonm == "광주광역시" {
        admin2 = strip_trailing_parenthetical(&admin2);
    }

    Ok(ExtractRow::from_point(
        latitude,
        longitude,
        "南韓",
        korea_admin1(sidonm, translations),
        admin2,
        admin3,
        admin4,
    ))
}

fn korea_admin1(name: &str, translations: &KoreaTranslations) -> String {
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
            .unwrap_or_else(|| name.to_string())
    } else {
        built_in.to_string()
    }
}

fn korea_admin2(sidonm: &str, sggnm: &str, translations: &KoreaTranslations) -> String {
    if sidonm == "세종특별자치시" {
        return sejong_admin2(sggnm);
    }
    translations
        .admin2_by_parent
        .get(&(sidonm.to_string(), sggnm.to_string()))
        .or_else(|| translations.fallback_by_name.get(sggnm))
        .cloned()
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

fn strip_trailing_parenthetical(value: &str) -> String {
    let trimmed = value.trim();
    if !trimmed.ends_with(')') {
        return trimmed.to_string();
    }
    let Some(start) = trimmed.rfind('(') else {
        return trimmed.to_string();
    };
    trimmed[..start].trim_end().to_string()
}

fn read_korea_stub(path: &Path) -> Result<KoreaTranslations, String> {
    let content = fs::read_to_string(path)
        .map_err(|error| format!("無法讀取 KR Wikidata stub {}：{error}", path.display()))?;
    let root: Value = serde_json::from_str(&content)
        .map_err(|error| format!("KR Wikidata JSON 解析失敗 {}：{error}", path.display()))?;
    let entries = root
        .get("translations")
        .and_then(Value::as_object)
        .ok_or_else(|| {
            format!(
                "KR Wikidata stub/cache 缺少 translations：{}",
                path.display()
            )
        })?;
    let mut translations = KoreaTranslations::default();
    for (key, value) in entries {
        let Some(translated) = korea_translation_value(value) else {
            continue;
        };
        if let Some(name) = key.strip_prefix("admin_1/KR/") {
            translations
                .admin1_by_name
                .insert(name.to_string(), translated);
        } else if let Some(rest) = key.strip_prefix("admin_2/KR/") {
            let mut parts = rest.splitn(2, '/');
            if let (Some(parent), Some(name)) = (parts.next(), parts.next()) {
                translations
                    .admin2_by_parent
                    .insert((parent.to_string(), name.to_string()), translated.clone());
                translations
                    .fallback_by_name
                    .insert(name.to_string(), translated);
            }
        } else {
            translations
                .fallback_by_name
                .insert(key.to_string(), translated.clone());
            translations
                .admin1_by_name
                .insert(key.to_string(), translated);
        }
    }
    Ok(translations)
}

fn korea_translation_value(value: &Value) -> Option<String> {
    match value {
        Value::String(value) => Some(value.clone()),
        Value::Object(object) => object
            .get("translated")
            .and_then(Value::as_str)
            .map(ToString::to_string),
        _ => None,
    }
}
