//! 逐 cell 的分層判定：T1（覆蓋）與 T2（被壓制）。
//!
//! **T0 從不單獨使用。** 剪枝實際成立的條件是 `T0 ∧ T1` 或 `T2`。
//! T1 是把盒語意轉成純最近鄰語意的那一步：
//!
//! > cell 內每個 `q` 都有保留點 `s` 在 25 km 內
//! > ⟹（25 km 引理）`q` 的最近保留點必在盒內
//! > ⟹ `ORDER BY earth_distance LIMIT 1` 選的就是全域最近保留點
//! > ⟹ 該 cell 上 **SQL ≡ 純最近鄰**。
//!
//! 25 km 引理：大圓 ≤ 25 km ⟹ 弦 ≤ 25 km ⟹ 每個地心軸分量差 ≤ `h` ⟹ 必在盒內。
//! （已以 100 萬隨機球面樣本驗證，0 例外。）
//!
//! **存在性 vs 窮盡性**：T1/T2 只需找到**一個**見證點，所以用 k 近鄰搜尋是安全的
//! ——漏掉候選只會讓證明失敗（保守）。T3 若要實作，其競爭者必須用半徑查詢窮盡
//! 列舉，漏掉一個競爭者會讓證明**錯誤成立**。這是 T3 未實作的原因。

use crate::pipeline::prune::cells::{V3, dist_interval};
use crate::pipeline::prune::geodata::R_QUERY;

/// 一個 cell 的幾何摘要，交給分層判定使用。
pub struct CellGeom<'a> {
    pub m: V3,
    pub rho: f64,
    pub corners: &'a [V3; 4],
    /// `q` 的地心分量區間。
    pub q_lo: V3,
    pub q_hi: V3,
}

/// 判定結果：解掉了就回傳見證點的列索引。
pub enum Verdict {
    /// T1 成立（需 T0）。
    Covered(u32),
    /// T2 成立：某保留點整個 cell 都必入盒且嚴格近於 p。
    Dominated(u32),
    /// 兩條都證不出來，需要細分。
    Unresolved,
}

/// 對單一 cell 嘗試 T1 與 T2。
///
/// `witnesses` 為候選見證點的列索引（已過濾：仍保留、且不是 p 自己）。
/// `t0` 為 p 的 T0 旗標——**T1 沒有它不成立**。
pub fn classify(
    cell: &CellGeom<'_>,
    witnesses: &[u32],
    xyz: &[V3],
    geocentric: &[V3],
    p_xyz: &V3,
    t0: bool,
) -> Verdict {
    // p 到 cell 的距離下界，供 T2 的「嚴格近於 p」比較。
    let (dp_lo, _) = dist_interval(&cell.m, cell.rho, cell.corners, p_xyz);

    let mut covered: Option<u32> = None;
    let mut dominated: Option<u32> = None;
    for &w in witnesses {
        let x = &xyz[w as usize];
        let (_, d_hi) = dist_interval(&cell.m, cell.rho, cell.corners, x);

        // T1：整個 cell 都在該保留點的 25 km 內。
        if t0 && d_hi <= R_QUERY && covered.is_none() {
            covered = Some(w);
        }

        // T2：該保留點在 cell 的每個 q 的盒內，且嚴格近於 p。
        if dominated.is_none() && d_hi < dp_lo {
            let (always_in, _) = crate::pipeline::prune::cells::box_status(
                &cell.q_lo,
                &cell.q_hi,
                &geocentric[w as usize],
            );
            if always_in {
                dominated = Some(w);
            }
        }
    }

    // Reason: 原型先判 T1 再判 T2（`done = has_t1 | has_t2`，`pick` 以 has_t1 優先），
    // 順序影響記錄哪個見證點，進而影響 pin 與跨趟重用。照抄。
    match (covered, dominated) {
        (Some(w), _) => Verdict::Covered(w),
        (None, Some(w)) => Verdict::Dominated(w),
        (None, None) => Verdict::Unresolved,
    }
}
