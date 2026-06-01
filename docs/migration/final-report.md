# Rust Migration Final Report

更新日期：2026-06-01

## 結論

本專案 production geodata pipeline 已由 Python 完整遷移至 Rust。release、
nightly、extract、prepare、locationiq、translate 與 pack 均以 `rust/` 下的 CLI
與 pipeline 實作為準。

## 已完成範圍

| 範圍 | 狀態 |
|---|---|
| CLI orchestration | Rust CLI 已覆蓋 production release flow。 |
| TW/JP/KR extract | Rust 支援 Shapefile、GeoJSON、CRS 轉換、centroid 與各國 handler 規則。 |
| GeoNames transform/load | Rust 已支援 admin1/cities500 replacement、extra data merge 與 handler country replacement。 |
| LocationIQ metadata | Rust 支援 API key、QPS、續跑、batch flush 與錯誤中止語意。 |
| Translate | Rust 已使用 native OpenCC 與 alternate names / metadata priority 流程。 |
| Pack | Rust 產生 `release.zip`、`release.tar.gz` 與 Immich 需要的 release tree。 |
| Workflow | release、auto-update 與 validation workflow 已切換到 Rust binary。 |
| Python retirement | Python production code、parity tools、pytest tests、lock/config 與 golden outputs 已退場。 |

## 驗證要求

PR 前需以目前 Rust-only worktree 執行：

```bash
cargo fmt --manifest-path rust/Cargo.toml --check
cargo clippy --manifest-path rust/Cargo.toml -- -D warnings
cargo test --manifest-path rust/Cargo.toml
cargo run --release --manifest-path rust/Cargo.toml -- release \
  --fixture-mode \
  --pass-locationiq \
  --output-folder /tmp/rust-release-smoke
```

若要驗證真實資料 release，使用本機 `geoname_data/` 與 `meta_data/` 執行 Rust
production release，並檢查 release tree、archive、row counts 與 checksums。

## 剩餘風險

- Rust binary runtime 仍可能依編譯方式需要系統動態函式庫；workflow 會輸出 `ldd`
  manifest 以便追蹤。
- LocationIQ 與上游資料來源屬外部服務；CI gate 應使用 fixture/stub，真實資料更新
  由人工或排程流程驗證。
- OpenCC crate 升級可能造成少量字形 variant 差異；需以使用者可接受性與資料品質評估。
