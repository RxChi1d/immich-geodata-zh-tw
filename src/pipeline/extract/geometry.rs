use std::collections::{HashMap, hash_map::Entry};
use std::thread;

use geo::{Centroid, Coord, LineString, MultiPolygon, Polygon};
use proj::Proj;

use super::types::{CentroidPipeline, Country, Feature, FeatureGeometry, WGS84_EPSG};

const PARALLEL_CENTROID_MIN_FEATURES: usize = 10_000;
const PARALLEL_CENTROID_MAX_THREADS: usize = 8;

pub(super) fn apply_country_centroids(
    country: Country,
    features: &mut [Feature],
    source_crs: Option<&str>,
) -> Result<(), String> {
    let centroids = match country.centroid_pipeline() {
        CentroidPipeline::ProjectedEpsg(epsg) => {
            calculate_centroids_projected(features, source_crs, &epsg_crs(epsg))?
        }
        CentroidPipeline::ProjectedProj4(proj4) => {
            calculate_centroids_projected(features, source_crs, proj4)?
        }
        CentroidPipeline::DynamicUtm(albers_proj4) => {
            calculate_centroids_utm(features, source_crs, albers_proj4)?
        }
    };
    for (feature, centroid) in features.iter_mut().zip(centroids) {
        feature.geometry = FeatureGeometry::Point(centroid);
    }
    Ok(())
}

fn calculate_centroids_projected(
    features: &[Feature],
    source_crs: Option<&str>,
    projected_crs: &str,
) -> Result<Vec<(f64, f64)>, String> {
    let from_crs = source_crs
        .ok_or_else(|| "extract 輸入缺少 CRS，無法對齊既有 CRS 轉換行為".to_string())?
        .to_string();
    let thread_count = centroid_thread_count(features.len());
    if thread_count > 1 {
        return calculate_centroids_projected_parallel(
            features,
            &from_crs,
            projected_crs,
            thread_count,
        );
    }
    let to_projected = build_transform(&from_crs, projected_crs)?;
    let to_wgs84 = build_transform(projected_crs, &epsg_crs(WGS84_EPSG))?;
    features
        .iter()
        .map(|feature| {
            let projected = transform_geometry(&feature.geometry, &to_projected)?;
            let (x, y) = geometry_centroid(&projected)?;
            to_wgs84
                .convert((x, y))
                .map_err(|error| format!("中心點轉回 WGS84 失敗：{error}"))
        })
        .collect()
}

fn calculate_centroids_projected_parallel(
    features: &[Feature],
    from_crs: &str,
    projected_crs: &str,
    thread_count: usize,
) -> Result<Vec<(f64, f64)>, String> {
    let chunk_size = features.len().div_ceil(thread_count);
    thread::scope(|scope| {
        let mut handles = Vec::new();
        for chunk in features.chunks(chunk_size) {
            let from_crs = from_crs.to_string();
            let projected_crs = projected_crs.to_string();
            handles.push(scope.spawn(move || {
                let to_projected = build_transform(&from_crs, &projected_crs)?;
                let to_wgs84 = build_transform(&projected_crs, &epsg_crs(WGS84_EPSG))?;
                chunk
                    .iter()
                    .map(|feature| {
                        let projected = transform_geometry(&feature.geometry, &to_projected)?;
                        let (x, y) = geometry_centroid(&projected)?;
                        to_wgs84
                            .convert((x, y))
                            .map_err(|error| format!("中心點轉回 WGS84 失敗：{error}"))
                    })
                    .collect::<Result<Vec<_>, String>>()
            }));
        }

        let mut centroids = Vec::with_capacity(features.len());
        for handle in handles {
            let chunk_centroids = handle
                .join()
                .map_err(|_| "平行 centroid worker 發生 panic".to_string())??;
            centroids.extend(chunk_centroids);
        }
        Ok(centroids)
    })
}

fn calculate_centroids_utm(
    features: &[Feature],
    source_crs: Option<&str>,
    albers_proj4: &str,
) -> Result<Vec<(f64, f64)>, String> {
    let wgs84_crs = epsg_crs(WGS84_EPSG);
    let from_crs =
        source_crs.ok_or_else(|| "extract 輸入缺少 CRS，無法對齊既有 CRS 轉換行為".to_string())?;
    let thread_count = centroid_thread_count(features.len());
    if thread_count > 1 {
        return calculate_centroids_utm_parallel(features, from_crs, albers_proj4, thread_count);
    }
    calculate_centroids_utm_chunk(features, from_crs, albers_proj4, &wgs84_crs)
}

fn calculate_centroids_utm_parallel(
    features: &[Feature],
    from_crs: &str,
    albers_proj4: &str,
    thread_count: usize,
) -> Result<Vec<(f64, f64)>, String> {
    let chunk_size = features.len().div_ceil(thread_count);
    thread::scope(|scope| {
        let mut handles = Vec::new();
        for chunk in features.chunks(chunk_size) {
            let from_crs = from_crs.to_string();
            let albers_proj4 = albers_proj4.to_string();
            handles.push(scope.spawn(move || {
                let wgs84_crs = epsg_crs(WGS84_EPSG);
                calculate_centroids_utm_chunk(chunk, &from_crs, &albers_proj4, &wgs84_crs)
            }));
        }

        let mut centroids = Vec::with_capacity(features.len());
        for handle in handles {
            let chunk_centroids = handle
                .join()
                .map_err(|_| "平行 centroid worker 發生 panic".to_string())??;
            centroids.extend(chunk_centroids);
        }
        Ok(centroids)
    })
}

fn calculate_centroids_utm_chunk(
    features: &[Feature],
    from_crs: &str,
    albers_proj4: &str,
    wgs84_crs: &str,
) -> Result<Vec<(f64, f64)>, String> {
    let to_albers = build_transform(wgs84_crs, albers_proj4)?;
    let from_albers = build_transform(albers_proj4, wgs84_crs)?;
    let mut centroids = Vec::with_capacity(features.len());
    let mut utm_transforms = HashMap::<i32, (Proj, Proj)>::new();
    if from_crs == wgs84_crs {
        for feature in features {
            centroids.push(calculate_utm_centroid_for_wgs84_geometry(
                &feature.geometry,
                &to_albers,
                &from_albers,
                &mut utm_transforms,
                wgs84_crs,
            )?);
        }
    } else {
        let to_wgs84 = build_transform(from_crs, wgs84_crs)?;
        for feature in features {
            let geometry = transform_geometry(&feature.geometry, &to_wgs84)?;
            centroids.push(calculate_utm_centroid_for_wgs84_geometry(
                &geometry,
                &to_albers,
                &from_albers,
                &mut utm_transforms,
                wgs84_crs,
            )?);
        }
    }
    Ok(centroids)
}

fn centroid_thread_count(feature_count: usize) -> usize {
    if feature_count < PARALLEL_CENTROID_MIN_FEATURES {
        return 1;
    }
    thread::available_parallelism()
        .map(usize::from)
        .unwrap_or(1)
        .min(PARALLEL_CENTROID_MAX_THREADS)
        .min(feature_count)
}

fn calculate_utm_centroid_for_wgs84_geometry(
    geometry: &FeatureGeometry,
    to_albers: &Proj,
    from_albers: &Proj,
    utm_transforms: &mut HashMap<i32, (Proj, Proj)>,
    wgs84_crs: &str,
) -> Result<(f64, f64), String> {
    let albers_geometry = transform_geometry(geometry, to_albers)?;
    let albers_centroid = geometry_centroid(&albers_geometry)?;
    let (center_lon, _) = from_albers
        .convert(albers_centroid)
        .map_err(|error| format!("Albers 中間中心點轉回 WGS84 失敗：{error}"))?;
    let utm_epsg = utm_epsg_for_lon(center_lon);
    let (to_utm, from_utm) = match utm_transforms.entry(utm_epsg) {
        Entry::Occupied(entry) => entry.into_mut(),
        Entry::Vacant(entry) => {
            let utm_crs = epsg_crs(utm_epsg);
            entry.insert((
                build_transform(wgs84_crs, &utm_crs)?,
                build_transform(&utm_crs, wgs84_crs)?,
            ))
        }
    };
    let utm_geometry = transform_geometry(geometry, to_utm)?;
    let utm_centroid = geometry_centroid(&utm_geometry)?;
    from_utm
        .convert(utm_centroid)
        .map_err(|error| format!("UTM 中心點轉回 WGS84 失敗：{error}"))
}

#[allow(dead_code)]
pub(super) fn transform_coords(
    lon: f64,
    lat: f64,
    from_epsg: i32,
    to_epsg: i32,
) -> Result<(f64, f64), String> {
    build_transform(&epsg_crs(from_epsg), &epsg_crs(to_epsg))?
        .convert((lon, lat))
        .map_err(|error| format!("座標轉換 EPSG:{from_epsg} → EPSG:{to_epsg} 失敗：{error}"))
}

fn build_transform(from_crs: &str, to_crs: &str) -> Result<Proj, String> {
    Proj::new_known_crs(from_crs, to_crs, None)
        .map_err(|error| format!("建立 CRS 轉換失敗：{from_crs} → {to_crs}：{error}"))
}

fn transform_geometry(
    geometry: &FeatureGeometry,
    transform: &Proj,
) -> Result<FeatureGeometry, String> {
    match geometry {
        FeatureGeometry::Point(point) => transform
            .convert(*point)
            .map(FeatureGeometry::Point)
            .map_err(|error| format!("Point 座標轉換失敗：{error}")),
        FeatureGeometry::Polygon(rings) => {
            Ok(FeatureGeometry::Polygon(transform_rings(rings, transform)?))
        }
        FeatureGeometry::MultiPolygon(polygons) => polygons
            .iter()
            .map(|rings| transform_rings(rings, transform))
            .collect::<Result<Vec<_>, _>>()
            .map(FeatureGeometry::MultiPolygon),
    }
}

fn transform_rings(
    rings: &[Vec<(f64, f64)>],
    transform: &Proj,
) -> Result<Vec<Vec<(f64, f64)>>, String> {
    let mut transformed_rings = Vec::with_capacity(rings.len());
    for ring in rings {
        let mut transformed_ring = Vec::with_capacity(ring.len());
        for point in ring {
            transformed_ring.push(
                transform
                    .convert(*point)
                    .map_err(|error| format!("座標轉換失敗：{error}"))?,
            );
        }
        transformed_rings.push(transformed_ring);
    }
    Ok(transformed_rings)
}

fn geometry_centroid(geometry: &FeatureGeometry) -> Result<(f64, f64), String> {
    match geometry {
        FeatureGeometry::Point(point) => Ok(*point),
        FeatureGeometry::Polygon(rings) => {
            let polygon = polygon_from_rings(rings)?;
            polygon
                .centroid()
                .map(|point| (point.x(), point.y()))
                .ok_or_else(|| "Polygon centroid 計算失敗".to_string())
        }
        FeatureGeometry::MultiPolygon(polygons) => {
            let polygons = polygons
                .iter()
                .map(|rings| polygon_from_rings(rings))
                .collect::<Result<Vec<_>, _>>()?;
            MultiPolygon::new(polygons)
                .centroid()
                .map(|point| (point.x(), point.y()))
                .ok_or_else(|| "MultiPolygon centroid 計算失敗".to_string())
        }
    }
}

fn polygon_from_rings(rings: &[Vec<(f64, f64)>]) -> Result<Polygon, String> {
    let exterior = rings
        .first()
        .ok_or_else(|| "Polygon 缺少外環".to_string())?;
    let interiors = rings[1..]
        .iter()
        .map(|ring| linestring_from_ring(ring))
        .collect();
    Ok(Polygon::new(linestring_from_ring(exterior), interiors))
}

fn linestring_from_ring(ring: &[(f64, f64)]) -> LineString {
    LineString::new(ring.iter().map(|(x, y)| Coord { x: *x, y: *y }).collect())
}

fn utm_epsg_for_lon(lon: f64) -> i32 {
    let zone = ((lon + 180.0) / 6.0).floor() as i32 + 1;
    32600 + zone.clamp(1, 60)
}

pub(super) fn epsg_crs(epsg: i32) -> String {
    format!("EPSG:{epsg}")
}
