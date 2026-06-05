# Python 到 Rust 遷移紀錄

更新日期：2026-06-05
狀態：已完成（本文件為歷史紀錄，非操作指南）

## 遷移背景

本專案 production geodata pipeline 原以 Python 實作。為提升處理效能與發布
可重現性，並維持單一 production path，已完整遷移至 Rust。release、nightly、
extract、prepare、locationiq、translate 與 pack 均以根目錄 crate（binary：
`immich-geodata`）的 CLI 與 pipeline 實作為準。

## 完成範圍

| 生產路徑 | Rust replacement | 說明 |
|---|---|---|
| CLI orchestration | `src/cli.rs` | 覆蓋 production release flow。 |
| 資料下載與前處理 | `src/pipeline/prepare.rs`、`prepare_download.rs` | GeoNames、Natural Earth 等來源下載。 |
| Shapefile/GeoJSON extract | `src/pipeline/extract.rs`、`extract/**` | 支援 CRS 轉換、centroid 與各國 handler 規則。 |
| Handler transform/load | `transform_cities_schema.rs`、`admin1_load.rs`、`cities500_load.rs` | admin1/cities500 replacement、extra data merge。 |
| LocationIQ metadata | `src/pipeline/locationiq.rs` | API key、QPS、續跑、batch flush 與錯誤中止語意。 |
| 翻譯 | `src/pipeline/translate.rs` | native OpenCC 與 alternate names / metadata priority 流程。 |
| Release packaging | `src/pipeline/pack.rs` | 產生 `release.zip`、`release.tar.gz` 與 Immich 需要的 release tree。 |
| Release / nightly workflow | `.github/workflows/release.yaml`、`auto-update.yaml` | 已切換到 Rust binary。 |
| Rust binary cache/build | `.github/workflows/build-rust-binary.yaml` | source hash 快取與 artifact 驗證。 |
| Python retirement | — | Python production code、parity tools、pytest tests、lock/config 與 golden outputs 已退場。 |

## 驗證方式

遷移期間與後續變更皆以下列 gates 驗證：

```bash
cargo fmt --check
cargo clippy -- -D warnings
cargo test
cargo run --release -- release \
  --fixture-mode \
  --pass-locationiq \
  --output-folder /tmp/rust-release-smoke
```

若要驗證真實資料 release，使用本機 `geoname_data/` 與 `meta_data/` 執行 Rust
production release，並檢查 release tree、archive、row counts 與 checksums。

## 版本邊界

migration PR 是 production implementation 的切換點。若需要回到 Python 版本，
應使用 Git 歷史回到 migration PR 前的 commit，而不是在 Rust-only branch 內維護
雙軌 production path。

## 剩餘風險（成文時紀錄）

- Rust binary runtime 仍可能依編譯方式需要系統動態函式庫；workflow 會輸出 `ldd`
  manifest 以便追蹤。
- LocationIQ 與上游資料來源屬外部服務；CI gate 應使用 fixture/stub，真實資料更新
  由人工或排程流程驗證。
- OpenCC crate 升級可能造成少量字形 variant 差異；需以使用者可接受性與資料品質評估。
