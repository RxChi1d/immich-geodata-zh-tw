use immich_geodata::pipeline::naer_prepare::{
    NaerPrepareOptions, clean_zh_name, detect_feature_hint, parse_naer_coordinate, run,
};

#[test]
fn coordinate_parser_handles_dirty_formats() {
    // 正常：標準度分
    let (lat, lon) = parse_naer_coordinate("7°25′N99°31′E").unwrap();
    assert!((lat - (7.0 + 25.0 / 60.0)).abs() < 1e-9);
    assert!((lon - (99.0 + 31.0 / 60.0)).abs() < 1e-9);
    // 邊界：HTML entities 與標籤
    let (lat, _) = parse_naer_coordinate("<p>28&deg;30&prime;N36&deg;31&prime;E</p>").unwrap();
    assert!((lat - 28.5).abs() < 1e-9);
    // 邊界：空白與彎引號變體
    assert!(parse_naer_coordinate("22° 37′ N 58° 00′ E").is_some());
    assert!(parse_naer_coordinate("42°58'N5°15'W").is_some());
    // 邊界：無分值、S/W 半球
    let (lat, lon) = parse_naer_coordinate("17°S150°W").unwrap();
    assert!(lat < 0.0 && lon < 0.0);
    // 失敗：緯度重複、缺方向、超界、空值
    assert!(parse_naer_coordinate("32°52′N32°52′N").is_none());
    assert!(parse_naer_coordinate("11°36′N37°23′").is_none());
    assert!(parse_naer_coordinate("160°15′S80°15′E").is_none());
    assert!(parse_naer_coordinate("").is_none());
    // 失敗：分值 >= 60（應拒絕，不得靜默換算為超值座標）
    assert!(parse_naer_coordinate("10°99′N20°99′E").is_none());
    assert!(parse_naer_coordinate("10°60′N20°00′E").is_none());
    assert!(parse_naer_coordinate("10°00′N20°60′E").is_none());
}

#[test]
fn feature_hint_heuristics() {
    // 正常：城市不標記
    assert!(!detect_feature_hint("Paris", "巴黎"));
    // 邊界：英文地物縮寫
    assert!(detect_feature_hint("Abana R. ", "亞巴納河"));
    assert!(detect_feature_hint("Abu Qir Bay", "阿布吉爾灣"));
    // 邊界：中文地形字尾（即使英文無標記）
    assert!(detect_feature_hint("Ban Don", "班洞灣"));
    // 邊界：舊金山英文無標記、中文以山結尾 → 仍標記（降權不刪列，
    // 既有大城市譯名來源充足，影響受控）
    assert!(detect_feature_hint("San Francisco", "舊金山"));
}

#[test]
fn zh_name_cleaning() {
    // 正常
    assert_eq!(clean_zh_name("科魯涅(科倫納)"), "科魯涅");
    // 邊界：方括號與全形括號
    assert_eq!(clean_zh_name("桂武里[縣](奎奴)"), "桂武里");
    assert_eq!(clean_zh_name("艾巴申（別名）"), "艾巴申");
    // 失敗：空字串維持空（由 caller 過濾）
    assert_eq!(clean_zh_name(""), "");
}

#[test]
fn run_produces_six_column_vendored_file() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("raw.csv");
    // 模擬原始 NAER CSV（UTF-8 BOM、引號欄位、髒座標、未知國名）
    std::fs::write(
        &input,
        "\u{feff}序號,英文名稱,中文名稱,所在國,經緯坐標,來源網站\n\
         1,[Ban] Kantang,甘冬,泰國,7°25′N99°31′E,https://x\n\
         2,\"Anec, L.\",阿內克湖,印尼,3°57′S128°03′E,https://x\n\
         3,Bay Minette,貝米內特,美國,30°53′N87°46′W,https://x\n\
         4,Broken,壞座標,美國,160°15′S80°15′E,https://x\n\
         5,Nowhere,未知國,火星國,10°N20°E,https://x\n",
    )
    .unwrap();
    let output = dir.path().join("naer_place_names.csv");
    let report = run(&NaerPrepareOptions {
        input: input.clone(),
        output: output.clone(),
        country_names_file: "data/vendor/i18n-iso-countries/langs/zh-tw.json".into(),
    })
    .unwrap();

    let content = std::fs::read_to_string(&output).unwrap();
    let mut lines = content.lines();
    assert_eq!(
        lines.next().unwrap(),
        "name_norm,name_zh,country_code,latitude,longitude,feature_hint"
    );
    // 正常：泰國對應 TH、註記剝離
    assert!(content.contains("kantang,甘冬,TH,"));
    // 邊界：引號欄位、倒裝、feature_hint=true（L. 標記＋湖字尾）
    assert!(content.contains("anec,阿內克湖,ID,"));
    assert!(
        content
            .lines()
            .find(|l| l.starts_with("anec,"))
            .unwrap()
            .ends_with(",true")
    );
    // 正常：美國
    assert!(content.contains("bay minette,貝米內特,US,"));
    // 失敗列：壞座標被丟棄
    assert!(!content.contains("壞座標"));
    // 邊界：未知國名保留、country_code 留空
    assert!(content.lines().any(|l| l.starts_with("nowhere,未知國,,")));
    // 報告統計
    assert_eq!(report.total, 5);
    assert_eq!(report.written, 4);
    assert_eq!(report.coordinate_failures, 1);
    assert_eq!(report.unmapped_countries, vec!["火星國".to_string()]);
}

#[test]
fn conflict_detection_groups_non_adjacent_rows() {
    // 兩筆同 name_norm＋同國＋距離 <5km 但譯名不同，因 name_zh 排序後不相鄰
    // （中間插一筆其他國家的同 name_norm 列），分組檢查仍須偵測到衝突。
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("raw.csv");
    // springfield US 兩個不同譯名（座標近）＋ springfield CA 一筆，
    // 排序後 CA 列會插在兩個 US 列之間（country_code C < U）。
    std::fs::write(
        &input,
        "\u{feff}序號,英文名稱,中文名稱,所在國,經緯坐標,來源網站\n\
         1,Springfield,斯普林菲爾德,美國,39°48′N89°39′E,https://x\n\
         2,Springfield,春田,加拿大,39°48′N89°39′E,https://x\n\
         3,Springfield,斯普林菲,美國,39°48′N89°39′E,https://x\n",
    )
    .unwrap();
    let output = dir.path().join("naer_place_names.csv");
    let report = run(&NaerPrepareOptions {
        input,
        output,
        country_names_file: "data/vendor/i18n-iso-countries/langs/zh-tw.json".into(),
    })
    .unwrap();
    // 兩個 US 列因排序不相鄰，windows(2) 會漏報；分組檢查應偵測到 1 組衝突。
    assert_eq!(report.conflicts, 1);
}

#[test]
fn suspicious_zero_coordinate_is_counted_and_kept() {
    // 0°N0°E 為可疑座標：解析成功但極可能為缺漏佔位；計數但列仍寫出。
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("raw.csv");
    std::fs::write(
        &input,
        "\u{feff}序號,英文名稱,中文名稱,所在國,經緯坐標,來源網站\n\
         1,Nullisland,空島,美國,0°N0°E,https://x\n",
    )
    .unwrap();
    let output = dir.path().join("naer_place_names.csv");
    let report = run(&NaerPrepareOptions {
        input,
        output: output.clone(),
        country_names_file: "data/vendor/i18n-iso-countries/langs/zh-tw.json".into(),
    })
    .unwrap();
    assert_eq!(report.suspicious_zero_coordinates, 1);
    assert_eq!(report.written, 1);
    let content = std::fs::read_to_string(&output).unwrap();
    assert!(content.contains("nullisland,空島,US,0.000000,0.000000,"));
}

#[test]
fn country_aliases_map_naer_terms_to_iso_codes() {
    // NAER 高頻國名用語與 zh-tw.json 不一致，alias 表須將其補回 ISO 碼。
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("raw.csv");
    std::fs::write(
        &input,
        "\u{feff}序號,英文名稱,中文名稱,所在國,經緯坐標,來源網站\n\
         1,Lae,萊城,巴紐,6°44′S147°00′E,https://x\n\
         2,Mostar,莫斯塔爾,波赫,43°20′N17°48′E,https://x\n\
         3,Dubai,杜拜,阿聯,25°16′N55°18′E,https://x\n\
         4,Port of Spain,西班牙港,千里達及托巴哥,10°39′N61°31′W,https://x\n\
         5,Majuro,馬久羅,馬紹爾群島,7°06′N171°22′E,https://x\n\
         6,Kinshasa,金夏沙,剛果{金夏沙},4°20′S15°19′E,https://x\n\
         7,Brazzaville,布拉薩市,剛果{布拉薩},4°16′S15°17′E,https://x\n",
    )
    .unwrap();
    let output = dir.path().join("naer_place_names.csv");
    let report = run(&NaerPrepareOptions {
        input: input.clone(),
        output: output.clone(),
        country_names_file: "data/vendor/i18n-iso-countries/langs/zh-tw.json".into(),
    })
    .unwrap();

    let content = std::fs::read_to_string(&output).unwrap();
    assert!(content.contains("lae,萊城,PG,"), "巴紐 應對應 PG");
    assert!(content.contains("mostar,莫斯塔爾,BA,"), "波赫 應對應 BA");
    assert!(content.contains("dubai,杜拜,AE,"), "阿聯 應對應 AE");
    assert!(
        content.contains("port of spain,西班牙港,TT,"),
        "千里達及托巴哥 應對應 TT"
    );
    assert!(
        content.contains("majuro,馬久羅,MH,"),
        "馬紹爾群島 應對應 MH"
    );
    // 剛果大括號為消歧註記而非多國邊界，皆為單一主權國家。
    assert!(
        content.contains("kinshasa,金夏沙,CD,"),
        "剛果{{金夏沙}} 應對應 CD"
    );
    assert!(
        content.contains("brazzaville,布拉薩市,CG,"),
        "剛果{{布拉薩}} 應對應 CG"
    );
    // 上述高頻國名皆對應成功，未對應清單不含這些名稱。
    assert!(report.unmapped_countries.is_empty());
}
