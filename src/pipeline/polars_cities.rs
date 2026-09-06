use std::collections::{HashMap, HashSet};

use crate::pipeline::table::CITIES_COLUMNS;

/// P 類（聚落）白名單：代表實際有人居住的地點。
///
/// Reason: 收點條件原本是 `population >= 100`，但那量錯了東西——Immich 反向地理查詢
/// 只需要座標加地名，跟登記人口無關，而 GeoNames 大量真實聚落的 population 是 0 或
/// 空白（沒人填，不是沒人住）。以官方圖資當 ground truth 實測行政區正確率，改用類別
/// 白名單後印尼 85.65% → 97.56%、臺灣 69.52% → 95.34%。
/// 未列入的 PPLQ（廢棄）、PPLW（已毀）、PPLH／PPLCH（歷史）刻意排除：廢村不該成為
/// 反向地理查詢的答案。
const SETTLEMENT_CODES: [&str; 13] = [
    "PPL", "PPLA", "PPLA2", "PPLA3", "PPLA4", "PPLA5", "PPLC", "PPLF", "PPLG", "PPLL", "PPLR",
    "PPLS", "STLMT",
];

/// A 類（行政區）白名單。
///
/// Reason: 只收 P 類時人煙稀少的區域會完全沒有可用點——冰島實測有 22.3% 的取樣點在
/// 25 km 內找不到任何點，Immich 只會顯示國名、state 為 null；補上 A 類後降到 4.4%。
/// 代價是約 1.3% 的聚落鄰近取樣會從鎮名變成轄區名，已裁決接受。行政區點的 population
/// 常是 0，因此 A 類尤其不能被人口門檻擋掉。
const ADMIN_CODES: [&str; 6] = ["ADM1", "ADM2", "ADM3", "ADM4", "ADM5", "ADMD"];

/// PPLX（區段／suburb）唯一收錄的國家。
///
/// Reason: 這不是本專案發明的政策，是鏡射 Immich 自身的匯入規則
/// `if ((lineSplit[7] === 'PPLX' && lineSplit[8] !== 'AU') || lineSplit[7] === 'PPLH') continue;`
/// ——澳洲的 PPLX 是 suburb，是當地最有用的地名；其他國家的 PPLX 就算收進來，Immich
/// 匯入時也會直接跳過。
const PPLX_COUNTRY_CODE: &str = "AU";

/// 依 GeoNames feature class/code 判斷一列 cities500 是否收錄。
///
/// base rows 與 extra rows 共用這條規則，不留來源例外。呼叫前需先確認欄位寬度。
pub fn is_admitted_city_row(row: &[String]) -> bool {
    let feature_class = row[6].as_str();
    let feature_code = row[7].as_str();
    let country_code = row[8].as_str();
    match feature_class {
        "P" => {
            SETTLEMENT_CODES.contains(&feature_code)
                || (feature_code == "PPLX" && country_code == PPLX_COUNTRY_CODE)
        }
        "A" => ADMIN_CODES.contains(&feature_code),
        _ => false,
    }
}

pub fn merge_extra_rows(
    base_rows: Vec<Vec<String>>,
    extra_rows: Vec<Vec<String>>,
) -> Result<Vec<Vec<String>>, String> {
    let mut appended = Vec::with_capacity(base_rows.len() + extra_rows.len());
    for row in base_rows {
        ensure_city_width(&row)?;
        if is_admitted_city_row(&row) {
            appended.push(row);
        }
    }
    // Reason: 以「過濾後」的 base 建立 id 索引即可。被規則丟掉的 base 列，其同 id 的
    // extra 列 feature class/code 相同，會被同一條規則丟掉，不會因為索引少了它而漏收。
    let existing_ids: HashSet<String> = appended.iter().map(|row| row[0].clone()).collect();
    for row in extra_rows {
        ensure_city_width(&row)?;
        if !existing_ids.contains(&row[0]) && is_admitted_city_row(&row) {
            appended.push(row);
        }
    }
    deduplicate_by_coordinate(appended)
}

pub fn deduplicate_by_coordinate(rows: Vec<Vec<String>>) -> Result<Vec<Vec<String>>, String> {
    if rows.is_empty() {
        return Ok(rows);
    }
    // Reason: 原條件為「population 最大 且 geoname_id 最小」同時成立才保留，兩個條件
    // 落在同一群的不同列時整群都不符合，座標群會整個消失（MY 展開後 1,344 群中有 10 群
    // 蒸發）。population 退出收點判斷後，改為單一 tie-break：同座標只留 geoname_id 最小者。
    let mut geoname_id_min: HashMap<(String, String), i64> = HashMap::new();
    let mut parsed_rows = Vec::with_capacity(rows.len());
    for row in rows {
        ensure_city_width(&row)?;
        let geoname_id = parse_geoname_id(&row)?;
        let key = (row[4].clone(), row[5].clone());
        geoname_id_min
            .entry(key.clone())
            .and_modify(|current| *current = (*current).min(geoname_id))
            .or_insert(geoname_id);
        parsed_rows.push(ParsedCityRow {
            row,
            coordinate_key: key,
            geoname_id,
        });
    }

    let mut deduped = Vec::with_capacity(geoname_id_min.len());
    for parsed in parsed_rows {
        if geoname_id_min.get(&parsed.coordinate_key) == Some(&parsed.geoname_id) {
            deduped.push(parsed.row);
        }
    }
    Ok(deduped)
}

struct ParsedCityRow {
    row: Vec<String>,
    coordinate_key: (String, String),
    geoname_id: i64,
}

fn parse_geoname_id(row: &[String]) -> Result<i64, String> {
    row[0]
        .parse::<i64>()
        .map_err(|error| format!("geoname_id 不是有效整數：{}；{error}", row[0]))
}

fn ensure_city_width(row: &[String]) -> Result<(), String> {
    if row.len() != CITIES_COLUMNS.len() {
        return Err(format!(
            "cities500 欄位數不符：expected={} actual={}",
            CITIES_COLUMNS.len(),
            row.len()
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deduplicate_matches_legacy_groupby_contract() {
        let rows = vec![city_row("5000", "1000"), city_row("4000", "1000")];

        let deduped = deduplicate_by_coordinate(rows).unwrap();

        assert_eq!(deduped.len(), 1);
        assert_eq!(deduped[0][0], "4000");
    }

    #[test]
    fn deduplicate_keeps_group_when_largest_population_is_not_lowest_id() {
        let rows = vec![city_row("5000", "9999"), city_row("4000", "10")];

        let deduped = deduplicate_by_coordinate(rows).unwrap();

        assert_eq!(
            deduped
                .iter()
                .map(|row| row[0].as_str())
                .collect::<Vec<_>>(),
            vec!["4000"]
        );
    }

    #[test]
    fn merge_skips_extra_rows_with_existing_id() {
        let base = vec![city_row("100", "500")];
        let extra = vec![city_row("100", "9999")];

        let merged = merge_extra_rows(base, extra).unwrap();

        assert_eq!(
            merged.iter().map(|row| row[0].as_str()).collect::<Vec<_>>(),
            vec!["100"]
        );
    }

    #[test]
    fn merge_admits_rows_regardless_of_population() {
        let extra = vec![
            feature_row("101", "P", "PPL", "US", "0", "24.00000000"),
            feature_row("102", "A", "ADM2", "MY", "0", "25.00000000"),
        ];

        let merged = merge_extra_rows(Vec::new(), extra).unwrap();

        assert_eq!(
            merged.iter().map(|row| row[0].as_str()).collect::<Vec<_>>(),
            vec!["101", "102"]
        );
    }

    #[test]
    fn merge_applies_the_same_rule_to_base_rows() {
        let base = vec![
            feature_row("200", "P", "PPL", "US", "1000", "24.00000000"),
            feature_row("201", "P", "PPLH", "US", "1000", "25.00000000"),
        ];

        let merged = merge_extra_rows(base, Vec::new()).unwrap();

        assert_eq!(
            merged.iter().map(|row| row[0].as_str()).collect::<Vec<_>>(),
            vec!["200"]
        );
    }

    #[test]
    fn admits_settlement_and_admin_codes() {
        for (feature_class, feature_code) in [
            ("P", "PPL"),
            ("P", "PPLA4"),
            ("P", "PPLA5"),
            ("P", "PPLR"),
            ("P", "STLMT"),
            ("A", "ADM1"),
            ("A", "ADMD"),
        ] {
            let row = feature_row("1", feature_class, feature_code, "MY", "0", "24.00000000");
            assert!(
                is_admitted_city_row(&row),
                "{feature_class}/{feature_code} 應被收錄"
            );
        }
    }

    #[test]
    fn rejects_abandoned_historical_and_non_place_rows() {
        for (feature_class, feature_code) in [
            ("P", "PPLQ"),
            ("P", "PPLW"),
            ("P", "PPLH"),
            ("P", "PPLCH"),
            ("A", "PCLI"),
            ("A", "TERR"),
            ("S", "AIRP"),
            ("T", "MT"),
        ] {
            let row = feature_row(
                "1",
                feature_class,
                feature_code,
                "MY",
                "9999",
                "24.00000000",
            );
            assert!(
                !is_admitted_city_row(&row),
                "{feature_class}/{feature_code} 不應被收錄"
            );
        }
    }

    #[test]
    fn admits_pplx_only_for_australia() {
        let australia = feature_row("1", "P", "PPLX", "AU", "0", "24.00000000");
        let elsewhere = feature_row("2", "P", "PPLX", "US", "9999", "24.00000000");

        assert!(is_admitted_city_row(&australia));
        assert!(!is_admitted_city_row(&elsewhere));
    }

    fn city_row(geoname_id: &str, population: &str) -> Vec<String> {
        city_row_with_coordinate(geoname_id, population, "37.00000000", "-122.00000000")
    }

    fn feature_row(
        geoname_id: &str,
        feature_class: &str,
        feature_code: &str,
        country_code: &str,
        population: &str,
        latitude: &str,
    ) -> Vec<String> {
        let mut row = city_row_with_coordinate(geoname_id, population, latitude, "120.00000000");
        row[6] = feature_class.to_string();
        row[7] = feature_code.to_string();
        row[8] = country_code.to_string();
        row
    }

    fn city_row_with_coordinate(
        geoname_id: &str,
        population: &str,
        latitude: &str,
        longitude: &str,
    ) -> Vec<String> {
        vec![
            geoname_id.to_string(),
            "Name".to_string(),
            "Name".to_string(),
            String::new(),
            latitude.to_string(),
            longitude.to_string(),
            "P".to_string(),
            "PPL".to_string(),
            "US".to_string(),
            String::new(),
            "CA".to_string(),
            String::new(),
            String::new(),
            String::new(),
            population.to_string(),
            String::new(),
            String::new(),
            "America/Los_Angeles".to_string(),
            "2024-01-01".to_string(),
        ]
    }
}
