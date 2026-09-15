# 本地資料處理（開發者）

本文說明如何在本機重現本專案的資料處理流程：從官方圖資提取行政區資料、產生 release，以及在不呼叫外部 API 的情況下驗證流程。一般使用者安裝資料不需要這些步驟。

## 1. 安裝依賴

本機資料處理使用 Rust CLI。請先安裝 Rust toolchain，並確認系統可使用 `pkg-config` 與 PROJ development library（例如 Ubuntu 的 `libproj-dev`）。

下載各國圖資的步驟另外會用到 `curl`、`jq` 與 `unzip`。

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

以下指令中的 `<版本>` 請替換為實際下載到的檔名版本；目前發布資料採用的來源版本記於 [NOTICE.md](../../NOTICE.md)。

### 臺灣

資料來源：[國土測繪中心（NLSC）](https://whgis-nlsc.moi.gov.tw/Opendata/Files.aspx)

```bash
# 1. 下載「村(里)界（TWD97經緯度）」資料並解壓縮
# 2. 執行提取命令
cargo run --release -- extract --country TW \
  --shapefile geoname_data/VILLAGE_NLSC_<版本>/VILLAGE_NLSC_<版本>.shp \
  --output data/handler/tw_geodata.csv
```

> [!NOTE]
> NLSC 下載頁以 ASP.NET postback 觸發下載，無法用 `curl` 直接取得檔案，需在瀏覽器
> 操作。下載到的檔名為 `OFiles_<guid>.zip`，解壓縮後才會出現 `VILLAGE_NLSC_<版本>`
> 目錄。版本號為民國日期，例如 `1150624` 代表民國 115 年 6 月 24 日。

### 日本

資料來源：[国土数値情報](https://nlftp.mlit.go.jp/ksj/gml/datalist/KsjTmplt-N03-2025.html)

```bash
# 1. 下載「行政区域データ（世界測地系）」並解壓縮
# 2. 執行提取命令
cargo run --release -- extract --country JP \
  --shapefile geoname_data/N03-<版本>_GML/N03-<版本>.shp \
  --output data/handler/jp_geodata.csv
```

> [!NOTE]
> 下載頁的連結需逐頁點選，也可以直接取用固定網址（`<年份>` 與 `<版本>` 屬於同一次
> 發布，例如 `2026` 與 `20260101`）：
>
> ```
> https://nlftp.mlit.go.jp/ksj/gml/data/N03/N03-<年份>/N03-<版本>_GML.zip
> ```

### 南韓

資料來源：[admdongkor](https://github.com/vuski/admdongkor)

```bash
# 1. 下載該版本的 GeoJSON（單一檔案，不需解壓縮）
VER=20260701   # 改成要下載的版本
curl -sSL --create-dirs -o "geoname_data/HangJeongDong_ver${VER}.geojson" \
  "https://raw.githubusercontent.com/vuski/admdongkor/master/ver${VER}/HangJeongDong_ver${VER}.geojson"

# 2. 執行提取命令
cargo run --release -- extract --country KR \
  --shapefile "geoname_data/HangJeongDong_ver${VER}.geojson" \
  --output data/handler/kr_geodata.csv
```

> [!NOTE]
> 版本號即該版生效日期（例如 `20260701`）。可用版本以 admdongkor 儲存庫根目錄下的
> `ver*` 目錄為準，每個目錄各含一個同名 GeoJSON。

### 泰國

資料來源：[Thailand COD-AB](https://data.humdata.org/dataset/cod-ab-tha)

```bash
# 1. 向 HDX API 取得目前的 shapefile 下載網址
URL=$(curl -sS "https://data.humdata.org/api/3/action/package_show?id=cod-ab-tha" \
  | jq -r '.result.resources[] | select(.name == "tha_admin_boundaries.shp.zip") | .url')

# 2. 下載並解壓縮（HDX 會 302 轉址到預簽網址，-L 不可省略）
mkdir -p geoname_data
curl -sSL -o geoname_data/tha_admin_boundaries.shp.zip "$URL"
unzip -q -d geoname_data/tha_admin_boundaries geoname_data/tha_admin_boundaries.shp.zip

# 3. 使用 tha_admin3.shp 提取 Admin 3 / Tambon 邊界資料
cargo run --release -- extract --country TH \
  --shapefile geoname_data/tha_admin_boundaries/tha_admin3.shp \
  --output data/handler/th_geodata.csv
```

> [!NOTE]
> HDX 的下載網址含資源 UUID，改版時會變動，因此請透過上述 API 取得當前網址，不要
> 沿用文件或既有腳本中的固定連結。同一個 API 回應中的 `last_modified` 即該版的發布
> 日期，可用來判斷上游是否已改版。

泰國提取會讀取或建立 `geoname_data/TH_wikidata_cache.json`，用於 Admin1/Admin2 繁中翻譯；Admin3 保留 COD-AB 官方英文。

### 印尼

資料來源：[BIG（Badan Informasi Geospasial）圖資服務](https://geoservices.big.go.id/rbi/rest/services/BATASWILAYAH/BATAS_DESAKEL_AR/MapServer/0)

BIG 沒有提供 desa（村級）圖資的單檔下載，需透過 ArcGIS REST 的 `query` 端點分批取得後合併。

```bash
L="https://geoservices.big.go.id/rbi/rest/services/BATASWILAYAH/BATAS_DESAKEL_AR/MapServer/0"

# 1. 確認 feature 總數、OBJECTID 上限與資料版本
curl -sS -G "$L/query" --data-urlencode "where=1=1" \
  --data-urlencode "returnCountOnly=true" --data-urlencode "f=json"
curl -sS -G "$L/query" --data-urlencode "where=1=1" --data-urlencode "f=json" \
  --data-urlencode 'outStatistics=[{"statisticType":"max","onStatisticField":"OBJECTID","outStatisticFieldName":"m"}]'
curl -sS -G "$L/query" --data-urlencode "where=OBJECTID=1" \
  --data-urlencode "outFields=METADATA" --data-urlencode "returnGeometry=false" \
  --data-urlencode "f=json"

# 2. 以 OBJECTID 區間分批下載（上限請依步驟 1 的結果調整）
mkdir -p geoname_data/idn_oid
for ((lo=0; lo<93730; lo+=1000)); do
  hi=$((lo+1000))
  f="geoname_data/idn_oid/oid_$(printf '%06d' $lo).geojson"
  # 驗收條件是「能完整解析」，不是「開頭是 {」——截斷的檔案開頭也是 {
  [ -s "$f" ] && python3 -c "import json,sys;json.load(open(sys.argv[1]))['features']" "$f" 2>/dev/null && continue
  curl -sS --max-time 300 -G "$L/query" \
    --data-urlencode "where=OBJECTID>$lo AND OBJECTID<=$hi" \
    --data-urlencode "outFields=WADMPR,WADMKK,WADMKC,WADMKD" \
    --data-urlencode "geometryPrecision=6" \
    --data-urlencode "outSR=4326" \
    --data-urlencode "f=geojson" -o "$f"
done

# 3. 合併為單一 GeoJSON，並核對 feature 數與省份數
# 逐檔解析、逐檔寫出，不把全部 features 讀進記憶體
# Reason: 每個 feature 解析後約 167 KB，84,503 筆需約 13 GB，序列化時峰值再翻倍
python3 - <<'EOF'
import glob, json, os, sys
OUT = 'geoname_data/idn_desa_<版本>.geojson'
count, provinces, bad = 0, set(), []
with open(OUT, 'w', encoding='utf-8') as out:
    out.write('{"type":"FeatureCollection","features":[')
    first = True
    for path in sorted(glob.glob('geoname_data/idn_oid/*.geojson')):
        try:
            features = json.load(open(path, encoding='utf-8'))['features']
        except (json.JSONDecodeError, KeyError):
            bad.append(path)
            continue
        for feature in features:
            if not first:
                out.write(',')
            first = False
            out.write(json.dumps(feature, ensure_ascii=False, separators=(',', ':')))
            count += 1
            provinces.add(feature['properties']['WADMPR'])
    out.write(']}')
print('feature 數:', count, '| 省份數:', len(provinces), '| 無法解析:', len(bad))
if bad or count != 84503:          # 總數請換成步驟 1 回報的值
    os.remove(OUT)
    sys.exit('核對不通過，已刪除輸出檔——重抓失敗批次後再合併')
EOF

# 4. 執行提取命令
cargo run --release -- extract --country ID \
  --shapefile geoname_data/idn_desa_<版本>.geojson \
  --output data/handler/id_geodata.csv
```

> [!IMPORTANT]
> 這一步有兩種**靜默**失敗，兩種都必須擋：
>
> 1. **HTML 錯誤頁**：服務端失敗時回傳 HTTP 200 與一段 HTML，而不是 HTTP 錯誤碼。
> 2. **截斷**：連線中途斷掉的檔案是合法的 JSON 前綴，**開頭也是 `{`**。2026-09-15
>    實測 94 批中有 13 個 5–17 MB 的大檔屬此類，全部通過了「開頭是 `{`」的檢查，
>    直到合併階段才被發現（少 10,750 個 feature、缺一個省）。
>
> 因此驗收條件是「**能被 `json.load` 完整解析且含 `features`**」，不是檔案開頭。
> 合併前仍要核對 feature 數與省份數與步驟 1 回報的一致。單批 1000 筆持續失敗的
> 區間改以 250 筆為單位重抓；仍失敗者再切半。
>
> OBJECTID 並非連續，上限（本次為 93730）大於 feature 總數（84503）屬正常現象。

資料版本記錄在圖徵的 `METADATA` 屬性（例如 `TASWIL1000020260612DESAKEL_AR`，代表 2026-06-12 版），不需另外從服務目錄查詢。

印尼提取會讀取或建立 `geoname_data/ID_wikidata_cache.json`，用於 Admin1（省）與 Admin2（縣市）繁中翻譯；Admin3（郡）與 Admin4（村）保留 BIG 官方印尼文。

> [!IMPORTANT]
> Wikidata 譯名的失敗是無聲的：查不到或驗證不過時 handler 會安靜地退回原文，不會報錯。
> 重跑後必須比對未翻譯地名的**完整清單**而非數量。失效形態、現行防線的適用範圍與快取
> 的清除方式，見 [Wikidata 譯名的已知失效形態](wikidata-translation.md)。

提取完成後，執行 `release` 時會自動整合這些資料。

## 3. 完整資料處理流程

### 註冊 LocationIQ API

至 [LocationIQ](https://locationiq.com/) 註冊帳號並取得 API key。

> [!NOTE]
> 只有要為 handler 以外的國家產生 metadata 時才需要 API key。若 `--country-code`
> 未指定任何非 handler 國家（例如只重新打包 TW/JP/KR/TH/ID 的官方圖資），
> LocationIQ 階段會自動跳過，`release` 不會要求 API key。

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
> LocationIQ API 有請求次數限制（可登入後於後台查看），請留意要處理的國家的地名數量。免費方案同時有 **2 req/s、60 req/min、5,000 req/day** 三條限制，前兩條互相矛盾，因此節流以較嚴的 60 req/min 為準：`--locationiq-qps` 預設為 1（間隔 1020 ms ≈ 58.8 req/min），付費方案才需調高。
>
> 當日額度用完時 CLI 會保留已查結果後**中止**，避免拿半套資料繼續往下發布。CI 的增量補查改用 `--locationiq-allow-partial`，讓額度用完算正常結束，已查結果照常提交、剩餘座標留待下次；本地跑滿一個國家時不要加這個旗標。
>
> 查詢進度記錄在 `data/locationiq/<國碼>.csv`（可用 `--locationiq-folder` 指定其他位置；舊版記錄在 `meta_data/<國碼>.csv`，將既有檔案移至新目錄即可續用），超過當日限制時更換 API key 或隔日重跑同一條指令即可續查，已查過的座標會自動跳過。加上 `--pass-cleanup` 可保留 `output/` 既有的中間產物，省去重新下載與前處理：
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
  --locationiq-qps 1
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
