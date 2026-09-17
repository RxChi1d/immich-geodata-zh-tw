//! cities500 與 admin1 的列結構，以及逐列翻譯規則。
//!
//! 自 `translate.rs` 拆出——城市名的來源優先序（GeoNames 中文別名 →
//! LocationIQ metadata → alternatenames 內的中文）就寫在這裡。

use std::collections::{HashMap, HashSet};

use crate::pipeline::naer_lookup::{NaerConfidence, NaerLookup};
use crate::pipeline::naer_stats::NaerStats;

use super::opencc::*;

pub(super) type AlternateLookup = HashMap<String, String>;

pub(super) struct MetadataLookup {
    countries: HashSet<String>,
    names_by_coordinate: HashMap<(String, String, String), String>,
}

impl MetadataLookup {
    pub(super) fn with_capacity(capacity: usize) -> Self {
        Self {
            countries: HashSet::new(),
            names_by_coordinate: HashMap::with_capacity(capacity),
        }
    }

    pub(super) fn insert_first(
        &mut self,
        country_code: &str,
        latitude: &str,
        longitude: &str,
        name: &str,
    ) {
        self.countries.insert(country_code.to_string());
        self.names_by_coordinate
            .entry((
                country_code.to_string(),
                latitude.to_string(),
                longitude.to_string(),
            ))
            .or_insert_with(|| name.to_string());
    }

    pub(super) fn get_city_name(&self, city: &CityRow) -> Option<&str> {
        if !self.countries.contains(city.country_code()) {
            return None;
        }
        self.names_by_coordinate
            .get(&city.metadata_key())
            .map(String::as_str)
    }
}

pub(super) struct CityRow {
    row: Vec<String>,
}

impl CityRow {
    pub(super) fn try_from_row(row: Vec<String>) -> Result<Self, String> {
        if row.len() != 19 {
            return Err(format!(
                "cities500 欄位數不符：expected=19 actual={}",
                row.len()
            ));
        }
        Ok(Self { row })
    }

    pub(super) fn geoname_id(&self) -> &str {
        &self.row[0]
    }

    pub(super) fn name(&self) -> &str {
        &self.row[1]
    }

    pub(super) fn asciiname(&self) -> &str {
        &self.row[2]
    }

    pub(super) fn alternatenames(&self) -> &str {
        &self.row[3]
    }

    pub(super) fn latitude(&self) -> &str {
        &self.row[4]
    }

    pub(super) fn longitude(&self) -> &str {
        &self.row[5]
    }

    pub(super) fn country_code(&self) -> &str {
        &self.row[8]
    }

    pub(super) fn metadata_key(&self) -> (String, String, String) {
        (
            self.country_code().to_string(),
            self.latitude().to_string(),
            self.longitude().to_string(),
        )
    }

    pub(super) fn apply_name(&mut self, name: String) {
        self.row[1] = name;
        self.row[2] = self.row[1].clone();
    }

    pub(super) fn sync_asciiname(&mut self) {
        self.row[2] = self.row[1].clone();
    }

    pub(super) fn into_row(self) -> Vec<String> {
        self.row
    }
}

pub(super) struct Admin1Row {
    row: Vec<String>,
}

impl Admin1Row {
    pub(super) fn try_from_row(row: Vec<String>) -> Result<Self, String> {
        if row.len() != 4 {
            return Err(format!(
                "admin1 欄位數不符：expected=4 actual={}",
                row.len()
            ));
        }
        Ok(Self { row })
    }

    pub(super) fn geoname_id(&self) -> &str {
        &self.row[3]
    }

    pub(super) fn code(&self) -> &str {
        &self.row[0]
    }

    pub(super) fn name(&self) -> &str {
        &self.row[1]
    }

    pub(super) fn asciiname(&self) -> &str {
        &self.row[2]
    }

    pub(super) fn apply_name(&mut self, name: String) {
        self.row[1] = name;
        self.row[2] = self.row[1].clone();
    }

    pub(super) fn sync_asciiname(&mut self) {
        self.row[2] = self.row[1].clone();
    }

    pub(super) fn into_row(self) -> Vec<String> {
        self.row
    }
}

pub(super) fn translate_cities_rows(
    rows: Vec<Vec<String>>,
    metadata: &MetadataLookup,
    alternate_names: &AlternateLookup,
    converter: &OpenCcConverter,
    naer: &NaerLookup,
    naer_stats: &mut NaerStats,
) -> Result<Vec<Vec<String>>, String> {
    let mut translated = Vec::with_capacity(rows.len());
    for row in rows {
        let mut city = CityRow::try_from_row(row)?;
        let mut naer_applied = false;
        let final_name = if city.country_code() == "TW" {
            Some(city.name().to_string())
        } else {
            // Reason: GeoNames 中文別名對應的是城市本身，LocationIQ metadata 回的是
            // Nominatim 的 `city`／`county`——在聚落標記稀疏處會退回轄區，把城市名
            // 塌成上一層（馬來西亞實測：蕉賴→吉隆坡、浮羅山背→喬治市）。因此
            // metadata 只作為「GeoNames 沒有中文名時」的補位來源，不再覆蓋既有譯名。
            // 此優先序只影響非 handler 國家：TW/JP/KR/TH/ID 的資料由 handler 寫入
            // cities500，不會進到 metadata lookup。
            //
            // Reason: 「塌成上一層」指的是**跨出該座標所屬的 city 單位**，不是
            // 「名字不等於該座標自己的名字」。多個座標共用一個 city 名是 handler
            // 的常態（印尼平均 211 點共用一個 kabupaten 名），不是缺陷。city 層級
            // 的判準與驗收指標見 `data/locationiq/README.md` 的「city 應該放哪一個
            // 行政層級」。
            let existing = alternate_names
                .get(city.geoname_id())
                .filter(|value| !value.is_empty())
                .map(|name| translate_alternate_name(name, converter))
                .or_else(|| {
                    metadata
                        .get_city_name(&city)
                        .filter(|value| !value.is_empty())
                        .and_then(|name| translate_metadata_name(name, converter))
                })
                .or_else(|| extract_chinese_name(city.alternatenames(), converter));
            let naer_match = match (
                city.latitude().parse::<f64>(),
                city.longitude().parse::<f64>(),
            ) {
                (Ok(latitude), Ok(longitude)) => naer.lookup_city(
                    city.name(),
                    city.asciiname(),
                    latitude,
                    longitude,
                    city.country_code(),
                    naer_stats,
                ),
                _ => None,
            };
            match naer_match {
                Some(matched) if matched.confidence == NaerConfidence::High => {
                    if existing.is_some() {
                        naer_stats.city_override += 1;
                    } else {
                        naer_stats.city_fill += 1;
                    }
                    // Reason: 距離分布僅統計「被採用」的匹配；demote 保留
                    // 既有譯名時不記錄，避免污染品質報告的採用距離語意。
                    naer_stats.record_city_distance(matched.distance_km);
                    naer_applied = true;
                    Some(matched.name_zh)
                }
                Some(matched) => {
                    if existing.is_none() {
                        naer_stats.city_fill += 1;
                        naer_stats.record_city_distance(matched.distance_km);
                        naer_applied = true;
                        Some(matched.name_zh)
                    } else {
                        naer_stats.city_demoted_kept_existing += 1;
                        existing
                    }
                }
                None => existing,
            }
        };

        if let Some(name) = final_name {
            if naer_applied {
                // Reason: NAER 為官方審譯結果，原樣使用、不經 '裏'→'里' 後處理。
                city.apply_name(name);
            } else {
                city.apply_name(name.replacen('裏', "里", 1));
            }
        } else {
            city.sync_asciiname();
        }
        if !city.name().is_empty() {
            translated.push(city.into_row());
        }
    }
    Ok(translated)
}

pub(super) fn translate_admin1_rows(
    rows: Vec<Vec<String>>,
    alternate_names: &AlternateLookup,
    converter: &OpenCcConverter,
    naer: &NaerLookup,
    admin1_centroids: &HashMap<String, (f64, f64)>,
    naer_stats: &mut NaerStats,
) -> Result<Vec<Vec<String>>, String> {
    let mut translated = Vec::with_capacity(rows.len());
    for row in rows {
        let mut admin1 = Admin1Row::try_from_row(row)?;
        if let Some(name) = alternate_names
            .get(admin1.geoname_id())
            .filter(|value| !value.is_empty())
        {
            let translated_name = if is_simplified_chinese(name, converter) {
                converter.s2t(name)
            } else {
                name.clone()
            };
            admin1.apply_name(translated_name);
        } else if let Some(name) = naer.lookup_admin1(
            admin1.name(),
            admin1.asciiname(),
            admin1.code(),
            admin1_centroids.get(admin1.code()).copied(),
            naer_stats,
        ) {
            // Reason: admin1 第一版僅補洞——只在既有來源無中文名時使用
            // NAER，覆寫待品質報告量化錯配率後再評估。
            naer_stats.admin1_fill += 1;
            admin1.apply_name(name);
        } else {
            admin1.sync_asciiname();
        }
        translated.push(admin1.into_row());
    }
    Ok(translated)
}

/// 測試用薄包裝：把 HashMap 形式的測試資料轉成 production 型別後，直接呼叫
/// `translate_cities_rows`。
///
/// Reason: 此處原本複製了一份優先序邏輯，導致 production 改為「metadata 補位」
/// 後，斷言舊行為的測試仍然通過。包裝只做型別轉換，不再重述任何規則。
#[cfg(test)]
pub(super) fn translate_cities(
    rows: &mut Vec<Vec<String>>,
    metadata: &HashMap<(String, String, String), String>,
    alternate_names: &AlternateLookup,
    converter: &OpenCcConverter,
) {
    let mut lookup = MetadataLookup::with_capacity(metadata.len());
    for ((country_code, latitude, longitude), name) in metadata {
        lookup.insert_first(country_code, latitude, longitude, name);
    }
    let naer = NaerLookup::default();
    let mut naer_stats = NaerStats::default();
    *rows = translate_cities_rows(
        std::mem::take(rows),
        &lookup,
        alternate_names,
        converter,
        &naer,
        &mut naer_stats,
    )
    .unwrap();
}

#[cfg(test)]
pub(super) fn translate_admin1(
    rows: &mut [Vec<String>],
    alternate_names: &HashMap<String, String>,
    converter: &OpenCcConverter,
) {
    for row in rows {
        if let Some(name) = alternate_names
            .get(&row[3])
            .filter(|value| !value.is_empty())
        {
            let translated = if is_simplified_chinese(name, converter) {
                converter.s2t(name)
            } else {
                name.clone()
            };
            row[1] = translated.clone();
            row[2] = translated;
        }
        row[2] = row[1].clone();
    }
}
