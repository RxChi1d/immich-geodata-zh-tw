use std::collections::HashMap;
use std::io::Write;

use immich_geodata::pipeline::naer_lookup::{
    NaerConfidence, NaerLookup, build_admin1_centroids, distance_km, normalize_lookup_name,
};

const HEADER: &str = "name_norm,name_zh,country_code,latitude,longitude,feature_hint\n";

fn write_naer_file(dir: &std::path::Path, rows: &[&str]) -> std::path::PathBuf {
    let path = dir.join("naer_place_names.csv");
    let mut file = std::fs::File::create(&path).unwrap();
    file.write_all(HEADER.as_bytes()).unwrap();
    for row in rows {
        writeln!(file, "{row}").unwrap();
    }
    path
}

#[test]
fn lookup_city_confidence_tiers() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_naer_file(
        dir.path(),
        &[
            "bay minette,貝米內特,US,30.8829,-87.7730,false", // 高信心
            "ban don,班洞灣,VN,10.0000,106.0000,true",        // feature_hint → 中
            "nocountry,無國碼鎮,,10.5000,20.5000,false",      // 空國碼 → 中
            "twincity,雙城甲,US,40.0000,-100.0000,false",     // 近距歧義 → 中
            "twincity,雙城乙,US,40.0100,-100.0100,false",
        ],
    );
    let lookup = NaerLookup::load(&path).unwrap();

    // 正常：國碼一致＋距離近＋非地物＋無歧義 → 高信心
    let m = lookup
        .lookup_city("Bay Minette", "Bay Minette", 30.8830, -87.7740, "US")
        .unwrap();
    assert_eq!(m.name_zh, "貝米內特");
    assert_eq!(m.confidence, NaerConfidence::High);

    // 邊界：feature_hint → 中信心
    let m = lookup
        .lookup_city("Ban Don", "Ban Don", 10.0010, 106.0010, "VN")
        .unwrap();
    assert_eq!(m.confidence, NaerConfidence::Medium);

    // 邊界：僅空國碼候選 → 中信心
    let m = lookup
        .lookup_city("Nocountry", "Nocountry", 10.5001, 20.5001, "ZZ")
        .unwrap();
    assert_eq!(m.name_zh, "無國碼鎮");
    assert_eq!(m.confidence, NaerConfidence::Medium);

    // 邊界：容差內兩個不同譯名且距離差 < 5 km → 中信心
    let m = lookup
        .lookup_city("Twincity", "Twincity", 40.0050, -100.0050, "US")
        .unwrap();
    assert_eq!(m.confidence, NaerConfidence::Medium);

    // 失敗：距離超限拒絕
    assert!(
        lookup
            .lookup_city("Bay Minette", "Bay Minette", 31.5, -87.7, "US")
            .is_none()
    );
    // 失敗：國碼不一致排除（座標相同也不行）
    assert!(
        lookup
            .lookup_city("Bay Minette", "Bay Minette", 30.8830, -87.7740, "CA")
            .is_none()
    );
    // 邊界：handler 國家直接跳過
    assert!(
        lookup
            .lookup_city("Bay Minette", "Bay Minette", 30.8830, -87.7740, "TW")
            .is_none()
    );
}

#[test]
fn lookup_city_merges_name_and_ascii_candidates() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_naer_file(dir.path(), &["zürich,蘇黎世,CH,47.3769,8.5417,false"]);
    let lookup = NaerLookup::load(&path).unwrap();
    // 非拉丁/變音 name 與 ascii_name 各自正規化後合併查詢
    let m = lookup
        .lookup_city("Zürich", "Zurich", 47.3770, 8.5420, "CH")
        .unwrap();
    assert_eq!(m.name_zh, "蘇黎世");
}

#[test]
fn lookup_admin1_fill_only_rules() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_naer_file(
        dir.path(),
        &[
            "alabama,阿拉巴馬,US,32.7794,-86.8287,false",
            "georgia,喬治亞,US,32.6781,-83.2229,false",
            "georgia,喬治亞國,,42.0000,43.5000,false", // 空國碼：admin1 不接受
        ],
    );
    let lookup = NaerLookup::load(&path).unwrap();
    let centroid = Some((32.8, -86.8));

    // 正常：US.AL 取出 US、名稱命中、質心驗證通過
    assert_eq!(
        lookup.lookup_admin1("Alabama", "Alabama", "US.AL", centroid),
        Some("阿拉巴馬".to_string())
    );
    // 邊界：GB.ENG 字母後綴——前綴 split 取 GB，無候選回 None 而非 panic
    assert_eq!(
        lookup.lookup_admin1("England", "England", "GB.ENG", centroid),
        None
    );
    // 邊界：多句點僅以第一個 split
    assert_eq!(
        lookup.lookup_admin1("Alabama", "Alabama", "US.AL.X", centroid),
        Some("阿拉巴馬".to_string())
    );
    // 失敗：畸形 code（無 '.'／空）
    assert_eq!(
        lookup.lookup_admin1("Alabama", "Alabama", "US", centroid),
        None
    );
    assert_eq!(
        lookup.lookup_admin1("Alabama", "Alabama", "", centroid),
        None
    );
    // 失敗：無質心（無轄下城市）→ 保守 None
    assert_eq!(
        lookup.lookup_admin1("Alabama", "Alabama", "US.AL", None),
        None
    );
    // 失敗：質心距離超過門檻（門檻 300 km）
    assert_eq!(
        lookup.lookup_admin1("Alabama", "Alabama", "US.AL", Some((50.0, 10.0))),
        None
    );
    // 邊界：空國碼候選不參與 admin1（喬治亞國不會干擾喬治亞州）
    assert_eq!(
        lookup.lookup_admin1("Georgia", "Georgia", "US.GA", Some((32.7, -83.2))),
        Some("喬治亞".to_string())
    );
}

#[test]
fn lookup_admin1_rejects_ambiguous_distinct_translations() {
    // 模擬 colorado 案例：同國同 name_norm、座標極近但譯名不同（科羅拉多 vs 科羅拉多州），
    // 質心無法可靠消歧 → 保守回 None。
    let dir = tempfile::tempdir().unwrap();
    let path = write_naer_file(
        dir.path(),
        &[
            "colorado,科羅拉多,US,39.0000,-105.5000,false",
            "colorado,科羅拉多州,US,39.0100,-105.5100,false",
        ],
    );
    let lookup = NaerLookup::load(&path).unwrap();
    // 質心落在兩候選之間，最近與次近距離差 < 5 km 且譯名不同 → 歧義放棄
    assert_eq!(
        lookup.lookup_admin1("Colorado", "Colorado", "US.CO", Some((39.005, -105.505))),
        None
    );
}

#[test]
fn lookup_admin1_accepts_duplicate_same_translation() {
    // 對照組：同譯名重複列（座標近）不構成歧義 → 正常回傳。
    let dir = tempfile::tempdir().unwrap();
    let path = write_naer_file(
        dir.path(),
        &[
            "ohio,俄亥俄,US,40.0000,-82.9000,false",
            "ohio,俄亥俄,US,40.0100,-82.9100,false",
        ],
    );
    let lookup = NaerLookup::load(&path).unwrap();
    assert_eq!(
        lookup.lookup_admin1("Ohio", "Ohio", "US.OH", Some((40.005, -82.905))),
        Some("俄亥俄".to_string())
    );
}

#[test]
fn build_admin1_centroids_averages_city_coords() {
    // cities500 schema：col4=lat col5=lon col8=country col10=admin1
    let row = |lat: &str, lon: &str, cc: &str, a1: &str| -> Vec<String> {
        let mut r = vec![String::new(); 19];
        r[4] = lat.into();
        r[5] = lon.into();
        r[8] = cc.into();
        r[10] = a1.into();
        r
    };
    let rows = vec![
        row("30.0", "-87.0", "US", "AL"),
        row("32.0", "-86.0", "US", "AL"),
        row("48.0", "2.0", "FR", "11"),
        row("0.0", "0.0", "", ""), // 空欄位忽略
    ];
    let centroids: HashMap<String, (f64, f64)> = build_admin1_centroids(&rows);
    let (lat, lon) = centroids.get("US.AL").copied().unwrap();
    assert!((lat - 31.0).abs() < 1e-9 && (lon + 86.5).abs() < 1e-9);
    assert!(centroids.contains_key("FR.11"));
    assert_eq!(centroids.len(), 2);
}

#[test]
fn load_rejects_malformed_files() {
    let dir = tempfile::tempdir().unwrap();
    // 失敗：欄位數錯誤
    let path = write_naer_file(dir.path(), &["only,three,cols"]);
    let err = NaerLookup::load(&path).unwrap_err();
    assert!(err.contains("欄位數"), "{err}");
    // 失敗：座標非數值
    let path = write_naer_file(dir.path(), &["x,某,US,abc,1.0,false"]);
    assert!(NaerLookup::load(&path).is_err());
    // 失敗：檔案不存在 → 錯誤訊息含重生成指引
    let err = NaerLookup::load(&dir.path().join("missing.csv")).unwrap_err();
    assert!(err.contains("naer-prepare"), "{err}");
    // 失敗：NaN 座標（含行號）
    let path = write_naer_file(dir.path(), &["x,某,US,NaN,1.0,false"]);
    let err = NaerLookup::load(&path).unwrap_err();
    assert!(err.contains("第 2 行"), "{err}");
    // 失敗：越界座標
    let path = write_naer_file(dir.path(), &["x,某,US,91.0,1.0,false"]);
    assert!(NaerLookup::load(&path).is_err());
    let path = write_naer_file(dir.path(), &["x,某,US,10.0,181.0,false"]);
    assert!(NaerLookup::load(&path).is_err());
    // 失敗：空 name_zh
    let path = write_naer_file(dir.path(), &["x,,US,10.0,20.0,false"]);
    assert!(NaerLookup::load(&path).is_err());
    // 失敗：feature_hint 非 true/false 不可靜默當 false
    let path = write_naer_file(dir.path(), &["x,某,US,10.0,20.0,yes"]);
    assert!(NaerLookup::load(&path).is_err());
}

#[test]
fn normalize_strips_annotations_inversion_and_diacritics() {
    // 正常
    assert_eq!(normalize_lookup_name("Paris"), "paris");
    // 邊界：方括號註記、圓括號別名、逗號倒裝、變音符號、多重空白
    assert_eq!(normalize_lookup_name("[Ban] Kantang"), "kantang");
    assert_eq!(normalize_lookup_name("Al Wajh (Wejh)"), "al wajh");
    assert_eq!(normalize_lookup_name("Abiad,  Bahr el"), "abiad");
    assert_eq!(normalize_lookup_name("Mazār-e Sharīf"), "mazar-e sharif");
    assert_eq!(normalize_lookup_name("  A   Coruna "), "a coruna");
    // 失敗：空字串
    assert_eq!(normalize_lookup_name(""), "");
}

#[test]
fn distance_handles_equirectangular_and_dateline() {
    // 正常：赤道 1 度經度 ≈ 111 km
    let d = distance_km(0.0, 0.0, 0.0, 1.0);
    assert!((d - 111.0).abs() < 1.0, "got {d}");
    // 邊界：跨 ±180° 日期變更線，兩點實際相距約 222 km 而非近 360 度
    let d = distance_km(0.0, 179.0, 0.0, -179.0);
    assert!(d < 250.0, "dateline not normalized: got {d}");
    // 失敗情境由型別保證（f64 輸入），驗證同點為 0
    assert_eq!(distance_km(25.0, 121.0, 25.0, 121.0), 0.0);
}
