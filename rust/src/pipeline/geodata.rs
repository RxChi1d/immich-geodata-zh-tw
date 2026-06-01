use std::collections::BTreeSet;
use std::path::Path;

use crate::pipeline::polars_table::read_geodata_rows_with_header;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeodataRecord {
    pub latitude: String,
    pub longitude: String,
    pub admin_1: String,
    pub admin_2: String,
}

pub fn read_geodata(path: &Path) -> Result<Vec<GeodataRecord>, String> {
    read_geodata_rows_with_header(path)?
        .into_iter()
        .map(|row| {
            if row.len() != 7 {
                return Err(format!(
                    "geodata 欄位數不符：expected=7 actual={} path={}",
                    row.len(),
                    path.display()
                ));
            }
            Ok(GeodataRecord {
                latitude: row[0].clone(),
                longitude: row[1].clone(),
                admin_1: row[3].clone(),
                admin_2: row[4].clone(),
            })
        })
        .collect()
}

pub fn normalize_admin_fields(records: &mut [GeodataRecord]) {
    for record in records {
        for value in [&mut record.admin_1, &mut record.admin_2] {
            if matches!(value.as_str(), "" | "\"\"" | "nan" | "None") {
                value.clear();
            }
        }
    }
}

pub fn sort_for_cities(records: &mut [GeodataRecord]) {
    records.sort_by(|left, right| {
        left.admin_1
            .cmp(&right.admin_1)
            .then(left.admin_2.cmp(&right.admin_2))
    });
}

pub fn admin1_mapping(records: &[GeodataRecord], country_code: &str) -> Vec<(String, String)> {
    let admin1_names: BTreeSet<String> = records
        .iter()
        .map(|record| record.admin_1.clone())
        .collect();
    let width = admin1_names.len().to_string().len();
    admin1_names
        .into_iter()
        .enumerate()
        .map(|(index, name)| {
            let code = format!("{country_code}.{:0width$}", index + 1);
            (name, code)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn admin1_mapping_uses_sorted_names_and_dynamic_width() {
        let records = vec![
            GeodataRecord {
                latitude: String::new(),
                longitude: String::new(),
                admin_1: "臺北市".to_string(),
                admin_2: String::new(),
            },
            GeodataRecord {
                latitude: String::new(),
                longitude: String::new(),
                admin_1: "臺中市".to_string(),
                admin_2: String::new(),
            },
        ];

        let mapping = admin1_mapping(&records, "TW");

        assert_eq!(
            mapping,
            vec![
                ("臺中市".to_string(), "TW.1".to_string()),
                ("臺北市".to_string(), "TW.2".to_string())
            ]
        );
    }
}
