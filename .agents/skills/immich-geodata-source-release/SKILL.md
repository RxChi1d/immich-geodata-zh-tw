---
name: immich-geodata-source-release
description: 當需要更新 immich-geodata-zh-tw 原始邊界資料、下載最新 TW/JP/KR 圖資、驗證 Shapefile/GeoJSON schema、執行 Rust extract/release pipeline，或驗證 release artifacts 時使用。
version: 1.0.0
license: MIT
---

# immich-geodata-zh-tw 圖資更新與發布

此 project skill 用於本儲存庫的圖資來源更新與 release artifact 重新產生。Rust CLI 是唯一 production path；所有 extract、release 與驗證流程都應以 `rust/` 下的實作為準。

## 事實來源

- 遵循 `AGENTS.md` 的語言、安全、測試與 commit 規則。
- 使用 `README.md` 開發者章節確認目前 Rust 指令與 runtime prerequisites。
- 使用 `docs/zh-tw/taiwan-admin-processing.md`、`docs/zh-tw/japan-admin-processing.md`、`docs/zh-tw/south-korea-admin-processing.md` 確認各國欄位語意。
- 需要對齊 production release 行為時，檢查 `.github/workflows/`。

## Runtime 流程

1. 先檢查 worktree；dirty state 下避免破壞性清理：

```bash
git status --short --branch
```

2. 若需要更新 upstream TW/JP/KR 原始圖資，從官方來源下載完整資料：
   - TW：NLSC 村(里)界（TWD97 經緯度）Shapefile。
   - JP：MLIT N03 全國行政區 Shapefile。
   - KR：admdongkor `HangJeongDong_ver*.geojson`。
3. extract 前先驗證新來源檔 schema。必要輸入是完整 TW NLSC 村里界 Shapefile、完整 JP MLIT N03 全國 Shapefile、完整 KR `HangJeongDong_ver*.geojson`。
4. 使用 Rust CLI 執行 extract，例如：

```bash
cargo run --release --manifest-path rust/Cargo.toml -- extract --country TW \
  --shapefile geoname_data/VILLAGE_NLSC_XXXXXX/VILLAGE_NLSC_XXXXXX.shp \
  --output meta_data/tw_geodata.csv
```

5. 使用 Rust CLI 產生 release artifacts。若只更新 handler countries（TW/JP/KR），跳過 LocationIQ：

```bash
cargo run --release --manifest-path rust/Cargo.toml -- release \
  --pass-locationiq \
  --overwrite
```

6. 若處理 non-handler countries，使用 `LOCATIONIQ_API_KEY` 或 `--locationiq-api-key`，所有紀錄中都要遮蔽 key；只有在續跑中斷工作時才使用 `--pass-cleanup`。
7. 驗證時回報相關 row counts、checksums、`cargo test --manifest-path rust/Cargo.toml`，以及必要的 Rust release smoke 或 production validation 指令。

## Guardrails

- 不要把外部 skill 文字複製成第二份操作手冊；此 skill 應保持精簡，細節以專案文件為準。
- 不要使用 sample 或衍生來源檔產生 release。
- source data 有變更時，不要在 extract 前先跑 release；release 會消耗 `meta_data/*_geodata.csv`。
- 不要外洩 API keys；使用 env vars，並在所有輸出中以 `[REDACTED]` 遮蔽。
- 新 KR 版本 extract 可能花時間處理 Wikidata cache；只因為慢而中斷前，先確認是否仍有進度。
