# 本地資料處理（開發者）

本文說明如何在本機重現本專案的資料處理流程：從官方圖資提取行政區資料、產生 release，以及在不呼叫外部 API 的情況下驗證流程。一般使用者安裝資料不需要這些步驟。

## 1. 安裝依賴

本機資料處理使用 Rust CLI。請先安裝 Rust toolchain，並確認系統可使用 `pkg-config` 與 PROJ development library（例如 Ubuntu 的 `libproj-dev`）。

### 官方預編譯 binary

GitHub Releases 目前只提供 Linux x86_64 的預編譯 binary，主要供 GitHub Actions、Linux server 與 Immich container 類環境使用。macOS 與 Windows 請使用本地編譯。

### 本地編譯

```bash
cargo build --release
```

編譯完成後，binary 位於 `target/release/immich-geodata`。也可以直接用 `cargo run` 執行：

```bash
cargo run --release -- help
```

## 2. 提取原始地理資料

`extract` 命令從 Shapefile 或 GeoJSON 提取資料，產生標準化 CSV。此步驟是選用的，僅在需要更新資料來源或處理新國家時執行。

### 臺灣

資料來源：[國土測繪中心（NLSC）](https://whgis-nlsc.moi.gov.tw/Opendata/Files.aspx)

```bash
# 1. 下載「村(里)界（TWD97經緯度）」資料並解壓縮
# 2. 執行提取命令
cargo run --release -- extract --country TW \
  --shapefile geoname_data/VILLAGE_NLSC_1140825/VILLAGE_NLSC_1140825.shp \
  --output meta_data/tw_geodata.csv
```

### 日本

資料來源：[国土数値情報](https://nlftp.mlit.go.jp/ksj/gml/datalist/KsjTmplt-N03-2025.html)

```bash
# 1. 下載「行政区域データ（世界測地系）」並解壓縮
# 2. 執行提取命令
cargo run --release -- extract --country JP \
  --shapefile geoname_data/N03-20250101_GML/N03-20250101.shp \
  --output meta_data/jp_geodata.csv
```

### 南韓

資料來源：[admdongkor](https://github.com/vuski/admdongkor)

```bash
# 1. 從 admdongkor 專案下載官方行政區邊界資料並解壓縮
# 2. 執行提取命令
cargo run --release -- extract --country KR \
  --shapefile geoname_data/HangJeongDong_verYYYYMMDD.geojson \
  --output meta_data/kr_geodata.csv
```

### 泰國

資料來源：[Thailand COD-AB](https://data.humdata.org/dataset/cod-ab-tha)

```bash
# 1. 下載 tha_admin_boundaries.shp.zip 並解壓縮
# 2. 使用 tha_admin3.shp 提取 Admin 3 / Tambon 邊界資料
cargo run --release -- extract --country TH \
  --shapefile geoname_data/tha_admin_boundaries/tha_admin3.shp \
  --output meta_data/th_geodata.csv
```

泰國提取會讀取或建立 `geoname_data/TH_wikidata_cache.json`，用於 Admin1/Admin2 繁中翻譯；Admin3 保留 COD-AB 官方英文。

### 印尼

資料來源：BIG（Badan Informasi Geospasial）官方 ArcGIS REST FeatureServer

```bash
# 1. 從 BIG 官方 REST 服務以分頁方式下載 desa 村級圖資（geometryPrecision=6，版本 TASWIL20230928）
#    下載方式與固定參數請參閱 docs/research/indonesia-handler.md
# 2. 執行提取命令
cargo run --release -- extract --country ID \
  --shapefile <path_to_BIG_desa_geojson> \
  --output meta_data/id_geodata.csv
```

印尼提取會讀取或建立 `geoname_data/ID_wikidata_cache.json`，用於 Admin1（省）與 Admin2（縣市）繁中翻譯；Admin3（郡）與 Admin4（村）保留 BIG 官方印尼文。詳細下載流程與固定參數請參閱[印尼 handler 研究文件](../research/indonesia-handler.md)。

提取完成後，執行 `release` 時會自動整合這些資料。

## 3. 完整資料處理流程

### 註冊 LocationIQ API

至 [LocationIQ](https://locationiq.com/) 註冊帳號並取得 API key。

### 執行資料處理

```bash
cargo run --release -- release \
  --locationiq-api-key "YOUR_API_KEY" \
  --country-code "US"
```

> [!NOTE]
> - `cargo run -- help` 只列出基本用法；完整選項請見 `src/cli.rs` 的 `parse_production_options`。
> - `--country-code` 可指定多個國家代碼，以空格分隔。
> - 臺灣、日本、南韓、泰國、印尼（TW/JP/KR/TH/ID）已改由官方圖資 handler 產生，不應以 LocationIQ 處理；此流程僅用於為其他國家產生 metadata。

> [!WARNING]
> LocationIQ API 有請求次數限制（可登入後於後台查看），請留意要處理的國家的地名數量。
>
> 查詢進度記錄在 `meta_data/<國碼>.csv`，超過當日限制時更換 API key 或隔日重跑同一條指令即可續查，已查過的座標會自動跳過。加上 `--pass-cleanup` 可保留 `output/` 既有的中間產物，省去重新下載與前處理：
>
> ```bash
> cargo run --release -- release --locationiq-api-key "YOUR_API_KEY" --country-code "US" --pass-cleanup
> ```
>
> API key 也可以用 `LOCATIONIQ_API_KEY` 環境變數提供。

## 4. 驗證

Rust CLI 提供 dry-run contract，可在不呼叫外部 API、不下載網路資料的情況下驗證 release orchestration：

```bash
cargo run -- release \
  --dry-run \
  --locationiq-api-key "fixture" \
  --country-code "KR" "TH" \
  --batch-size 100 \
  --locationiq-qps 2
```

需要驗證 release archive 與 `update_data.sh` 所需的目錄結構時，可使用 fixture mode 產生本地 smoke artifact：

```bash
cargo run -- release \
  --fixture-mode \
  --pass-locationiq \
  --output-folder /tmp/rust-release-smoke
```

release 與 nightly production workflow 皆使用 Rust production path，並保留 fixture release smoke 作為前置檢查。真實 GeoNames / Natural Earth 下載與 LocationIQ quota path 的自動測試仍需使用 fixture、stub 或明確的 dry-run gate。

## 程式碼檢查

```bash
cargo fmt --check
cargo clippy -- -D warnings
cargo test
```
