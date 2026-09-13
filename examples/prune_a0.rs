//! A0 黃金值對照：邊集 sha256、T0 計數、逐國統計。
//! 用法：cargo run --release --example prune_a0 -- <cities500.txt> <admin1CodesASCII.txt>

use std::collections::BTreeMap;
use std::path::Path;

use immich_geodata::pipeline::prune::{delaunay::Delaunay, geodata::Geo};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let g = Geo::load(Path::new(&args[1]), Path::new(&args[2])).expect("載入失敗");
    println!("rows {}   labels {}", g.n(), g.n_labels);

    let t = std::time::Instant::now();
    let d = Delaunay::build(&g).expect("Delaunay 建構失敗");
    println!(
        "simplices {}   edges {}   ({:.1}s)",
        d.simplices.len(),
        d.edges.len(),
        t.elapsed().as_secs_f64()
    );
    println!("edges_sha256  {}", d.edges_sha256());

    let (has_same, all_same) = d.neighbor_flags(&g);
    let t0: usize = all_same.iter().filter(|&&b| b).count();
    let hs: usize = has_same.iter().filter(|&&b| b).count();
    println!("T0 全球 {t0}   存在同 label 鄰居 {hs}");

    let mut per: BTreeMap<&str, [usize; 3]> = BTreeMap::new();
    for i in 0..g.n() {
        let e = per.entry(g.country[i].as_str()).or_insert([0; 3]);
        e[0] += 1;
        e[1] += all_same[i] as usize;
        e[2] += has_same[i] as usize;
    }
    let mut v: Vec<_> = per.into_iter().filter(|(_, c)| c[1] > 0).collect();
    v.sort_by_key(|(_, c)| std::cmp::Reverse(c[1]));
    for (cc, c) in v {
        println!("  {cc}: rows {} T0 {} has_same {}", c[0], c[1], c[2]);
    }
}
