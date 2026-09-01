# 印尼行政區處理邏輯

> 本文件說明本專案如何處理印尼地區的地理資訊，是 [README 支援地區與語言策略](../../README.md#支援地區與語言策略) 的詳細版本。

## 資料來源

本專案針對印尼地區的地理資訊處理，以 **印尼地理空間資訊局（BIG）** 的官方行政區邊界資料為核心：

- **來源**：印尼地理空間資訊局（Badan Informasi Geospasial, BIG）官方 REST 服務
- **資料提供**：BIG，透過官方 ArcGIS REST 圖資服務（`BATASWILAYAH/BATAS_DESAKEL_AR` MapServer）發布
- **資料層級**：desa（村級）邊界，屬性同時含省、縣市、郡、村各級行政名稱
- **下載日期**：2026-08-24
- **資料版本**：`TASWIL1000020260612DESAKEL_AR`（2026-06-12，記錄於圖徵的 `METADATA` 屬性）
- **下載精度參數**：`geometryPrecision=6`（保留小數 6 位，不做幾何簡化的全精度輸出）
- **座標系**：屬性 `SRS_ID = "4326"`，直接視為 WGS84（EPSG:4326）處理

下載步驟與必要的錯誤檢查請見[本地資料處理](development.md#印尼)。

### 授權合規立場

BIG 圖資為印尼官方公開地理資料。本專案**僅將其作為衍生加工的輸入**，輸出的是
經反向地理編碼最佳化後的地名與代表座標資料（cities500 風格的單點 metadata），
**不散布、不重新發行 BIG 的原始向量圖資（polygon 邊界）**。原始圖資請逕向 BIG
官方服務取得。若未來需要可重新發行的開放圖資作為替代或交叉驗證，**GADM** 與
**HDX（OCHA）** 的印尼 COD-AB 為已知備案來源。

## 行政區層級

> [!NOTE]
> `admin_3` 與 `admin_4` 只存在於本專案的中介 CSV，用於保留來源行政層級供追溯與除錯，不會輸出到 Immich 使用的 cities500。Immich 顯示的最細層級是 `admin_2`。代表點密度取決於 extract 的來源 feature 顆粒度（印尼為 desa，且 multipart 圖徵會逐 part 拆列），與這兩個欄位無關。


BIG desa 圖資的屬性提供下列行政層級：

- **Admin 1**：省（Provinsi，BIG 欄位 `WADMPR`）
- **Admin 2**：縣／市（Kabupaten / Kota，BIG 欄位 `WADMKK`）
- **Admin 3**：郡（Kecamatan，BIG 欄位 `WADMKC`）
- **Admin 4**：村（Desa / Kelurahan，BIG 欄位 `WADMKD`）

本專案使用 desa 層級圖資作為 extract 來源（以村級 polygon 提升定位密度），輸出欄位如下：

| 輸出欄位 | 來源欄位 | 說明 |
|---|---|---|
| `country` | 固定值 | `印尼` |
| `admin_1` | Wikidata / `WADMPR` | 省的繁中翻譯；缺少中文時回退 BIG 官方印尼文 |
| `admin_2` | Wikidata / `WADMKK` | 縣／市的繁中翻譯；缺少中文時回退 BIG 官方印尼文 |
| `admin_3` | `WADMKC` | 郡（Kecamatan）官方印尼文原文 |
| `admin_4` | `WADMKD` | 村（Desa / Kelurahan）官方印尼文原文 |

比照 TH / KR handler，`admin_1` / `admin_2` 為翻譯後繁中，`admin_3` 以下沿用印尼文原文。

`admin_2` 的譯名查表分三段：先以（省, 縣市）配對查詢；查不到時改用全域同名後備表
（同一個 `WADMKK` 在各省譯名一致者才收錄，譯名不一致者剔除以免張冠李戴）；仍無結果
才回退 BIG 原文。

> **未定義行政區過濾**：BIG 圖資中 `WADMPR` 或 `WADMKK` 為空者，多為
> 「Area tidak terdefinisi」（未定義行政區），無法對應到省／縣市，extract
> 時直接跳過。

> **筆數說明**：本批次 desa 圖資可用 feature 共 84,468 筆（原始 84,503 筆，
> 35 筆因缺 `WADMPR` / `WADMKK` 被過濾）。extract 輸出的列數會多於行政區數，
> 原因是同一個 desa 若由多個不相連的 polygon 組成（multipart 邊界），**每個
> part 各自計算 Albers centroid 並輸出成獨立的一列**。這批 desa 經 multipart
> 分列後共產生 108,673 個候選點，與 `data/handler/id_geodata.csv` 現有列數一致。
> 因此「輸出列數」與「行政區數」不會一致，這是預期行為。feature 筆數為
> 2026-08-24 下載批次的離線統計，不隨每次 release 重新計算。

## 名稱策略

印尼 handler 沿用泰國 / 南韓 handler 的 Wikidata translator 流程：**Admin 1
與 Admin 2 走標準 P131 鏈驗證，並以 instance-of（P31）類別過濾候選**；缺少可靠
中文時回退 BIG 官方印尼文。名稱決策同樣分為兩層：先決定是否信任 Wikidata 結果，
再決定採用哪一個語言的 label。

### 第一層：是否信任 Wikidata 結果（P131 行政隸屬驗證）

| 層級 | parent QID | P131 驗證 | 無法通過驗證時 |
|---|---|---|---|
| **Admin 1**（省） | 印尼（`Q252`） | 取第一個通過的候選 | 回退 BIG 官方印尼文 |
| **Admin 2**（縣／市） | 所屬省的 QID | 取第一個通過的候選 | 回退 BIG 官方印尼文 |

候選依搜尋排序逐一驗證，取第一個通過 P131 的實體；全數未通過時回退 BIG 官方印尼文。
WDQS 查詢本身失敗（逾時、回應無法解析）視同該候選未通過，且不寫入快取，因此暫時性
網路問題只會讓該筆回退原文，不會把錯誤結論固化在 cache 中。

Admin 2 的 parent QID 來自第一層 Admin 1 翻譯所解析出的省 QID。實驗以 WDQS
全量重驗 38 省，`(wdt:P131)+ → Q252` 通過率 38/38（100%）、P31 含 `Q5098`
通過率 38/38（100%），標準 P131 規則不會使既有正確的 admin1 翻譯回退。此為建置
handler 時的離線驗證結果，記錄見[印尼 handler 研究報告](../research/indonesia-handler.md)。

#### P31 候選類別過濾（五個 class）

實驗反查確認候選需落在下列五個 instance-of 類別之一，方視為合法行政區。兩級各用一組
允許集合：Admin 1 只允許 `Q5098`，Admin 2 允許其餘四個類別。

| QID | 英文 label | 中文 | 用途 |
|---|---|---|---|
| `Q5098` | province of Indonesia | 印度尼西亞省 | Admin 1（省） |
| `Q3191695` | regency of Indonesia | 縣（kabupaten） | Admin 2 標準 |
| `Q3199141` | city of Indonesia | 市（kota） | Admin 2 標準 |
| `Q4272761` | administrative city of Indonesia | 行政市 | 雅加達五行政市 |
| `Q11127777` | administrative regency of Indonesia | 行政縣 | 雅加達千島群島 |

> [!IMPORTANT]
> 後兩個類別（`Q4272761` / `Q11127777`）為先導實驗的遺漏。雅加達特區的
> 5 個城區與千島群島使用的是「行政市 / 行政縣」這兩個特殊 class，而非一般的
> kota / kabupaten。初次以三類別驗證時，雅加達 6 區命中為 0/6；補上兩個類別
> 後達 6/6。因此 handler 的 P31 過濾集**必須**含完整五個 class。

#### 排除選舉區（dapil）

印尼 Wikidata 上的「選區（daerah pemilihan / dapil）」常與行政區同名，
含國會（DPR / DPD）與地方議會（DPR-D）選區。反查得選區類別為
`Q56072658`（electoral district in Indonesia）與 `Q109540666`（DPR-D
electoral district）。這兩個類別不在上述允許集合中，因此選區在候選
過濾階段即被剔除；label 含 dapil / DPR / DPRD / DPD / pemilihan / electoral 等單詞者，
另由關鍵字斷詞比對排除。「省名＋羅馬數字」型態的選區（如 `Jawa Barat V`、
`DKI Jakarta II`、`Sumatera Utara II`）雖會與省名碰撞，但同樣因 P31 不在允許集合中而
被剔除。

### 搜尋語言：印尼文（id）為主、英文為驗證後備

搜尋以印尼文（`id`）為主要語言；英文（`en`）僅作為人工驗證的後備，不進入自動流程。

> [!IMPORTANT]
> 「印尼文為主」是經實驗驗證的選擇。
>
> - **Admin 1（38 省全取）**：id 與 en 皆 38/38（100%）rank-1 命中。兩者並列，
>   採 id 以維持與 admin2 一致。
> - **Admin 2（94 筆測試集）**：id 為 **94/94（100%）** rank-1，en 為
>   90/94（95.7%）。en 的 4 筆失敗全為 `Kota X` 型市級（Kota Tegal、
>   Kota Kediri、Kota Madiun、Kota Probolinggo），原因是 en label 偏好同名
>   的裸名縣，使市退到 rank-2；同樣案例以 id 搜尋皆 rank-1。
>
> 測試集建構（`seed=42` 可重現）：結構性同名類別全取 52 筆（26 對「裸名 +
> Kota 前綴」並存者，bare 與 Kota 各取一），加上隨機 50 筆，dedup 合併為 94 筆。
> 上述命中率為建置 handler 時的離線實驗結果，不隨程式碼驗證，原始記錄見
> [印尼 handler 研究報告](../research/indonesia-handler.md)。

#### 搜尋字串正規化

BIG 的 `WADMKK` 對縣級多半只存「Kabupaten」之後的地名，與同名的 kota
（市）同名，直接搜尋會抓錯實體。因此搜尋字串與查詢表 key 分離，規則如下：

- 以 `Kota ` 開頭者（市）：保持原樣。
- 其餘（縣）：加上 `Kabupaten ` 前綴後再搜尋。
- 雅加達特區城區：依下節規則展開為通用名稱。

### 雅加達特區正規化

BIG 圖資以官方全名儲存雅加達五城區與千島群島，但 Wikidata 以通用名稱為主 label，
不正規化會搜不到正確實體。展開規則與驗證結果（皆 rank-1、P131 皆對 `Q3630`）：

| WADMKK（BIG 原文） | 正規化查詢 | QID | P31 |
|---|---|---|---|
| Kota Adm. Jakarta Barat | Jakarta Barat | `Q10116` | `Q4272761` |
| Kota Adm. Jakarta Pusat | Jakarta Pusat | `Q10109` | `Q4272761` |
| Kota Adm. Jakarta Selatan | Jakarta Selatan | `Q10114` | `Q4272761` |
| Kota Adm. Jakarta Timur | Jakarta Timur | `Q10111` | `Q4272761` |
| Kota Adm. Jakarta Utara | Jakarta Utara | `Q10113` | `Q4272761` |
| Adm. Kep. Seribu | Kepulauan Seribu | `Q10107` | `Q11127777` |

#### DKI / DKJ 說明

雅加達省的 Wikidata 實體為 `Q3630`，主 label 仍為 "Jakarta"
（en="Jakarta"、zh="雅加达"、zh-tw="雅加達"、id="Jakarta"），P31 含
`Q5098`（仍登錄為省級）。雅加達近年由 DKI（Daerah Khusus Ibukota，首都特區）
更名為 **DKJ（Daerah Khusus Jakarta，雅加達特區）**，但此改名尚未反映於
Wikidata 主 label。因此 **admin1 搜尋字串採用 BIG 的 `WADMPR` 原值
「DKI Jakarta」**（rank-1 命中 `Q3630`），不依賴會變動的主 label。同理，
「Daerah Istimewa Yogyakarta」（日惹特區）也直接採 `WADMPR` 原值搜尋。

### 第二層：語言 label 優先序

當一筆 item 決定採用 Wikidata 結果後，依下列順序挑選名稱：

1. Wikidata `zh-tw` label
2. Wikidata `zh-hant` label
3. Wikidata `zh` label，並透過 OpenCC 簡轉繁（`cn2t` preset）轉為繁體中文
4. Wikidata 的中文維基百科（zhwiki）sitelink 標題，經 zh.wikipedia 繁簡轉換 API 轉為繁體
5. BIG 官方印尼文原文（`WADMPR` / `WADMKK`）

> **為何 fallback 排除英文**：handler 將 translator 的 `fallback_langs` 明確
> 設為 `["zh-hant", "zh"]`（比照 `thailand_wikidata.rs`），排除預設鏈中的
> `en` 與 source（`id`）。BIG 來源無官方英文欄位，若不排除 `en`，無中文
> label 的縣市會回退 Wikidata 英文（如 Aceh Tengah → Central Aceh），與
> 「缺中文時保留官方印尼文原文」的設計相違。
>
> **為何需要簡轉繁**：印尼地名的繁中標籤在 Wikidata 上稀少，`zh` 與部分
> `zh-hant` label 混雜簡體（如 Papua 的 `zh-hant` 為「巴布亚省」）。`zh`
> label 由 translator 以 OpenCC 轉繁；`zh-hant` label 則在 handler 消費層
> 以「安全字元級簡轉繁白名單」補正。

> **為何不用完整 OpenCC 簡轉繁後處理 `zh-hant`**：OpenCC 的完整簡轉繁對「本身即正體」
> 的字會過度轉換成異體（`里→裏`、`占→佔`、`岩→巖`、`干→乾`、`群→羣`）。
> 實測對真實 ID 輸出套用完整簡轉繁會改壞 24 個正確譯名（如 井里汶縣 →
> 井裏汶縣、峇里巴板 → 峇裏巴板），只修好 1 個（巴布亚省 → 巴布亞省）。
> 因此 handler 改用「簡體獨有、無正體歧義」的字元白名單（見
> `indonesia_normalize::fix_simplified_chars`），只修真正的簡體字，對已
> 正確的繁體專名為冪等、零回歸。

admin1 的 zh 系 label 覆蓋為 38/38（100%）；新巴布亞 4 省皆為**意譯**而非音譯（下表為
經消費層安全簡轉繁與補省字尾後的最終輸出，其中 Papua Barat Daya 的原始 `zh` label 為
簡體「西南巴布亚省」）：

| 省 | QID | 最終 admin1 輸出 |
|---|---|---|
| Papua Barat Daya | `Q115253263` | 西南巴布亞省 |
| Papua Pegunungan | `Q112810104` | 高地巴布亞省 |
| Papua Selatan | `Q61439296` | 南巴布亞省 |
| Papua Tengah | `Q12486766` | 中巴布亞省 |

少數 Wikidata 真實缺漏中文者（無任何 zh 系標籤）依 fallback 回退 BIG 官方
印尼文原文（不回退英文）。實際回退規模與名單以 release 重新 extract 後的
`geoname_data/ID_wikidata_cache.json` 為準。

### 消費層守門

handler 採用 Wikidata 譯名前，還會對字串本身做兩道檢查：

- **去除消歧括號**：Wikidata 同名實體的 label 可能帶消歧後綴（如「薩米縣 (巴布亞省)」），
  尾端括號一律移除，避免把 Wikidata 的內部消歧格式輸出給使用者。
- **純中文檢查**：譯名含 ASCII 字母或不含漢字者視為無效，回退 BIG 官方印尼文。此判定
  攔下的是純拉丁字串（如 `East Barito`）與中英夾雜的半翻譯（如「西Kutai區」）。

兩道檢查對即時查詢、cache 與 fixture stub 一視同仁（見 `label_sanitize`），舊 cache 殘留
的錯誤譯名不會繞過守門。

### Wikidata cache

Wikidata cache 位置為：

```text
geoname_data/ID_wikidata_cache.json
```

Fixture 測試會使用 `ID_wikidata_stub.json`，避免測試依賴即時網路查詢。

## 座標策略

BIG schema **無官方代表點欄位**（屬性僅含行政碼、名稱、面積等），故須由幾何
自行決定代表座標。本專案採用 **Albers 等積投影下的幾何 centroid**，每個
MultiPolygon part 各取一個 centroid（與 multipart 分列一致），不引入
`representative_point` fallback。

實驗以 desa 每 part 的 Albers centroid 為候選點、kecamatan 內 bbox rejection
取樣為模擬 GPS，做 BallTree haversine 最近鄰匹配：

| 座標策略 | admin2 命中率 |
|---|---:|
| Albers centroid | **96.99%** |
| representative_point | 96.80% |

centroid 整體勝出 0.19 個百分點。雖然群島國家有 2.21%（104,470 個 part 中
2,311 個）的 centroid 落在該 part 幾何之外，但**不需要** fallback：候選點僅作
最近鄰匹配的代表座標（非顯示座標），centroid 落外數十至數百公尺通常不影響
「最近 part」判定；且 representative_point 為保證落在幾何內，反而把代表點推向
凹形內凹邊，降低代表性。分區域命中率 7 區中 6 區 centroid 勝出，各區穩定
落在 96%~98%，無明顯弱區。

上述命中率為建置 handler 時的離線實驗結果，不隨程式碼驗證，完整方法與數據見
[印尼投影與座標實驗](../research/idn-handler-projection-coordinate-experiment.md)。

## 投影策略

印尼 handler 使用單一 Indonesia Albers 等積投影計算 centroid：

```text
+proj=aea +lat_1=1 +lat_2=-8 +lat_0=-3 +lon_0=118 +x_0=0 +y_0=0 +ellps=GRS80 +towgs84=0,0,0,0,0,0,0 +units=m +no_defs
```

設計依據：印尼緯度範圍約 6°N~11°S（共約 17°），標準緯線置於南北邊界內側約
1/6 與 5/6 處以最小化面積變形——`lat_1` 取 +1°（赤道北側陸塊較少，內收）、
`lat_2` 取 −8°、`lat_0` = −3°（緯度中心）、`lon_0` = 118°（群島經度中心）、
橢球採 GRS80（與 SRGI 2013 / WGS84 一致）。

此策略沒有採用日本與南韓 handler 的 dynamic UTM 流程，與泰國採單一 Albers
的先例一致。實測 Albers 與 dynamic UTM centroid 的差異極小：

| 樣本群 | n | 中位 | 平均 | p99 | 最大 |
|---|---:|---:|---:|---:|---:|
| 全體抽樣 | 5,000 | 0.0108 m | 0.1850 m | 3.79 m | 32.98 m |
| 面積前 100 大 | 100 | 2.49 m | 3.17 m | 11.53 m | 12.12 m |
| 跨經度前 200 | 200 | 2.52 m | 3.25 m | 12.14 m | 32.98 m |

一般 desa 兩法差異為公分級（中位 0.011 m）；即使在極端跨經度散群島樣本
（最大差異 32.98 m，出現於 Maluku 省 Maluku Tengah 縣 Nua Nea 村）也僅
數十公尺，遠小於村級行政區的空間粒度，不影響最近鄰 admin2 歸屬。基於準確度、
效能與實作簡潔性，印尼採用單一 Albers 直接計算 centroid。上表同為離線實驗結果，
來源見[印尼投影與座標實驗](../research/idn-handler-projection-coordinate-experiment.md)。

## 時區處理

印尼橫跨三個時區，本專案以 38 省的 per-province 對照表解析：

| 時區 | IANA | UTC 偏移 | 省數 | 省份（BIG WADMPR 原文） |
|---|---|---|---:|---|
| WIB（Waktu Indonesia Barat） | `Asia/Jakarta` | UTC+7 | 18 | Aceh、Sumatera Utara、Sumatera Barat、Riau、Kepulauan Riau、Jambi、Sumatera Selatan、Kepulauan Bangka Belitung、Bengkulu、Lampung、DKI Jakarta、Banten、Jawa Barat、Jawa Tengah、Daerah Istimewa Yogyakarta、Jawa Timur、Kalimantan Barat、Kalimantan Tengah |
| WITA（Waktu Indonesia Tengah） | `Asia/Makassar` | UTC+8 | 12 | Kalimantan Selatan、Kalimantan Timur、Kalimantan Utara、Bali、Nusa Tenggara Barat、Nusa Tenggara Timur、Sulawesi Utara、Sulawesi Tengah、Sulawesi Selatan、Sulawesi Tenggara、Gorontalo、Sulawesi Barat |
| WIT（Waktu Indonesia Timur） | `Asia/Jayapura` | UTC+9 | 8 | Maluku、Maluku Utara、Papua、Papua Barat、Papua Selatan、Papua Tengah、Papua Pegunungan、Papua Barat Daya |

時區在 transform 階段（cities500 schema）解析。時區歸屬以 **BIG WADMPR
原文拼寫**（`WIB/WITA/WIT_PROVINCES`）為唯一權威 key——原文穩定、語言無關，
不受翻譯形態與未來 Wikidata label 漂移影響。

transform 端只拿得到 handler 的「最終省名」（Wikidata 繁中譯名經安全簡轉繁與
補省字尾正規化後的形態，如 `中爪哇省`、`巴釐省`、`巴布亞省`），canonical CSV
schema 不含 WADMPR 欄位。因此對照表另附一份「最終省名 → WADMPR 原文」對照
（`PROVINCE_ZH_TW`）作為查詢入口；此繁中 key **必須與 handler 最終 admin1
輸出逐字一致**，由 `indonesia_timezone` 的
`handler_admin1_outputs_resolve_timezone` 測試斷言 38 省最終形態全部命中對照表，避免
兩處漂移。省名未命中對照表時，transform 直接回報錯誤讓 release 失敗，不會靜默套用
WIB，確保問題在發版前暴露。

## 注意事項

- 印尼 Admin 1 / Admin 2 會使用 Wikidata 繁中翻譯，兩級都須通過 P131 行政隸屬
  驗證（Admin 1 對印尼 `Q252`、Admin 2 對所屬省的 QID）；驗證失敗或 Wikidata
  沒有可靠中文結果時，回退至 BIG 官方印尼文。
- 印尼 Admin 3（郡）/ Admin 4（村）保留 BIG 官方印尼文，避免大量低層級地名在
  Wikidata 中出現錯配或不穩定翻譯。
- BIG 原始向量圖資不在本專案散布範圍內；僅散布反向地理編碼最佳化後的衍生 metadata。
- 所有座標決策、命中率與搜尋語言實驗皆以 `seed=42` 固定隨機種子，結果可重現。
- 在本機重現提取流程的指令請見[本地資料處理](development.md#2-提取原始地理資料)。
