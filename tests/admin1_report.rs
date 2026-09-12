use std::collections::BTreeMap;
use std::fs;

use immich_geodata::pipeline::admin1_correct::{
    CityPoint, PointSample, evaluate_points, learn_admin1_mapping,
};
use immich_geodata::pipeline::admin1_report::{fixes_csv_path, report_lines, write_fixes_csv};
use immich_geodata::pipeline::ne_admin1::NeAdmin1Index;

fn fixture_index() -> NeAdmin1Index {
    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures/admin1_correct/ne_admin1_my_th.geojson");
    NeAdmin1Index::load(&path).expect("fixture NE GeoJSON 應可載入")
}

fn gn_id_to_code() -> BTreeMap<i64, String> {
    [
        (1733037, "MY.12"),
        (1733041, "MY.07"),
        (1733042, "MY.06"),
        (1733046, "MY.14"),
        (1733048, "MY.02"),
        (1733049, "MY.01"),
    ]
    .into_iter()
    .map(|(id, code)| (id, code.to_string()))
    .collect()
}

fn known_codes() -> BTreeMap<String, String> {
    gn_id_to_code()
        .into_values()
        .map(|code| (code.clone(), code))
        .collect()
}

fn training() -> Vec<PointSample> {
    [
        ("雪兰莪州", 101.428, 3.3078),
        ("吉隆坡", 101.698, 3.13836),
        ("彭亨州", 102.491, 3.84557),
    ]
    .into_iter()
    .flat_map(|(name, longitude, latitude)| {
        (0..20).map(move |_| PointSample {
            longitude,
            latitude,
            locationiq_admin1: name.to_string(),
        })
    })
    .collect()
}

fn point(
    id: &str,
    name: &str,
    longitude: f64,
    latitude: f64,
    admin1: &str,
    liq: &str,
) -> CityPoint {
    CityPoint {
        geoname_id: id.to_string(),
        name: name.to_string(),
        longitude,
        latitude,
        country_code: "MY".to_string(),
        original_admin1: admin1.to_string(),
        locationiq_admin1: Some(liq.to_string()),
    }
}

fn sample_points() -> Vec<CityPoint> {
    vec![
        // 採納：兩來源一致且與原值不符。
        point("1735162", "Setapak", 101.727, 3.207, "12", "吉隆坡"),
        // 拒絕：NE 獨排眾議。
        point("13100287", "Mukim Tioman", 104.17, 2.7825, "01", "彭亨州"),
        // 拒絕：名稱無可信映射。名字刻意含逗號與引號，驗證跳脫。
        point(
            "9999",
            "Town, \"Quoted\"",
            101.62246,
            3.12036,
            "14",
            "未知州",
        ),
    ]
}

fn candidates() -> Vec<immich_geodata::pipeline::admin1_correct::Candidate> {
    let index = fixture_index();
    let codes = gn_id_to_code();
    let mapping = learn_admin1_mapping(&training(), &index, &codes);
    evaluate_points(&sample_points(), &index, &mapping, &codes, &known_codes())
}

fn temp_dir() -> tempfile::TempDir {
    tempfile::tempdir().expect("建立暫存目錄")
}

#[test]
fn fixes_csv_path_follows_locationiq_naming() {
    let path = fixes_csv_path(std::path::Path::new("data/locationiq"), "MY");
    assert!(path.ends_with("MY_admin1_fixes.csv"));
}

#[test]
fn csv_contains_both_verdicts_and_is_sorted() {
    let dir = temp_dir();
    let path = dir.path().join("MY_admin1_fixes.csv");
    write_fixes_csv(&path, &candidates()).expect("寫出應成功");
    let content = fs::read_to_string(&path).expect("讀取");
    let lines: Vec<&str> = content.lines().collect();

    assert!(lines[0].starts_with("geoname_id,"), "第一行應為表頭");
    let ids: Vec<&str> = lines[1..]
        .iter()
        .map(|line| line.split(',').next().unwrap())
        .collect();
    let mut sorted = ids.clone();
    sorted.sort_unstable();
    assert_eq!(ids, sorted, "資料列應依 geoname_id 排序");
    assert!(content.contains("accepted"), "應含採納列");
    assert!(content.contains("rejected"), "應含拒絕列");
}

#[test]
fn csv_is_byte_identical_across_runs() {
    // Reason: 這個檔案的用途是跨週 diff。只要輸出不穩定，每週都會產生假差異，
    // review 的人第三週就開始跳過它，觀測面等於失效。
    let dir = temp_dir();
    let first = dir.path().join("first.csv");
    let second = dir.path().join("second.csv");
    write_fixes_csv(&first, &candidates()).expect("第一次寫出");
    write_fixes_csv(&second, &candidates()).expect("第二次寫出");
    assert_eq!(
        fs::read(&first).unwrap(),
        fs::read(&second).unwrap(),
        "兩次寫出必須逐位元相同"
    );
}

#[test]
fn csv_schema_is_fixed_with_no_timestamp_column() {
    // Reason: 原本以「內容不含當年年份」偵測時間戳，但座標與 geoname_id 本來就
    // 可能含到年份數字（`{:.5}` 的 3.20260、geoname_id 12026xxx），那個斷言會在
    // 某一年無預警轉紅。改成釘死表頭：多出時間戳欄位一樣擋得住，且與日期無關。
    let dir = temp_dir();
    let path = dir.path().join("MY_admin1_fixes.csv");
    write_fixes_csv(&path, &candidates()).expect("寫出");
    let content = fs::read_to_string(&path).unwrap();
    assert_eq!(
        content.lines().next(),
        Some(
            "geoname_id,name,latitude,longitude,original_admin1,natural_earth_admin1,\
             locationiq_admin1_name,locationiq_admin1,boundary_km,verdict,reasons"
        ),
        "表頭應與 HEADER 完全一致：{content}"
    );
}

#[test]
fn rejected_row_lists_reasons_with_primary_first() {
    let dir = temp_dir();
    let path = dir.path().join("MY_admin1_fixes.csv");
    write_fixes_csv(&path, &candidates()).expect("寫出");
    let content = fs::read_to_string(&path).unwrap();
    let tioman = content
        .lines()
        .find(|line| line.contains("Mukim Tioman"))
        .expect("應有 Mukim Tioman 列");
    assert!(
        tioman.contains("sources_do_not_agree"),
        "應記錄主因：{tioman}"
    );
}

#[test]
fn field_containing_comma_is_quoted() {
    let dir = temp_dir();
    let path = dir.path().join("MY_admin1_fixes.csv");
    write_fixes_csv(&path, &candidates()).expect("寫出");
    let content = fs::read_to_string(&path).unwrap();
    assert!(
        content.contains("\"Town, \"\"Quoted\"\"\""),
        "含逗號與引號的名稱應被正確跳脫：{content}"
    );
}

#[test]
fn report_lines_cover_all_four_categories() {
    // Reason: handoff §3 要求四類報告缺一不可。第 3 類「來源不一致但未修正」
    // 沒有的話，整州學錯會隱形；第 4 類是既有缺陷的持續能見度。
    let index = fixture_index();
    let codes = gn_id_to_code();
    let mapping = learn_admin1_mapping(&training(), &index, &codes);
    let lines = report_lines("MY", &candidates(), &mapping, 791);
    let joined = lines.join("\n");
    for key in [
        "no_trusted_mapping=",
        "natural_earth_unusable=",
        "sources_do_not_agree=",
        "original_code_unknown=",
        "accepted=",
        "queried_points=791",
    ] {
        assert!(joined.contains(key), "報告缺少 {key}：\n{joined}");
    }
}
