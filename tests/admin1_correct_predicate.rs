use std::collections::BTreeMap;

use immich_geodata::pipeline::admin1_correct::{
    CityPoint, PointSample, RejectReason, Verdict, evaluate_points, learn_admin1_mapping,
};
use immich_geodata::pipeline::ne_admin1::NeAdmin1Index;

/// 三個大矩形，彼此不重疊，另加一個與 3001 重疊的小矩形製造多重命中。
fn fixture_geojson() -> &'static str {
    r#"{
      "type": "FeatureCollection",
      "features": [
        {"type":"Feature","properties":{"gn_id":3001},
         "geometry":{"type":"Polygon","coordinates":[[[0,0],[10,0],[10,10],[0,10],[0,0]]]}},
        {"type":"Feature","properties":{"gn_id":3002},
         "geometry":{"type":"Polygon","coordinates":[[[20,0],[30,0],[30,10],[20,10],[20,0]]]}},
        {"type":"Feature","properties":{"gn_id":3003},
         "geometry":{"type":"Polygon","coordinates":[[[40,0],[50,0],[50,10],[40,10],[40,0]]]}},
        {"type":"Feature","properties":{"gn_id":3004},
         "geometry":{"type":"Polygon","coordinates":[[[5,5],[6,5],[6,6],[5,6],[5,5]]]}}
      ]
    }"#
}

fn index() -> NeAdmin1Index {
    NeAdmin1Index::from_geojson_str(fixture_geojson()).expect("fixture GeoJSON 應可解析")
}

fn code_table() -> BTreeMap<i64, String> {
    BTreeMap::from([
        (3001, "XX.01".to_string()),
        (3002, "XX.02".to_string()),
        (3003, "XX.03".to_string()),
        (3004, "XX.04".to_string()),
    ])
}

/// 在指定矩形深處產生樣本，讓「甲州→XX.01」「乙州→XX.02」成為可信映射。
fn training_samples() -> Vec<PointSample> {
    let mut samples = Vec::new();
    for (box_index, name) in [(0usize, "甲州"), (1, "乙州")] {
        for offset in 0..25 {
            samples.push(PointSample {
                longitude: 20.0 * box_index as f64 + 1.0 + (offset % 5) as f64 * 0.4,
                latitude: 1.0 + (offset / 5) as f64 * 0.4,
                locationiq_admin1: name.to_string(),
            });
        }
    }
    samples
}

fn city(
    id: &str,
    longitude: f64,
    latitude: f64,
    original_admin1: &str,
    locationiq_admin1: Option<&str>,
) -> CityPoint {
    CityPoint {
        geoname_id: id.to_string(),
        name: format!("City{id}"),
        longitude,
        latitude,
        country_code: "XX".to_string(),
        original_admin1: original_admin1.to_string(),
        locationiq_admin1: locationiq_admin1.map(str::to_string),
    }
}

fn known_codes() -> BTreeMap<String, String> {
    BTreeMap::from([
        ("XX.01".to_string(), "Alpha".to_string()),
        ("XX.02".to_string(), "Beta".to_string()),
        ("XX.03".to_string(), "Gamma".to_string()),
        ("XX.04".to_string(), "Delta".to_string()),
    ])
}

fn evaluate(points: &[CityPoint]) -> Vec<immich_geodata::pipeline::admin1_correct::Candidate> {
    let index = index();
    let mapping = learn_admin1_mapping(&training_samples(), &index, &code_table());
    evaluate_points(points, &index, &mapping, &code_table(), &known_codes())
}

#[test]
fn disagreement_with_agreeing_locationiq_is_accepted() {
    // 點在 3002 深處（→ XX.02），原值 XX.01，LocationIQ 說乙州（→ XX.02）。
    let candidates = evaluate(&[city("1", 25.0, 5.0, "01", Some("乙州"))]);
    assert_eq!(candidates.len(), 1);
    let candidate = &candidates[0];
    assert_eq!(candidate.verdict, Verdict::Accepted);
    assert_eq!(candidate.corrected_admin1.as_deref(), Some("02"));
    assert!(candidate.reasons.is_empty());
}

#[test]
fn agreeing_sources_produce_no_candidate_row() {
    // Reason: 兩來源都同意原值的點不可能直接翻成危險狀態，收進報表只是雜訊。
    let candidates = evaluate(&[city("1", 25.0, 5.0, "02", Some("乙州"))]);
    assert!(candidates.is_empty(), "不應產生候選：{candidates:?}");
}

#[test]
fn point_close_to_boundary_with_agreeing_sources_is_accepted() {
    // (20.01, 5.0) 距 3002 西界僅約 1.1 km，兩來源仍一致，應採納。
    //
    // Reason: 曾有一版謂詞要求「距邊界 ≥2km」。實測在 MY 上，該規則的真陽性
    // 攔截數為 0，卻砍掉 6 筆兩來源一致的修正（Setapak 0.31km、SS2 1.55km、
    // Bandar Utama 1.15km、Mutiara Damansara 1.71km 等，其中 4 筆已人工查核為
    // 真實上游錯誤）。GeoNames 標錯行政區的地方本來就集中在邊界——吉隆坡是
    // 嵌在雪蘭莪裡的飛地——用「離邊界遠」當安全條件等於排除功能存在的理由。
    // 這個測試把該決定釘死：重新加回距離門檻會讓它失敗。
    let candidates = evaluate(&[city("1", 20.01, 5.0, "01", Some("乙州"))]);
    assert_eq!(candidates.len(), 1);
    assert_eq!(candidates[0].verdict, Verdict::Accepted);
    assert_eq!(candidates[0].corrected_admin1.as_deref(), Some("02"));
    let boundary_km = candidates[0].boundary_km.expect("應記錄邊界距離供診斷");
    assert!(boundary_km < 2.0, "此點確實貼近邊界，實際 {boundary_km}");
}

#[test]
fn locationiq_disagreeing_with_natural_earth_is_rejected() {
    // NE 說 XX.02，LocationIQ 說甲州（→ XX.01）。兩者未指向同一代碼。
    let candidates = evaluate(&[city("1", 25.0, 5.0, "01", Some("甲州"))]);
    assert_eq!(candidates.len(), 1);
    assert_eq!(candidates[0].verdict, Verdict::Rejected);
    assert!(matches!(
        candidates[0].reasons.first(),
        Some(RejectReason::SourcesDoNotAgree { .. })
    ));
}

#[test]
fn name_without_trusted_mapping_is_rejected() {
    let candidates = evaluate(&[city("1", 25.0, 5.0, "01", Some("丙州"))]);
    assert_eq!(candidates.len(), 1);
    assert_eq!(candidates[0].verdict, Verdict::Rejected);
    assert!(matches!(
        candidates[0].reasons.first(),
        Some(RejectReason::NoTrustedMapping)
    ));
}

#[test]
fn point_outside_every_polygon_is_rejected() {
    let candidates = evaluate(&[city("1", 80.0, 80.0, "01", Some("乙州"))]);
    assert_eq!(candidates.len(), 1);
    assert_eq!(candidates[0].verdict, Verdict::Rejected);
    assert!(matches!(
        candidates[0].reasons.first(),
        Some(RejectReason::NaturalEarthNoHit)
    ));
}

#[test]
fn point_in_overlapping_polygons_is_rejected() {
    let candidates = evaluate(&[city("1", 5.5, 5.5, "02", Some("甲州"))]);
    assert_eq!(candidates.len(), 1);
    assert_eq!(candidates[0].verdict, Verdict::Rejected);
    assert!(matches!(
        candidates[0].reasons.first(),
        Some(RejectReason::NaturalEarthMultipleHit { .. })
    ));
}

#[test]
fn original_code_missing_from_admin1_codes_is_reported_not_corrected() {
    // Reason: 原值本身就不在 admin1CodesASCII（全球 17 種鍵／39 點的既有缺陷）。
    // 那是另一個問題，混進來會讓本功能的失敗範圍變模糊，故只回報不修正。
    let candidates = evaluate(&[city("1", 25.0, 5.0, "99", Some("乙州"))]);
    assert_eq!(candidates.len(), 1);
    assert_eq!(candidates[0].verdict, Verdict::Rejected);
    assert!(matches!(
        candidates[0].reasons.first(),
        Some(RejectReason::OriginalCodeUnknown { .. })
    ));
}

#[test]
fn point_without_locationiq_metadata_is_rejected() {
    let candidates = evaluate(&[city("1", 25.0, 5.0, "01", None)]);
    assert_eq!(candidates.len(), 1);
    assert_eq!(candidates[0].verdict, Verdict::Rejected);
    assert!(matches!(
        candidates[0].reasons.first(),
        Some(RejectReason::NoLocationiqAdmin1)
    ));
}

#[test]
fn all_applicable_reasons_are_recorded_with_primary_first() {
    // 原代碼不在 admin1CodesASCII + 兩來源不一致：都要列出，主因在前。
    let candidates = evaluate(&[city("1", 25.0, 5.0, "99", Some("甲州"))]);
    assert_eq!(candidates.len(), 1);
    let reasons = &candidates[0].reasons;
    assert!(reasons.len() >= 2, "應記錄全部原因，實際 {reasons:?}");
    assert!(matches!(
        reasons.first(),
        Some(RejectReason::OriginalCodeUnknown { .. })
    ));
    assert!(
        reasons
            .iter()
            .any(|reason| matches!(reason, RejectReason::SourcesDoNotAgree { .. }))
    );
}

#[test]
fn candidates_are_sorted_by_geoname_id() {
    let points = vec![
        city("30", 25.0, 5.0, "01", Some("乙州")),
        city("10", 25.5, 5.5, "01", Some("乙州")),
        city("20", 26.0, 6.0, "01", Some("乙州")),
    ];
    let ids: Vec<String> = evaluate(&points)
        .into_iter()
        .map(|candidate| candidate.geoname_id)
        .collect();
    assert_eq!(ids, vec!["10", "20", "30"]);
}

#[test]
fn foreign_country_natural_earth_code_is_never_accepted() {
    // Reason: NE 會把邊境點判進鄰國的多邊形（實測 Bukit Kayu Hitam 落在 TH.68）。
    // 兩來源即使一致，代碼去不掉本國前綴就沒有可寫入的值——此時必須是 Rejected，
    // 否則報表的 accepted 數會多於實際寫入數，而那個差額沒有任何訊號。
    let foreign_codes = BTreeMap::from([
        (3001, "XX.01".to_string()),
        (3002, "YY.09".to_string()),
        (3003, "XX.03".to_string()),
        (3004, "XX.04".to_string()),
    ]);
    let index = index();
    let mapping = learn_admin1_mapping(&training_samples(), &index, &foreign_codes);
    assert_eq!(
        mapping.code_for("乙州"),
        Some("YY.09"),
        "前提：映射指向鄰國"
    );

    let candidates = evaluate_points(
        &[city("1", 25.0, 5.0, "01", Some("乙州"))],
        &index,
        &mapping,
        &foreign_codes,
        &known_codes(),
    );
    assert_eq!(candidates.len(), 1);
    assert_eq!(candidates[0].verdict, Verdict::Rejected);
    assert_eq!(candidates[0].corrected_admin1, None);
    // Reason: 被拒絕就必須讀得出原因。reasons 空白的拒絕列等於無聲缺口。
    assert!(
        candidates[0]
            .reasons
            .iter()
            .any(|reason| matches!(reason, RejectReason::NaturalEarthForeignCountry { .. })),
        "應記錄 NE 命中鄰國，實際 {:?}",
        candidates[0].reasons
    );
}
