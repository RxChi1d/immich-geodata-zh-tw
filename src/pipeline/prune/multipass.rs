//! 多趟獨立集刪除。
//!
//! 單趟只能刪掉 Delaunay 圖的一個**獨立集**（平面三角化約 25%），所以必須重複：
//! 每趟重算當下保留集合的 Delaunay，重新證明，再選一個獨立集。
//!
//! Reason: T0 的論證需要 p 的 Delaunay 鄰居存活。若 p 與鄰居同時被刪，
//! 論證的歸納在「縮減後的點集其 Delaunay 會新增邊」這一步斷掉。這條線已因類似的
//! 推理漏洞翻車兩次，故維持獨立集約束。
//!
//! passes 2+ 只重新證明「上一趟已證出但被選取規則跳過」的候選。
//! Reason: 刪點只會讓保留集合變小，T1 的覆蓋與 T2 的見證都更難成立，所以上一趟
//! 證不出來的候選這一趟幾乎不可能變成證得出來。跳過它們是保守的（少刪，不會錯刪）。

use super::delaunay::Delaunay;
use super::geodata::Geo;
use super::prove::{Neighbors, prove_one};

pub struct PassLog {
    pub pass: usize,
    pub candidates: usize,
    pub proved: usize,
    pub deleted: usize,
    pub cumulative: usize,
    pub seconds: f64,
}

pub struct Config {
    pub budget: u32,
    pub max_pass: usize,
    /// 低於此刪除量即視為收斂。
    pub min_delete: usize,
    /// 證明階段使用的執行緒數。**不改變輸出**——選取階段仍為循序固定順序。
    ///
    /// - `0`：預設，留四分之一的核心給系統
    /// - `-1`：吃滿全部核心
    /// - `N > 0`：指定 N 條
    ///
    /// Reason: 用數值而非布林旗標。行為是「要用多少核心」，不是「是不是在 CI」——
    /// 同一台桌機也可能想吃滿，而 CI 也可能想限制。沿用 scikit-learn `n_jobs`
    /// 的 `-1 = 全部` 慣例，差別是預設會留餘裕，因為這個工具會在使用者桌機上跑。
    pub threads: i32,
}

/// 把 [`Config::threads`] 解析成實際的執行緒數。
///
/// `available_parallelism` 會讀 cgroup 配額，所以在容器裡拿到的是容器的限制而非
/// 宿主機核心數，不需另外處理。
pub fn resolve_threads(threads: i32) -> usize {
    let n = std::thread::available_parallelism()
        .map(|v| v.get())
        .unwrap_or(1);
    match threads {
        0 => (n - n / 4).max(1),
        t if t < 0 => n,
        t => (t as usize).min(n * 4),
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            // Reason: 預算 = 證明單一候選時最多可切出的 cell 數。ablation（跑到收斂、
            // 全量資料）顯示是平滑的報酬遞減，沒有明顯膝點：每檔加倍，多刪的點數
            // 約砍半而時間加倍，邊際成本每檔漲約 2.5 倍。
            //
            //   預算   刪除量   佔比     時間    每多刪 1000 點的秒數
            //    256  139,661  28.66%   133 s      —
            //    512  148,913  30.55%   189 s     6.0
            //   1024  154,292  31.66%   271 s    15.3
            //   2048  158,322  32.48%   419 s    36.8   ← 採用
            //   4096  160,651  32.96%   677 s   110.8
            //
            // 取 2048：比 512 多刪 9,409 點只多花 3.8 分鐘；再往上到 4096 要多付
            // 4.3 分鐘卻只換 2,329 點。時間離 CI 的 6 小時上限還很遠，不是限制因素。
            budget: 2048,
            // Reason: 跑到收斂而非固定趟數。預算 512 跑 10 趟是 135,893，
            // 跑到收斂是 148,913（+9.6%），成本只有 17 秒。上限純粹是防呆。
            max_pass: 30,
            min_delete: 500,
            threads: 0,
        }
    }
}

/// 回傳 `(deleted 遮罩, 每趟紀錄)`。
pub fn run(
    g: &Geo,
    cfg: &Config,
    on_pass: impl FnMut(&PassLog),
) -> Result<(Vec<bool>, Vec<PassLog>), String> {
    // Reason: 傳 None 而非一個立刻丟掉的 Vec。production 的第一趟候選是數十萬筆，
    // 每筆配一個 String 只為了馬上釋放，等於白配上百 MB。
    run_with_dump(g, cfg, None, on_pass)
}

/// 同 [`run`]，但把每趟逐候選的證明結果寫進 `dump`，供與原型逐項對照。
///
/// 格式：`pass<TAB>geoname_id<TAB>ok<TAB>cells<TAB>depth`
pub fn run_with_dump(
    g: &Geo,
    cfg: &Config,
    mut dump: Option<&mut Vec<String>>,
    mut on_pass: impl FnMut(&PassLog),
) -> Result<(Vec<bool>, Vec<PassLog>), String> {
    let n = g.n();
    let mut kept = vec![true; n];
    let mut deleted = vec![false; n];
    // passes 2+ 的候選池（全域列索引）。`None` 表示第一趟。
    let mut alive: Option<Vec<u32>> = None;
    let mut log = Vec::new();

    for pass in 1..=cfg.max_pass {
        let t0 = std::time::Instant::now();

        // 每趟以當下保留集合重建圖。
        let sub: Vec<u32> = (0..n as u32).filter(|&i| kept[i as usize]).collect();
        let gg = g.subset(&sub);
        // Reason: 退化點（例如兩列座標完全相同）會讓 Qhull 少回傳一個凸包頂點，
        // Delaunay::build 因此回報錯誤。這條路徑會在每週自動更新的 release 中跑，
        // 必須以 Err 往上傳給 pipeline，不能 panic 成一串 backtrace。
        let d = Delaunay::build(&gg).map_err(|e| format!("第 {pass} 趟 Delaunay 建構失敗：{e}"))?;
        let (has_same, all_same) = d.neighbor_flags(&gg);
        let local_rows: Vec<u32> = (0..gg.n() as u32).collect();
        let nb = Neighbors::build(&gg.xyz, &local_rows);

        // 候選（區域索引），按 geoname_id 穩定排序。
        // Reason: 走訪順序決定貪婪獨立集選誰，必須與原型一致才能逐項對照。
        let mut cand: Vec<u32> = match &alive {
            None => (0..gg.n() as u32)
                .filter(|&i| has_same[i as usize])
                .collect(),
            Some(prev) => {
                let mut back = vec![u32::MAX; n];
                for (local, &global) in sub.iter().enumerate() {
                    back[global as usize] = local as u32;
                }
                prev.iter()
                    .filter_map(|&gidx| {
                        let l = back[gidx as usize];
                        (l != u32::MAX && has_same[l as usize]).then_some(l)
                    })
                    .collect()
            }
        };
        cand.sort_by_key(|&i| (gg.gid[i as usize], i));
        if cand.is_empty() {
            break;
        }

        // 證明階段。**逐候選完全獨立**：只讀 kept_local／k 近鄰索引／座標，
        // 不互看、只寫自己的結果，所以可以直接平行。
        //
        // Reason: 用 indexed collect 而非依完成順序 push——依完成順序會讓結果的
        // 排列取決於執行緒排程，輸出就不可重現了。後面的獨立集選取階段必須
        // **循序且固定順序**，那一段不可平行。
        let kept_local = vec![true; gg.n()];
        let prove_one_candidate =
            |c: u32| prove_one(&gg, &nb, c, all_same[c as usize], &kept_local, cfg.budget);

        let n_threads = resolve_threads(cfg.threads);
        let results: Vec<_> = if n_threads > 1 {
            use rayon::prelude::*;
            // Reason: 逐候選 par_iter 而非自己切 chunk。成本中位數約 21 個 cell，
            // 但尾巴會撞到預算上限，差三個數量級——靜態分塊會讓多數執行緒閒置
            // 等尾巴，要靠 rayon 的 work-stealing。
            // Reason: 用自建 pool 而非全域 pool——全域 pool 只能設定一次，
            // 同一個行程若跑多趟或被當成函式庫呼叫，第二次設定會靜默失敗。
            rayon::ThreadPoolBuilder::new()
                .num_threads(n_threads)
                .build()
                .expect("無法建立 rayon thread pool")
                .install(|| cand.par_iter().map(|&c| prove_one_candidate(c)).collect())
        } else {
            cand.iter().map(|&c| prove_one_candidate(c)).collect()
        };

        if let Some(dump) = dump.as_deref_mut() {
            for (&c, r) in cand.iter().zip(&results) {
                dump.push(format!(
                    "{}\t{}\t{}\t{}\t{}",
                    pass, gg.gid[c as usize], r.ok as u8, r.cells_used, r.max_depth
                ));
            }
        }

        // 獨立集選取。必須循序且固定順序——平行化會讓「誰被刪、誰被 pin」
        // 取決於執行緒排程，輸出不可重現。
        let mut del_local = vec![false; gg.n()];
        let mut pin = vec![false; gg.n()];
        let mut skipped: Vec<u32> = Vec::new();
        let mut proved = 0usize;
        let mut n_del = 0usize;
        for (&c, r) in cand.iter().zip(&results) {
            if !r.ok {
                continue;
            }
            proved += 1;
            let blocked = pin[c as usize]
                || d.neighbors(c as usize)
                    .iter()
                    .any(|&x| del_local[x as usize])
                || r.witnesses.iter().any(|&w| del_local[w as usize]);
            if blocked {
                skipped.push(sub[c as usize]);
                continue;
            }
            del_local[c as usize] = true;
            n_del += 1;
            for &w in &r.witnesses {
                pin[w as usize] = true;
            }
        }

        for (l, &is_del) in del_local.iter().enumerate() {
            if is_del {
                deleted[sub[l] as usize] = true;
                kept[sub[l] as usize] = false;
            }
        }
        alive = Some(skipped);
        let entry = PassLog {
            pass,
            candidates: cand.len(),
            proved,
            deleted: n_del,
            cumulative: deleted.iter().filter(|&&b| b).count(),
            seconds: t0.elapsed().as_secs_f64(),
        };
        on_pass(&entry);
        let converged = n_del < cfg.min_delete;
        log.push(entry);
        if converged {
            break;
        }
    }

    Ok((deleted, log))
}

#[cfg(test)]
mod tests {
    /// 與 `resolve_threads` 同一套規則，但核心數由參數給定，便於測試各種機器。
    fn resolve_with(threads: i32, n: usize) -> usize {
        match threads {
            0 => (n - n / 4).max(1),
            t if t < 0 => n,
            t => (t as usize).min(n * 4),
        }
    }

    #[test]
    fn default_reserves_a_quarter_of_the_cores() {
        // Reason: 這個工具會在使用者的桌機上跑，吃滿核心會讓系統失去回應。
        // 用比例而非固定減法——固定「減 2」在 2 核上會變 0，等於關掉平行化。
        for (cores, expect) in [(1, 1), (2, 2), (3, 3), (4, 3), (8, 6), (16, 12), (32, 24)] {
            assert_eq!(
                resolve_with(0, cores),
                expect,
                "{cores} 核的預設值應為 {expect}"
            );
        }
    }

    #[test]
    fn negative_one_uses_every_core() {
        for cores in [1, 2, 4, 32] {
            assert_eq!(resolve_with(-1, cores), cores, "-1 應吃滿 {cores} 核");
        }
    }

    #[test]
    fn explicit_count_is_honoured_but_bounded() {
        assert_eq!(resolve_with(8, 32), 8, "明確指定應照數字走");
        // 超額訂閱有上限，避免手滑打了 100000 就開出一堆執行緒。
        assert_eq!(resolve_with(100_000, 4), 16, "上限為核心數的四倍");
    }

    #[test]
    fn every_setting_yields_at_least_one_thread() {
        for threads in [-5, -1, 0, 1, 7] {
            for cores in [1, 2, 4, 32] {
                assert!(
                    resolve_with(threads, cores) >= 1,
                    "threads={threads} cores={cores} 不得為 0"
                );
            }
        }
    }
}
