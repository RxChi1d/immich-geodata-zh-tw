# 生產替換路線圖

更新日期：2026-06-01

本文件保留 Python production path 替換為 Rust production path 的完成摘要。詳細
per-stage parity 工具與 Python golden outputs 已隨 Python retirement 退場。

## 完成狀態

| 生產路徑 | Rust replacement | 狀態 |
|---|---|---|
| CLI orchestration | `rust/src/cli.rs` | 完成 |
| 資料下載與前處理 | `rust/src/pipeline/prepare.rs`、`prepare_download.rs` | 完成 |
| Shapefile/GeoJSON extract | `rust/src/pipeline/extract.rs`、`extract/**` | 完成 |
| Handler transform/load | `transform_cities_schema.rs`、`admin1_load.rs`、`cities500_load.rs` | 完成 |
| LocationIQ metadata | `rust/src/pipeline/locationiq.rs` | 完成 |
| 翻譯 | `rust/src/pipeline/translate.rs` | 完成 |
| Release packaging | `rust/src/pipeline/pack.rs` | 完成 |
| Release / nightly workflow | `.github/workflows/release.yaml`、`auto-update.yaml` | 完成 |
| Rust binary cache/build | `.github/workflows/build-rust-binary.yaml` | 完成 |

## PR 前 Gate

1. Rust fmt/clippy/test 通過。
2. Rust fixture release smoke 通過。
3. 真實資料 release gate 通過，至少覆蓋目前 production country set。
4. 搜尋確認沒有 Python production 指令、Python package metadata 或 Python workflow
   dependency 殘留。
5. 推送分支後 GitHub Actions 在最新 HEAD 通過。

## 版本邊界

此 migration PR 是 production implementation 的切換點。若需要回到 Python 版本，
應使用 Git 歷史回到 migration PR 前的 commit，而不是在 Rust-only branch 內維護
雙軌 production path。
