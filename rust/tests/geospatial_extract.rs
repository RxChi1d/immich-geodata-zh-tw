use immich_geodata_migration::cli::RunOptions;
use immich_geodata_migration::pipeline::extract;
use std::fs;
use std::path::PathBuf;

fn repo_path(relative: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join(relative)
}

/// 確認 Wikidata 國家的 fixture stub 存在。
///
/// Reason: load_context 的離線保證僅靠 stub 檔案存在——stub 缺失時會
/// 靜默改打真實 Wikidata API，把離線測試變成不穩定的網路測試。
/// 在這裡前置斷言，讓缺檔成為明確的測試失敗而非隱性網路依賴。
fn assert_wikidata_stub_exists(country_code: &str) {
    let stub = repo_path("fixtures/parity/geospatial_extract/extract_sources")
        .join(format!("{country_code}_wikidata_stub.json"));
    assert!(
        stub.exists(),
        "fixture 缺少 {country_code} 的 Wikidata stub（{}），缺檔會使測試改打真實 Wikidata API",
        stub.display()
    );
}

#[test]
fn wikidata_countries_have_fixture_stubs() {
    for country_code in ["KR", "TH"] {
        assert_wikidata_stub_exists(country_code);
    }
}

#[test]
fn thailand_geospatial_fixture_extracts_admin3_rows() {
    assert_wikidata_stub_exists("TH");
    let output_dir =
        std::env::temp_dir().join(format!("immich-geodata-th-extract-{}", std::process::id()));
    let _ = fs::remove_dir_all(&output_dir);

    let options = RunOptions {
        fixture: Some("geospatial_extract".to_string()),
        fixtures_dir: repo_path("fixtures/parity"),
        output_dir: output_dir.clone(),
    };

    extract::run(&options).unwrap();

    let output = output_dir
        .join("geospatial_extract")
        .join("extract")
        .join("TH.csv");
    let content = fs::read_to_string(&output).unwrap();
    let lines: Vec<&str> = content.lines().collect();

    assert_eq!(
        lines,
        vec![
            "latitude,longitude,country,admin_1,admin_2,admin_3,admin_4",
            "13.75128928,100.49209665,泰國,曼谷,帕那空,Phraborom Maharatchawang,",
            "6.59758329,99.54902264,泰國,沙敦,沙敦府治縣,Ko Sarai,",
            "19.77429073,99.22556549,泰國,清邁,芳縣,Mae Kha,",
            "19.81745339,99.27628673,泰國,清邁,芳縣,Mae Kha,",
        ]
    );

    let _ = fs::remove_dir_all(&output_dir);
}
