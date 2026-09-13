//! release pipeline 的剪枝階段：translate 之後、pack 之前。
//!
//! 刪除「刪了也不改變任何 Immich 反向地理編碼答案」的點，縮小解壓後的
//! `cities500.txt` 與匯入後的 `geodata_places` 表。
//!
//! 完整規格、證明與辯論紀錄見主 worktree 的 `notes/plan-voronoi-pruning.md`
//! 與 `notes/debate-staging-2026-09-11/`（兩者皆 gitignore）。

use std::path::{Path, PathBuf};

use super::geodata::Geo;
use super::multipass;

pub struct PruneOptions {
    /// translate 產出的 `cities500_translated.txt`。
    pub cities_file: PathBuf,
    /// admin1 名稱表，用於組出 label（國碼＋admin1 名稱＋地名）。
    pub admin1_file: PathBuf,
    /// 就地覆寫 `cities_file`；`None` 時寫到這裡。
    pub output_file: Option<PathBuf>,
    pub config: multipass::Config,
}

#[derive(Debug)]
pub struct PruneReport {
    pub rows_in: usize,
    pub rows_out: usize,
    pub deleted: usize,
    pub passes: usize,
    pub seconds: f64,
}

pub fn run(options: &PruneOptions) -> Result<PruneReport, String> {
    let started = std::time::Instant::now();
    let g = Geo::load(&options.cities_file, &options.admin1_file)
        .map_err(|e| format!("剪枝：載入 {} 失敗：{e}", options.cities_file.display()))?;
    let rows_in = g.n();

    // Reason: 用 println! 而非 log 巨集。本 crate 從未安裝 logger 實作，
    // log::info! 會被靜默丟掉，這個階段要跑好幾分鐘，中途完全沒有輸出。
    // 其餘 pipeline 階段也一律 println!（stage=... 形式）。
    let (deleted, log) = multipass::run(&g, &options.config, |p| {
        println!(
            "stage=prune pass={} 候選={} 證出={} 刪={} 累計={} ({:.2}%) {:.0}s",
            p.pass,
            p.candidates,
            p.proved,
            p.deleted,
            p.cumulative,
            100.0 * p.cumulative as f64 / rows_in as f64,
            p.seconds
        );
    })?;

    let drop: std::collections::HashSet<i64> = (0..g.n())
        .filter(|&i| deleted[i])
        .map(|i| g.gid[i])
        .collect();

    // Reason: 必須在寫出**之前**數。就地覆寫（output_file 為 None）會把來源檔換掉，
    // 寫完再數只會數到剪枝後的結果，交叉檢查等於自我比較，永遠通過或永遠失敗。
    let file_rows = count_lines(&options.cities_file)?;

    let dst = options
        .output_file
        .clone()
        .unwrap_or_else(|| options.cities_file.clone());
    let rows_out = write_kept(&options.cities_file, &dst, &drop)?;

    // Reason: Geo::load 會套用 Immich 的匯入過濾（PPLX 非 AU、PPLH），
    // 所以 rows_in 是「Immich 會載入的列數」，而檔案本身可能更多列。
    // 用寫出的列數與刪除數交叉檢查，避免過濾規則兩處不一致而靜默漏刪。
    if file_rows - drop.len() != rows_out {
        return Err(format!(
            "剪枝：列數對不上（原檔 {file_rows} − 刪除 {} ≠ 寫出 {rows_out}）",
            drop.len()
        ));
    }

    Ok(PruneReport {
        rows_in,
        rows_out,
        deleted: drop.len(),
        passes: log.len(),
        seconds: started.elapsed().as_secs_f64(),
    })
}

fn count_lines(path: &Path) -> Result<usize, String> {
    use std::io::BufRead;
    let f = std::fs::File::open(path).map_err(|e| format!("無法開啟 {}：{e}", path.display()))?;
    Ok(std::io::BufReader::new(f).lines().count())
}

fn write_kept(
    src: &Path,
    dst: &Path,
    drop: &std::collections::HashSet<i64>,
) -> Result<usize, String> {
    use std::io::{BufRead, BufWriter, Write};
    let fi = std::fs::File::open(src).map_err(|e| format!("無法開啟 {}：{e}", src.display()))?;
    // Reason: 就地覆寫時不能邊讀邊寫，先寫暫存檔再 rename。
    let tmp = dst.with_extension("prune.tmp");
    let fo = std::fs::File::create(&tmp).map_err(|e| format!("無法建立 {}：{e}", tmp.display()))?;
    let mut w = BufWriter::new(fo);
    let mut n = 0usize;
    for line in std::io::BufReader::new(fi).lines() {
        let line = line.map_err(|e| format!("讀取 {} 失敗：{e}", src.display()))?;
        let gid: i64 = line
            .split('\t')
            .next()
            .and_then(|s| s.parse().ok())
            .ok_or_else(|| {
                // Reason: 以字元而非位元組截斷——cities500 的地名是 UTF-8，
                // 位元組切片切在字元中間會讓錯誤處理自己 panic。
                let head: String = line.chars().take(60).collect();
                format!("無法解析 geoname_id：{head}")
            })?;
        if !drop.contains(&gid) {
            writeln!(w, "{line}").map_err(|e| format!("寫入失敗：{e}"))?;
            n += 1;
        }
    }
    w.flush().map_err(|e| format!("flush 失敗：{e}"))?;
    // Reason: 用完整路徑——參數 `drop` 遮蔽了 prelude 的 `drop` 函式。
    std::mem::drop(w);
    std::fs::rename(&tmp, dst)
        .map_err(|e| format!("無法將 {} 改名為 {}：{e}", tmp.display(), dst.display()))?;
    Ok(n)
}
