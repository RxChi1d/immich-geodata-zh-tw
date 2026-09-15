//! 印尼 kecamatan（郡）的繁中譯名查表。
//!
//! kecamatan 是 Immich 顯示的城市名（見 `docs/zh-tw/city-level-criteria.md`），
//! 但 Wikidata 對這一層的中文覆蓋不足 3%，單靠既有的 admin1／admin2 translator
//! 拿不到譯名。此處改以離線彙整的對照表補位，來源與收錄規則見
//! `data/vendor/indonesia/README.md`。
//!
//! 查表以「名稱 + 座標」雙重條件，比照 NAER 的消歧作法：kecamatan 名稱在全國
//! 不唯一（`Bandung` 同時是萬隆市與數個郡的名字），只比對名稱會把萬隆的譯名
//! 套到別省的同名郡上。

use std::collections::HashMap;
use std::path::Path;

/// 譯名可採用的最大距離（公里）。
///
/// Reason: 表中座標是該 kecamatan 全部代表點的平均，而 extract 逐個 desa
/// 多邊形產生代表點，兩者本來就有偏移；印尼最大的 kecamatan 跨距超過 100 km，
/// 門檻太嚴會讓邊緣的 desa 拿不到譯名。15 km 沿用 NAER city 匹配的既有門檻。
const MAX_DISTANCE_KM: f64 = 15.0;

#[derive(Clone, Debug, Default)]
pub(super) struct KecamatanNames {
    by_name: HashMap<String, Vec<KecamatanEntry>>,
}

#[derive(Clone, Debug)]
struct KecamatanEntry {
    name_zh: String,
    latitude: f64,
    longitude: f64,
}

impl KecamatanNames {
    /// 讀取 vendored 對照表；檔案不存在時回傳空表（該國譯名全部回退原文）。
    pub(super) fn load(path: &Path) -> Result<Self, String> {
        if !path.exists() {
            return Ok(Self::default());
        }
        let text = std::fs::read_to_string(path)
            .map_err(|error| format!("無法讀取 {}：{error}", path.display()))?;
        let mut by_name: HashMap<String, Vec<KecamatanEntry>> = HashMap::new();
        for (index, line) in text.lines().enumerate().skip(1) {
            if line.trim().is_empty() {
                continue;
            }
            let fields: Vec<&str> = line.split(',').collect();
            if fields.len() < 4 {
                return Err(format!(
                    "{} 第 {} 行欄位不足：{line}",
                    path.display(),
                    index + 1
                ));
            }
            let latitude = fields[2].parse::<f64>().map_err(|error| {
                format!(
                    "{} 第 {} 行緯度無法解析：{error}",
                    path.display(),
                    index + 1
                )
            })?;
            let longitude = fields[3].parse::<f64>().map_err(|error| {
                format!(
                    "{} 第 {} 行經度無法解析：{error}",
                    path.display(),
                    index + 1
                )
            })?;
            by_name
                .entry(fields[0].to_string())
                .or_default()
                .push(KecamatanEntry {
                    name_zh: fields[1].to_string(),
                    latitude,
                    longitude,
                });
        }
        Ok(Self { by_name })
    }

    /// 依名稱與座標取繁中譯名；查無或距離超過門檻時回傳 `None`（呼叫端回退原文）。
    pub(super) fn lookup(&self, name: &str, latitude: f64, longitude: f64) -> Option<&str> {
        let entries = self.by_name.get(name)?;
        entries
            .iter()
            .map(|entry| {
                (
                    entry,
                    distance_km(latitude, longitude, entry.latitude, entry.longitude),
                )
            })
            .filter(|(_, distance)| *distance <= MAX_DISTANCE_KM)
            .min_by(|left, right| left.1.total_cmp(&right.1))
            .map(|(entry, _)| entry.name_zh.as_str())
    }

    pub(super) fn is_empty(&self) -> bool {
        self.by_name.is_empty()
    }
}

/// 等距圓柱近似的地表距離。印尼全境緯度在 ±11 度內，此近似的誤差遠小於門檻。
fn distance_km(lat1: f64, lon1: f64, lat2: f64, lon2: f64) -> f64 {
    const KM_PER_DEGREE: f64 = 111.0;
    let mean_latitude = ((lat1 + lat2) / 2.0).to_radians();
    let north = (lat1 - lat2) * KM_PER_DEGREE;
    let east = (lon1 - lon2) * KM_PER_DEGREE * mean_latitude.cos();
    north.hypot(east)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_table(dir: &Path, body: &str) -> std::path::PathBuf {
        let path = dir.join("kecamatan_zh.csv");
        std::fs::write(
            &path,
            format!("name,name_zh,latitude,longitude,source\n{body}"),
        )
        .unwrap();
        path
    }

    #[test]
    fn missing_file_yields_empty_table() {
        let table = KecamatanNames::load(Path::new("/nonexistent/kecamatan_zh.csv")).unwrap();
        assert!(table.is_empty());
        assert_eq!(table.lookup("Ubud", -8.5, 115.26), None);
    }

    #[test]
    fn lookup_requires_name_and_coordinate() {
        let dir = tempfile::tempdir().unwrap();
        let path = write_table(dir.path(), "Ubud,烏布,-8.506700,115.262500,wikidata\n");
        let table = KecamatanNames::load(&path).unwrap();

        assert_eq!(table.lookup("Ubud", -8.5067, 115.2625), Some("烏布"));
        // 名稱相符但座標在別的島——不可採用。
        assert_eq!(table.lookup("Ubud", -6.2, 106.8), None);
        assert_eq!(table.lookup("Kuta", -8.5067, 115.2625), None);
    }

    /// 同名的 kecamatan 必須各自對到最近的那一筆。
    ///
    /// Reason: `Bandung` 既是西爪哇的萬隆市，也是數個郡的名字。只比對名稱會
    /// 讓萬隆的譯名套到東爪哇的同名郡上。
    #[test]
    fn duplicate_names_resolve_by_distance() {
        let dir = tempfile::tempdir().unwrap();
        let path = write_table(
            dir.path(),
            "Bandung,萬隆,-6.914700,107.609200,wikidata\n\
             Bandung,班東,-7.550000,111.880000,naer\n",
        );
        let table = KecamatanNames::load(&path).unwrap();

        assert_eq!(table.lookup("Bandung", -6.9147, 107.6092), Some("萬隆"));
        assert_eq!(table.lookup("Bandung", -7.55, 111.88), Some("班東"));
        // 兩筆都超過門檻時不猜。
        assert_eq!(table.lookup("Bandung", 1.0, 120.0), None);
    }

    #[test]
    fn malformed_rows_fail_loudly() {
        let dir = tempfile::tempdir().unwrap();
        let path = write_table(dir.path(), "Ubud,烏布,not-a-number,115.2625,wikidata\n");
        let error = KecamatanNames::load(&path).unwrap_err();
        assert!(error.contains("緯度無法解析"), "{error}");
    }
}
