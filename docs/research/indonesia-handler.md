# 印尼 BIG 圖資 handler 評估

> **文件狀態：歷史紀錄**
> 本文記錄當時的研究與決策，不保證與目前實作同步。現行行為請參閱[印尼行政區處理](../zh-tw/indonesia-admin-processing.md)。
> 本文彙整階段二（投影與座標）與階段三（Wikidata 語言、P31、P131、覆蓋率）的評估數據與設計選項。

## 資料來源

- 資料提供：印尼地理空間資訊局（Badan Informasi Geospasial, BIG）
- 取得方式：BIG 官方 ArcGIS REST FeatureServer
- 資料層級：desa（村級）邊界，屬性同時含省／縣市／郡／村各級行政名稱
- 下載日期：2026-06-06
- 資料版本：`TASWIL20230928`
- 下載精度：`geometryPrecision=6`（保留小數 6 位，全精度、無幾何簡化）
- 座標系：屬性 `SRS_ID = "SRGI 2013"`，視為 EPSG:4326（公尺級差異可忽略）

### 授權合規立場

BIG 圖資為印尼官方公開地理資料。本專案僅將其作為衍生加工輸入，輸出反向地理
編碼最佳化後的地名與代表座標（cities500 風格單點 metadata），不散布、不重新
發行 BIG 原始向量圖資。GADM 與 HDX（OCHA）印尼 COD-AB 為已知的可重新發行
開放圖資備案。

### BIG REST 下載可重現指令與固定參數

> [!WARNING]
> 本節記錄的是當時使用的 FeatureServer 與 `resultOffset` 分頁方式，該端點已下線，
> `resultOffset` 分頁在現行服務上也會失敗。現行的下載步驟見
> [本地資料處理](../zh-tw/development.md#印尼)。

從 BIG FeatureServer 以 `query` 端點分頁抓取 desa polygon，固定參數如下：

| 參數 | 值 | 說明 |
|---|---|---|
| `where` | `1=1` | 取全量 |
| `outFields` | `WADMPR,WADMKK,WADMKC,WADMKD`（或 `*`） | 至少需省／縣市／郡／村四級欄位 |
| `geometryPrecision` | `6` | 全精度、無簡化 |
| `outSR` | `4326` | 輸出 WGS84（SRGI 2013 視為等同） |
| `f` | `geojson` | 輸出 GeoJSON |
| `resultRecordCount` / `resultOffset` | 依服務上限分頁 | 逐頁抓取後合併 |

範例（單頁，實際需依 `resultOffset` 分頁直到無 `exceededTransferLimit`）：

```bash
curl -G "<BIG_DESA_FEATURESERVER>/query" \
  --data-urlencode "where=1=1" \
  --data-urlencode "outFields=WADMPR,WADMKK,WADMKC,WADMKD" \
  --data-urlencode "geometryPrecision=6" \
  --data-urlencode "outSR=4326" \
  --data-urlencode "f=geojson" \
  --data-urlencode "resultRecordCount=2000" \
  --data-urlencode "resultOffset=0" \
  -o idn_desa_page0.geojson
```

> 服務端點 URL 隨 BIG 服務目錄調整，使用前以官方服務目錄確認 desa 層
> FeatureServer 路徑。下載後分頁合併成完整 desa GeoJSON 再餵入 extract。

## 行政層級與欄位映射

BIG desa 屬性提供四級行政名稱：

| 輸出欄位 | 來源欄位 | 說明 |
|---|---|---|
| `latitude` / `longitude` | 幾何 centroid | Albers 投影後每 part centroid |
| `country` | 固定 `印尼` | 專案使用 zh-tw 國名 |
| `admin_1` | Wikidata / `WADMPR` | 省繁中翻譯，缺中文回退官方印尼文 |
| `admin_2` | Wikidata / `WADMKK` | 縣／市繁中翻譯，缺中文回退官方印尼文 |
| `admin_3` | `WADMKC` | 郡（Kecamatan）官方印尼文 |
| `admin_4` | `WADMKD` | 村（Desa / Kelurahan）官方印尼文 |

`WADMPR` 或 `WADMKK` 空白者為「Area tidak terdefinisi」（未定義行政區），
extract 直接過濾。

---

# 階段二實驗全文：投影法與座標策略決策

本段記錄印尼（IDN）handler 在新增前的階段二正式實驗結果，用以決定 extract
pipeline 的「投影法」與「代表座標策略」。所有實驗為純本地計算，未呼叫任何
Wikidata / 網路 API。

## 摘要結論

| 決策項目 | 結論 | 主要判準數據 |
|----------|------|--------------|
| 投影法 | **Albers 單一等積投影** | Albers vs dynamic UTM centroid 差異：中位 0.011 m、平均 0.185 m、p99 3.79 m、最大 32.98 m（皆遠小於行政區粒度，且最大值僅出現在極端跨經度散群島樣本） |
| 座標策略 | **等積投影幾何 centroid** | centroid 命中率 96.99% > representative_point 96.80%；centroid 落 part 外僅 2.21%，且整體命中率仍較高，無需 fallback |
| admin2 命中率（全精度） | **96.99%** | 與先導簡化版 97.17% 一致（差約 0.18 個百分點，符合全精度幾何細節增加之預期） |

最終 Albers proj4：

```
+proj=aea +lat_1=1 +lat_2=-8 +lat_0=-3 +lon_0=118 +x_0=0 +y_0=0 +ellps=GRS80 +towgs84=0,0,0,0,0,0,0 +units=m +no_defs
```

> 實驗設定與實驗 A–C 的完整數據記錄於[印尼 handler 階段二實驗報告](idn-handler-projection-coordinate-experiment.md)，此處不重複轉載。

## 階段二可重現性

- 隨機種子：`seed=42`（全實驗）。
- 共用工具：`/tmp/idn_exp/formal/common.py`
- 實驗 A：`/tmp/idn_exp/formal/exp_a_projection.py` → `result_a.json`
- 實驗 B+C：`/tmp/idn_exp/formal/exp_bc_hitrate.py` → `result_bc.json`
- 依賴：shapely 2.1.2、numpy 2.4.6、scikit-learn、pyproj 3.7.2
  （venv：`/tmp/idn_exp/.venv`）。

---

# 階段三實驗全文：Wikidata 正式實驗報告

- 日期：2026-06-06
- 國家：印度尼西亞（Indonesia）
- 國家 QID：Q252
- 資料來源：縣市清單（521 列、515 筆唯一 (省,縣市)；5 列為 multipart polygon 重複）
- Guardrails：所有 Wikidata API / WDQS 呼叫單線循序、每次 sleep≥1.2s、429
  尊重 Retry-After（WDQS 封頂 60s 防過長懲罰）、附 User-Agent；所有 QID/label
  即時查證，不憑訓練資料。

## 1. Q252 國家 QID 查證

wbgetentities(Q252) 即時結果：

| lang | label |
|------|-------|
| en | Indonesia |
| zh | 印度尼西亚 |
| zh-tw | 印度尼西亞 |
| id | Indonesia |
| zh-hant | 印度尼西亞 |

確認 Q252 = 印度尼西亞，作為 handler `country_qid` 常數 hardcode（附中文名
註解）。P31 為國家/主權國家類別、無 P131，符合頂層國家預期。

## 2. P31 class 反查（含排除集）

對已知實體即時反查 P31：

| 實體 | QID | P31 | 結論 |
|------|-----|-----|------|
| 西爪哇省 | Q3724 | Q5098 | 省 class |
| Kabupaten Bandung | Q10332 | Q3191695 | 縣 class |
| Kota Bandung | Q10389 | Q3199141, Q137890005, Q19943591 | 市 class（後兩為 Kota Besar/Kotapraja 規模分類，不納過濾集） |
| Papua Pegunungan | Q112810104 | Q5098 | 省 class |
| Jakarta Barat | Q10116 | **Q4272761** | 行政市（先導遺漏） |
| Kepulauan Seribu | Q10107 | **Q11127777** | 行政縣（先導遺漏） |

**確認 admin2 P31 過濾集（5 class）**：

| QID | en label | 中文 | 用途 |
|-----|----------|------|------|
| Q5098 | province of Indonesia | 印度尼西亞省 | admin1 |
| Q3191695 | regency of Indonesia | 縣 (kabupaten) | admin2 標準 |
| Q3199141 | city of Indonesia | 市 (kota) | admin2 標準 |
| Q4272761 | administrative city of Indonesia | 行政市 | 雅加達 5 行政市 |
| Q11127777 | administrative regency of Indonesia | 行政縣 | 雅加達千島群島 |

**排除集（electoral district / dapil）**：對 dapil 實體反查 P31 得
Q56072658（electoral district in Indonesia）、Q109540666（DPR-D electoral
district）。dapil 名稱（如 "Jawa Barat V"、"DKI Jakarta II"、"Sumatera
Utara II"）會與省名碰撞，靠 P31=Q5098 排除。

## 3. admin1 搜尋語言實驗（全取 38 省）

方法：每省 BIG 省名直接當查詢字串，wbsearchentities(limit=7)，id 與 en 各
一次；正確判定 = 候選 P31 含 Q5098 且 P131 含 Q252。

| 語言 | present | rank1 |
|------|---------|-------|
| id | 38/38 | 38/38 (100%) |
| en | 38/38 | 38/38 (100%) |

特殊省名抽查（皆 rank1）：DKI Jakarta=Q3630、Daerah Istimewa
Yogyakarta=Q3741、新巴布亞 4 省 Q112810104/Q61439296/Q115253263/
Q12486766。rank2 多為同名 dapil，由 Q5098 過濾排除。

## 4. admin2 搜尋語言實驗

**測試集建構**：
- 結構性同名類別全取 52 筆：從唯一縣市清單找出 26 對「裸名 + Kota 前綴」
  並存者（Bandung↔Kota Bandung … Tegal↔Kota Tegal），bare 與 Kota 各取，
  共 52 實體。
- 隨機 50 筆：`python random.Random(42).sample(rows_sorted, 50)`，rows_sorted
  為 515 筆唯一 (province, WADMKK) 依 (province,name) 排序清單。
- dedup 合併 = 94 筆。

**查詢字串正規化**：裸名 → "Kabupaten "+裸名；"Kota X" → 保持；雅加達
縮寫展開（見第 5 節，但 5 個 Kota Adm. + 千島群島不在 94 測試集內，另於
第 5 節獨立驗證）。

**正確判定** = 候選 P31 含 {Q3191695, Q3199141}（雅加達加 Q4272761/
Q11127777）且 P131 含所屬 admin1 QID。

| 語言 | 同名類別 (52) rank1 | 隨機50 rank1 | 合計 rank1 |
|------|--------------------|-------------|-----------|
| id | 52/52 | 50/50 | **94/94 (100%)** |
| en | 48/52 | 50/50 | 90/94 (95.7%) |

**en 失敗案例分析**（4 筆，全為 Kota X，id 皆 rank1）：

| 縣市 | 省 | id_rank | en_rank | 原因 |
|------|----|---------|---------|------|
| Kota Tegal | Jawa Tengah | 1 | 2 | en label 偏好裸名縣，市退 rank2 |
| Kota Kediri | Jawa Timur | 1 | 2 | 同上 |
| Kota Madiun | Jawa Timur | 1 | 2 | 同上 |
| Kota Probolinggo | Jawa Timur | 1 | 2 | 同上 |

## 5. 雅加達正規化驗證

| WADMKK | 正規化查詢 | rank | QID | P31 |
|--------|-----------|------|-----|-----|
| Kota Adm. Jakarta Barat | Jakarta Barat | 1 | Q10116 | Q4272761 |
| Kota Adm. Jakarta Pusat | Jakarta Pusat | 1 | Q10109 | Q4272761 |
| Kota Adm. Jakarta Selatan | Jakarta Selatan | 1 | Q10114 | Q4272761 |
| Kota Adm. Jakarta Timur | Jakarta Timur | 1 | Q10111 | Q4272761 |
| Kota Adm. Jakarta Utara | Jakarta Utara | 1 | Q10113 | Q4272761 |
| Adm. Kep. Seribu | Kepulauan Seribu | 1 | Q10107 | Q11127777 |

展開後 **6/6 全 rank1**，P131 皆=Q3630。**關鍵**：初次以先導 3-class 驗證為
0/6（雅加達 6 區用 Q4272761/Q11127777 特殊 class）；加入兩 class 後 6/6。
此為先導遺漏，handler 過濾集必須含 5 class。

DKI Jakarta 省實體（Q3630）查證：label en="Jakarta"、zh="雅加达"、
zh-tw="雅加達"、id="Jakarta"；P31 含 Q5098（仍登錄為省級）。Wikidata 目前
主 label 仍為 "Jakarta"，DKJ（Daerah Khusus Jakarta）改名尚未反映於主 label。
**admin1 搜尋字串建議用 "DKI Jakarta"（WADMPR 原值，rank1 命中 Q3630），
不依賴主 label。**

## 6. P131 全量重驗（WDQS）

單次 SPARQL（POST，VALUES 38 省 QID）：

| 指標 | 結果 |
|------|------|
| (wdt:P131)+ → Q252 通過 | 38/38 (100%) |
| P31 含 Q5098 | 38/38 (100%) |
| zh label 覆蓋 | 38/38 (100%) |

新巴布亞 4 省 zh label 品質（皆**意譯**，非音譯）：

| 省 | QID | zh |
|----|-----|----|
| Papua Barat Daya | Q115253263 | 西南巴布亚省（簡體，需 s2t） |
| Papua Pegunungan | Q112810104 | 高地巴布亞省 |
| Papua Selatan | Q61439296 | 南巴布亞省 |
| Papua Tengah | Q12486766 | 中巴布亞省 |

標準 P131 規則不使既有正確 admin1 翻譯回退。

## 7. zh-tw vs zh 標籤策略

抽 10 個已驗證命中縣市（seed=42）比較：

| 縣市 | QID | zh | zh-tw | zh-hant |
|------|-----|----|-------|---------|
| Rokan Hilir | Q7759 | 下罗干县 | — | 下羅乾縣 |
| Bandung | Q10332 | 萬隆縣 | — | — |
| Kota Denpasar | Q11506 | 丹帕沙 | 丹帕沙 | 丹帕沙 |
| Magelang | Q10621 | 馬格朗縣 | — | — |
| Kota Magelang | Q11017 | 馬格朗 | — | 馬格朗 |
| Sumedang | Q10382 | 双木丹县 | — | 雙木丹縣 |
| Cirebon | Q10368 | 井里汶县 | — | 井里汶縣 |
| Kota Sungai Penuh | Q7592 | 雙溪珀努 | — | 雙溪珀努 |
| Luwu Utara | Q14606 | 北鲁乌县 | — | — |
| Kota Bima | Q14128 | 比馬 | — | 比馬 |

**zh 有值 10/10、zh-tw 僅 1/10**。zh 標籤混雜簡繁（下罗干县/双木丹县/
北鲁乌县 為簡體）。

**結論**：zh-tw 標籤幾乎不存在，必須比照 `thailand_wikidata.rs` 慣例——
WikidataClientOptions 主語言設 "zh-tw"、fallback `["zh-hant","zh"]`，再經
`translate.rs` 的 OpenCC s2t（簡→繁）轉換補齊繁體。OpenCC zh→zh-tw 路徑對
印尼簡體 zh 標籤可行。

## admin2 zh 覆蓋率（94 命中樣本）

| 標籤 | 覆蓋 |
|------|------|
| zh | 89/94 (94.7%) |
| zh-hant | 56/94 (59.6%) |
| zh-tw | 4/94 (4.3%) |
| 任一 zh 系 | 90/94 (95.7%) |

先導全集（515 唯一）zh 88%。4 個無任何 zh 系標籤（Pekalongan Q10623、
Solok Q6058、Pasaman Barat Q6103、Kepahiang Q7940）為 Wikidata 真實缺漏，
依 fallback 回退印尼官方名。

## 階段三綜合決策摘要

1. **country_qid**：Q252（hardcode）。
2. **admin1 主搜尋語言**：id（與 en 並列 100%，採 id 維持一致）。
3. **admin2 主搜尋語言**：id（100% > en 95.7%；同名 Kota/Kabupaten 對上 id 完勝）。
4. **fallback**：en 為驗證後備，必過 P31 class + P131 過濾。
5. **admin2 P31 過濾集（5 class）**：Q3191695、Q3199141、Q4272761、Q11127777
   （admin1 用 Q5098）。
6. **排除 class**：Q56072658、Q109540666（dapil 電選區）。
7. **雅加達正規化**：Kota Adm. Jakarta X→Jakarta X；Adm. Kep. Seribu→Kepulauan Seribu。
8. **admin1 字串**：DKI Jakarta / Daerah Istimewa Yogyakarta 用 WADMPR 原值。
9. **翻譯輸出**：主語言 zh-tw、fallback [zh-hant, zh] + OpenCC s2t；admin1 zh
   100%、admin2 zh ~88–95%，缺漏回退官方名。

## 階段三中間數據檔案（/tmp/idn_exp/wikidata/）

- `wd.py` — Wikidata/WDQS helper（單線、sleep、Retry-After、UA）
- `provinces.txt` / `province_qid.json` — 38 省與 QID 映射
- `kabkota_all.txt` / `samename_pairs.json` — 縣市清單與 26 同名對
- `admin2_samename.json` / `admin2_random50.json` / `admin2_testset.json` — 測試集（seed=42）
- `step1_2_entities.json` — Q252 與 P31 反查
- `admin1_results.json` / `admin2_results.json` — 搜尋語言實驗完整候選與 rank
- `jakarta_results.json` — 雅加達正規化驗證
- `wdqs_admin1.json` — WDQS 全量 P131/zh 驗證
- `zhtw_compare.json` / `admin2_zh_coverage.json` — zh-tw 策略與覆蓋率
