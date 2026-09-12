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
use std::fs;
use std::path::Path;

use geo::algorithm::bounding_rect::BoundingRect;
use geo::algorithm::closest_point::ClosestPoint;
use geo::algorithm::contains::Contains;
use geo::{Closest, Coord, Distance, Haversine, LineString, MultiPolygon, Point, Polygon, Rect};
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
        let root: Value = serde_json::from_str(content)
            .map_err(|error| format!("NE admin-1 GeoJSON 解析失敗：{error}"))?;
        let features = root
            .get("features")
            .and_then(Value::as_array)
            .ok_or_else(|| "NE admin-1 GeoJSON 缺少 features 陣列".to_string())?;

        let mut polygons_by_id: BTreeMap<i64, Vec<Polygon<f64>>> = BTreeMap::new();
        for feature in features {
            let Some(gn_id) = feature
                .get("properties")
                .and_then(|properties| properties.get("gn_id"))
                .and_then(Value::as_i64)
                .filter(|value| *value > 0)
            else {
                continue;
            };
            let Some(geometry) = feature.get("geometry") else {
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

fn polygons_from_geometry(geometry: &Value) -> Result<Vec<Polygon<f64>>, String> {
    let geometry_type = geometry
        .get("type")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let coordinates = match geometry.get("coordinates") {
        Some(value) => value,
        None => return Ok(Vec::new()),
    };
    match geometry_type {
        "Polygon" => Ok(vec![polygon_from_rings(coordinates)?]),
        "MultiPolygon" => coordinates
            .as_array()
            .ok_or_else(|| "GeoJSON MultiPolygon coordinates 格式錯誤".to_string())?
            .iter()
            .map(polygon_from_rings)
            .collect(),
        // Reason: NE admin-1 只含面狀幾何，其餘型別（含 null geometry）略過即可，
        // 不需視為錯誤中止整份檔案的載入。
        _ => Ok(Vec::new()),
    }
}

fn polygon_from_rings(value: &Value) -> Result<Polygon<f64>, String> {
    let rings = value
        .as_array()
        .ok_or_else(|| "GeoJSON Polygon coordinates 格式錯誤".to_string())?;
    let mut parsed = rings.iter().map(ring_from_value);
    let exterior = parsed
        .next()
        .transpose()?
        .ok_or_else(|| "GeoJSON Polygon 缺少外環".to_string())?;
    let interiors = parsed.collect::<Result<Vec<_>, _>>()?;
    Ok(Polygon::new(exterior, interiors))
}

fn ring_from_value(value: &Value) -> Result<LineString<f64>, String> {
    let points = value
        .as_array()
        .ok_or_else(|| "GeoJSON Polygon ring 格式錯誤".to_string())?;
    let coords = points
        .iter()
        .map(|point| {
            let pair = point
                .as_array()
                .ok_or_else(|| "GeoJSON 座標點格式錯誤".to_string())?;
            let longitude = pair
                .first()
                .and_then(Value::as_f64)
                .ok_or_else(|| "GeoJSON 座標點缺少經度".to_string())?;
            let latitude = pair
                .get(1)
                .and_then(Value::as_f64)
                .ok_or_else(|| "GeoJSON 座標點缺少緯度".to_string())?;
            Ok(Coord {
                x: longitude,
                y: latitude,
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    Ok(LineString::new(coords))
}
