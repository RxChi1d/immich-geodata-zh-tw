//! Voronoi 點剪枝：刪除「刪了也不改變任何 Immich 反向地理編碼答案」的點。
//!
//! 完整規格、證明與辯論紀錄見主 worktree 的 `notes/plan-voronoi-pruning.md`
//! 與 `notes/debate-staging-2026-09-11/`（兩者皆 gitignore）。

pub mod cells;
pub mod delaunay;
pub mod geodata;
pub mod multipass;
pub mod prove;
pub mod stage;
