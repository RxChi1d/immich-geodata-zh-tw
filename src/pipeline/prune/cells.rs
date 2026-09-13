//! 心射投影 cell 與保守區間界。
//!
//! 以 p 為中心的心射（gnomonic）投影：切平面座標 `(u, v)`（公尺）對應球面方向
//! `c(u, v) = normalize( n + (u/R)·e1 + (v/R)·e2 )`。
//! 大圓在此投影下映為直線，故直邊矩形反投影後是**測地凸**的四邊形——patch 內任一點
//! 都是四個角點的正規化凸組合 `q = Σλᵢcᵢ / |Σλᵢcᵢ|`。**經緯度矩形沒有這個性質。**
//!
//! 兩組界都成立，一律取交集：
//!
//! - **球冠界**：`patch ⊆ cap(m, ρ)`，`m` 為角點正規化質心、`ρ` 為 `m` 到角點的最大
//!   角距。證明：`q·m = Σλᵢ(cᵢ·m) / |Σλᵢcᵢ| ≥ cos ρ / 1 = cos ρ`。對大 cell 仍有效，
//!   但把方形當成球，很鬆。
//! - **角點界**：分子 `Σλᵢ(cᵢ·x)` 對 `λ` 線性，故落在角點值的 min/max 之間；
//!   分母 `|Σλᵢcᵢ| ∈ [cos ρ, 1]`。小 cell 下 `cos ρ ≈ 1`，此界幾乎精確。
//!
//! Reason: 所有區間一律**向外**放寬涵蓋浮點誤差。向外放寬讓證明更難成立，是保守的；
//! 與被禁止的「小於幾公尺就當安全」epsilon 出口方向相反。原型曾只用球冠界，
//! 導致 Rp 邊界追蹤爆量（20 km 半寬 cell 的地心分量寬度 56.57 km，真值恰為 40 km）。

use super::geodata::{R_EARTH, h_box};

pub const WIDEN_REL: f64 = 1e-12;
/// 公尺。
pub const WIDEN_ABS: f64 = 1e-3;

pub type V3 = [f64; 3];

#[inline]
pub fn dot(a: &V3, b: &V3) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

#[inline]
pub fn cross(a: &V3, b: &V3) -> V3 {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

#[inline]
pub fn norm(a: &V3) -> f64 {
    dot(a, a).sqrt()
}

#[inline]
pub fn normalize(a: &V3) -> V3 {
    let n = norm(a);
    [a[0] / n, a[1] / n, a[2] / n]
}

/// p 的局部正交標架 `(e1, e2, n)`。
pub fn frame(p: &V3) -> (V3, V3, V3) {
    let n = normalize(p);
    // Reason: 選與 n 夾角夠大的參考軸，否則 cross 會退化成零向量。
    let tmp: V3 = if n[0].abs() > 0.9 {
        [0.0, 1.0, 0.0]
    } else {
        [1.0, 0.0, 0.0]
    };
    let e1 = normalize(&cross(&n, &tmp));
    let e2 = cross(&n, &e1);
    (e1, e2, n)
}

/// cell 的矩形範圍（切平面公尺）。
#[derive(Clone, Copy, Debug)]
pub struct Rect {
    pub u0: f64,
    pub u1: f64,
    pub v0: f64,
    pub v1: f64,
}

impl Rect {
    pub fn split(&self) -> [Rect; 4] {
        let (mu, mv) = (0.5 * (self.u0 + self.u1), 0.5 * (self.v0 + self.v1));
        [
            Rect {
                u0: self.u0,
                u1: mu,
                v0: self.v0,
                v1: mv,
            },
            Rect {
                u0: mu,
                u1: self.u1,
                v0: self.v0,
                v1: mv,
            },
            Rect {
                u0: self.u0,
                u1: mu,
                v0: mv,
                v1: self.v1,
            },
            Rect {
                u0: mu,
                u1: self.u1,
                v0: mv,
                v1: self.v1,
            },
        ]
    }
}

/// cell 四角的單位向量。
pub fn corners(e1: &V3, e2: &V3, n: &V3, r: &Rect) -> [V3; 4] {
    let uu = [r.u0, r.u1, r.u1, r.u0];
    let vv = [r.v0, r.v0, r.v1, r.v1];
    std::array::from_fn(|i| {
        let (a, b) = (uu[i] / R_EARTH, vv[i] / R_EARTH);
        normalize(&[
            n[0] + a * e1[0] + b * e2[0],
            n[1] + a * e1[1] + b * e2[1],
            n[2] + a * e1[2] + b * e2[2],
        ])
    })
}

/// patch 的外接球冠 `(m, ρ)`。
pub fn cap(c: &[V3; 4]) -> (V3, f64) {
    let mut s = [0.0; 3];
    for v in c {
        for k in 0..3 {
            s[k] += v[k];
        }
    }
    let m = normalize(&s);
    let cos_min = c.iter().map(|v| dot(v, &m)).fold(f64::INFINITY, f64::min);
    let rho = cos_min.clamp(-1.0, 1.0).acos() * (1.0 + WIDEN_REL) + 1e-15;
    (m, rho)
}

/// `|Σλᵢcᵢ|` 的下界 `cos ρ`。
#[inline]
fn cr(rho: f64) -> f64 {
    rho.min(std::f64::consts::FRAC_PI_2).cos().max(1e-12)
}

/// `d(q, x)` 的保守區間（公尺），`x` 為單位向量。
pub fn dist_interval(m: &V3, rho: f64, c: &[V3; 4], x: &V3) -> (f64, f64) {
    let theta = dot(m, x).clamp(-1.0, 1.0).acos();
    let mut lo = R_EARTH * (theta - rho).max(0.0) - WIDEN_ABS;
    let mut hi = R_EARTH * (theta + rho).min(std::f64::consts::PI) + WIDEN_ABS;

    let crv = cr(rho);
    let (mut n_lo, mut n_hi) = (f64::INFINITY, f64::NEG_INFINITY);
    for ci in c {
        let d = dot(ci, x);
        n_lo = n_lo.min(d);
        n_hi = n_hi.max(d);
    }
    let dot_lo = if n_lo >= 0.0 { n_lo } else { n_lo / crv };
    let dot_hi = if n_hi >= 0.0 { n_hi / crv } else { n_hi };
    // Reason: 這個相對放寬項在原型裡只存在於 cells.py，`prove.py` 內嵌了自己**沒有**
    // 它的版本——而 A1 對 PostgreSQL 驗證的是 cells.py 那版，所以原型的證明器實際用的
    // 界比驗證過的鬆。這裡保留 w（較嚴、方向保守），代價為零：pass 1 的判定與刪除
    // 集合與原型完全相同，只有 34 個被拒候選的 cell 計數差 4。
    let w = WIDEN_REL * dot_lo.abs().max(dot_hi.abs()) + 1e-15;
    lo = lo.max(R_EARTH * (dot_hi + w).clamp(-1.0, 1.0).acos() - WIDEN_ABS);
    hi = hi.min(R_EARTH * (dot_lo - w).clamp(-1.0, 1.0).acos() + WIDEN_ABS);
    (lo.max(0.0), hi)
}

/// `q` 的三個地心座標分量區間（公尺）。
pub fn geocentric_interval(m: &V3, rho: f64, c: &[V3; 4]) -> (V3, V3) {
    let half = 2.0 * (rho.min(std::f64::consts::PI) / 2.0).sin();
    let w = R_EARTH * half * (1.0 + WIDEN_REL) + WIDEN_ABS;
    let crv = cr(rho);
    let mut lo = [0.0; 3];
    let mut hi = [0.0; 3];
    for k in 0..3 {
        let centre = m[k] * R_EARTH;
        let (mut a, mut b) = (centre - w, centre + w);
        let n_lo = c.iter().map(|v| v[k]).fold(f64::INFINITY, f64::min);
        let n_hi = c.iter().map(|v| v[k]).fold(f64::NEG_INFINITY, f64::max);
        let c_lo = (if n_lo >= 0.0 { n_lo } else { n_lo / crv }) * R_EARTH;
        let c_hi = (if n_hi >= 0.0 { n_hi / crv } else { n_hi }) * R_EARTH;
        let ww = WIDEN_REL * c_lo.abs().max(c_hi.abs()) + WIDEN_ABS;
        a = a.max(c_lo - ww);
        b = b.min(c_hi + ww);
        lo[k] = a;
        hi[k] = b;
    }
    (lo, hi)
}

/// `xg`（地心公尺）相對 cell 的盒關係：`(always_in, maybe_in)`。
pub fn box_status(q_lo: &V3, q_hi: &V3, xg: &V3) -> (bool, bool) {
    let h = h_box();
    let mut always = true;
    let mut maybe = true;
    for k in 0..3 {
        let lo = xg[k] - q_hi[k];
        let hi = xg[k] - q_lo[k];
        always &= lo >= -h && hi <= h;
        maybe &= lo <= h && hi >= -h;
    }
    (always, maybe)
}

/// cell 內均勻取樣一個球面點（僅供驗證區間界，不用於證明）。
pub fn sample_in_cell(e1: &V3, e2: &V3, n: &V3, r: &Rect, tu: f64, tv: f64) -> V3 {
    let u = (r.u0 + tu * (r.u1 - r.u0)) / R_EARTH;
    let v = (r.v0 + tv * (r.v1 - r.v0)) / R_EARTH;
    normalize(&[
        n[0] + u * e1[0] + v * e2[0],
        n[1] + u * e1[1] + v * e2[1],
        n[2] + u * e1[2] + v * e2[2],
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn axis_frame() -> (V3, V3, V3) {
        frame(&[1.0, 0.0, 0.0])
    }

    #[test]
    fn frame_is_orthonormal_including_the_x_pole() {
        // Reason: n[0] > 0.9 這條分支若寫錯，cross 會退化成零向量而靜靜產出 NaN。
        for p in [
            [1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            [0.0, 0.0, 1.0],
            [0.6, 0.8, 0.0],
        ] {
            let (e1, e2, n) = frame(&p);
            for (a, b) in [(&e1, &e2), (&e1, &n), (&e2, &n)] {
                assert!(dot(a, b).abs() < 1e-12, "標架不正交: {p:?}");
            }
            for v in [&e1, &e2, &n] {
                assert!((norm(v) - 1.0).abs() < 1e-12, "標架未正規化: {p:?}");
            }
        }
    }

    #[test]
    fn geocentric_interval_is_tight_for_a_known_cell() {
        // 半寬 20 km 的 cell，真實的地心分量寬度恰為 40 km。
        // Reason: 只用球冠界時這裡會得到 56.57 km（把方形當成球），該鬆度讓
        // Rp 邊界追蹤爆量。角點界把它收回到真值附近。
        let (e1, e2, n) = axis_frame();
        let r = Rect {
            u0: -20_000.0,
            u1: 20_000.0,
            v0: -20_000.0,
            v1: 20_000.0,
        };
        let c = corners(&e1, &e2, &n, &r);
        let (m, rho) = cap(&c);
        let (lo, hi) = geocentric_interval(&m, rho, &c);
        let width = hi[1] - lo[1];
        assert!(
            (width - 40_000.0).abs() < 50.0,
            "地心分量寬度應接近 40 km，實際 {width}"
        );
    }

    #[test]
    fn dist_interval_contains_every_sampled_point() {
        // 保守性檢查：cell 內取樣點的真實距離必須落在宣稱的區間內。
        let (e1, e2, n) = axis_frame();
        let r = Rect {
            u0: -5_000.0,
            u1: 15_000.0,
            v0: -8_000.0,
            v1: 12_000.0,
        };
        let c = corners(&e1, &e2, &n, &r);
        let (m, rho) = cap(&c);
        let x = normalize(&[0.999, 0.03, 0.02]);
        let (lo, hi) = dist_interval(&m, rho, &c, &x);
        for i in 0..=10 {
            for j in 0..=10 {
                let q = sample_in_cell(&e1, &e2, &n, &r, i as f64 / 10.0, j as f64 / 10.0);
                let d = R_EARTH * dot(&q, &x).clamp(-1.0, 1.0).acos();
                assert!(
                    d >= lo && d <= hi,
                    "取樣點距離 {d} 落在區間 [{lo}, {hi}] 之外"
                );
            }
        }
    }

    #[test]
    fn geocentric_interval_contains_every_sampled_point() {
        let (e1, e2, n) = axis_frame();
        let r = Rect {
            u0: -12_000.0,
            u1: 9_000.0,
            v0: -3_000.0,
            v1: 18_000.0,
        };
        let c = corners(&e1, &e2, &n, &r);
        let (m, rho) = cap(&c);
        let (lo, hi) = geocentric_interval(&m, rho, &c);
        for i in 0..=10 {
            for j in 0..=10 {
                let q = sample_in_cell(&e1, &e2, &n, &r, i as f64 / 10.0, j as f64 / 10.0);
                for k in 0..3 {
                    let g = q[k] * R_EARTH;
                    assert!(g >= lo[k] && g <= hi[k], "地心分量 {k} 的 {g} 超出區間");
                }
            }
        }
    }

    #[test]
    fn splitting_a_rect_covers_it_exactly() {
        let r = Rect {
            u0: -4.0,
            u1: 8.0,
            v0: -2.0,
            v1: 10.0,
        };
        let parts = r.split();
        let area: f64 = parts.iter().map(|p| (p.u1 - p.u0) * (p.v1 - p.v0)).sum();
        assert!((area - (r.u1 - r.u0) * (r.v1 - r.v0)).abs() < 1e-9);
    }
}
