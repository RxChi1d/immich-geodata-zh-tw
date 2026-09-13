//! 多趟剪枝執行，逐趟印出與原型對照的數字。
//! 用法：cargo run --release --example prune_run -- <cities500> <admin1> [pass 上限]

use std::path::Path;

use immich_geodata::pipeline::prune::{geodata::Geo, multipass};

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
    let mut cfg = multipass::Config {
        threads: std::env::var("PRUNE_THREADS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(0),
        ..Default::default()
    };
    if let Some(m) = a.get(3) {
        cfg.max_pass = m.parse().expect("pass 上限必須是整數");
    }
    if let Ok(b) = std::env::var("PRUNE_BUDGET") {
        cfg.budget = b.parse().expect("預算必須是整數");
    }
    println!(
        "rows {}  labels {}  預算 {}  上限 {} 趟",
        g.n(),
        g.n_labels,
        cfg.budget,
        cfg.max_pass
    );

    let mut dump: Vec<String> = Vec::new();
    let (deleted, _) = multipass::run_with_dump(&g, &cfg, &mut dump, |p| {
        println!(
            "pass {}: 候選 {:>7} 證出 {:>7} 刪 {:>6} 累計 {:>7} ({:.2}%) {:.0}s",
            p.pass,
            p.candidates,
            p.proved,
            p.deleted,
            p.cumulative,
            100.0 * p.cumulative as f64 / g.n() as f64,
            p.seconds
        );
    })
    .expect("剪枝失敗");

    let n = deleted.iter().filter(|&&b| b).count();
    println!(
        "總刪除 {n} / {} = {:.2}%",
        g.n(),
        100.0 * n as f64 / g.n() as f64
    );
    let ids: Vec<i64> = (0..g.n())
        .filter(|&i| deleted[i])
        .map(|i| g.gid[i])
        .collect();
    std::fs::write(
        out_path("prune_deleted.txt"),
        ids.iter()
            .map(|i| i.to_string())
            .collect::<Vec<_>>()
            .join("\n"),
    )
    .expect("寫出失敗");
    std::fs::write(out_path("prune_proven.txt"), dump.join("\n")).expect("寫出失敗");
    println!("被刪 geoname_id 已寫入 prune_deleted.txt；逐候選證明結果寫入 prune_proven.txt");

    // 寫出剪枝後的 cities500，供差分與體積量測。
    let drop: std::collections::HashSet<i64> = ids.into_iter().collect();
    let src = std::fs::read_to_string(&a[1]).expect("重讀來源失敗");
    let mut kept_lines = String::with_capacity(src.len());
    let mut n_out = 0usize;
    for line in src.lines() {
        let gid: i64 = line
            .split('\t')
            .next()
            .unwrap()
            .parse()
            .expect("geoname_id 必須是整數");
        if !drop.contains(&gid) {
            kept_lines.push_str(line);
            kept_lines.push('\n');
            n_out += 1;
        }
    }
    let out = out_path("cities500_pruned.txt");
    std::fs::write(&out, kept_lines).expect("寫出剪枝檔失敗");
    println!("剪枝後 {n_out} 列 → {out}");
}
