use immich_geodata::cli::RunOptions;
use immich_geodata::pipeline::extract;
use std::fs;
use std::path::PathBuf;

fn repo_path(relative: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(relative)
}

/// 確認 Wikidata 國家的 fixture stub 存在。
///
/// Reason: load_context 的離線保證僅靠 stub 檔案存在——stub 缺失時會
/// 靜默改打真實 Wikidata API，把離線測試變成不穩定的網路測試。
/// 在這裡前置斷言，讓缺檔成為明確的測試失敗而非隱性網路依賴。
fn assert_wikidata_stub_exists(country_code: &str) {
    let stub = repo_path("fixtures/extract_handlers/extract_sources")
        .join(format!("{country_code}_wikidata_stub.json"));
    assert!(
        stub.exists(),
        "fixture 缺少 {country_code} 的 Wikidata stub（{}），缺檔會使測試改打真實 Wikidata API",
        stub.display()
    );
}

#[test]
fn wikidata_countries_have_fixture_stubs() {
    for country_code in ["KR", "TH", "ID"] {
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
        fixture: Some("extract_handlers".to_string()),
        fixtures_dir: repo_path("fixtures"),
        output_dir: output_dir.clone(),
    };

    extract::run(&options).unwrap();

    let output = output_dir
        .join("extract_handlers")
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

#[test]
fn indonesia_geospatial_fixture_extracts_translated_rows() {
    assert_wikidata_stub_exists("ID");
    let output_dir =
        std::env::temp_dir().join(format!("immich-geodata-id-extract-{}", std::process::id()));
    let _ = fs::remove_dir_all(&output_dir);

    let options = RunOptions {
        fixture: Some("extract_handlers".to_string()),
        fixtures_dir: repo_path("fixtures"),
        output_dir: output_dir.clone(),
    };

    extract::run(&options).unwrap();

    let output = output_dir
        .join("extract_handlers")
        .join("extract")
        .join("ID.csv");
    let content = fs::read_to_string(&output).unwrap();
    let lines: Vec<&str> = content.lines().collect();

    // Reason: 驗證以下完整邊界案例——
    //   (1) 雅加達城區「Kota Adm.」前綴正規化後翻譯（中雅加達）
    //   (2) 千島群島「Adm. Kep. Seribu」前綴正規化後翻譯（千島群島）
    //   (3) Kota/Kabupaten 同名對（萬隆市 vs 萬隆縣）以 parent-scoped 查詢區分
    //   (4) admin1/admin2 走繁中翻譯、admin3/admin4 沿用 BIG 原文
    //   (5) MultiPolygon（馬魯古／安汶）每 part 各取 centroid 輸出一列
    //       （兩列座標不同，不落海）
    //   (6) WADMPR/WADMKK 空白者（未定義行政區）被過濾不輸出
    //   WIB 代表省（DKI Jakarta）、WITA 代表省（Bali）、WIT 代表省（Maluku）
    //   各至少一筆。
    assert_eq!(
        lines,
        vec![
            "latitude,longitude,country,admin_1,admin_2,admin_3,admin_4",
            "-8.67999994,115.26,印尼,峇里省,登巴薩,Denpasar Selatan,Sanur",
            "-6.89999997,107.61,印尼,西爪哇省,萬隆市,Coblong,Lebak Siliwangi",
            "-6.91999997,107.6,印尼,西爪哇省,萬隆縣,Cicendo,Pasirkaliki",
            "-6.16999998,106.82,印尼,雅加達,中雅加達,Gambir,Gambir",
            "-5.72999999,106.58,印尼,雅加達,千島群島,Kepulauan Seribu Utara,Pulau Kelapa",
            "-3.70000002,128.19,印尼,馬魯古省,安汶,Sirimau,Batu Merah",
            "-3.64000002,128.31,印尼,馬魯古省,安汶,Sirimau,Batu Merah",
        ]
    );

    let _ = fs::remove_dir_all(&output_dir);
}
