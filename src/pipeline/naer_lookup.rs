//! NAER《外國地名譯名》runtime 匹配：名稱正規化、座標消歧與信心分級。

use unicode_normalization::UnicodeNormalization;
use unicode_normalization::char::is_combining_mark;

/// 剝離括號類註記（圓括號、全形括號、方括號），保留括號外內容。
/// 匹配 key 正規化與 naer-prepare 的中文名清理共用此單一實作，
/// 確保兩側的括號剝離規則永不分歧（單邊修改會使 vendored 檔與
/// 查詢 key 空間不對稱，命中率靜默下降）。
pub fn strip_bracket_annotations(name: &str) -> String {
    let mut kept = String::with_capacity(name.len());
    let mut depth = 0usize;
    for c in name.chars() {
        match c {
            '(' | '（' | '[' | '〔' => depth += 1,
            ')' | '）' | ']' | '〕' => depth = depth.saturating_sub(1),
            _ if depth == 0 => kept.push(c),
            _ => {}
        }
    }
    kept
}

/// 將地名正規化為匹配 key：去除 `[..]`/`(..)` 註記、取逗號前段、
/// NFKD 分解並移除 combining marks（變音符號折疊）、小寫、壓縮空白。
/// 與 naer-prepare 產生 `name_norm` 共用此單一實作。
pub fn normalize_lookup_name(name: &str) -> String {
    let kept = strip_bracket_annotations(name);
    let before_comma = kept.split(',').next().unwrap_or_default();
    let folded: String = before_comma
        .nfkd()
        .filter(|c| !is_combining_mark(*c))
        .collect();
    folded
        .to_lowercase()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// equirectangular 距離近似（公里）。容差層級（15 km / 300 km）不需
/// haversine 精度。
pub fn distance_km(lat1: f64, lon1: f64, lat2: f64, lon2: f64) -> f64 {
    let dlat_km = (lat2 - lat1) * 111.0;
    // Reason: 經度差需正規化跨 ±180°，避免日期變更線兩側被視為相距近 360°。
    let dlon = ((lon2 - lon1) + 180.0).rem_euclid(360.0) - 180.0;
    let dlon_km = dlon * 111.0 * ((lat1 + lat2) / 2.0).to_radians().cos();
    (dlat_km * dlat_km + dlon_km * dlon_km).sqrt()
}

use std::collections::{HashMap, HashSet};
use std::path::Path;

use crate::pipeline::extract;
use crate::pipeline::naer_stats::NaerStats;
use crate::pipeline::table::read_delimited;

/// 城市匹配距離容差（公里）。
// Reason: NAER 座標精度 ±1 角分（約 2 km）＋ GeoNames 城市中心點偏移
// 緩衝；實測此容差下美國同名錯配率趨近 0。
pub const NAER_CITY_DISTANCE_KM: f64 = 15.0;
/// 近距歧義門檻（公里）：容差內多個不同譯名且最近與次近差小於此值
/// 視為歧義，降為中信心。
pub const NAER_AMBIGUITY_MARGIN_KM: f64 = 5.0;
/// admin1 質心驗證門檻（公里）。
// Reason: admin1 幅員大、質心為近似值，門檻放寬；初值待品質報告校準。
pub const NAER_ADMIN1_DISTANCE_KM: f64 = 300.0;

#[derive(Debug, Clone, PartialEq)]
pub struct NaerEntry {
    pub name_zh: String,
    pub country_code: String,
    pub latitude: f64,
    pub longitude: f64,
    pub feature_hint: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NaerConfidence {
    /// 國碼一致、距離合格、非地物、無歧義 → 可覆寫既有譯名。
    High,
    /// 有弱化訊號（空國碼、地物標記、近距歧義）→ 僅補洞。
    Medium,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NaerMatch {
    pub name_zh: String,
    pub confidence: NaerConfidence,
    /// 被採用候選的座標消歧距離（公里），供品質報告距離分布統計。
    pub distance_km: f64,
}

// Reason: 空的查表只在單元測試中用來建構 translate_cities_rows 的參數。
// production 一律經 load() 讀 vendored 檔；若 Default 對 production 可見，
// 誤用會靜默退化成「NAER 完全沒有比對到」而無任何訊號。
#[cfg_attr(test, derive(Default))]
pub struct NaerLookup {
    entries: HashMap<String, Vec<NaerEntry>>,
    handler_countries: HashSet<String>,
}

impl std::fmt::Debug for NaerLookup {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NaerLookup")
            .field("entries_count", &self.entries.len())
            .field("handler_countries", &self.handler_countries)
            .finish()
    }
}

impl NaerLookup {
    pub fn load(path: &Path) -> Result<Self, String> {
        let rows = read_delimited(path, ',', true).map_err(|error| {
            format!(
                "無法載入 NAER 譯名檔 {}：{error}；vendored 檔應隨 repo 存在，\
                 若遺失請以 `cargo run -- naer-prepare --input <原始CSV> --output {}` 重新生成",
                path.display(),
                path.display()
            )
        })?;
        let mut entries: HashMap<String, Vec<NaerEntry>> = HashMap::with_capacity(rows.len());
        for (index, row) in rows.iter().enumerate() {
            let line = index + 2; // 含 header 的人類行號
            if row.len() != 6 {
                return Err(format!(
                    "NAER 譯名檔 {} 第 {line} 行欄位數不符：expected=6 actual={}",
                    path.display(),
                    row.len()
                ));
            }
            let latitude: f64 = row[3].parse().map_err(|error| {
                format!(
                    "NAER 譯名檔 {} 第 {line} 行 latitude 非數值：{error}",
                    path.display()
                )
            })?;
            let longitude: f64 = row[4].parse().map_err(|error| {
                format!(
                    "NAER 譯名檔 {} 第 {line} 行 longitude 非數值：{error}",
                    path.display()
                )
            })?;
            // Reason: 嚴格驗證座標與必填欄位——vendored 檔損毀或手動誤編時及早
            // 失敗（含行號與原因），避免 NaN／越界座標進入距離計算造成靜默錯配。
            if !latitude.is_finite() || !longitude.is_finite() {
                return Err(format!(
                    "NAER 譯名檔 {} 第 {line} 行座標非有限數值：lat={latitude} lon={longitude}",
                    path.display()
                ));
            }
            if !(-90.0..=90.0).contains(&latitude) {
                return Err(format!(
                    "NAER 譯名檔 {} 第 {line} 行 latitude 超出範圍：{latitude}（須 |lat|≤90）",
                    path.display()
                ));
            }
            if !(-180.0..=180.0).contains(&longitude) {
                return Err(format!(
                    "NAER 譯名檔 {} 第 {line} 行 longitude 超出範圍：{longitude}（須 |lon|≤180）",
                    path.display()
                ));
            }
            if row[0].trim().is_empty() {
                return Err(format!(
                    "NAER 譯名檔 {} 第 {line} 行 name_norm 為空",
                    path.display()
                ));
            }
            if row[1].trim().is_empty() {
                return Err(format!(
                    "NAER 譯名檔 {} 第 {line} 行 name_zh 為空",
                    path.display()
                ));
            }
            // Reason: feature_hint 僅允許 "true"/"false"；其他值不可靜默當 false，
            // 否則地物降權標記會在資料污染時失效。
            let feature_hint = match row[5].as_str() {
                "true" => true,
                "false" => false,
                other => {
                    return Err(format!(
                        "NAER 譯名檔 {} 第 {line} 行 feature_hint 非 true/false：{other}",
                        path.display()
                    ));
                }
            };
            // Reason: CSV 的 name_norm 欄位雖已由 naer-prepare 正規化，仍再過一次
            // normalize_lookup_name，確保載入端與查詢端使用完全相同的 key 空間，
            // 防止 vendored 檔手動編輯或字符編碼差異造成不一致。
            let key = normalize_lookup_name(&row[0]);
            entries.entry(key).or_default().push(NaerEntry {
                name_zh: row[1].clone(),
                country_code: row[2].clone(),
                latitude,
                longitude,
                feature_hint,
            });
        }
        // Reason: handler 國家清單由 extract 的 Country enum 單一事實來源導出，
        // 新增 handler 國家時 NAER 跳過邏輯自動同步。
        let handler_countries = extract::handler_country_codes()
            .into_iter()
            .map(str::to_string)
            .collect();
        Ok(Self {
            entries,
            handler_countries,
        })
    }

    fn candidates_for(&self, name: &str, ascii_name: &str) -> Vec<&NaerEntry> {
        let mut keys = vec![normalize_lookup_name(name)];
        let ascii_key = normalize_lookup_name(ascii_name);
        if !keys.contains(&ascii_key) {
            keys.push(ascii_key);
        }
        let mut merged: Vec<&NaerEntry> = Vec::new();
        for key in keys.iter().filter(|key| !key.is_empty()) {
            if let Some(found) = self.entries.get(key.as_str()) {
                for entry in found {
                    if !merged.iter().any(|kept| std::ptr::eq(*kept, entry)) {
                        merged.push(entry);
                    }
                }
            }
        }
        merged
    }

    pub fn lookup_city(
        &self,
        name: &str,
        ascii_name: &str,
        latitude: f64,
        longitude: f64,
        country_code: &str,
        stats: &mut NaerStats,
    ) -> Option<NaerMatch> {
        if self.handler_countries.contains(country_code) {
            return None;
        }
        let candidates = self.candidates_for(name, ascii_name);
        // Reason: 「name 完全無候選」為常態（多數城市不在 NAER 詞典中），
        // 不計入拒絕；只有候選存在卻被消歧規則排除才是品質觀測點。
        let has_candidates = !candidates.is_empty();
        let exact: Vec<&NaerEntry> = candidates
            .iter()
            .copied()
            .filter(|entry| entry.country_code == country_code)
            .collect();
        // 國碼一致者優先；僅在無一致候選時讓空國碼候選參與（降中信心）。
        let (pool, country_demoted) = if exact.is_empty() {
            let empties: Vec<&NaerEntry> = candidates
                .into_iter()
                .filter(|entry| entry.country_code.is_empty())
                .collect();
            (empties, true)
        } else {
            (exact, false)
        };
        // Reason: 有候選但國碼全不符、且無空國碼候選可降級 → 國碼拒絕。
        if pool.is_empty() {
            if has_candidates {
                stats.city_rejected_country += 1;
            }
            return None;
        }
        let mut scored: Vec<(f64, &NaerEntry)> = pool
            .into_iter()
            .map(|entry| {
                (
                    distance_km(latitude, longitude, entry.latitude, entry.longitude),
                    entry,
                )
            })
            .filter(|(distance, _)| *distance <= NAER_CITY_DISTANCE_KM)
            .collect();
        if scored.is_empty() {
            // Reason: pool 非空但全部超出距離容差 → 距離拒絕。
            stats.city_rejected_distance += 1;
            return None;
        }
        scored.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
        let (best_distance, best) = scored[0];
        let ambiguous = scored.iter().skip(1).any(|(distance, entry)| {
            entry.name_zh != best.name_zh && (distance - best_distance) < NAER_AMBIGUITY_MARGIN_KM
        });
        let confidence = if country_demoted || best.feature_hint || ambiguous {
            NaerConfidence::Medium
        } else {
            NaerConfidence::High
        };
        // Reason: 距離分布僅統計「被採用」的匹配，但採用與否由 caller 依
        // 既有譯名是否存在決定（中信心遇既有譯名會 demote 不採用），故
        // 距離經 distance_km 欄位帶回、由 caller 在實際套用時記錄。
        Some(NaerMatch {
            name_zh: best.name_zh.clone(),
            confidence,
            distance_km: best_distance,
        })
    }

    /// admin1 僅補洞模式：caller 只在既有來源無中文名時呼叫。
    pub fn lookup_admin1(
        &self,
        name: &str,
        ascii_name: &str,
        admin1_code: &str,
        centroid: Option<(f64, f64)>,
        stats: &mut NaerStats,
    ) -> Option<String> {
        let mut parts = admin1_code.splitn(2, '.');
        let country_code = parts.next().unwrap_or_default();
        // 畸形 code（無 '.' 或空前綴）安全回 None。
        if country_code.is_empty() || parts.next().is_none() {
            return None;
        }
        if self.handler_countries.contains(country_code) {
            return None;
        }
        // admin1 不接受空國碼候選；國碼一致者方為有效候選。
        let pool: Vec<&NaerEntry> = self
            .candidates_for(name, ascii_name)
            .into_iter()
            .filter(|entry| entry.country_code == country_code)
            .collect();
        // Reason: 國碼一致候選不存在屬常態（多數 admin1 不在詞典中），不計拒絕。
        if pool.is_empty() {
            return None;
        }
        // 無質心（無轄下城市）→ 保守放棄；有候選卻無法驗證計入拒絕。
        let Some((centroid_lat, centroid_lon)) = centroid else {
            stats.admin1_rejected_no_centroid += 1;
            return None;
        };
        let mut scored: Vec<(f64, &NaerEntry)> = pool
            .into_iter()
            .map(|entry| {
                (
                    distance_km(centroid_lat, centroid_lon, entry.latitude, entry.longitude),
                    entry,
                )
            })
            .filter(|(distance, _)| *distance <= NAER_ADMIN1_DISTANCE_KM)
            .collect();
        if scored.is_empty() {
            // Reason: 候選存在但質心驗證全部超距 → 距離拒絕。
            stats.admin1_rejected_distance += 1;
            return None;
        }
        scored.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
        let (best_distance, best) = scored[0];
        // Reason: spec「無法消歧則保守放棄」——若存在次近候選，其譯名與最近者不同
        // 且質心距離差小於歧義門檻，質心無法可靠裁決何者正確（如 colorado →
        // 科羅拉多／科羅拉多州），admin1 僅補洞、寧缺勿錯，回 None。
        let ambiguous = scored.iter().skip(1).any(|(distance, entry)| {
            entry.name_zh != best.name_zh && (distance - best_distance) < NAER_AMBIGUITY_MARGIN_KM
        });
        if ambiguous {
            stats.admin1_rejected_ambiguous += 1;
            return None;
        }
        // Reason: admin1 質心為近似值，距離量級遠大於 city；仍記錄分布以供
        // 校準 NAER_ADMIN1_DISTANCE_KM 門檻（初值待品質報告確立）。
        stats.record_admin1_distance(best_distance);
        Some(best.name_zh.clone())
    }
}

/// 以「未翻譯」的 cities500 列計算各 admin1 的城市質心。
/// key 為 `CC.ADMIN1`，與 admin1CodesASCII row[0] 對齊。
/// 必須在 translate_cities_rows 消費 cities_rows 之前呼叫。
pub fn build_admin1_centroids(cities_rows: &[Vec<String>]) -> HashMap<String, (f64, f64)> {
    let mut sums: HashMap<String, (f64, f64, usize)> = HashMap::new();
    for row in cities_rows {
        if row.len() < 11 || row[8].is_empty() || row[10].is_empty() {
            continue;
        }
        let (Ok(lat), Ok(lon)) = (row[4].parse::<f64>(), row[5].parse::<f64>()) else {
            continue;
        };
        let entry = sums
            .entry(format!("{}.{}", row[8], row[10]))
            .or_insert((0.0, 0.0, 0));
        entry.0 += lat;
        entry.1 += lon;
        entry.2 += 1;
    }
    sums.into_iter()
        .map(|(key, (lat, lon, count))| (key, (lat / count as f64, lon / count as f64)))
        .collect()
}
