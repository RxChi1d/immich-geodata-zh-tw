//! 球面 Delaunay：以 3D 凸包（Qhull `Qt`）求得，邊集、鄰接表與 T0 旗標。
//!
//! Reason: 單位球面上的點，其 3D 凸包的 facet 恰為球面 Delaunay 三角形。
//! 不加抖動——抖動會讓結果不可重現，且退化情形由 `Qt` 自行三角化。

use sha2::{Digest, Sha256};

use super::geodata::Geo;

/// 球面 Delaunay 的結果。
pub struct Delaunay {
    /// 凸包 facet，每列三個列索引。
    pub simplices: Vec<[u32; 3]>,
    /// 去重後的邊集，每列 `(i, j)` 且 `i < j`，字典序排序。
    pub edges: Vec<[u32; 2]>,
    /// CSR 鄰接表的 offset，長度 `n + 1`。
    pub indptr: Vec<u32>,
    /// CSR 鄰接表的鄰居列索引。
    pub indices: Vec<u32>,
}

impl Delaunay {
    pub fn build(g: &Geo) -> Result<Self, String> {
        let n = g.n();
        // Reason: `Qt` 把非單體 facet（共面點）三角化，與 scipy 的
        // `ConvexHull(xyz, qhull_options="Qt")` 一致。少了它，共面處會回傳 4 個以上
        // 頂點的 facet，靜靜被跳過 → 邊集缺角、點的度數變 0。
        let qh = qhull::Qh::builder()
            .compute(true)
            .qhull_args(["Qt"])
            .map_err(|e| format!("Qhull 參數設定失敗: {e:?}"))?
            .build_from_iter(g.xyz.iter().copied())
            .map_err(|e| format!("Qhull 建構失敗: {e:?}"))?;

        let mut simplices: Vec<[u32; 3]> = Vec::new();
        for f in qh.facets() {
            let Some(vs) = f.vertices() else { continue };
            // Reason: 必須用 `point_id`（輸入點的列索引），不是 `index`（Qhull 內部
            // 頂點序號）。本專案已因「全域列索引 vs 其他索引」錯位犯錯三次，每次都不
            // 拋例外、只安靜給出看似合理的錯數字。
            let verts: Vec<u32> = vs
                .iter()
                .map(|v| v.point_id(&qh).expect("facet 頂點必須有 point_id") as u32)
                .collect();
            if verts.len() != 3 {
                return Err(format!(
                    "Qt 之後仍出現 {} 個頂點的 facet，三角化未生效",
                    verts.len()
                ));
            }
            simplices.push([verts[0], verts[1], verts[2]]);
        }
        if simplices.is_empty() {
            return Err("Qhull 未回傳任何三角形 facet".into());
        }

        let mut edges: Vec<[u32; 2]> = Vec::with_capacity(simplices.len() * 3);
        for s in &simplices {
            for (a, b) in [(s[0], s[1]), (s[0], s[2]), (s[1], s[2])] {
                edges.push(if a < b { [a, b] } else { [b, a] });
            }
        }
        edges.sort_unstable();
        edges.dedup();

        // Reason: 每個點都必須是凸包頂點。球面上若有點落在包內表示座標重複或退化，
        // 那會讓 Voronoi cell 的定義失效，必須當成錯誤而不是靜靜跳過。
        let mut degree = vec![0u32; n];
        for e in &edges {
            degree[e[0] as usize] += 1;
            degree[e[1] as usize] += 1;
        }
        if let Some(i) = degree.iter().position(|&d| d == 0) {
            return Err(format!("列 {i} 不是凸包頂點（度數 0），需處理退化點"));
        }

        let mut indptr = vec![0u32; n + 1];
        for i in 0..n {
            indptr[i + 1] = indptr[i] + degree[i];
        }
        let mut cursor = indptr.clone();
        let mut indices = vec![0u32; edges.len() * 2];
        for e in &edges {
            let (i, j) = (e[0] as usize, e[1] as usize);
            indices[cursor[i] as usize] = e[1];
            cursor[i] += 1;
            indices[cursor[j] as usize] = e[0];
            cursor[j] += 1;
        }
        // Reason: 鄰居排序讓走訪順序與建構順序無關，這是平行化後輸出可重現的前提之一。
        for i in 0..n {
            indices[indptr[i] as usize..indptr[i + 1] as usize].sort_unstable();
        }

        Ok(Self {
            simplices,
            edges,
            indptr,
            indices,
        })
    }

    /// 邊集的 sha256，與 Python 原型的黃金值逐位元對照。
    ///
    /// Reason: 原型以 `int64` 寫出，這裡必須用同樣的寬度與位元組序，否則雜湊不可比。
    pub fn edges_sha256(&self) -> String {
        let mut h = Sha256::new();
        for e in &self.edges {
            h.update((e[0] as i64).to_le_bytes());
            h.update((e[1] as i64).to_le_bytes());
        }
        h.finalize().iter().map(|b| format!("{b:02x}")).collect()
    }

    pub fn neighbors(&self, i: usize) -> &[u32] {
        &self.indices[self.indptr[i] as usize..self.indptr[i + 1] as usize]
    }

    /// `(has_same, all_same)`：是否存在同 label 鄰居 / 是否全部鄰居同 label（T0）。
    pub fn neighbor_flags(&self, g: &Geo) -> (Vec<bool>, Vec<bool>) {
        let n = g.n();
        let mut has_same = vec![false; n];
        let mut all_same = vec![true; n];
        for e in &self.edges {
            let (i, j) = (e[0] as usize, e[1] as usize);
            if g.label[i] == g.label[j] {
                has_same[i] = true;
                has_same[j] = true;
            } else {
                all_same[i] = false;
                all_same[j] = false;
            }
        }
        (has_same, all_same)
    }
}
