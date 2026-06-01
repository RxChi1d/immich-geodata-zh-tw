use std::fs;
use std::path::Path;

use serde_json::{Map, Value};
use shapefile::{Shape, dbase};

use super::geometry::epsg_crs;
use super::types::{
    Coordinate, Country, Feature, FeatureAttributes, FeatureGeometry, MultiPolygonRings,
    PolygonRings, WGS84_EPSG,
};

#[derive(Debug)]
enum CoordinateValue {
    Number(f64),
    Array(Vec<CoordinateValue>),
}

pub(super) fn read_shapefile(country: Country, path: &Path) -> Result<Vec<Feature>, String> {
    let mut reader = shapefile::Reader::from_path(path)
        .map_err(|error| format!("無法讀取 Shapefile {}：{error}", path.display()))?;
    let crs = read_prj(path)?;
    let mut features = Vec::with_capacity(reader.shape_count().unwrap_or(0));
    for item in reader.iter_shapes_and_records() {
        let (shape, record) = item
            .map_err(|error| format!("讀取 Shapefile record 失敗 {}：{error}", path.display()))?;
        let Some(geometry) = geometry_from_shape(shape)? else {
            continue;
        };
        let feature_crs = if features.is_empty() {
            crs.clone()
        } else {
            None
        };
        features.push(Feature {
            geometry,
            attributes: record_to_attributes(record, country),
            crs: feature_crs,
        });
    }
    Ok(features)
}

fn read_prj(path: &Path) -> Result<Option<String>, String> {
    let prj_path = path.with_extension("prj");
    if !prj_path.exists() {
        return Ok(None);
    }
    fs::read_to_string(&prj_path)
        .map(|content| Some(content.trim().to_string()))
        .map_err(|error| format!("無法讀取 PRJ {}：{error}", prj_path.display()))
}

fn geometry_from_shape(shape: Shape) -> Result<Option<FeatureGeometry>, String> {
    match shape {
        Shape::NullShape => Ok(None),
        Shape::Point(point) => Ok(Some(FeatureGeometry::Point((point.x, point.y)))),
        Shape::PointM(point) => Ok(Some(FeatureGeometry::Point((point.x, point.y)))),
        Shape::PointZ(point) => Ok(Some(FeatureGeometry::Point((point.x, point.y)))),
        Shape::Polygon(polygon) => {
            let polygons = polygon
                .rings()
                .iter()
                .map(|ring| {
                    let points = ring
                        .points()
                        .iter()
                        .map(|point| (point.x, point.y))
                        .collect();
                    (ring_is_outer(ring), points)
                })
                .collect();
            polygon_geometry_from_parts(polygons)
        }
        Shape::PolygonM(polygon) => {
            let polygons = polygon
                .rings()
                .iter()
                .map(|ring| {
                    let points = ring
                        .points()
                        .iter()
                        .map(|point| (point.x, point.y))
                        .collect();
                    (ring_is_outer(ring), points)
                })
                .collect();
            polygon_geometry_from_parts(polygons)
        }
        Shape::PolygonZ(polygon) => {
            let polygons = polygon
                .rings()
                .iter()
                .map(|ring| {
                    let points = ring
                        .points()
                        .iter()
                        .map(|point| (point.x, point.y))
                        .collect();
                    (ring_is_outer(ring), points)
                })
                .collect();
            polygon_geometry_from_parts(polygons)
        }
        other => Err(format!(
            "不支援的 Shapefile geometry type：{:?}",
            other.shapetype()
        )),
    }
}

fn ring_is_outer<PointType>(ring: &shapefile::PolygonRing<PointType>) -> bool {
    matches!(ring, shapefile::PolygonRing::Outer(_))
}

fn polygon_geometry_from_parts(
    rings: Vec<(bool, Vec<(f64, f64)>)>,
) -> Result<Option<FeatureGeometry>, String> {
    let mut polygons: Vec<Vec<Vec<(f64, f64)>>> = Vec::new();
    for (is_outer, points) in rings {
        if is_outer {
            polygons.push(vec![points]);
        } else {
            let Some(last_polygon) = polygons.last_mut() else {
                return Err("Shapefile Polygon 包含沒有外環的內環".to_string());
            };
            last_polygon.push(points);
        }
    }
    match polygons.len() {
        0 => Ok(None),
        1 => Ok(Some(FeatureGeometry::Polygon(polygons.remove(0)))),
        _ => Ok(Some(FeatureGeometry::MultiPolygon(polygons))),
    }
}

fn record_to_attributes(mut record: dbase::Record, country: Country) -> FeatureAttributes {
    let mut attributes = FeatureAttributes::empty(country);
    for key in country.extract_attribute_keys() {
        if let Some(value) = record.remove(key)
            && let Some(value) = field_value_to_string(value)
        {
            attributes.set(key, value);
        }
    }
    attributes
}

fn field_value_to_string(value: dbase::FieldValue) -> Option<String> {
    match value {
        dbase::FieldValue::Character(value) => value,
        dbase::FieldValue::Numeric(value) => value.map(|value| value.to_string()),
        dbase::FieldValue::Logical(value) => value.map(|value| value.to_string()),
        dbase::FieldValue::Date(value) => value.map(|value| value.to_string()),
        dbase::FieldValue::Float(value) => value.map(|value| value.to_string()),
        dbase::FieldValue::Integer(value) => Some(value.to_string()),
        dbase::FieldValue::Currency(value) => Some(value.to_string()),
        dbase::FieldValue::DateTime(value) => Some(format!("{value:?}")),
        dbase::FieldValue::Double(value) => Some(value.to_string()),
        dbase::FieldValue::Memo(value) => Some(value),
    }
}

pub(super) fn read_geojson_features_from_content(
    country: Country,
    content: &str,
    path: &Path,
) -> Result<Vec<Feature>, String> {
    let root: Value = serde_json::from_str(content)
        .map_err(|error| format!("GeoJSON JSON 解析失敗 {}：{error}", path.display()))?;
    let source_crs = geojson_source_crs(&root);
    let features = root
        .get("features")
        .and_then(Value::as_array)
        .ok_or_else(|| format!("GeoJSON 缺少 features array：{}", path.display()))?;
    features
        .iter()
        .enumerate()
        .map(|(index, feature)| {
            feature_from_geojson_value(
                country,
                feature,
                (index == 0).then_some(source_crs.as_str()),
            )
        })
        .collect()
}

fn feature_from_geojson_value(
    country: Country,
    feature: &Value,
    source_crs: Option<&str>,
) -> Result<Feature, String> {
    let geometry = feature
        .get("geometry")
        .ok_or_else(|| "GeoJSON feature 缺少 geometry".to_string())?;
    Ok(Feature {
        geometry: feature_geometry_from_geojson_value(geometry)?,
        attributes: feature
            .get("properties")
            .and_then(Value::as_object)
            .map(|properties| geojson_attributes_from_object(properties, country))
            .unwrap_or_else(|| FeatureAttributes::empty(country)),
        crs: source_crs.map(ToString::to_string),
    })
}

fn geojson_attributes_from_object(
    properties: &Map<String, Value>,
    country: Country,
) -> FeatureAttributes {
    let mut attributes = FeatureAttributes::empty(country);
    for key in country.extract_attribute_keys() {
        if let Some(value) = properties.get(*key).and_then(json_value_to_string) {
            attributes.set(key, value);
        }
    }
    attributes
}

fn json_value_to_string(value: &Value) -> Option<String> {
    match value {
        Value::Null => None,
        Value::String(value) => Some(value.clone()),
        Value::Number(value) => Some(value.to_string()),
        Value::Bool(value) => Some(value.to_string()),
        Value::Array(_) | Value::Object(_) => None,
    }
}

fn feature_geometry_from_geojson_value(geometry: &Value) -> Result<FeatureGeometry, String> {
    let geometry_type = geometry
        .get("type")
        .and_then(Value::as_str)
        .ok_or_else(|| "GeoJSON geometry 缺少 type".to_string())?;
    let coordinates = geometry
        .get("coordinates")
        .ok_or_else(|| "GeoJSON geometry 缺少 coordinates".to_string())?;
    let coordinates = coordinate_value_from_json(coordinates)?;
    match geometry_type {
        "Point" => coordinate_pair(&coordinates)
            .map(FeatureGeometry::Point)
            .ok_or_else(|| "GeoJSON Point coordinates 格式錯誤，預期 [lon, lat]".to_string()),
        "Polygon" => coordinate_polygon(&coordinates).map(FeatureGeometry::Polygon),
        "MultiPolygon" => coordinate_multi_polygon(&coordinates).map(FeatureGeometry::MultiPolygon),
        other => Err(format!("不支援的 GeoJSON geometry type：{other}")),
    }
}

fn coordinate_value_from_json(value: &Value) -> Result<CoordinateValue, String> {
    match value {
        Value::Number(number) => number
            .as_f64()
            .map(CoordinateValue::Number)
            .ok_or_else(|| "GeoJSON coordinates 數值超出 f64 範圍".to_string()),
        Value::Array(values) => values
            .iter()
            .map(coordinate_value_from_json)
            .collect::<Result<Vec<_>, _>>()
            .map(CoordinateValue::Array),
        _ => Err("GeoJSON coordinates 只能包含數值或陣列".to_string()),
    }
}

fn geojson_source_crs(root: &Value) -> String {
    root.get("crs")
        .and_then(|crs| crs.get("properties"))
        .and_then(|properties| properties.get("name"))
        .and_then(Value::as_str)
        .and_then(normalize_geojson_crs_name)
        .unwrap_or_else(|| epsg_crs(WGS84_EPSG))
}

fn normalize_geojson_crs_name(name: &str) -> Option<String> {
    if name.eq_ignore_ascii_case("CRS84") || name.eq_ignore_ascii_case("urn:ogc:def:crs:OGC::CRS84")
    {
        return Some(epsg_crs(WGS84_EPSG));
    }
    if let Some(rest) = name
        .strip_prefix("EPSG:")
        .or_else(|| name.strip_prefix("epsg:"))
        && rest.chars().all(|char| char.is_ascii_digit())
    {
        return Some(format!("EPSG:{rest}"));
    }
    let epsg = name
        .rsplit([':', '/'])
        .find(|part| !part.is_empty() && part.chars().all(|char| char.is_ascii_digit()))?;
    Some(format!("EPSG:{epsg}"))
}

fn coordinate_polygon(value: &CoordinateValue) -> Result<PolygonRings, String> {
    let CoordinateValue::Array(rings) = value else {
        return Err("GeoJSON Polygon coordinates 格式錯誤".to_string());
    };
    if rings.is_empty() {
        return Err("GeoJSON Polygon coordinates 不可為空".to_string());
    }
    rings.iter().map(coordinate_ring).collect()
}

fn coordinate_multi_polygon(value: &CoordinateValue) -> Result<MultiPolygonRings, String> {
    let CoordinateValue::Array(polygons) = value else {
        return Err("GeoJSON MultiPolygon coordinates 格式錯誤".to_string());
    };
    if polygons.is_empty() {
        return Err("GeoJSON MultiPolygon coordinates 不可為空".to_string());
    }
    polygons.iter().map(coordinate_polygon).collect()
}

fn coordinate_ring(value: &CoordinateValue) -> Result<Vec<(f64, f64)>, String> {
    let CoordinateValue::Array(points) = value else {
        return Err("GeoJSON Polygon ring 格式錯誤".to_string());
    };
    if points.is_empty() {
        return Err("GeoJSON Polygon ring 不可為空".to_string());
    }
    points
        .iter()
        .map(|point| {
            coordinate_pair(point)
                .ok_or_else(|| "GeoJSON coordinate pair 格式錯誤，預期 [lon, lat]".to_string())
        })
        .collect()
}

fn coordinate_pair(value: &CoordinateValue) -> Option<Coordinate> {
    let CoordinateValue::Array(values) = value else {
        return None;
    };
    let lon = match values.first()? {
        CoordinateValue::Number(value) => *value,
        CoordinateValue::Array(_) => return None,
    };
    let lat = match values.get(1)? {
        CoordinateValue::Number(value) => *value,
        CoordinateValue::Array(_) => return None,
    };
    Some((lon, lat))
}
