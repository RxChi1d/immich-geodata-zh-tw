use crate::cli::RunOptions;
use crate::pipeline::fixtures::{Fixture, load_fixtures};
use crate::pipeline::geodata::{
    GeodataRecord, admin1_mapping, normalize_admin_fields, read_geodata, sort_for_cities,
};
use crate::pipeline::polars_table::write_cities_rows;
use crate::pipeline::table::format_coordinate;
use std::collections::HashMap;
use std::path::Path;

pub fn run(options: &RunOptions) -> Result<(), String> {
    let fixtures = load_fixtures(&options.fixtures_dir, options.fixture.as_deref())?;
    for fixture in fixtures {
        if !fixture.supports_stage("transform_cities_schema") {
            continue;
        }
        run_fixture(&fixture, options)?;
    }
    Ok(())
}

fn run_fixture(fixture: &Fixture, options: &RunOptions) -> Result<(), String> {
    for country in &fixture.manifest.countries {
        run_country(fixture, options, country)?;
    }
    Ok(())
}

fn run_country(fixture: &Fixture, options: &RunOptions, country: &str) -> Result<(), String> {
    let mut rows = build_country_city_rows(fixture, country, fixture.manifest.base_geoname_id)?;
    sort_city_rows_for_golden(&mut rows);
    let output = options
        .output_dir
        .join(&fixture.manifest.name)
        .join("transform_cities_schema")
        .join(format!("{country}.csv"));
    write_cities_rows(&output, b',', true, &rows)?;
    println!(
        "stage=transform_cities_schema fixture={} country={} rows={}",
        fixture.manifest.name,
        country,
        rows.len()
    );
    Ok(())
}

pub fn build_country_city_rows(
    fixture: &Fixture,
    country_code: &str,
    base_geoname_id: i64,
) -> Result<Vec<Vec<String>>, String> {
    let input = fixture
        .root
        .join("geodata")
        .join(format!("{}_geodata.csv", country_code.to_lowercase()));
    build_city_rows_from_geodata(
        &input,
        country_code,
        base_geoname_id,
        &fixture.manifest.modification_date,
        CoordinateFormat::Fixed,
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoordinateFormat {
    Fixed,
    Compact,
}

pub fn build_city_rows_from_geodata(
    input: &Path,
    country_code: &str,
    base_geoname_id: i64,
    modification_date: &str,
    coordinate_format: CoordinateFormat,
) -> Result<Vec<Vec<String>>, String> {
    let raw_records = read_geodata(input)?;
    let mapping = admin1_mapping(&raw_records, country_code);
    let admin1_code_by_name: HashMap<String, String> = mapping
        .into_iter()
        .map(|(name, code)| {
            let short_code = code.split('.').next_back().unwrap_or_default().to_string();
            (name, short_code)
        })
        .collect();
    let mut records = raw_records;
    normalize_admin_fields(&mut records);
    sort_for_cities(&mut records);

    let profile = country_profile(country_code)?;
    let city_level = profile.city_level;
    let mut rows = Vec::new();
    let mut skipped_without_city = 0_usize;
    for record in records.iter() {
        // Reason: 選定層級沒有值的列不能收——`name` 為空時 Immich 顯示不出城市，
        // 而退回上一層會讓同一國混用兩種層級（見 city-level-criteria 條件 3）。
        // 印尼實測 115 列屬此類，全為 BIG 圖資的「Area Tidak Terdefinisi」未定義區。
        let city_name = city_level.name_of(record);
        if city_name.is_empty() {
            skipped_without_city += 1;
            continue;
        }
        let admin1_code = admin1_code_by_name
            .get(&record.admin_1)
            .cloned()
            .unwrap_or_default();
        // Reason: 多時區國家（如印尼）的時區依 admin1 解析。此處 record.admin_1
        //         已是 handler 最終省名（s2t + 補省正規化後）；indonesia_timezone
        //         以「最終省名 → WADMPR 原文 → 時區」解析，原文為權威 key。
        let timezone = profile.timezone_for_admin1(&record.admin_1)?;
        // Reason: ID 用「已產出列數」而非記錄索引。呼叫端以
        // `max_id = base_id + rows.len() - 1` 推進下一國的起始 ID
        // （`cities500_load::replace_country_cities`），若這裡用記錄索引，
        // 被跳過的列會讓實際 ID 超出保留區間而與下一國撞號。
        rows.push(vec![
            (base_geoname_id + rows.len() as i64).to_string(),
            city_name.to_string(),
            city_name.to_string(),
            String::new(),
            format_city_coordinate(&record.latitude, coordinate_format)?,
            format_city_coordinate(&record.longitude, coordinate_format)?,
            "A".to_string(),
            city_level.feature_code().to_string(),
            country_code.to_string(),
            String::new(),
            admin1_code,
            String::new(),
            String::new(),
            String::new(),
            "0".to_string(),
            String::new(),
            String::new(),
            timezone.to_string(),
            modification_date.to_string(),
        ]);
    }
    if skipped_without_city > 0 {
        println!(
            "stage=transform_cities_schema country={country_code} city_level={city_level:?} \
             skipped_without_city={skipped_without_city}"
        );
    }
    Ok(rows)
}

fn format_city_coordinate(
    value: &str,
    coordinate_format: CoordinateFormat,
) -> Result<String, String> {
    let fixed = format_coordinate(value)?;
    if coordinate_format == CoordinateFormat::Fixed {
        return Ok(fixed);
    }

    let Some((integer, decimal)) = fixed.split_once('.') else {
        return Ok(fixed);
    };
    let trimmed = decimal.trim_end_matches('0');
    if trimmed.is_empty() {
        Ok(format!("{integer}.0"))
    } else {
        Ok(format!("{integer}.{trimmed}"))
    }
}

/// city（Immich 顯示的城市名）取自哪一個行政層級。
///
/// Reason: 層級的選擇條件見 `docs/zh-tw/city-level-criteria.md`。不設預設值——
/// 新增國家必須在 `country_profile` 明示，否則會默默沿用別國的層級，而
/// Immich 只讀 `name`／`admin1`／國碼，選錯之後沒有任何階段能補救。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CityLevel {
    /// 第二級行政區：臺灣鄉鎮市區、日本市区町村、南韓 시군구、泰國 อำเภอ。
    Admin2,
    /// 第三級行政區：印尼 kecamatan。
    Admin3,
}

impl CityLevel {
    /// 取該層級的名稱；空字串代表這一列在此層級沒有行政區。
    fn name_of<'a>(&self, record: &'a GeodataRecord) -> &'a str {
        match self {
            Self::Admin2 => &record.admin_2,
            Self::Admin3 => &record.admin_3,
        }
    }

    /// GeoNames feature code，須落在 `polars_cities::ADMIN_CODES` 白名單內，
    /// 否則該列會在 cities500 收點階段被丟掉。
    fn feature_code(&self) -> &'static str {
        match self {
            Self::Admin2 => "ADM2",
            Self::Admin3 => "ADM3",
        }
    }
}

// Reason: 不衍生 PartialEq/Eq——欄位含函式指標，指標相等性無實質意義
//         （編譯器警告），且 CountryProfile 不需要比較。
#[derive(Debug, Clone, Copy)]
pub struct CountryProfile {
    pub country_name: &'static str,
    /// city 取自哪一個行政層級。
    pub city_level: CityLevel,
    /// 國家預設時區（單一時區國家）。
    ///
    /// 多時區國家（印尼）以 `timezone_for_admin1` 依省解析；解析失敗
    /// 時回傳錯誤（不靜默回退）。
    pub timezone: &'static str,
    /// 多時區國家的 per-province 時區解析函式（key 為繁中省名）。
    timezone_resolver: Option<fn(&str) -> Option<&'static str>>,
}

impl CountryProfile {
    /// 依 admin1（繁中省名）解析時區。
    ///
    /// 單一時區國家直接回傳預設時區；多時區國家（設有 resolver）解析
    /// 失敗時回傳錯誤而非靜默回退。
    ///
    /// Reason: 多時區國家的省名未命中對照表（如 Wikidata 譯名漂移）若
    /// 靜默回退預設時區，WITA/WIT 省份會被錯標為 WIB 而無人察覺；
    /// 讓 release 直接失敗才能在發版前暴露問題。
    pub fn timezone_for_admin1(&self, admin1: &str) -> Result<&'static str, String> {
        match self.timezone_resolver {
            Some(resolver) => resolver(admin1).ok_or_else(|| {
                format!(
                    "無法解析{}admin1「{admin1}」的時區：省名未命中時區對照表\
                     （可能為 Wikidata 譯名漂移），請校準該國 timezone resolver 的對照表",
                    self.country_name
                )
            }),
            None => Ok(self.timezone),
        }
    }
}

pub fn country_profile(country_code: &str) -> Result<CountryProfile, String> {
    match country_code {
        "TW" => Ok(CountryProfile {
            country_name: "臺灣",
            city_level: CityLevel::Admin2,
            timezone: "Asia/Taipei",
            timezone_resolver: None,
        }),
        "JP" => Ok(CountryProfile {
            country_name: "日本",
            city_level: CityLevel::Admin2,
            timezone: "Asia/Tokyo",
            timezone_resolver: None,
        }),
        "KR" => Ok(CountryProfile {
            country_name: "南韓",
            city_level: CityLevel::Admin2,
            timezone: "Asia/Seoul",
            timezone_resolver: None,
        }),
        "TH" => Ok(CountryProfile {
            country_name: "泰國",
            city_level: CityLevel::Admin2,
            timezone: "Asia/Bangkok",
            timezone_resolver: None,
        }),
        "ID" => Ok(CountryProfile {
            country_name: "印尼",
            // Reason: kabupaten/kota 平均 3,705 km²，三格無法定位座標——烏布顯示
            //         「巴釐省・吉亞尼亞爾縣」，而那不是任何人用來指稱該地的名字。
            //         kecamatan 通過辨識性、密度（中位數 11 點、單點單位 0%）與
            //         一致性；權威中文名不可得（Wikidata 約 5%、NAER 2.6%），
            //         已裁決接受印尼文原文，詳見 docs/zh-tw/city-level-criteria.md。
            city_level: CityLevel::Admin3,
            // Reason: 印尼跨 WIB/WITA/WIT 三時區，per-province 解析見
            //         indonesia_timezone；後備預設取最多省份的 WIB。
            timezone: "Asia/Jakarta",
            timezone_resolver: Some(crate::pipeline::indonesia_timezone::timezone_for_province),
        }),
        other => Err(format!("transform_cities_schema 尚未支援國家：{other}")),
    }
}

pub fn sort_city_rows_for_golden(rows: &mut [Vec<String>]) {
    rows.sort_by(|left, right| {
        left[8]
            .cmp(&right[8])
            .then(left[10].cmp(&right[10]))
            .then(left[1].cmp(&right[1]))
            .then(left[0].cmp(&right[0]))
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compact_coordinate_format_matches_polars_float_output() {
        assert_eq!(
            format_city_coordinate("120.79553010", CoordinateFormat::Compact).unwrap(),
            "120.7955301"
        );
        assert_eq!(
            format_city_coordinate("23.86220960", CoordinateFormat::Compact).unwrap(),
            "23.8622096"
        );
        assert_eq!(
            format_city_coordinate("24.00000000", CoordinateFormat::Compact).unwrap(),
            "24.0"
        );
    }

    /// 各國的 city 層級必須是明示的，且與 `docs/zh-tw/city-level-criteria.md`
    /// 第 3 節的實測表一致。
    #[test]
    fn registered_city_levels_match_documented_decision() {
        for country in ["TW", "JP", "KR", "TH"] {
            assert_eq!(
                country_profile(country).unwrap().city_level,
                CityLevel::Admin2,
                "{country} 的 city 應取第二級行政區"
            );
        }
        assert_eq!(
            country_profile("ID").unwrap().city_level,
            CityLevel::Admin3,
            "印尼的 city 應取 kecamatan（第三級）"
        );
    }

    #[test]
    fn city_level_feature_code_stays_in_admin_whitelist() {
        // Reason: feature code 不在 polars_cities::ADMIN_CODES 白名單內時，
        // handler 產生的列會在 cities500 收點階段被整批丟掉而沒有錯誤訊息。
        for (level, expected) in [(CityLevel::Admin2, "ADM2"), (CityLevel::Admin3, "ADM3")] {
            assert_eq!(level.feature_code(), expected);
            let mut row = vec![String::new(); crate::pipeline::table::CITIES_COLUMNS.len()];
            row[6] = "A".to_string();
            row[7] = level.feature_code().to_string();
            row[8] = "ID".to_string();
            assert!(
                crate::pipeline::polars_cities::is_admitted_city_row(&row),
                "{expected} 應在 cities500 收點白名單內"
            );
        }
    }

    /// 選定層級沒有值的列要跳過，且產出的 geoname_id 必須連續。
    ///
    /// Reason: 呼叫端以 `max_id = base_id + rows.len() - 1` 推進下一國的起始
    /// ID。若這裡改用記錄索引編號，被跳過的列會讓實際 ID 超出保留區間，
    /// 與下一個國家撞號——而 geoname_id 撞號沒有任何守衛會擋。
    #[test]
    fn rows_missing_city_level_value_are_skipped_with_contiguous_ids() {
        let dir = tempfile::tempdir().unwrap();
        let input = dir.path().join("id_geodata.csv");
        std::fs::write(
            &input,
            "latitude,longitude,country,admin_1,admin_2,admin_3,admin_4\n\
             1.0,101.0,ID,巴釐省,巴東縣,,Area Tidak Terdefinisi\n\
             2.0,102.0,ID,巴釐省,巴東縣,Kuta,Legian\n\
             3.0,103.0,ID,巴釐省,吉亞尼亞爾縣,Ubud,Ubud\n",
        )
        .unwrap();

        let rows =
            build_city_rows_from_geodata(&input, "ID", 500, "2026-06-06", CoordinateFormat::Fixed)
                .unwrap();

        // sort_for_cities 依 (admin_1, admin_2) 排序，「吉亞尼亞爾縣」排在
        // 「巴東縣」之前，所以 Ubud 先於 Kuta。
        let names: Vec<&str> = rows.iter().map(|row| row[1].as_str()).collect();
        assert_eq!(names, vec!["Ubud", "Kuta"], "空 admin_3 的列應被跳過");
        let ids: Vec<&str> = rows.iter().map(|row| row[0].as_str()).collect();
        assert_eq!(ids, vec!["500", "501"], "ID 必須連續，不留被跳過列的空號");
        assert!(rows.iter().all(|row| row[7] == "ADM3"));
    }

    #[test]
    fn fixed_coordinate_format_keeps_golden_precision() {
        assert_eq!(
            format_city_coordinate("23.86220960", CoordinateFormat::Fixed).unwrap(),
            "23.86220960"
        );
    }
}
