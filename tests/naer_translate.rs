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
    let metadata_dir = dir.path().join("locationiq");
    std::fs::create_dir_all(&metadata_dir).unwrap();
    let output_dir = dir.path().join("out");

    let stats = run_production(&ProductionTranslateOptions {
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

    // 品質統計：採用計數逐項對應三個案例 + admin1 補洞
    assert_eq!(stats.city_fill, 1, "{}", stats.log_line());
    assert_eq!(stats.city_override, 1, "{}", stats.log_line());
    assert_eq!(stats.city_demoted_kept_existing, 1, "{}", stats.log_line());
    assert_eq!(stats.admin1_fill, 1, "{}", stats.log_line());
    // 邊界：距離分布只計「被採用」匹配（補洞+覆寫=2），demote 不記錄
    let city_total = stats.city_distance.near + stats.city_distance.mid + stats.city_distance.far;
    assert_eq!(city_total, 2, "{}", stats.log_line());
}

// 共用：寫檔輔助與最小 cities500 列建構。
fn write_file(dir: &std::path::Path, name: &str, content: &str) -> std::path::PathBuf {
    let path = dir.join(name);
    std::fs::File::create(&path)
        .unwrap()
        .write_all(content.as_bytes())
        .unwrap();
    path
}

fn city_row(id: &str, name: &str, alts: &str, lat: &str, lon: &str, cc: &str, a1: &str) -> String {
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
}

#[test]
fn naer_applied_name_bypasses_inversion_replacement() {
    // 驗證：naer_applied 的官方審譯名原樣保留，不經 '裏'→'里' 後處理。
    let dir = tempfile::tempdir().unwrap();
    // 城市無既有中文 → NAER 高信心補洞；name_zh 含「裏」字
    let cities = city_row("10", "Innerton", "", "40.0", "-100.0", "US", "CA");
    let cities_file = write_file(dir.path(), "cities500_optimized.txt", &cities);
    let admin1_file = write_file(
        dir.path(),
        "admin1CodesASCII_optimized.txt",
        "US.CA\tCalifornia\tCalifornia\t5332921\n",
    );
    let alternate_file = write_file(
        dir.path(),
        "alternate_chinese_name.csv",
        "geoname_id,name\n",
    );
    let naer_file = write_file(
        dir.path(),
        "naer_place_names.csv",
        "name_norm,name_zh,country_code,latitude,longitude,feature_hint\n\
         innerton,裏鎮,US,40.0,-100.0,false\n",
    );
    let metadata_dir = dir.path().join("locationiq");
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
    // 邊界：NAER 譯名保留「裏」，未被替換成「里」
    assert!(cities_out.contains("裏鎮"), "{cities_out}");
    assert!(!cities_out.contains("里鎮"), "{cities_out}");
}

#[test]
fn handler_country_skips_naer_end_to_end() {
    // 驗證：handler 國家即使名稱與座標皆命中 NAER，也不套用 NAER 譯名。
    // Reason: 用 JP 而非 TW——TW 在 translate_cities_rows 有更早的特殊分支
    // 不會進入 lookup_city，唯有 JP/KR/TH 走一般路徑才能鎖住 lookup 內部的
    // handler 跳過機制。
    let dir = tempfile::tempdir().unwrap();
    // JP 城市，name 在 NAER 中有對應且座標一致
    let cities = city_row("20", "Handlertown", "", "35.7", "139.7", "JP", "13");
    let cities_file = write_file(dir.path(), "cities500_optimized.txt", &cities);
    let admin1_file = write_file(
        dir.path(),
        "admin1CodesASCII_optimized.txt",
        "JP.13\tTokyo\tTokyo\t1850144\n",
    );
    let alternate_file = write_file(
        dir.path(),
        "alternate_chinese_name.csv",
        "geoname_id,name\n",
    );
    let naer_file = write_file(
        dir.path(),
        "naer_place_names.csv",
        "name_norm,name_zh,country_code,latitude,longitude,feature_hint\n\
         handlertown,韓德勒鎮,JP,35.7,139.7,false\n",
    );
    let metadata_dir = dir.path().join("locationiq");
    std::fs::create_dir_all(&metadata_dir).unwrap();
    let output_dir = dir.path().join("out");

    let stats = run_production(&ProductionTranslateOptions {
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
    // JP 為 handler 國家 → lookup_city 直接跳過，NAER 譯名不套用
    assert!(!cities_out.contains("韓德勒鎮"), "{cities_out}");
    // 無既有譯名 → 保留原始 name
    assert!(cities_out.contains("Handlertown"), "{cities_out}");
    // handler 跳過屬常態，不計入任何採用或拒絕統計
    assert_eq!(stats, Default::default(), "{}", stats.log_line());
}

#[test]
fn missing_naer_file_fails_fast() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("locationiq")).unwrap();
    let touch = |name: &str, content: &str| {
        let path = dir.path().join(name);
        std::fs::write(&path, content).unwrap();
        path
    };
    let error = run_production(&ProductionTranslateOptions {
        metadata_dir: dir.path().join("locationiq"),
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
