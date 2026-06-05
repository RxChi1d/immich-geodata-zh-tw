use std::collections::{HashMap, HashSet};

use crate::pipeline::table::CITIES_COLUMNS;

pub fn merge_extra_rows(
    base_rows: Vec<Vec<String>>,
    extra_rows: Vec<Vec<String>>,
    min_population: u32,
) -> Result<Vec<Vec<String>>, String> {
    let existing_ids: HashSet<String> = base_rows.iter().map(|row| row[0].clone()).collect();
    let mut appended = base_rows;
    for row in extra_rows {
        ensure_city_width(&row)?;
        let population = row[14]
            .parse::<u32>()
            .map_err(|error| format!("population 不是有效整數：{}；{error}", row[14]))?;
        if !existing_ids.contains(&row[0]) && population >= min_population {
            appended.push(row);
        }
    }
    deduplicate_by_coordinate(appended)
}

pub fn deduplicate_by_coordinate(rows: Vec<Vec<String>>) -> Result<Vec<Vec<String>>, String> {
    if rows.is_empty() {
        return Ok(rows);
    }
    let mut stats: HashMap<(String, String), CoordinateStats> = HashMap::new();
    let mut parsed_rows = Vec::with_capacity(rows.len());
    for row in rows {
        ensure_city_width(&row)?;
        let population = parse_population(&row)?;
        let geoname_id = parse_geoname_id(&row)?;
        let key = (row[4].clone(), row[5].clone());
        stats
            .entry(key.clone())
            .and_modify(|stat| {
                stat.population_max = stat.population_max.max(population);
                stat.geoname_id_min = stat.geoname_id_min.min(geoname_id);
            })
            .or_insert(CoordinateStats {
                population_max: population,
                geoname_id_min: geoname_id,
            });
        parsed_rows.push(ParsedCityRow {
            row,
            coordinate_key: key,
            population,
            geoname_id,
        });
    }

    let mut deduped = Vec::with_capacity(parsed_rows.len());
    for parsed in parsed_rows {
        let Some(stat) = stats.get(&parsed.coordinate_key) else {
            continue;
        };
        if parsed.population == stat.population_max && parsed.geoname_id == stat.geoname_id_min {
            deduped.push(parsed.row);
        }
    }
    Ok(deduped)
}

struct ParsedCityRow {
    row: Vec<String>,
    coordinate_key: (String, String),
    population: u32,
    geoname_id: i64,
}

#[derive(Debug, Clone, Copy)]
struct CoordinateStats {
    population_max: u32,
    geoname_id_min: i64,
}

fn parse_population(row: &[String]) -> Result<u32, String> {
    row[14]
        .parse::<u32>()
        .map_err(|error| format!("population 不是有效整數：{}；{error}", row[14]))
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
    fn merge_filters_existing_ids_and_min_population() {
        let base = vec![city_row("100", "500")];
        let extra = vec![
            city_row("100", "9999"),
            city_row_with_coordinate("101", "99", "24.00000000", "120.00000000"),
            city_row_with_coordinate("102", "100", "25.00000000", "121.00000000"),
        ];

        let merged = merge_extra_rows(base, extra, 100).unwrap();

        assert_eq!(
            merged.iter().map(|row| row[0].as_str()).collect::<Vec<_>>(),
            vec!["100", "102"]
        );
    }

    fn city_row(geoname_id: &str, population: &str) -> Vec<String> {
        city_row_with_coordinate(geoname_id, population, "37.00000000", "-122.00000000")
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
