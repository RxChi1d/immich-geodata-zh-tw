use std::io::Write;

use immich_geodata::pipeline::translate::{ProductionTranslateOptions, run_production};

// 建構最小 production translate 輸入：cities/admin1/alternate/metadata/naer。
#[test]
fn naer_layer_fills_overrides_and_demotes() {
    let dir = tempfile::tempdir().unwrap();
    let write = |name: &str, content: &str| {
        let path = dir.path().join(name);
        std::fs::File::create(&path)
            .unwrap()
            .write_all(content.as_bytes())
            .unwrap();
        path
    };
    // cities500 19 欄：0=id 1=name 2=ascii 3=alts 4=lat 5=lon 8=cc 10=admin1 17=tz
    let city = |id: &str, name: &str, alts: &str, lat: &str, lon: &str, cc: &str, a1: &str| {
        let mut row = vec![String::new(); 19];
        row[0] = id.into();
        row[1] = name.into();
        row[2] = name.into();
        row[3] = alts.into();
        row[4] = lat.into();
        row[5] = lon.into();
        row[8] = cc.into();
        row[10] = a1.into();
        row.join("\t")
    };
    let cities = [
        // 案例 1（補洞）：無任何既有中文 → NAER 高信心填入
        city("1", "Fillville", "", "30.0", "-87.0", "US", "AL"),
        // 案例 2（覆寫）：alternate 有簡轉繁譯名 → NAER 高信心覆寫
        city("2", "Overridetown", "", "31.0", "-86.0", "US", "AL"),
        // 案例 3（降級保留）：alternate 有譯名、NAER 為 feature_hint → 保留既有
        city("3", "Demoteton", "", "32.0", "-85.0", "US", "AL"),
    ]
    .join("\n");
    let cities_file = write("cities500_optimized.txt", &cities);
    // admin1 4 欄：US.AL 無 alternate 譯名 → NAER 補洞
    let admin1_file = write(
        "admin1CodesASCII_optimized.txt",
        "US.AL\tAlabama\tAlabama\t4829764\n",
    );
    let alternate_file = write(
        "alternate_chinese_name.csv",
        "geoname_id,name\n2,奥弗赖德镇\n3,德莫顿\n",
    );
    let naer_file = write(
        "naer_place_names.csv",
        "name_norm,name_zh,country_code,latitude,longitude,feature_hint\n\
         alabama,阿拉巴馬,US,31.0,-86.0,false\n\
         demoteton,德莫頓灣,US,32.0,-85.0,true\n\
         fillville,菲爾維爾,US,30.0,-87.0,false\n\
         overridetown,歐弗萊鎮,US,31.0,-86.0,false\n",
    );
    let metadata_dir = dir.path().join("meta_data");
    std::fs::create_dir_all(&metadata_dir).unwrap();
    let output_dir = dir.path().join("out");

    run_production(&ProductionTranslateOptions {
        metadata_dir,
        data_dir: dir.path().to_path_buf(),
        cities_file,
        admin1_file,
        alternate_name_file: alternate_file,
        naer_file,
        output_dir: output_dir.clone(),
        profile: false,
    })
    .unwrap();

    let cities_out = std::fs::read_to_string(output_dir.join("cities500_translated.txt")).unwrap();
    // 正常：補洞
    assert!(cities_out.contains("菲爾維爾"), "{cities_out}");
    // 邊界：高信心覆寫既有簡轉繁譯名
    assert!(cities_out.contains("歐弗萊鎮"), "{cities_out}");
    assert!(!cities_out.contains("奧弗賴德鎮"));
    // 邊界：feature_hint 中信心 → 保留既有譯名（經 OpenCC 轉繁）
    assert!(cities_out.contains("德莫頓"), "{cities_out}");
    assert!(!cities_out.contains("德莫頓灣"));

    let admin1_out =
        std::fs::read_to_string(output_dir.join("admin1CodesASCII_translated.txt")).unwrap();
    // 正常：admin1 補洞
    assert!(admin1_out.contains("阿拉巴馬"), "{admin1_out}");
}

#[test]
fn missing_naer_file_fails_fast() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("meta_data")).unwrap();
    let touch = |name: &str, content: &str| {
        let path = dir.path().join(name);
        std::fs::write(&path, content).unwrap();
        path
    };
    let error = run_production(&ProductionTranslateOptions {
        metadata_dir: dir.path().join("meta_data"),
        data_dir: dir.path().to_path_buf(),
        cities_file: touch("cities500_optimized.txt", ""),
        admin1_file: touch("admin1CodesASCII_optimized.txt", ""),
        alternate_name_file: touch("alternate_chinese_name.csv", "geoname_id,name\n"),
        naer_file: dir.path().join("missing_naer.csv"),
        output_dir: dir.path().join("out"),
        profile: false,
    })
    .unwrap_err();
    assert!(error.contains("naer-prepare"), "{error}");
}
