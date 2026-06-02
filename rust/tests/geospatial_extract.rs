use immich_geodata_migration::cli::RunOptions;
use immich_geodata_migration::pipeline::extract;
use std::fs;
use std::path::PathBuf;

fn repo_path(relative: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join(relative)
}

#[test]
fn thailand_geospatial_fixture_extracts_admin3_rows() {
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
