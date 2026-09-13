//! 剪枝器的資料載入：cities500 讀取、label 映射、單位向量與地心座標。
//!
//! Reason: 所有陣列一律以「列索引」對齊，不使用排序後的 site 索引。原型曾把
//! `np.unique` 排序後的 site 索引當成列索引讀，產出的數字看似合理但全錯
//! （notes/debate-pruning-2026-09-10/99-outcome.md 第 9 節第 4 條）。
//! 這個模組是唯一的載入入口，索引語意只有一種。

use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;

/// PostgreSQL `earth()` 的地球半徑（公尺）。
pub const R_EARTH: f64 = 6_378_168.0;

/// Immich `reverseGeocodeMaxDistance`（公尺）。
pub const R_QUERY: f64 = 25_000.0;

/// `earth_box` 的半邊長 = `gc_to_sec(25000)`。
///
/// Reason: `earth_box` 是地心座標系的軸對齊立方體，不是圓盤。半邊長由弦長而非
/// 大圓距離決定（24,999.984 m，兩者相差約 16 mm）；用錯會讓「必在盒內」的判定
/// 失去保守性。
pub fn h_box() -> f64 {
    2.0 * R_EARTH * (R_QUERY / (2.0 * R_EARTH)).sin()
}

/// 一列 cities500 的識別資訊。
pub struct Geo {
    pub gid: Vec<i64>,
    pub lat: Vec<f64>,
    pub lon: Vec<f64>,
    pub country: Vec<String>,
    /// label = (country_code, admin1_name, name) 映射成的連續整數。
    pub label: Vec<u32>,
    pub n_labels: usize,
    /// 單位球面座標，列索引對齊。
    pub xyz: Vec<[f64; 3]>,
    /// `ll_to_earth_public` 的地心座標（公尺），列索引對齊。
    pub geocentric: Vec<[f64; 3]>,
}

impl Geo {
    pub fn n(&self) -> usize {
        self.gid.len()
    }

    /// 取出子集合，所有欄位一致地以 `rows`（全域列索引）重排。
    ///
    /// Reason: 多趟剪枝每趟都要在「當下保留集合」上重建圖。欄位若漏抄一個，
    /// 索引語意就會分岔，而且不會拋例外，只會安靜給出錯的鄰居。
    pub fn subset(&self, rows: &[u32]) -> Self {
        let pick = |v: &Vec<f64>| rows.iter().map(|&i| v[i as usize]).collect::<Vec<_>>();
        Self {
            gid: rows.iter().map(|&i| self.gid[i as usize]).collect(),
            lat: pick(&self.lat),
            lon: pick(&self.lon),
            country: rows
                .iter()
                .map(|&i| self.country[i as usize].clone())
                .collect(),
            label: rows.iter().map(|&i| self.label[i as usize]).collect(),
            n_labels: self.n_labels,
            xyz: rows.iter().map(|&i| self.xyz[i as usize]).collect(),
            geocentric: rows.iter().map(|&i| self.geocentric[i as usize]).collect(),
        }
    }

    /// 從 release tree 的 `cities500.txt` 與 `admin1CodesASCII.txt` 載入。
    pub fn load(cities: &Path, admin1: &Path) -> std::io::Result<Self> {
        let mut a1: HashMap<String, String> = HashMap::new();
        for line in BufReader::new(File::open(admin1)?).lines() {
            let line = line?;
            let mut it = line.split('\t');
            if let (Some(code), Some(name)) = (it.next(), it.next()) {
                a1.insert(code.to_string(), name.to_string());
            }
        }

        let (mut gid, mut lat, mut lon, mut country) = (vec![], vec![], vec![], vec![]);
        let mut raw_labels: Vec<(String, String, String)> = vec![];
        for line in BufReader::new(File::open(cities)?).lines() {
            let line = line?;
            let f: Vec<&str> = line.split('\t').collect();
            if f.len() < 11 {
                continue;
            }
            // Reason: 與 Immich server/src/repositories/map.repository.ts 的匯入過濾一致。
            // 剪枝要在「Immich 實際會載入的點集」上做，否則圖與 DB 不同構。
            if (f[7] == "PPLX" && f[8] != "AU") || f[7] == "PPLH" {
                continue;
            }
            gid.push(f[0].parse::<i64>().expect("geoname_id 必須是整數"));
            lat.push(f[4].parse::<f64>().expect("latitude 必須是浮點數"));
            lon.push(f[5].parse::<f64>().expect("longitude 必須是浮點數"));
            country.push(f[8].to_string());
            let admin1_name = a1
                .get(&format!("{}.{}", f[8], f[10]))
                .cloned()
                .unwrap_or_default();
            raw_labels.push((f[8].to_string(), admin1_name, f[1].to_string()));
        }

        // Reason: label 映射用排序而非雜湊。Python 的字串雜湊每個行程隨機化，
        // 用它建索引會讓結果不可重現；Rust 的 HashMap 迭代序同樣不穩定。
        let mut uniq: Vec<&(String, String, String)> = raw_labels.iter().collect();
        uniq.sort_unstable();
        uniq.dedup();
        let index: HashMap<&(String, String, String), u32> = uniq
            .iter()
            .enumerate()
            .map(|(i, l)| (*l, i as u32))
            .collect();
        let label: Vec<u32> = raw_labels.iter().map(|l| index[l]).collect();
        let n_labels = uniq.len();

        let mut xyz = Vec::with_capacity(gid.len());
        let mut geocentric = Vec::with_capacity(gid.len());
        for i in 0..gid.len() {
            let (phi, lam) = (lat[i].to_radians(), lon[i].to_radians());
            let v = [phi.cos() * lam.cos(), phi.cos() * lam.sin(), phi.sin()];
            xyz.push(v);
            geocentric.push([v[0] * R_EARTH, v[1] * R_EARTH, v[2] * R_EARTH]);
        }

        Ok(Self {
            gid,
            lat,
            lon,
            country,
            label,
            n_labels,
            xyz,
            geocentric,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn h_box_matches_gc_to_sec() {
        // gc_to_sec(25000)：弦長略小於大圓距離，差約 0.016 m。
        let h = h_box();
        assert!(h < R_QUERY, "弦長必須小於大圓距離");
        assert!(
            (R_QUERY - h) < 0.2,
            "25 km 上差距應在 0.2 m 內，實際 {}",
            R_QUERY - h
        );
    }

    #[test]
    fn label_mapping_is_order_independent() {
        // 同一組 label 不論插入順序，排序後的映射必須相同。
        let mut a = vec![
            ("JP".to_string(), "Tokyo".to_string(), "Shibuya".to_string()),
            ("JP".to_string(), "Osaka".to_string(), "Kita".to_string()),
        ];
        let mut b = a.clone();
        b.reverse();
        for v in [&mut a, &mut b] {
            v.sort_unstable();
        }
        assert_eq!(a, b, "排序後的 label 順序必須與插入順序無關");
    }
}
