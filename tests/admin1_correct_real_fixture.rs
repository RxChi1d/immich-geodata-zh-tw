//! 以真實 Natural Earth 幾何驗證謂詞的正反兩面。
//!
//! fixture `ne_admin1_my_th.geojson` 由 NE 10m admin-1 裁切出馬來西亞 16 個
//! 一級行政區與泰國宋卡府（Bukit Kayu Hitam 被 NE 判進宋卡，需要它才能重現）。
//!
//! Reason: 合成矩形無法重現 NE 在真實邊界上的行為——吉隆坡是嵌在雪蘭莪裡的
//! 飛地、刁曼島離本島 30 km、玻璃市與宋卡隔著國界。handoff §6 要求把
//! 「NE 獨排眾議」的邊界案例做成負面 fixture，只有真實幾何做得到。

use std::collections::BTreeMap;

use immich_geodata::pipeline::admin1_correct::{
    CityPoint, PointSample, RejectReason, Verdict, evaluate_points, learn_admin1_mapping,
};
use immich_geodata::pipeline::ne_admin1::NeAdmin1Index;

fn fixture_index() -> NeAdmin1Index {
    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures/admin1_correct/ne_admin1_my_th.geojson");
    NeAdmin1Index::load(&path).expect("fixture NE GeoJSON 應可載入")
}

/// `admin1CodesASCII` 中對應這批 gn_id 的代碼（第 4 欄 → 第 1 欄）。
fn gn_id_to_code() -> BTreeMap<i64, String> {
    [
        (1733035, "MY.04"),
        (1733036, "MY.13"),
        (1733037, "MY.12"),
        (1733038, "MY.11"),
        (1733039, "MY.16"),
        (1733040, "MY.08"),
        (1733041, "MY.07"),
        (1733042, "MY.06"),
        (1733043, "MY.05"),
        (1733044, "MY.03"),
        (1733046, "MY.14"),
        (1733047, "MY.09"),
        (1733048, "MY.02"),
        (1733049, "MY.01"),
        (1734240, "MY.15"),
        (1996552, "MY.17"),
        (1606146, "TH.68"),
    ]
    .into_iter()
    .map(|(id, code)| (id, code.to_string()))
    .collect()
}

fn known_admin1_codes() -> BTreeMap<String, String> {
    gn_id_to_code()
        .into_values()
        .map(|code| (code.clone(), code))
        .collect()
}

/// LocationIQ 中文州名與其 NE 標註點（`label_longitude` / `label_latitude`）。
const TRAINING_ANCHORS: &[(&str, f64, f64)] = &[
    ("雪兰莪州", 101.428, 3.3078),
    ("吉隆坡", 101.698, 3.13836),
    ("吉打州", 100.647, 5.80958),
    ("霹雳州", 101.059, 4.74737),
    ("马六甲州", 102.293, 2.32162),
    ("柔佛州", 103.412, 2.00649),
    ("彭亨州", 102.491, 3.84557),
];

/// 每個州名以自身標註點重複 20 次作為訓練樣本。
///
/// Reason: 標註點是 NE 自己提供、保證落在該多邊形內的代表點。用它建立映射，
/// 訓練資料就與待驗證的 17 個案例完全無關——不會發生「拿要檢驗的點去教會
/// 映射認得答案」這種循環。
fn training_samples() -> Vec<PointSample> {
    TRAINING_ANCHORS
        .iter()
        .flat_map(|(name, longitude, latitude)| {
            (0..20).map(move |_| PointSample {
                longitude: *longitude,
                latitude: *latitude,
                locationiq_admin1: name.to_string(),
            })
        })
        .collect()
}

/// (geoname_id, 名稱, 經度, 緯度, cities500 原 admin1, LocationIQ 州名)
type Case = (
    &'static str,
    &'static str,
    f64,
    f64,
    &'static str,
    &'static str,
);

/// 兩來源一致且與 GeoNames 不符——應採納。第 4 欄之後為期望的新代碼。
const ACCEPTED_CASES: &[(Case, &str)] = &[
    (
        ("13118198", "SS2", 101.62246, 3.12036, "14", "雪兰莪州"),
        "12",
    ),
    (
        (
            "13118277",
            "Bandar Utama",
            101.61407,
            3.14818,
            "14",
            "雪兰莪州",
        ),
        "12",
    ),
    (
        (
            "13118370",
            "Mutiara Damansara",
            101.60822,
            3.15724,
            "14",
            "雪兰莪州",
        ),
        "12",
    ),
    (("1735162", "Setapak", 101.727, 3.207, "12", "吉隆坡"), "14"),
    (
        ("1735168", "Ampang", 101.76667, 3.15, "14", "雪兰莪州"),
        "12",
    ),
    (
        ("1744366", "Serdang", 100.62476, 5.20372, "07", "吉打州"),
        "02",
    ),
    (
        (
            "1769612",
            "Kampong Dungun",
            101.31667,
            3.21667,
            "07",
            "雪兰莪州",
        ),
        "12",
    ),
    (("1777077", "Cheras", 101.726, 3.108, "12", "吉隆坡"), "14"),
];

/// NE 獨排眾議的邊界案例——謂詞必須全部拒絕。
const NE_ALONE_CASES: &[Case] = &[
    (
        "12680194",
        "Taman Melati",
        101.72324,
        3.22124,
        "14",
        "吉隆坡",
    ),
    (
        "12750644",
        "Taman Melawati",
        101.74825,
        3.21083,
        "12",
        "雪兰莪州",
    ),
    ("13100287", "Mukim Tioman", 104.17, 2.7825, "01", "彭亨州"),
    ("1734399", "Selama", 100.69262, 5.2236, "07", "霹雳州"),
    (
        "1734407",
        "Bandar Baharu",
        100.49549,
        5.13428,
        "02",
        "吉打州",
    ),
    ("1734736", "Lubok China", 102.0695, 2.4536, "04", "马六甲州"),
    (
        "1734897",
        "Padang Endau",
        103.61504,
        2.65568,
        "01",
        "柔佛州",
    ),
    ("1734949", "Kampung Tekek", 104.1592, 2.8147, "06", "彭亨州"),
    (
        "1736283",
        "Bukit Kayu Hitam",
        100.41893,
        6.51668,
        "02",
        "吉打州",
    ),
];

fn city_point(case: &Case) -> CityPoint {
    let (geoname_id, name, longitude, latitude, original_admin1, locationiq_admin1) = *case;
    CityPoint {
        geoname_id: geoname_id.to_string(),
        name: name.to_string(),
        longitude,
        latitude,
        country_code: "MY".to_string(),
        original_admin1: original_admin1.to_string(),
        locationiq_admin1: Some(locationiq_admin1.to_string()),
    }
}

fn evaluate(points: &[CityPoint]) -> Vec<immich_geodata::pipeline::admin1_correct::Candidate> {
    let index = fixture_index();
    let codes = gn_id_to_code();
    let mapping = learn_admin1_mapping(&training_samples(), &index, &codes);
    evaluate_points(points, &index, &mapping, &codes, &known_admin1_codes())
}

#[test]
fn training_anchors_produce_every_expected_mapping() {
    // Reason: 映射沒學成的話，底下每個案例都會以「無可信映射」被拒，
    // 負面測試會全綠但什麼都沒驗到。先把前提釘死。
    let index = fixture_index();
    let mapping = learn_admin1_mapping(&training_samples(), &index, &gn_id_to_code());
    let expected = [
        ("雪兰莪州", "MY.12"),
        ("吉隆坡", "MY.14"),
        ("吉打州", "MY.02"),
        ("霹雳州", "MY.07"),
        ("马六甲州", "MY.04"),
        ("柔佛州", "MY.01"),
        ("彭亨州", "MY.06"),
    ];
    for (name, code) in expected {
        assert_eq!(mapping.code_for(name), Some(code), "{name} 未學出預期代碼");
    }
}

#[test]
fn natural_earth_alone_boundary_cases_are_all_rejected() {
    let points: Vec<CityPoint> = NE_ALONE_CASES.iter().map(city_point).collect();
    let candidates = evaluate(&points);
    assert_eq!(
        candidates.len(),
        NE_ALONE_CASES.len(),
        "每筆都應成為候選列（NE 與原值不符），實際 {}",
        candidates.len()
    );
    for candidate in &candidates {
        assert_eq!(
            candidate.verdict,
            Verdict::Rejected,
            "{} 不應被採納：NE={:?} LIQ={:?}",
            candidate.name,
            candidate.natural_earth_code,
            candidate.locationiq_code
        );
        assert!(
            matches!(
                candidate.reasons.first(),
                Some(RejectReason::SourcesDoNotAgree { .. })
            ),
            "{} 的主因應為兩來源不一致，實際 {:?}",
            candidate.name,
            candidate.reasons
        );
        assert!(candidate.corrected_admin1.is_none());
    }
}

#[test]
fn verified_upstream_errors_are_all_accepted() {
    // 這 8 筆已逐筆對照官方／OSM 資料人工查核為真實的上游錯誤。
    let points: Vec<CityPoint> = ACCEPTED_CASES
        .iter()
        .map(|(case, _)| city_point(case))
        .collect();
    let candidates = evaluate(&points);
    let expected: BTreeMap<&str, &str> = ACCEPTED_CASES
        .iter()
        .map(|(case, corrected)| (case.0, *corrected))
        .collect();
    assert_eq!(candidates.len(), expected.len());
    for candidate in &candidates {
        assert_eq!(
            candidate.verdict,
            Verdict::Accepted,
            "{} 應被採納，實際原因 {:?}",
            candidate.name,
            candidate.reasons
        );
        assert_eq!(
            candidate.corrected_admin1.as_deref(),
            expected.get(candidate.geoname_id.as_str()).copied(),
            "{} 修正後的代碼不符",
            candidate.name
        );
    }
}

#[test]
fn applying_accepted_corrections_yields_no_new_candidates() {
    // 冪等：把修正寫回後重跑，不應再產生任何候選。
    let corrected: Vec<CityPoint> = ACCEPTED_CASES
        .iter()
        .map(|(case, corrected)| {
            let mut point = city_point(case);
            point.original_admin1 = corrected.to_string();
            point
        })
        .collect();
    let candidates = evaluate(&corrected);
    assert!(
        candidates.is_empty(),
        "修正後不應再有候選，實際 {:?}",
        candidates
            .iter()
            .map(|candidate| (&candidate.name, &candidate.reasons))
            .collect::<Vec<_>>()
    );
}

#[test]
fn every_rejected_candidate_states_at_least_one_reason() {
    // Reason: 通用不變量。verdict=rejected 但 reasons 空白的列，在紀錄檔裡看得到
    // 被拒絕卻讀不出為什麼——這種無聲缺口正是這份檔案要消除的東西。以正反兩批
    // 真實案例一起檢查，避免只有特例被測到。
    let points: Vec<CityPoint> = NE_ALONE_CASES
        .iter()
        .chain(ACCEPTED_CASES.iter().map(|(case, _)| case))
        .map(city_point)
        .collect();
    for candidate in evaluate(&points) {
        if candidate.verdict == Verdict::Rejected {
            assert!(
                !candidate.reasons.is_empty(),
                "{} 被拒絕卻沒有記錄任何原因",
                candidate.name
            );
        }
    }
}
