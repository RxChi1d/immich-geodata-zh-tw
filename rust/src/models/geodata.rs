#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeodataRow {
    pub latitude: String,
    pub longitude: String,
    pub country: String,
    pub admin_1: String,
    pub admin_2: String,
    pub admin_3: String,
    pub admin_4: String,
}
