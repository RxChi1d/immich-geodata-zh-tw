use immich_geodata::pipeline::ne_admin1::{NeAdmin1Index, NeHit};

/// 兩個相鄰矩形與一個與第一個重疊的矩形，涵蓋唯一命中、無命中與多重命中。
///
/// Reason: 經緯度採用赤道附近的小數值，讓「度」與「公里」的換算接近
/// 1 度 ≈ 111 km，測試裡的距離斷言才能用手算的期望值驗證。
fn fixture_geojson() -> &'static str {
    r#"{
      "type": "FeatureCollection",
      "features": [
        {
          "type": "Feature",
          "properties": {"gn_id": 1001},
          "geometry": {
            "type": "Polygon",
            "coordinates": [[[0.0,0.0],[1.0,0.0],[1.0,1.0],[0.0,1.0],[0.0,0.0]]]
          }
        },
        {
          "type": "Feature",
          "properties": {"gn_id": 1002},
          "geometry": {
            "type": "MultiPolygon",
            "coordinates": [[[[1.0,0.0],[2.0,0.0],[2.0,1.0],[1.0,1.0],[1.0,0.0]]]]
          }
        },
        {
          "type": "Feature",
          "properties": {"gn_id": 1003},
          "geometry": {
            "type": "Polygon",
            "coordinates": [[[0.4,0.4],[0.6,0.4],[0.6,0.6],[0.4,0.6],[0.4,0.4]]]
          }
        }
      ]
    }"#
}

fn index() -> NeAdmin1Index {
    NeAdmin1Index::from_geojson_str(fixture_geojson()).expect("fixture GeoJSON 應可解析")
}

#[test]
fn point_well_inside_single_polygon_is_unique_hit() {
    // (0.2, 0.8) 只落在 1001 內，距最近邊界 0.2 度 ≈ 22 km。
    match index().locate(0.2, 0.8) {
        NeHit::Unique { gn_id, boundary_km } => {
            assert_eq!(gn_id, 1001);
            assert!(
                (boundary_km - 22.2).abs() < 1.0,
                "距邊界應約 22.2 km，實際 {boundary_km}"
            );
        }
        other => panic!("應為唯一命中，實際 {other:?}"),
    }
}

#[test]
fn point_near_edge_reports_small_boundary_distance() {
    // (0.99, 0.5) 距 x=1.0 這條邊 0.01 度 ≈ 1.1 km，應小於 2 km 門檻。
    match index().locate(0.99, 0.5) {
        NeHit::Unique { gn_id, boundary_km } => {
            assert_eq!(gn_id, 1001);
            assert!(boundary_km < 2.0, "距邊界應小於 2 km，實際 {boundary_km}");
        }
        other => panic!("應為唯一命中，實際 {other:?}"),
    }
}

#[test]
fn point_outside_every_polygon_has_no_hit() {
    assert!(matches!(index().locate(5.0, 5.0), NeHit::None));
}

#[test]
fn point_inside_overlapping_polygons_is_multiple_hit() {
    match index().locate(0.5, 0.5) {
        NeHit::Multiple(mut ids) => {
            ids.sort_unstable();
            assert_eq!(ids, vec![1001, 1003]);
        }
        other => panic!("應為多重命中，實際 {other:?}"),
    }
}

#[test]
fn feature_without_gn_id_is_skipped() {
    let json = r#"{
      "type": "FeatureCollection",
      "features": [
        {
          "type": "Feature",
          "properties": {"gn_id": null},
          "geometry": {
            "type": "Polygon",
            "coordinates": [[[0.0,0.0],[1.0,0.0],[1.0,1.0],[0.0,1.0],[0.0,0.0]]]
          }
        }
      ]
    }"#;
    let index = NeAdmin1Index::from_geojson_str(json).expect("應可解析");
    assert!(matches!(index.locate(0.5, 0.5), NeHit::None));
}

#[test]
fn non_positive_gn_id_is_skipped() {
    // Reason: NE 以 -99 或 0 表示缺值，這些值在 admin1CodesASCII 沒有對應列，
    // 若當成有效 gn_id 會讓後續 join 無聲失敗。
    let json = r#"{
      "type": "FeatureCollection",
      "features": [
        {
          "type": "Feature",
          "properties": {"gn_id": -99},
          "geometry": {
            "type": "Polygon",
            "coordinates": [[[0.0,0.0],[1.0,0.0],[1.0,1.0],[0.0,1.0],[0.0,0.0]]]
          }
        }
      ]
    }"#;
    let index = NeAdmin1Index::from_geojson_str(json).expect("應可解析");
    assert!(matches!(index.locate(0.5, 0.5), NeHit::None));
}
