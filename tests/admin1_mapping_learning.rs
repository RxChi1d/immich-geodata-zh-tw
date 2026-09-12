use std::collections::BTreeMap;

use immich_geodata::pipeline::admin1_correct::{
    LearnedMapping, MappingRejection, PointSample, learn_admin1_mapping,
};
use immich_geodata::pipeline::ne_admin1::NeAdmin1Index;

/// 三個互不重疊的矩形，分別對應 gn_id 2001/2002/2003。
fn fixture_geojson() -> &'static str {
    r#"{
      "type": "FeatureCollection",
      "features": [
        {"type":"Feature","properties":{"gn_id":2001},
         "geometry":{"type":"Polygon","coordinates":[[[0,0],[10,0],[10,10],[0,10],[0,0]]]}},
        {"type":"Feature","properties":{"gn_id":2002},
         "geometry":{"type":"Polygon","coordinates":[[[20,0],[30,0],[30,10],[20,10],[20,0]]]}},
        {"type":"Feature","properties":{"gn_id":2003},
         "geometry":{"type":"Polygon","coordinates":[[[40,0],[50,0],[50,10],[40,10],[40,0]]]}}
      ]
    }"#
}

fn index() -> NeAdmin1Index {
    NeAdmin1Index::from_geojson_str(fixture_geojson()).expect("fixture GeoJSON 應可解析")
}

fn code_table() -> BTreeMap<i64, String> {
    BTreeMap::from([
        (2001, "XX.01".to_string()),
        (2002, "XX.02".to_string()),
        (2003, "XX.03".to_string()),
    ])
}

/// 在指定矩形內部產生 `count` 個彼此不同的點，全部遠離邊界。
fn samples_in(box_index: usize, name: &str, count: usize) -> Vec<PointSample> {
    let base_longitude = 20.0 * box_index as f64 + 2.0;
    (0..count)
        .map(|offset| PointSample {
            longitude: base_longitude + (offset % 5) as f64 * 0.5,
            latitude: 2.0 + (offset / 5) as f64 * 0.5,
            locationiq_admin1: name.to_string(),
        })
        .collect()
}

fn learn(samples: Vec<PointSample>) -> LearnedMapping {
    learn_admin1_mapping(&samples, &index(), &code_table())
}

#[test]
fn clean_name_with_enough_samples_becomes_trusted() {
    let mapping = learn(samples_in(0, "甲州", 25));
    assert_eq!(mapping.code_for("甲州"), Some("XX.01"));
}

#[test]
fn nineteen_samples_is_below_threshold() {
    let mapping = learn(samples_in(0, "甲州", 19));
    assert_eq!(mapping.code_for("甲州"), None);
    assert!(matches!(
        mapping.rejection_for("甲州"),
        Some(MappingRejection::TooFewSamples { resolved: 19 })
    ));
}

#[test]
fn twenty_samples_meets_threshold() {
    // Reason: 門檻寫成「≥20」，19/20 這組相鄰值把「>」與「>=」的差別釘死。
    let mapping = learn(samples_in(0, "甲州", 20));
    assert_eq!(mapping.code_for("甲州"), Some("XX.01"));
}

#[test]
fn six_percent_noise_is_rejected() {
    // 47 點在 2001、3 點在 2002 → 非第一名佔 6%，超過 5% 門檻。
    let mut samples = samples_in(0, "乙州", 47);
    samples.extend(samples_in(1, "乙州", 3));
    let mapping = learn(samples);
    assert_eq!(mapping.code_for("乙州"), None);
    match mapping.rejection_for("乙州") {
        Some(MappingRejection::TooNoisy { ratio, .. }) => {
            assert!(
                (ratio - 0.06).abs() < 1e-9,
                "雜訊比例應為 0.06，實際 {ratio}"
            );
        }
        other => panic!("應因雜訊被拒，實際 {other:?}"),
    }
}

#[test]
fn four_percent_noise_is_accepted() {
    // 48 點在 2001、2 點在 2002 → 非第一名佔 4%，低於 5% 門檻。
    let mut samples = samples_in(0, "丙州", 48);
    samples.extend(samples_in(1, "丙州", 2));
    let mapping = learn(samples);
    assert_eq!(mapping.code_for("丙州"), Some("XX.01"));
}

#[test]
fn exactly_five_percent_noise_is_rejected() {
    // Reason: 規格寫「非第一名合計 <5%」，5% 本身不通過。
    let mut samples = samples_in(0, "丁州", 38);
    samples.extend(samples_in(1, "丁州", 2));
    let mapping = learn(samples);
    assert_eq!(mapping.code_for("丁州"), None);
}

#[test]
fn unresolvable_points_do_not_count_toward_noise() {
    // Reason: NE 無命中代表覆蓋不足，不是「名稱對應到多個代碼」的歧義。
    // 把它算進雜訊會讓海岸線密集的州平白失去映射。
    let mut samples = samples_in(0, "戊州", 25);
    samples.extend((0..25).map(|offset| PointSample {
        longitude: 100.0 + offset as f64 * 0.1,
        latitude: 60.0,
        locationiq_admin1: "戊州".to_string(),
    }));
    let mapping = learn(samples);
    assert_eq!(mapping.code_for("戊州"), Some("XX.01"));
    assert_eq!(mapping.unresolved_for("戊州"), 25);
}

#[test]
fn gn_id_without_admin1_code_is_unresolved() {
    // Reason: 上游沒有該行政區的記錄時必須跳過，絕不自造代碼——geonameid
    // 是全球共用識別碼，自造會撞上游現有或日後新增的 ID。
    let samples = samples_in(2, "己州", 25);
    let partial = BTreeMap::from([(2001, "XX.01".to_string()), (2002, "XX.02".to_string())]);
    let mapping = learn_admin1_mapping(&samples, &index(), &partial);
    assert_eq!(mapping.code_for("己州"), None);
    assert_eq!(mapping.unresolved_for("己州"), 25);
}

#[test]
fn learning_is_deterministic_across_input_order() {
    let mut forward = samples_in(0, "庚州", 25);
    forward.extend(samples_in(1, "辛州", 25));
    let mut reversed = forward.clone();
    reversed.reverse();
    assert_eq!(
        learn(forward).into_sorted_pairs(),
        learn(reversed).into_sorted_pairs()
    );
}
