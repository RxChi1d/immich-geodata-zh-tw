//! 驗證 admin1 修正器在 translate 階段的接線：欄位索引、寫回 cities500 列、
//! 紀錄檔輸出位置，以及缺少 NE 圖資時的略過行為。

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use immich_geodata::pipeline::admin1_correct_stage::{self, MetadataAdmin1};

fn fixture_natural_earth() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures/admin1_correct/ne_admin1_my_th.geojson")
}

/// `admin1CodesASCII` 格式：代碼、名稱、ASCII 名稱、geonameid。
fn admin1_rows() -> Vec<Vec<String>> {
    [
        ("MY.01", "Johor", 1733049),
        ("MY.02", "Kedah", 1733048),
        ("MY.06", "Pahang", 1733042),
        ("MY.07", "Perak", 1733041),
        ("MY.12", "Selangor", 1733037),
        ("MY.14", "Kuala Lumpur", 1733046),
    ]
    .into_iter()
    .map(|(code, name, gn_id)| {
        vec![
            code.to_string(),
            name.to_string(),
            name.to_string(),
            gn_id.to_string(),
        ]
    })
    .collect()
}

/// 19 欄的 cities500 列，只填修正器會讀寫的欄位。
fn city_row(
    geoname_id: &str,
    name: &str,
    latitude: &str,
    longitude: &str,
    country_code: &str,
    admin1: &str,
    admin2: &str,
) -> Vec<String> {
    let mut row = vec![String::new(); 19];
    row[0] = geoname_id.to_string();
    row[1] = name.to_string();
    row[4] = latitude.to_string();
    row[5] = longitude.to_string();
    row[8] = country_code.to_string();
    row[10] = admin1.to_string();
    row[11] = admin2.to_string();
    row[12] = "L3".to_string();
    row[13] = "L4".to_string();
    row
}

/// 以 NE 標註點重複 20 次建立可信映射，另加三個待判定的真實案例。
///
/// Reason: 訓練樣本與待判定案例完全分離，避免拿要檢驗的點去教會映射認得答案。
fn metadata() -> MetadataAdmin1 {
    let mut metadata = HashMap::new();
    for (name, longitude, latitude) in [
        ("雪兰莪州", 101.428, 3.3078),
        ("吉隆坡", 101.698, 3.13836),
        ("彭亨州", 102.491, 3.84557),
    ] {
        for offset in 0..20 {
            // Reason: metadata 鍵是字串，同一個座標只會留下一筆。加上極小的
            // 偏移讓 20 個樣本成為不同的鍵，同時仍落在同一個多邊形深處。
            let latitude = format!("{:.6}", latitude + offset as f64 * 0.0001);
            metadata.insert(
                ("MY".to_string(), latitude, format!("{longitude}")),
                name.to_string(),
            );
        }
    }
    for (latitude, longitude, name) in [
        ("3.207", "101.727", "吉隆坡"),    // Setapak：應修正
        ("2.7825", "104.17", "彭亨州"),    // Mukim Tioman：NE 獨排眾議，應拒絕
        ("3.3078", "101.428", "雪兰莪州"), // 與原值一致：不應成為候選
    ] {
        metadata.insert(
            (
                "MY".to_string(),
                latitude.to_string(),
                longitude.to_string(),
            ),
            name.to_string(),
        );
    }
    metadata
}

fn cities_rows() -> Vec<Vec<String>> {
    vec![
        city_row("1735162", "Setapak", "3.207", "101.727", "MY", "12", "A01"),
        city_row(
            "13100287",
            "Mukim Tioman",
            "2.7825",
            "104.17",
            "MY",
            "01",
            "",
        ),
        city_row("9001", "Agreeing", "3.3078", "101.428", "MY", "12", ""),
        // 另一國的列不得被動到。
        city_row("9002", "Elsewhere", "35.6", "139.7", "JP", "13", "B01"),
    ]
}

fn run_stage(natural_earth: &Path, dir: &Path) -> Result<Vec<Vec<String>>, String> {
    let mut rows = cities_rows();
    admin1_correct_stage::run(
        &mut rows,
        &admin1_rows(),
        &metadata(),
        natural_earth,
        &dir.join("admin2Codes.txt"),
        dir,
    )?;
    Ok(rows)
}

#[test]
fn accepted_correction_is_written_back_into_cities_rows() {
    let dir = tempfile::tempdir().expect("暫存目錄");
    let rows = run_stage(&fixture_natural_earth(), dir.path()).expect("應成功");
    assert_eq!(rows[0][10], "14", "Setapak 的 admin1 應被改為吉隆坡");
    assert_eq!(rows[0][12], "L3", "admin3 不得被更動");
    assert_eq!(rows[0][13], "L4", "admin4 不得被更動");
}

#[test]
fn rejected_and_agreeing_rows_keep_their_original_admin1() {
    let dir = tempfile::tempdir().expect("暫存目錄");
    let rows = run_stage(&fixture_natural_earth(), dir.path()).expect("應成功");
    assert_eq!(rows[1][10], "01", "Mukim Tioman 應保留原值");
    assert_eq!(rows[2][10], "12", "兩來源一致的列應保留原值");
}

#[test]
fn rows_of_countries_without_metadata_are_untouched() {
    let dir = tempfile::tempdir().expect("暫存目錄");
    let rows = run_stage(&fixture_natural_earth(), dir.path()).expect("應成功");
    assert_eq!(rows[3][10], "13");
    assert_eq!(rows[3][11], "B01");
}

#[test]
fn fixes_csv_is_written_next_to_locationiq_results() {
    let dir = tempfile::tempdir().expect("暫存目錄");
    run_stage(&fixture_natural_earth(), dir.path()).expect("應成功");
    let path = dir.path().join("MY_admin1_fixes.csv");
    assert!(path.exists(), "應寫出 MY_admin1_fixes.csv");
    let content = fs::read_to_string(&path).expect("讀取");
    assert!(content.contains("Setapak"), "應含採納列：{content}");
    assert!(content.contains("Mukim Tioman"), "應含拒絕列：{content}");
    assert!(
        !content.contains("Agreeing"),
        "兩來源一致的列不應進入紀錄：{content}"
    );
}

#[test]
fn missing_natural_earth_file_skips_without_error() {
    // Reason: NE 圖資由 prepare 下載。尚未下載時 translate 仍應跑完其餘工作，
    // 而不是整個 release 失敗。
    let dir = tempfile::tempdir().expect("暫存目錄");
    let rows = run_stage(&dir.path().join("does-not-exist.geojson"), dir.path())
        .expect("缺檔應略過而非失敗");
    assert_eq!(rows[0][10], "12", "略過時不得修改任何 admin1");
    assert!(!dir.path().join("MY_admin1_fixes.csv").exists());
}

#[test]
fn admin2_is_cleared_only_when_the_new_key_exists() {
    let dir = tempfile::tempdir().expect("暫存目錄");
    fs::write(
        dir.path().join("admin2Codes.txt"),
        "MY.14.A01\tSome District\tSome District\t9999999\n",
    )
    .expect("寫入 admin2Codes");
    let rows = run_stage(&fixture_natural_earth(), dir.path()).expect("應成功");
    assert_eq!(rows[0][10], "14");
    assert_eq!(rows[0][11], "", "新鍵存在時 admin2 應被清空");
}

#[test]
fn admin2_survives_when_the_new_key_does_not_exist() {
    let dir = tempfile::tempdir().expect("暫存目錄");
    fs::write(
        dir.path().join("admin2Codes.txt"),
        "MY.14.ZZZ\tOther District\tOther District\t9999999\n",
    )
    .expect("寫入 admin2Codes");
    let rows = run_stage(&fixture_natural_earth(), dir.path()).expect("應成功");
    assert_eq!(rows[0][10], "14");
    assert_eq!(rows[0][11], "A01", "新鍵不存在時 admin2 應保留");
}

#[test]
fn learned_mapping_ignores_cities500_admin1_values() {
    // handoff §6 的不變量：竄改 cities500 的 admin1 欄，學出的映射不變。
    //
    // Reason: 這是防循環驗證的防線。若學習過程讀了 cities500 的 admin1，
    // GeoNames 標錯的點會把錯誤對應餵回映射，修正器就學會維持現狀——錯誤
    // 愈多的州愈學不出修正，而那正是最需要修正的州。
    let dir = tempfile::tempdir().expect("暫存目錄");
    let baseline = run_stage(&fixture_natural_earth(), dir.path()).expect("應成功");

    let tampered_dir = tempfile::tempdir().expect("暫存目錄");
    let mut tampered = cities_rows();
    for row in tampered.iter_mut() {
        if row[8] == "MY" && row[0] != "1735162" {
            // 把其餘馬來西亞列的 admin1 換成明顯錯誤的值。
            row[10] = "99".to_string();
        }
    }
    admin1_correct_stage::run(
        &mut tampered,
        &admin1_rows(),
        &metadata(),
        &fixture_natural_earth(),
        &tampered_dir.path().join("admin2Codes.txt"),
        tampered_dir.path(),
    )
    .expect("應成功");

    assert_eq!(
        tampered[0][10], baseline[0][10],
        "Setapak 的修正結果不應受其他列的 admin1 影響"
    );
    assert_eq!(tampered[0][10], "14");
}
