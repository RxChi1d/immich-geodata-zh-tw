//! 分層證明器。對候選 `p` 證明：刪除 `p` 後，`Rp` 中每個查詢位置的 SQL 標籤不變。
//!
//! `Rp = { q : p 在 earth_box(q, 25 km) 內 }`——**不是 p 的 Voronoi cell**。
//! 因為盒對角 `h·√3 ≈ 43.3 km`，故 `Rp ⊆ disc(p, 43.5 km)`，`ROOT_HALF` 由此而來。
//!
//! Reason: 「`Rp` 是 Voronoi cell」這個誤解在辯論中讓雙方各推錯一輪
//! （`notes/debate-staging-2026-09-11/02-errata.md` E1）。定義寫在這裡，不要憑記憶轉述。
//!
//! 分層見 `tiers`。細分採**逐層 wave**：同一深度的未解 cell 一起處理，
//! 預算檢查針對整個 wave 且在擴張**前**。
//!
//! Reason: 預算檢查若放在擴張後，會衝到 189,077 個 cell（上限 50,000）。

pub mod tiers;

use kiddo::{ImmutableKdTree, SquaredEuclidean};

use super::cells::{Rect, V3, cap, corners, frame, geocentric_interval};
use super::geodata::Geo;
use tiers::{CellGeom, Verdict, classify};

pub const MAX_DEPTH: u16 = 24;
pub const MAX_CELLS: u32 = 50_000;
/// 盒對角 `h·√3 ≈ 43.3 km`，向上取整。
pub const ROOT_HALF: f64 = 43_500.0;
pub const K_WITNESS: usize = 24;

/// 證明失敗的原因。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reject {
    /// 預算或深度耗盡。
    Exhausted,
}

pub struct Outcome {
    pub ok: bool,
    pub reject: Option<Reject>,
    pub cells_used: u32,
    pub max_depth: u16,
    /// 用到的見證點列索引，去重後遞增排序。
    pub witnesses: Vec<u32>,
}

/// 單位向量的 k 近鄰索引。
///
/// Reason: 用 3D 弦長（Euclidean）而非 haversine。兩者在球面上單調對應，
/// k 近鄰集合相同，但弦長不必算 arccos，且沒有極區的數值問題。
pub struct Neighbors {
    tree: ImmutableKdTree<f64, 3>,
    /// 樹內序號 → 全域列索引。
    row_of: Vec<u32>,
}

impl Neighbors {
    pub fn build(xyz: &[V3], rows: &[u32]) -> Self {
        let pts: Vec<[f64; 3]> = rows.iter().map(|&r| xyz[r as usize]).collect();
        Self {
            tree: ImmutableKdTree::new_from_slice(&pts).expect("kd-tree 建構失敗"),
            row_of: rows.to_vec(),
        }
    }

    /// 最近的 `k` 個全域列索引。
    pub fn nearest(&self, q: &V3, k: usize) -> Vec<u32> {
        self.tree
            .query(q)
            .nearest_n::<SquaredEuclidean<f64>>(std::num::NonZeroUsize::new(k).expect("k 必須為正"))
            .execute()
            .into_iter()
            .map(|n| self.row_of[n.item as usize])
            .collect()
    }
}

/// 對單一候選 `p` 執行證明。
///
/// `kept` 為全域保留遮罩（長度 `g.n()`），`t0` 為 p 的 T0 旗標。
///
/// Reason: 逐候選處理而非批次。原型的 `CHUNK=4000` 是 numpy 向量化細節；
/// 逐候選讓「全域列索引 vs 批次序號」這整類錯誤在型別層面消失——本專案已因該錯位
/// 犯錯三次，每次都不拋例外、只安靜給出看似合理的錯數字。
pub fn prove_one(
    g: &Geo,
    nb: &Neighbors,
    p: u32,
    t0: bool,
    kept: &[bool],
    budget_cells: u32,
) -> Outcome {
    debug_assert_eq!(kept.len(), g.n(), "kept 必須是全域遮罩");
    debug_assert!(kept[p as usize], "候選自己必須still在保留集合內");

    let p_xyz = g.xyz[p as usize];
    let p_geo = g.geocentric[p as usize];
    let (e1, e2, n) = frame(&p_xyz);
    let h = super::geodata::h_box();

    let mut wave: Vec<(Rect, u16)> = vec![(
        Rect {
            u0: -ROOT_HALF,
            u1: ROOT_HALF,
            v0: -ROOT_HALF,
            v1: ROOT_HALF,
        },
        0,
    )];
    // Reason: 從 1 起算——root cell 本身要計入預算。原型是 `used = np.ones(nc)`，
    // 從 0 起算等於預算多一格，會讓部分候選多走一層細分而證出原型證不出的結果
    // （pass 1 實測多證出 74 筆，且共同證出者的 cells 數全部恰好差 1）。
    let mut used: u32 = 1;
    let mut max_depth: u16 = 0;
    let mut witnesses: Vec<u32> = Vec::new();

    while !wave.is_empty() {
        let mut unresolved: Vec<(Rect, u16)> = Vec::new();
        for (rect, depth) in &wave {
            max_depth = max_depth.max(*depth);
            let c = corners(&e1, &e2, &n, rect);
            let (m, rho) = cap(&c);
            let (q_lo, q_hi) = geocentric_interval(&m, rho, &c);

            // 1) cell 是否與 Rp 相交：p 是否可能落在該 cell 某個 q 的盒內。
            //    不相交表示刪 p 對這塊區域毫無影響，直接解掉。
            let intersects_rp = (0..3).all(|k| p_geo[k] - q_hi[k] <= h && p_geo[k] - q_lo[k] >= -h);
            if !intersects_rp {
                continue;
            }

            let cand = nb.nearest(&m, K_WITNESS.min(g.n()));
            let live: Vec<u32> = cand
                .into_iter()
                .filter(|&w| w != p && kept[w as usize])
                .collect();

            let cell = CellGeom {
                m,
                rho,
                corners: &c,
                q_lo,
                q_hi,
            };
            match classify(&cell, &live, &g.xyz, &g.geocentric, &p_xyz, t0) {
                Verdict::Covered(w) | Verdict::Dominated(w) => witnesses.push(w),
                Verdict::Unresolved => unresolved.push((*rect, *depth)),
            }
        }

        if unresolved.is_empty() {
            witnesses.sort_unstable();
            witnesses.dedup();
            return Outcome {
                ok: true,
                reject: None,
                cells_used: used,
                max_depth,
                witnesses,
            };
        }

        // 2) 未解 cell 二分。預算與深度檢查都在擴張**前**。
        let cnt = unresolved.len() as u32;
        if used + 4 * cnt > budget_cells || unresolved.iter().any(|(_, d)| *d >= MAX_DEPTH) {
            witnesses.sort_unstable();
            witnesses.dedup();
            return Outcome {
                ok: false,
                reject: Some(Reject::Exhausted),
                cells_used: used,
                max_depth,
                witnesses,
            };
        }
        used += 4 * cnt;

        wave = unresolved
            .iter()
            .flat_map(|(r, d)| r.split().into_iter().map(move |c| (c, d + 1)))
            .collect();
    }

    witnesses.sort_unstable();
    witnesses.dedup();
    Outcome {
        ok: true,
        reject: None,
        cells_used: used,
        max_depth,
        witnesses,
    }
}
