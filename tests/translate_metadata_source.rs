use std::io::Write;
use std::path::{Path, PathBuf};

use immich_geodata::pipeline::translate::{ProductionTranslateOptions, run_production};

/// translate 只載入 LocationIQ 產物 `{CC}.csv`（production 位於
/// `data/locationiq/`），handler extract 產物 `{cc}_geodata.csv`（位於
/// `meta_data/`）不參與查表。目錄雖已分離，檔名守衛仍需擋住誤放或路徑指錯的
/// 檔案，以下測試即針對該守衛，以「城市名是否被 metadata 的 admin_2 取代」
/// 觀測實際載入結果。
struct TranslateCase {
    dir: tempfile::TempDir,
    metadata_dir: PathBuf,
}

impl TranslateCase {
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let metadata_dir = dir.path().join("meta_data");
        std::fs::create_dir_all(&metadata_dir).unwrap();
        Self { dir, metadata_dir }
    }

    fn write(&self, path: &Path, content: &str) -> PathBuf {
        std::fs::File::create(path)
            .unwrap()
            .write_all(content.as_bytes())
            .unwrap();
        path.to_path_buf()
    }

    /// 寫入一份 metadata CSV（欄位同 GEODATA_COLUMNS），admin_2 為譯名來源。
    fn write_metadata(&self, file_name: &str, latitude: &str, longitude: &str, admin2: &str) {
        self.write(
            &self.metadata_dir.join(file_name),
            &format!(
                "latitude,longitude,country,admin_1,admin_2,admin_3,admin_4\n\
                 {latitude},{longitude},測試國,測試省,{admin2},,\n"
            ),
        );
    }

    /// 以單一城市列執行 production translate，回傳翻譯後的該列名稱。
    fn translate_single_city(&self, country_code: &str, latitude: &str, longitude: &str) -> String {
        let mut row = vec![String::new(); 19];
        row[0] = "1".into();
        row[1] = "Untranslated".into();
        row[2] = "Untranslated".into();
        row[4] = latitude.into();
        row[5] = longitude.into();
        row[8] = country_code.into();
        row[10] = "01".into();
        let cities_file = self.write(
            &self.dir.path().join("cities500_optimized.txt"),
            &row.join("\t"),
        );
        let admin1_file = self.write(
            &self.dir.path().join("admin1CodesASCII_optimized.txt"),
            &format!("{country_code}.01\tTest\tTest\t9999999\n"),
        );
        let alternate_file = self.write(
            &self.dir.path().join("alternate_chinese_name.csv"),
            "geoname_id,name\n",
        );
        let naer_file = self.write(
            &self.dir.path().join("naer_place_names.csv"),
            "name_norm,name_zh,country_code,latitude,longitude,feature_hint\n",
        );
        let output_dir = self.dir.path().join("out");

        run_production(&ProductionTranslateOptions {
            metadata_dir: self.metadata_dir.clone(),
            data_dir: self.dir.path().to_path_buf(),
            cities_file,
            admin1_file,
            alternate_name_file: alternate_file,
            naer_file,
            output_dir: output_dir.clone(),
            profile: false,
        })
        .unwrap();

        let output = std::fs::read_to_string(output_dir.join("cities500_translated.txt")).unwrap();
        output
            .lines()
            .next()
            .unwrap()
            .split('\t')
            .nth(1)
            .unwrap()
            .to_string()
    }
}

// 正常情境：LocationIQ 產物 `{CC}.csv` 仍被載入並套用。
#[test]
fn locationiq_metadata_file_is_loaded() {
    let case = TranslateCase::new();
    case.write_metadata("US.csv", "30.0", "-87.0", "菲爾維爾");

    assert_eq!(
        case.translate_single_city("US", "30.0", "-87.0"),
        "菲爾維爾"
    );
}

// 邊界情境：`--country-code us` 會產出小寫檔名，國碼須正規化為大寫才對得上
// cities500 的大寫國碼。
#[test]
fn lowercase_metadata_filename_is_normalized() {
    let case = TranslateCase::new();
    case.write_metadata("us.csv", "30.0", "-87.0", "菲爾維爾");

    assert_eq!(
        case.translate_single_city("US", "30.0", "-87.0"),
        "菲爾維爾"
    );
}

// 回歸情境：handler extract 產物不得被當成 metadata 載入——舊版以檔名 stem
// 直接當國碼，使 `{cc}_geodata.csv` 成為國碼 `jp_geodata` 的死 key。
#[test]
fn handler_geodata_file_is_not_loaded_as_metadata() {
    let case = TranslateCase::new();
    case.write_metadata("jp_geodata.csv", "35.0", "136.0", "亀山市");

    // 以 stem 當國碼是唯一能命中該檔的查詢方式；修正後必須落空，城市名維持原樣。
    assert_eq!(
        case.translate_single_city("jp_geodata", "35.0", "136.0"),
        "Untranslated"
    );
}

// 失敗情境：非 CSV 或非 ISO-2 命名的檔案都不進入 metadata，且不使流程失敗。
#[test]
fn non_metadata_files_are_skipped_without_error() {
    let case = TranslateCase::new();
    case.write_metadata("US.csv", "30.0", "-87.0", "菲爾維爾");
    case.write_metadata("tw_geodata.csv", "30.0", "-87.0", "不該被載入");
    case.write(&case.metadata_dir.join("README.md"), "not metadata\n");
    case.write(&case.metadata_dir.join("USA.csv"), "latitude\n");

    assert_eq!(
        case.translate_single_city("US", "30.0", "-87.0"),
        "菲爾維爾"
    );
}

// 邊界情境：同一國碼的大小寫檔名並存時，取字典序第一個檔案，其餘略過。
// 若改依 read_dir 順序，同一棵檔案樹在不同機器上會翻出不同結果。
#[test]
fn duplicate_country_code_resolves_deterministically() {
    let case = TranslateCase::new();
    case.write_metadata("US.csv", "30.0", "-87.0", "菲爾維爾");
    case.write_metadata("us.csv", "30.0", "-87.0", "不該被採用");

    assert_eq!(
        case.translate_single_city("US", "30.0", "-87.0"),
        "菲爾維爾"
    );
}

// 邊界情境：LocationIQ 匯出檔若寫成大寫副檔名，仍應被視為 metadata。
// 大小寫敏感的比對會讓它連 skip 記錄都沒有，等同靜默失效。
#[test]
fn uppercase_csv_extension_is_recognized() {
    let case = TranslateCase::new();
    case.write_metadata("US.CSV", "30.0", "-87.0", "菲爾維爾");

    assert_eq!(
        case.translate_single_city("US", "30.0", "-87.0"),
        "菲爾維爾"
    );
}
