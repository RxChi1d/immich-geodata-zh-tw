#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CityRow {
    pub geoname_id: String,
    pub name: String,
    pub asciiname: String,
    pub alternatenames: String,
    pub latitude: String,
    pub longitude: String,
    pub feature_class: String,
    pub feature_code: String,
    pub country_code: String,
    pub cc2: String,
    pub admin1_code: String,
    pub admin2_code: String,
    pub admin3_code: String,
    pub admin4_code: String,
    pub population: u32,
    pub elevation: String,
    pub dem: String,
    pub timezone: String,
    pub modification_date: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Admin1Row {
    pub id: String,
    pub name: String,
    pub asciiname: String,
    pub geoname_id: String,
}
