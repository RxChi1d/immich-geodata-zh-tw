//! Natural Earth 10m admin-1 邊界索引。
//!
//! 提供「座標 → NE 行政區 `gn_id`」的點位查詢，供 LocationIQ admin1 修正器
//! 判定某個城鎮實際落在哪個一級行政區內。
//!
//! # 為什麼用 `gn_id` 而不是 `gn_a1_code`
//!
//! NE 的 feature 另有 `gn_a1_code` 欄位直接寫著行政區代碼（例如 `MY.14`），
//! 看似可省去與 `admin1CodesASCII` 的對接。實測全球有 42 筆與
//! `admin1CodesASCII` 的權威代碼不一致（越南最嚴重，例如 NE 標 `VN.58`
//! 而權威值為 `VN.40`）。
//!
//! Reason: `gn_id` 是 GeoNames 的永久識別碼，與 `admin1CodesASCII` 第 4 欄
//! 精確對接；`gn_a1_code` 是 NE 自行維護的副本，會隨上游改碼而過期。用 ID
//! 對接還有一個好處：上游把行政區「名稱」寫錯不影響對接結果。

use std::collections::BTreeMap;
use std::fmt;
use std::fs;
use std::path::Path;

use geo::algorithm::bounding_rect::BoundingRect;
use geo::algorithm::closest_point::ClosestPoint;
use geo::algorithm::contains::Contains;
use geo::{Closest, Coord, Distance, Haversine, LineString, MultiPolygon, Point, Polygon, Rect};
use serde::Deserialize;
use serde::de::{self, IgnoredAny, MapAccess, SeqAccess, Visitor};
use serde_json::Value;

/// 點位查詢結果。
#[derive(Debug, Clone, PartialEq)]
pub enum NeHit {
    /// 只落在一個行政區內。`boundary_km` 為該點到該區邊界的最短距離（公里）。
    Unique { gn_id: i64, boundary_km: f64 },
    /// 不落在任何行政區內（離島、海岸線外側，或該國不在 NE 的 admin-1 覆蓋範圍）。
    None,
    /// 落在多個行政區內（NE 多邊形重疊）。`gn_id` 已排序以確保可重現。
    Multiple(Vec<i64>),
}

/// 依 `gn_id` 聚合的 NE admin-1 多邊形索引。
pub struct NeAdmin1Index {
    /// Reason: 用 `BTreeMap` 而非 `HashMap`——迭代順序決定 `NeHit::Multiple`
    /// 的內容順序與錯誤訊息的行序，雜湊順序會讓同一份輸入在不同執行產生
    /// 不同輸出。
    regions: BTreeMap<i64, Region>,
}

struct Region {
    geometry: MultiPolygon<f64>,
    /// Reason: NE 10m 有 4,596 個 feature，逐點對全部多邊形做
    /// point-in-polygon 是 O(點數 × 頂點總數)。外接矩形先篩掉絕大多數候選，
    /// 只有少數真正可能命中的區域才進入精確判定。
    bounds: Rect<f64>,
}

impl NeAdmin1Index {
    /// 從檔案載入 NE admin-1 GeoJSON。
    pub fn load(path: &Path) -> Result<Self, String> {
        let content = fs::read_to_string(path)
            .map_err(|error| format!("無法讀取 NE admin-1 GeoJSON {}：{error}", path.display()))?;
        Self::from_geojson_str(&content)
    }

    /// 從 GeoJSON 內容建立索引。
    ///
    /// 缺少 `gn_id`、`gn_id` 非正整數，或幾何不是 Polygon／MultiPolygon 的
    /// feature 會被略過。
    ///
    /// Reason: NE 以 `null`、`0` 或 `-99` 表示缺值，這些值在
    /// `admin1CodesASCII` 沒有對應列。若當成有效 `gn_id` 收下，後續對接會
    /// 查不到代碼而無聲產生零個映射，比在此略過更難診斷。
    pub fn from_geojson_str(content: &str) -> Result<Self, String> {
        let collection: FeatureCollection = serde_json::from_str(content)
            .map_err(|error| format!("NE admin-1 GeoJSON 解析失敗：{error}"))?;
        let features = collection
            .features
            .ok_or_else(|| "NE admin-1 GeoJSON 缺少 features 陣列".to_string())?;

        let mut polygons_by_id: BTreeMap<i64, Vec<Polygon<f64>>> = BTreeMap::new();
        for feature in features {
            let Some(gn_id) = feature
                .properties
                .and_then(|properties| properties.gn_id)
                .as_ref()
                .and_then(Value::as_i64)
                .filter(|value| *value > 0)
            else {
                continue;
            };
            let Some(geometry) = feature.geometry else {
                continue;
            };
            let polygons = polygons_from_geometry(geometry)?;
            if polygons.is_empty() {
                continue;
            }
            polygons_by_id.entry(gn_id).or_default().extend(polygons);
        }

        let regions = polygons_by_id
            .into_iter()
            .filter_map(|(gn_id, polygons)| {
                let geometry = MultiPolygon::new(polygons);
                geometry
                    .bounding_rect()
                    .map(|bounds| (gn_id, Region { geometry, bounds }))
            })
            .collect();
        Ok(Self { regions })
    }

    /// 查詢座標落在哪個行政區內。
    pub fn locate(&self, longitude: f64, latitude: f64) -> NeHit {
        let point = Point::new(longitude, latitude);
        let hits: Vec<i64> = self
            .regions
            .iter()
            .filter(|(_, region)| region.bounds.contains(&point))
            .filter(|(_, region)| region.geometry.contains(&point))
            .map(|(gn_id, _)| *gn_id)
            .collect();

        match hits.len() {
            0 => NeHit::None,
            1 => {
                let gn_id = hits[0];
                let boundary_km = self
                    .regions
                    .get(&gn_id)
                    .map(|region| boundary_distance_km(&region.geometry, &point))
                    .unwrap_or(0.0);
                NeHit::Unique { gn_id, boundary_km }
            }
            _ => NeHit::Multiple(hits),
        }
    }

    /// 索引中的行政區數量。
    pub fn len(&self) -> usize {
        self.regions.len()
    }

    pub fn is_empty(&self) -> bool {
        self.regions.is_empty()
    }
}

/// 計算點到多邊形邊界（外環與所有內環）的最短距離，單位公里。
///
/// Reason: 直接對 `Polygon` 呼叫 `closest_point` 會在點位於內部時回傳
/// `Intersection`（距離 0），那是「到多邊形」的距離而非「到邊界」的距離。
/// 修正器要判斷的是「離邊界夠不夠遠」，所以必須逐環計算。
fn boundary_distance_km(geometry: &MultiPolygon<f64>, point: &Point<f64>) -> f64 {
    let mut nearest = f64::INFINITY;
    for polygon in geometry {
        let rings = std::iter::once(polygon.exterior()).chain(polygon.interiors());
        for ring in rings {
            if let Some(distance) = ring_distance_km(ring, point) {
                nearest = nearest.min(distance);
            }
        }
    }
    if nearest.is_finite() { nearest } else { 0.0 }
}

fn ring_distance_km(ring: &LineString<f64>, point: &Point<f64>) -> Option<f64> {
    match ring.closest_point(point) {
        Closest::Intersection(closest) | Closest::SinglePoint(closest) => {
            Some(Haversine.distance(*point, closest) / 1000.0)
        }
        // Reason: `Indeterminate` 只在退化幾何（例如零長度環）出現，無法給出
        // 有意義的距離。回傳 None 讓呼叫端改用其他環的結果，而不是誤報 0 km
        // ——0 km 會讓該點被「距邊界 <2km」規則拒絕，把幾何瑕疵偽裝成安全判斷。
        Closest::Indeterminate => None,
    }
}

/// GeoJSON 的 `FeatureCollection` 外殼。
///
/// Reason: `features` 以 `Option` 接住，缺欄位時才能回傳專案自訂的中文訊息，
/// 而不是 serde 的 `missing field \`features\``。
#[derive(Deserialize)]
struct FeatureCollection {
    #[serde(default)]
    features: Option<Vec<Feature>>,
}

#[derive(Deserialize)]
struct Feature {
    #[serde(default)]
    properties: Option<Properties>,
    #[serde(default)]
    geometry: Option<Geometry>,
}

/// NE feature 的屬性；除 `gn_id` 外一律略過。
///
/// Reason: `gn_id` 以 `Value` 而非 `i64` 接住，才能保留舊版
/// `Value::as_i64()` 的寬容度——上游把它寫成 `null`、浮點數或字串時都只是
/// 「取不到值」而略過該 feature，不會讓整份檔案解析失敗。全檔僅 4,596 個，
/// 這裡用 `Value` 的記憶體成本可忽略。
#[derive(Deserialize)]
struct Properties {
    #[serde(default)]
    gn_id: Option<Value>,
}

/// 面狀幾何。非 Polygon／MultiPolygon 一律歸為 `Unsupported`。
enum Geometry {
    Polygon(Vec<Ring>),
    MultiPolygon(Vec<Vec<Ring>>),
    Unsupported,
}

/// 多邊形的一個環，解析時直接寫入 `LineString`。
///
/// Reason: 中介不留 `Vec<Position>`，省掉一份與最終幾何等大的複本。
struct Ring(LineString<f64>);

fn polygons_from_geometry(geometry: Geometry) -> Result<Vec<Polygon<f64>>, String> {
    match geometry {
        Geometry::Polygon(rings) => Ok(vec![polygon_from_rings(rings)?]),
        Geometry::MultiPolygon(polygons) => polygons.into_iter().map(polygon_from_rings).collect(),
        // Reason: NE admin-1 只含面狀幾何，其餘型別（含 null geometry）略過即可，
        // 不需視為錯誤中止整份檔案的載入。
        Geometry::Unsupported => Ok(Vec::new()),
    }
}

fn polygon_from_rings(rings: Vec<Ring>) -> Result<Polygon<f64>, String> {
    let mut rings = rings.into_iter();
    let exterior = rings
        .next()
        .ok_or_else(|| "GeoJSON Polygon 缺少外環".to_string())?;
    let interiors = rings.map(|ring| ring.0).collect();
    Ok(Polygon::new(exterior.0, interiors))
}

impl<'de> Deserialize<'de> for Geometry {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_map(GeometryVisitor)
    }
}

struct GeometryVisitor;

impl<'de> Visitor<'de> for GeometryVisitor {
    type Value = Geometry;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("GeoJSON geometry 物件")
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
        let mut kind: Option<String> = None;
        let mut geometry = Geometry::Unsupported;
        let mut deferred: Option<Value> = None;

        while let Some(key) = map.next_key::<String>()? {
            match key.as_str() {
                "type" => kind = map.next_value()?,
                "coordinates" => match kind.as_deref() {
                    Some("Polygon") => geometry = Geometry::Polygon(map.next_value()?),
                    Some("MultiPolygon") => geometry = Geometry::MultiPolygon(map.next_value()?),
                    Some(_) => {
                        map.next_value::<IgnoredAny>()?;
                    }
                    // Reason: GeoJSON 不保證 "type" 排在 "coordinates" 之前。NE 的
                    // 輸出一律 type 在前，這條路徑實務上不會走到；保留它是為了
                    // 上游改變欄位順序時只讓「那一個 feature」退回泛型 Value，
                    // 而不是整份檔案解析失敗。
                    None => deferred = Some(map.next_value()?),
                },
                _ => {
                    map.next_value::<IgnoredAny>()?;
                }
            }
        }

        if let Some(value) = deferred {
            geometry = match kind.as_deref() {
                Some("Polygon") => {
                    Geometry::Polygon(serde_json::from_value(value).map_err(de::Error::custom)?)
                }
                Some("MultiPolygon") => Geometry::MultiPolygon(
                    serde_json::from_value(value).map_err(de::Error::custom)?,
                ),
                _ => Geometry::Unsupported,
            };
        }
        Ok(geometry)
    }
}

impl<'de> Deserialize<'de> for Ring {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_seq(RingVisitor)
    }
}

struct RingVisitor;

impl<'de> Visitor<'de> for RingVisitor {
    type Value = Ring;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("GeoJSON 座標點陣列")
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Self::Value, A::Error> {
        let mut coords: Vec<Coord<f64>> = Vec::with_capacity(seq.size_hint().unwrap_or(0));
        while let Some(coord) = seq.next_element::<Position>()? {
            coords.push(coord.0);
        }
        Ok(Ring(LineString::new(coords)))
    }
}

/// 單一座標點。
struct Position(Coord<f64>);

impl<'de> Deserialize<'de> for Position {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_seq(PositionVisitor)
    }
}

struct PositionVisitor;

impl<'de> Visitor<'de> for PositionVisitor {
    type Value = Position;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("GeoJSON 座標點 [經度, 緯度]")
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Self::Value, A::Error> {
        let longitude: f64 = seq
            .next_element()?
            .ok_or_else(|| de::Error::custom("GeoJSON 座標點缺少經度"))?;
        let latitude: f64 = seq
            .next_element()?
            .ok_or_else(|| de::Error::custom("GeoJSON 座標點缺少緯度"))?;
        // Reason: GeoJSON 允許第三個高程值。丟棄多餘元素以維持與舊版
        // `pair.first()`／`pair.get(1)` 相同的寬容度。
        while seq.next_element::<IgnoredAny>()?.is_some() {}
        Ok(Position(Coord {
            x: longitude,
            y: latitude,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 幾何指紋：把每個 region 的 gn_id、外環／內環頂點依序餵進 SHA-256。
    ///
    /// Reason: `regions` 是私有欄位，等價驗證只能在模組內做。指紋涵蓋順序、
    /// 環的巢狀結構與每個 f64 的位元組，解析方式若改變輸出必然改指紋。
    fn geometry_digest(index: &NeAdmin1Index) -> String {
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        for (gn_id, region) in &index.regions {
            hasher.update(gn_id.to_le_bytes());
            for polygon in &region.geometry {
                for ring in std::iter::once(polygon.exterior()).chain(polygon.interiors()) {
                    hasher.update((ring.0.len() as u64).to_le_bytes());
                    for coord in &ring.0 {
                        hasher.update(coord.x.to_le_bytes());
                        hasher.update(coord.y.to_le_bytes());
                    }
                }
            }
        }
        hasher
            .finalize()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect()
    }

    /// 真實 NE admin-1 圖資的幾何指紋，取自改用型別化解析「之前」的
    /// `serde_json::Value` 版本。
    ///
    /// Reason: 這次改動只換解析方式、不動輸出，指紋是唯一能證明這件事的
    /// 驗收條件。解析路徑再被調整時，此值不得改變；真要改，得先說明為何
    /// 輸出應該變。
    const REAL_DATA_DIGEST: &str =
        "fa336e6ac3deabd33daa145ee0d9fe0a33f63ebea8f106efae4792806893e2de";
    const REAL_DATA_REGIONS: usize = 4394;

    #[test]
    #[ignore = "需要真實 NE admin-1 圖資，預設不在 repo 內"]
    fn real_geojson_parses_to_unchanged_geometry() {
        let path = Path::new("geoname_data/ne_10m_admin_1_states_provinces.geojson");
        if !path.exists() {
            panic!("請先以 prepare 階段下載 {}", path.display());
        }
        let index = NeAdmin1Index::load(path).expect("載入 NE admin-1 圖資");
        assert_eq!(index.len(), REAL_DATA_REGIONS);
        assert_eq!(geometry_digest(&index), REAL_DATA_DIGEST);
    }
}
