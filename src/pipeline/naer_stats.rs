//! NAER 翻譯層的 translate 階段統計：採用計數、拒絕原因分類與距離分布桶。
//!
//! 設計切分：「採用結果」計數（fill/override/demote）與 city 距離分布由
//! translate.rs 在實際套用譯名時歸類記錄——只有 caller 知道既有譯名是否
//! 存在、匹配是否真正被採用（中信心 demote 不採用即不記錄距離）。
//! 「拒絕原因」計數由 lookup_city / lookup_admin1 內部歸類——消歧細節只有
//! lookup 能準確判斷；admin1 距離亦在 lookup 內記錄，因其唯一 caller 對
//! Some 回傳必定採用（fill-only）。

/// 距離分布桶：對「被採用」的匹配記錄其消歧距離，供品質報告判讀座標
/// 吻合度。桶界為 [0,1km)、[1,5km)、[5,15km]（city 容差上限 15km；
/// admin1 容差較寬，超過 5km 一律歸入 far 桶以共用同一摘要結構）。
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct DistanceBuckets {
    /// [0, 1km)
    pub near: usize,
    /// [1km, 5km)
    pub mid: usize,
    /// [5km, 容差上限]
    pub far: usize,
}

impl DistanceBuckets {
    /// 依距離（公里）歸入對應桶。
    fn record(&mut self, distance_km: f64) {
        if distance_km < 1.0 {
            self.near += 1;
        } else if distance_km < 5.0 {
            self.mid += 1;
        } else {
            self.far += 1;
        }
    }
}

/// translate 階段的 NAER 統計，輸出於品質報告 log。
///
/// 採用計數（city_fill/city_override/city_demoted_kept_existing/admin1_fill）
/// 由 translate.rs 歸類；拒絕計數與距離分布由 lookup 內部掛鉤填入。
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct NaerStats {
    // --- 採用計數（由 translate.rs 歸類）---
    /// city 既無中文名、NAER 補洞。
    pub city_fill: usize,
    /// city 既有中文名、NAER 高信心覆寫。
    pub city_override: usize,
    /// city 既有中文名、NAER 為中信心 → 保留既有、不覆寫。
    pub city_demoted_kept_existing: usize,
    /// admin1 既無中文名、NAER 補洞。
    pub admin1_fill: usize,

    // --- city 拒絕計數（由 lookup_city 內部歸類）---
    // Reason: handler 跳過與「name 完全無候選」為常態，不計入拒絕；以下
    // 兩項為「候選存在但被消歧規則排除」的真實拒絕，是品質警訊的觀測點。
    /// 有候選但全部超出距離容差（>15km）。
    pub city_rejected_distance: usize,
    /// 有候選但國碼全不符、且無空國碼候選可降級使用。
    pub city_rejected_country: usize,

    // --- admin1 拒絕計數（由 lookup_admin1 內部歸類）---
    /// 有候選但該 admin1 無質心（無轄下城市）可驗證。
    pub admin1_rejected_no_centroid: usize,
    /// 有候選但質心驗證全部超距（>300km）。
    pub admin1_rejected_distance: usize,
    /// 距離合格但近距存在不同譯名 → 質心無法消歧、保守放棄。
    pub admin1_rejected_ambiguous: usize,

    // --- 被採用匹配的距離分布 ---
    /// city 被採用匹配的座標消歧距離分布。
    pub city_distance: DistanceBuckets,
    /// admin1 被採用匹配的質心驗證距離分布。
    pub admin1_distance: DistanceBuckets,
}

impl NaerStats {
    /// 記錄一筆「被採用」的 city 匹配距離。
    pub fn record_city_distance(&mut self, distance_km: f64) {
        self.city_distance.record(distance_km);
    }

    /// 記錄一筆「被採用」的 admin1 匹配距離。
    pub fn record_admin1_distance(&mut self, distance_km: f64) {
        self.admin1_distance.record(distance_km);
    }

    /// 輸出全部統計為單行 log，欄位以 `key=value` 形式呈現、空白分隔，
    /// 方便以 grep/awk 解析與跨版本對照。
    pub fn log_line(&self) -> String {
        format!(
            "stage=translate naer \
             city_fill={} city_override={} city_demoted_kept_existing={} admin1_fill={} \
             city_rejected_distance={} city_rejected_country={} \
             admin1_rejected_no_centroid={} admin1_rejected_distance={} admin1_rejected_ambiguous={} \
             city_dist_0_1km={} city_dist_1_5km={} city_dist_5_15km={} \
             admin1_dist_0_1km={} admin1_dist_1_5km={} admin1_dist_5km_plus={}",
            self.city_fill,
            self.city_override,
            self.city_demoted_kept_existing,
            self.admin1_fill,
            self.city_rejected_distance,
            self.city_rejected_country,
            self.admin1_rejected_no_centroid,
            self.admin1_rejected_distance,
            self.admin1_rejected_ambiguous,
            self.city_distance.near,
            self.city_distance.mid,
            self.city_distance.far,
            self.admin1_distance.near,
            self.admin1_distance.mid,
            self.admin1_distance.far,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn distance_buckets_classify_boundaries() {
        let mut buckets = DistanceBuckets::default();
        // 邊界：0 與 0.999 落 near；1.0 落 mid（左閉右開）。
        buckets.record(0.0);
        buckets.record(0.999);
        buckets.record(1.0);
        buckets.record(4.999);
        buckets.record(5.0);
        buckets.record(15.0);
        assert_eq!(buckets.near, 2);
        assert_eq!(buckets.mid, 2);
        assert_eq!(buckets.far, 2);
    }

    #[test]
    fn log_line_contains_all_fields() {
        let stats = NaerStats {
            city_fill: 1,
            city_override: 2,
            city_demoted_kept_existing: 3,
            admin1_fill: 4,
            city_rejected_distance: 5,
            city_rejected_country: 6,
            admin1_rejected_no_centroid: 7,
            admin1_rejected_distance: 8,
            admin1_rejected_ambiguous: 9,
            city_distance: DistanceBuckets {
                near: 10,
                mid: 11,
                far: 12,
            },
            admin1_distance: DistanceBuckets {
                near: 13,
                mid: 14,
                far: 15,
            },
        };
        let line = stats.log_line();
        for token in [
            "stage=translate",
            "naer",
            "city_fill=1",
            "city_override=2",
            "city_demoted_kept_existing=3",
            "admin1_fill=4",
            "city_rejected_distance=5",
            "city_rejected_country=6",
            "admin1_rejected_no_centroid=7",
            "admin1_rejected_distance=8",
            "admin1_rejected_ambiguous=9",
            "city_dist_0_1km=10",
            "city_dist_1_5km=11",
            "city_dist_5_15km=12",
            "admin1_dist_0_1km=13",
            "admin1_dist_1_5km=14",
            "admin1_dist_5km_plus=15",
        ] {
            assert!(line.contains(token), "log_line 缺少 `{token}`：{line}");
        }
    }
}
