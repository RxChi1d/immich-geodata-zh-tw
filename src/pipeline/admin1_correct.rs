//! LocationIQ admin1 修正器。
//!
//! 把 LocationIQ 逆地理查詢回傳的一級行政區名稱，對照 Natural Earth 的
//! admin-1 邊界，學出「LocationIQ 行政區名稱 → GeoNames admin1 代碼」的映射，
//! 用來修正 GeoNames 標錯行政區的城鎮。
//!
//! # 映射為什麼要自己學
//!
//! LocationIQ 回傳的是當地語言／中文的行政區名稱，與 `admin1CodesASCII` 的
//! ASCII 名稱無法可靠對名（例如「檳城」對 `Penang`，NE 則寫 `Pulau Pinang`）。
//! 改以「點落在哪個 NE 多邊形」決定代碼，再統計每個名稱最常落在哪個代碼上。
//!
//! Reason: 學習過程只吃座標與 LocationIQ 名稱，**完全不讀 cities500 既有的
//! admin1 欄位**。若讀了，等於拿待驗證的資料去驗證自己——GeoNames 標錯的點
//! 會把錯誤的對應餵回映射，讓修正器學會維持現狀。這條不變量由
//! [`PointSample`] 的型別保證：它根本沒有承載 cities500 admin1 的欄位。

use std::collections::BTreeMap;

use crate::pipeline::ne_admin1::{NeAdmin1Index, NeHit};

/// 建立映射所需的最少已解析樣本數。
const MIN_RESOLVED_SAMPLES: usize = 20;

/// 非第一名代碼的容許上限（1/20 = 5%），達到即視為該名稱歧義過大。
const NOISE_DENOMINATOR: usize = 20;

/// 學習映射的單一樣本。
///
/// Reason: 刻意只有三個欄位。不變量「學習不讀 cities500 admin1」在型別層就
/// 成立——沒有欄位可以承載它，未來修改也無法在不改型別的情況下偷渡進來。
#[derive(Debug, Clone, PartialEq)]
pub struct PointSample {
    pub longitude: f64,
    pub latitude: f64,
    pub locationiq_admin1: String,
}

/// 某個 LocationIQ 名稱未能建立可信映射的原因。
#[derive(Debug, Clone, PartialEq)]
pub enum MappingRejection {
    /// 已解析樣本不足。
    TooFewSamples { resolved: usize },
    /// 非第一名代碼占比達到門檻，名稱歧義過大。
    TooNoisy {
        ratio: f64,
        resolved: usize,
        top_code: String,
    },
}

/// 單一 LocationIQ 名稱的學習統計。
#[derive(Debug, Clone)]
pub struct NameStats {
    pub name: String,
    /// NE 唯一命中且該 gn_id 查得到 admin1 代碼的樣本數。
    pub resolved: usize,
    /// NE 無命中、多重命中，或 gn_id 查不到代碼的樣本數。
    pub unresolved: usize,
    pub counts_by_code: BTreeMap<String, usize>,
    pub rejection: Option<MappingRejection>,
}

/// 學習結果：可信映射與完整統計。
#[derive(Debug, Clone)]
pub struct LearnedMapping {
    trusted: BTreeMap<String, String>,
    stats: BTreeMap<String, NameStats>,
}

impl LearnedMapping {
    /// 取得某個 LocationIQ 名稱的可信 admin1 代碼。
    pub fn code_for(&self, name: &str) -> Option<&str> {
        self.trusted.get(name).map(String::as_str)
    }

    /// 取得某個名稱未能建立映射的原因。
    pub fn rejection_for(&self, name: &str) -> Option<MappingRejection> {
        self.stats
            .get(name)
            .and_then(|stats| stats.rejection.clone())
    }

    /// 取得某個名稱無法解析的樣本數。
    pub fn unresolved_for(&self, name: &str) -> usize {
        self.stats.get(name).map_or(0, |stats| stats.unresolved)
    }

    /// 全部名稱的統計，依名稱排序。
    pub fn stats(&self) -> impl Iterator<Item = &NameStats> {
        self.stats.values()
    }

    /// 可信映射的 (名稱, 代碼) 配對，依名稱排序。
    pub fn into_sorted_pairs(self) -> Vec<(String, String)> {
        self.trusted.into_iter().collect()
    }

    pub fn trusted_len(&self) -> usize {
        self.trusted.len()
    }
}

/// 從樣本學出「LocationIQ 行政區名稱 → admin1 代碼」的可信映射。
///
/// `gn_id_to_code` 來自 `admin1CodesASCII`（第 4 欄 geonameid → 第 1 欄代碼）。
/// 查不到代碼的 gn_id 一律計入 `unresolved`，絕不自造代碼。
pub fn learn_admin1_mapping(
    samples: &[PointSample],
    index: &NeAdmin1Index,
    gn_id_to_code: &BTreeMap<i64, String>,
) -> LearnedMapping {
    let mut stats: BTreeMap<String, NameStats> = BTreeMap::new();

    for sample in samples {
        let entry = stats
            .entry(sample.locationiq_admin1.clone())
            .or_insert_with(|| NameStats {
                name: sample.locationiq_admin1.clone(),
                resolved: 0,
                unresolved: 0,
                counts_by_code: BTreeMap::new(),
                rejection: None,
            });

        // Reason: 只有「唯一命中」才算數。多重命中代表 NE 多邊形重疊，
        // 無法判定該點屬於哪一區；無命中代表 NE 覆蓋不到。兩者都不該
        // 為任何代碼投票，否則會把不確定性當成證據。
        let code = match index.locate(sample.longitude, sample.latitude) {
            NeHit::Unique { gn_id, .. } => gn_id_to_code.get(&gn_id),
            NeHit::None | NeHit::Multiple(_) => None,
        };
        match code {
            Some(code) => {
                entry.resolved += 1;
                *entry.counts_by_code.entry(code.clone()).or_insert(0) += 1;
            }
            None => entry.unresolved += 1,
        }
    }

    let mut trusted = BTreeMap::new();
    for entry in stats.values_mut() {
        match evaluate(entry) {
            Ok(code) => {
                trusted.insert(entry.name.clone(), code);
            }
            Err(rejection) => entry.rejection = Some(rejection),
        }
    }

    LearnedMapping { trusted, stats }
}

/// 判定單一名稱能否建立可信映射，回傳代碼或拒絕原因。
fn evaluate(stats: &NameStats) -> Result<String, MappingRejection> {
    if stats.resolved < MIN_RESOLVED_SAMPLES {
        return Err(MappingRejection::TooFewSamples {
            resolved: stats.resolved,
        });
    }

    // Reason: BTreeMap 依代碼字典序迭代，配合「嚴格大於才替換」，同票時固定
    // 取字典序最小的代碼。若用 HashMap 或 `>=`，同票會讓同一份輸入在不同
    // 執行產生不同映射。
    let (top_code, top_count) = stats
        .counts_by_code
        .iter()
        .fold(
            None,
            |best: Option<(&String, usize)>, (code, count)| match best {
                Some((_, best_count)) if best_count >= *count => best,
                _ => Some((code, *count)),
            },
        )
        .expect("resolved >= MIN_RESOLVED_SAMPLES 時 counts_by_code 不可能為空");

    let non_top = stats.resolved - top_count;
    // Reason: 以整數比較取代浮點數——`non_top / resolved >= 1/20` 等價於
    // `non_top * 20 >= resolved`，避開 5% 這類二進位無法精確表示的門檻在
    // 邊界值上的不確定性。
    if non_top * NOISE_DENOMINATOR >= stats.resolved {
        return Err(MappingRejection::TooNoisy {
            ratio: non_top as f64 / stats.resolved as f64,
            resolved: stats.resolved,
            top_code: top_code.clone(),
        });
    }

    Ok(top_code.clone())
}

/// 待評估的城鎮點。
#[derive(Debug, Clone, PartialEq)]
pub struct CityPoint {
    pub geoname_id: String,
    pub name: String,
    pub longitude: f64,
    pub latitude: f64,
    pub country_code: String,
    /// cities500 現有的 admin1 代碼，不含國碼前綴（例如 `12`）。
    pub original_admin1: String,
    /// LocationIQ 回傳的一級行政區名稱。
    pub locationiq_admin1: Option<String>,
}

/// 候選點的裁決結果。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    Accepted,
    Rejected,
}

/// 候選點被拒絕的原因。
///
/// 變體順序即主因優先序：愈前面愈根本，`primary_rank` 依此排序。
#[derive(Debug, Clone, PartialEq)]
pub enum RejectReason {
    /// 原 admin1 代碼不在 `admin1CodesASCII`（既有缺陷，只回報不修正）。
    OriginalCodeUnknown { code: String },
    /// 該點沒有 LocationIQ 的行政區資料。
    NoLocationiqAdmin1,
    /// LocationIQ 名稱未建立可信映射。
    NoTrustedMapping,
    /// NE 無命中。
    NaturalEarthNoHit,
    /// NE 多重命中。
    NaturalEarthMultipleHit { gn_ids: Vec<i64> },
    /// NE 與 LocationIQ 未指向同一代碼。
    SourcesDoNotAgree {
        natural_earth: String,
        locationiq: String,
    },
    /// NE 命中的多邊形屬於鄰國，代碼去不掉本國前綴。
    NaturalEarthForeignCountry { code: String },
}

impl RejectReason {
    fn primary_rank(&self) -> u8 {
        match self {
            Self::OriginalCodeUnknown { .. } => 0,
            Self::NoLocationiqAdmin1 => 1,
            Self::NoTrustedMapping => 2,
            Self::NaturalEarthNoHit => 3,
            Self::NaturalEarthMultipleHit { .. } => 4,
            Self::SourcesDoNotAgree { .. } => 5,
            // Reason: 排在 SourcesDoNotAgree 之後。邊境點通常兩個原因都成立，
            // 而「兩來源不一致」對判讀更有用——它同時告訴你兩邊各自說了什麼。
            Self::NaturalEarthForeignCountry { .. } => 6,
        }
    }
}

/// 一筆候選：任一可判定來源與原值不符的點。
#[derive(Debug, Clone)]
pub struct Candidate {
    pub geoname_id: String,
    pub name: String,
    pub longitude: f64,
    pub latitude: f64,
    pub country_code: String,
    pub original_admin1: String,
    /// NE 唯一命中所對應的 admin1 代碼（完整鍵，例如 `MY.14`）。
    pub natural_earth_code: Option<String>,
    /// 該點到 NE 多邊形邊界的最短距離（公里）。
    ///
    /// Reason: 純診斷欄位，不參與裁決。曾有一版謂詞要求「距邊界 ≥2km」，
    /// 實測在 MY 上只砍掉真正的修正（Setapak、SS2、Bandar Utama 等 6 筆，
    /// 其中 4 筆已人工查核為真實上游錯誤），而它想擋的 9 筆「NE 獨排眾議」
    /// 全部已由 `SourcesDoNotAgree` 攔下——GeoNames 標錯行政區的地方本來就
    /// 集中在邊界，用「離邊界遠」當安全條件等於排除這個功能存在的理由。
    /// 保留數值供報表診斷用。
    pub boundary_km: Option<f64>,
    pub locationiq_admin1: Option<String>,
    /// LocationIQ 名稱經可信映射得到的 admin1 代碼（完整鍵）。
    pub locationiq_code: Option<String>,
    pub verdict: Verdict,
    /// 採納時要寫入的新 admin1 代碼，不含國碼前綴。
    pub corrected_admin1: Option<String>,
    /// 全部適用的拒絕原因，主因在前。採納時為空。
    pub reasons: Vec<RejectReason>,
}

/// 評估所有城鎮點，回傳依 `geoname_id` 排序的候選清單。
///
/// 只有「任一可判定來源與原值不符」的點會成為候選。兩來源都同意原值、或兩者
/// 皆無法判定的點不進清單。
///
/// Reason: 一列該進報表，當且僅當它的裁決結果可能在週與週之間翻轉。同意原值的
/// 點要先變成分歧列才可能出事，而那本身就是一列新增；把它們全部收進來只會讓
/// 真正該看的列淹沒在無事發生的雜訊裡。
pub fn evaluate_points(
    points: &[CityPoint],
    index: &NeAdmin1Index,
    mapping: &LearnedMapping,
    gn_id_to_code: &BTreeMap<i64, String>,
    known_admin1_codes: &BTreeMap<String, String>,
) -> Vec<Candidate> {
    let mut candidates: Vec<Candidate> = points
        .iter()
        .filter_map(|point| {
            evaluate_point(point, index, mapping, gn_id_to_code, known_admin1_codes)
        })
        .collect();
    // Reason: 輸出檔案要能跨週 diff，排序必須與輸入順序無關。
    candidates.sort_by(|left, right| left.geoname_id.cmp(&right.geoname_id));
    candidates
}

fn evaluate_point(
    point: &CityPoint,
    index: &NeAdmin1Index,
    mapping: &LearnedMapping,
    gn_id_to_code: &BTreeMap<i64, String>,
    known_admin1_codes: &BTreeMap<String, String>,
) -> Option<Candidate> {
    let original_key = format!("{}.{}", point.country_code, point.original_admin1);
    let mut reasons = Vec::new();

    let hit = index.locate(point.longitude, point.latitude);
    let (natural_earth_code, boundary_km) = match &hit {
        NeHit::Unique { gn_id, boundary_km } => {
            (gn_id_to_code.get(gn_id).cloned(), Some(*boundary_km))
        }
        NeHit::None => {
            reasons.push(RejectReason::NaturalEarthNoHit);
            (None, None)
        }
        NeHit::Multiple(gn_ids) => {
            reasons.push(RejectReason::NaturalEarthMultipleHit {
                gn_ids: gn_ids.clone(),
            });
            (None, None)
        }
    };
    // Reason: NE 命中了，但該 gn_id 在 admin1CodesASCII 沒有對應列。上游沒有
    // 這個行政區的記錄就只能跳過——geonameid 是全球共用識別碼，自造代碼會撞上
    // 上游現有或日後新增的 ID，讓同一州在 Immich 裡裂成兩個。
    if matches!(hit, NeHit::Unique { .. }) && natural_earth_code.is_none() {
        reasons.push(RejectReason::NaturalEarthNoHit);
    }

    let locationiq_code = match point.locationiq_admin1.as_deref() {
        None => {
            reasons.push(RejectReason::NoLocationiqAdmin1);
            None
        }
        Some(name) => match mapping.code_for(name) {
            Some(code) => Some(code.to_string()),
            None => {
                reasons.push(RejectReason::NoTrustedMapping);
                None
            }
        },
    };

    let natural_earth_differs = natural_earth_code
        .as_deref()
        .is_some_and(|code| code != original_key);
    let locationiq_differs = locationiq_code
        .as_deref()
        .is_some_and(|code| code != original_key);
    if !natural_earth_differs && !locationiq_differs {
        return None;
    }

    if !known_admin1_codes.contains_key(&original_key) {
        reasons.push(RejectReason::OriginalCodeUnknown {
            code: original_key.clone(),
        });
    }
    if let (Some(ne_code), Some(liq_code)) = (&natural_earth_code, &locationiq_code)
        && ne_code != liq_code
    {
        reasons.push(RejectReason::SourcesDoNotAgree {
            natural_earth: ne_code.clone(),
            locationiq: liq_code.clone(),
        });
    }

    // Reason: 沒有這一條，「NE 命中鄰國且兩來源一致」會產生一列 verdict=rejected
    // 但 reasons 空白的紀錄——看得到被拒絕，卻讀不出為什麼。這種無聲缺口正是
    // 這份紀錄檔要消除的東西。
    if let Some(code) = natural_earth_code.as_deref()
        && !code.starts_with(&format!("{}.", point.country_code))
    {
        reasons.push(RejectReason::NaturalEarthForeignCountry {
            code: code.to_string(),
        });
    }

    reasons.sort_by_key(RejectReason::primary_rank);

    // Reason: NE 會把邊境點判進鄰國的多邊形（實測 Bukit Kayu Hitam 落在 TH.68）。
    // 代碼去不掉本國前綴時就沒有可寫入的值，此時絕不能標成 accepted——否則報表的
    // accepted 數會多於實際寫入數，而那個差額沒有任何訊號。
    let corrected_admin1 = natural_earth_code
        .as_deref()
        .and_then(|code| code.strip_prefix(&format!("{}.", point.country_code)))
        .map(str::to_string)
        .filter(|_| reasons.is_empty() && natural_earth_differs);
    let accepted = corrected_admin1.is_some();

    Some(Candidate {
        geoname_id: point.geoname_id.clone(),
        name: point.name.clone(),
        longitude: point.longitude,
        latitude: point.latitude,
        country_code: point.country_code.clone(),
        original_admin1: point.original_admin1.clone(),
        natural_earth_code,
        boundary_km,
        locationiq_admin1: point.locationiq_admin1.clone(),
        locationiq_code,
        verdict: if accepted {
            Verdict::Accepted
        } else {
            Verdict::Rejected
        },
        corrected_admin1,
        reasons,
    })
}
