//! A1 區間界驗收（Rust 端）：讀取共用的 cell 定義，輸出本實作宣稱的區間與盒關係。
//!
//! 輸入 `a1_cells.tsv`：`cell_id  p_row  u0  u1  v0  v1  x0  x1  x2`
//! 輸出 `a1_rust_claims.tsv`：`gid(=cell*3+j)  d_lo  d_hi  always  maybe`
//!
//! 由 `a1_check.py` 拿真實 PostgreSQL 的 earth_distance / earth_box 比對。
//! 用法：cargo run --release --example prune_a1 -- <cities500> <admin1> <a1_cells.tsv>

use std::path::Path;

use immich_geodata::pipeline::prune::cells::{
    Rect, box_status, cap, corners, dist_interval, frame, geocentric_interval,
};
use immich_geodata::pipeline::prune::geodata::Geo;

/// 產物一律寫進 PRUNE_OUT 指定的目錄（預設當前目錄），不要污染 repo。
fn out_path(name: &str) -> String {
    match std::env::var("PRUNE_OUT") {
        Ok(d) => format!("{d}/{name}"),
        Err(_) => name.to_string(),
    }
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let g = Geo::load(Path::new(&a[1]), Path::new(&a[2])).expect("載入失敗");
    let text = std::fs::read_to_string(&a[3]).expect("讀取 cell 定義失敗");

    let mut out = String::new();
    let mut n_cell = 0usize;
    for line in text.lines() {
        let f: Vec<&str> = line.split('\t').collect();
        let cell_id: usize = f[0].parse().unwrap();
        let p: usize = f[1].parse().unwrap();
        let r = Rect {
            u0: f[2].parse().unwrap(),
            u1: f[3].parse().unwrap(),
            v0: f[4].parse().unwrap(),
            v1: f[5].parse().unwrap(),
        };
        let (e1, e2, n) = frame(&g.xyz[p]);
        let c = corners(&e1, &e2, &n, &r);
        let (m, rho) = cap(&c);
        let (q_lo, q_hi) = geocentric_interval(&m, rho, &c);
        for (j, xs) in f[6..9].iter().enumerate() {
            let x: usize = xs.parse().unwrap();
            let (d_lo, d_hi) = dist_interval(&m, rho, &c, &g.xyz[x]);
            let (always, maybe) = box_status(&q_lo, &q_hi, &g.geocentric[x]);
            out.push_str(&format!(
                "{}\t{:.17e}\t{:.17e}\t{}\t{}\n",
                cell_id * 3 + j,
                d_lo,
                d_hi,
                always as u8,
                maybe as u8
            ));
        }
        n_cell += 1;
    }
    std::fs::write(out_path("a1_rust_claims.tsv"), out).expect("寫出失敗");
    println!("已寫出 a1_rust_claims.tsv（{n_cell} 個 cell）");
}
