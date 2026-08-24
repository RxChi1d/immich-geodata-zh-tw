# 印尼 handler 階段二實驗：投影法與座標策略決策

> **文件狀態：歷史紀錄**
> 本文記錄當時的研究與決策，不保證與目前實作同步。現行行為請參閱[印尼行政區處理](../zh-tw/indonesia-admin-processing.md)。

本報告記錄印尼（IDN）handler 在新增前的階段二正式實驗結果，用以決定
extract pipeline 的「投影法」與「代表座標策略」。所有實驗為純本地計算，
未呼叫任何 Wikidata / 網路 API。

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

## 共同實驗設定

- **隨機種子**：`seed=42`（全部實驗一致，可重現）。
- **資料**：全精度 BIG 圖資（`geometryPrecision=6`，無幾何簡化）。
  - desa（村級）：`/tmp/idn_data/desa/*.geojson`，可用 feature 83,461 筆
    （原始 83,462 筆，1 筆因缺 `WADMPR`/`WADMKK` 過濾）。
  - kecamatan（郡級）：`/tmp/idn_data/kec/*.geojson`，7,299 筆。
- **座標系**：屬性 `SRS_ID = "SRGI 2013"`，與 WGS84 在公尺級差異可忽略，
  視為 EPSG:4326。資料含 Z=0 維度，計算前一律降為 2D。
- **行政欄位**：`WADMPR`=省、`WADMKK`=縣市（admin2）、`WADMKD`=村名。
- **記憶體管理**：`iter_features` 逐檔載入、處理完即釋放（1.3 GB JSON）；
  實驗 A 因需面積排序而保留全幾何於記憶體（機器可用 51 GB，安全）。

### Albers 投影設計依據

印尼村級資料經度範圍約 99.96°E~117.94°E（本批次；全國約 95°E~141°E），
緯度約 6°N~11°S（共約 17°）。Albers 等積投影標準緯線最佳實務為置於南北
邊界內側約 1/6 與 5/6 處，以最小化整體面積變形：

- `lat_1 ≈ 6 − 17/6 ≈ +3.2°` → 取 **+1°**（赤道北側陸塊較少，內收）。
- `lat_2 ≈ 6 − 5×17/6 ≈ −8.2°` → 取 **−8°**。
- `lat_0 = −3°`（緯度中心）、`lon_0 = 118°`（群島經度中心）。
- 橢球採 GRS80（與 SRGI 2013 / WGS84 一致）。

此設計與 TH（泰國）handler 採單一 Albers 投影的先例一致：對等積投影而言，
centroid 計算對標準緯線的具體選值極不敏感（差異見實驗 A）。

## 實驗 A：Albers vs dynamic UTM centroid 差異

### 方法

- 抽樣 5,000 個 desa polygon（`seed=42`），刻意涵蓋極端案例：
  - 面積前 100 大（Albers 投影面積）。
  - 跨經度前 200（bbox 經度跨度最大）。
  - 其餘隨機抽樣補足至 5,000。
- 對每個樣本同時計算：
  - (a) **Albers**：固定上述單一 proj4 投影後取 centroid。
  - (b) **dynamic UTM**：依 polygon 中心經度選 UTM 帶
    （`zone = ⌊(lon+180)/6⌋+1`），中心緯度 ≥0 用 north（EPSG 326xx）、
    <0 用 south（327xx）；投影後取 centroid。
- 以 haversine 計算兩法 centroid 距離（公尺）。

### 結果（公尺）

| 樣本群 | n | 中位 | 平均 | p99 | 最大 |
|--------|---|------|------|-----|------|
| 全體抽樣 | 5,000 | 0.0108 | 0.1850 | 3.79 | 32.98 |
| 面積前 100 大 | 100 | 2.49 | 3.17 | 11.53 | 12.12 |
| 跨經度前 200 | 200 | 2.52 | 3.25 | 12.14 | 32.98 |

最大差異案例：Maluku 省 Maluku Tengah 縣 `Nua Nea` 村，bbox 經度跨
1.23°、緯度跨 3.74°（散布多島嶼的 multipart，bbox 巨大但實際陸地破碎），
差異 32.98 m。

### 判讀

- 一般 desa（佔絕大多數）兩法差異為**公分級**（中位 0.011 m）。
- 即使在面積最大、跨經度最廣的極端樣本，差異也僅**公尺至數十公尺級**，
  遠小於印尼村級行政區的空間粒度（村半徑通常數百公尺至數公里）。
- 此差異不足以改變最近鄰匹配的 admin2 歸屬。

**判準命中**：差異公尺級以下（中位/平均皆 <1 m，p99 <4 m）→ **採 Albers
單一等積投影**。優點：pipeline 不需逐 polygon 切換 UTM 帶、行為單純可重現、
與 TH 先例一致。dynamic UTM 不帶來實質精度收益，反而增加複雜度。

## 實驗 B：命中率重驗（全精度幾何 + Albers centroid）

### 方法（比照先導 `experiment.py`）

- **候選點集**：desa 每個 MultiPolygon part 各出一列（比照 production
  extract「multipart 每 part 一列」），以 Albers 投影 centroid 取點。
- **模擬 GPS**：每個 kecamatan feature 內 bbox rejection sampling 取 3 點
  （`seed=42`），真值 admin2 = 該 feature 的 `(WADMPR, WADMKK)`。
- **匹配**：haversine 最近候選點（`BallTree`），命中 = 候選點
  `(省, 縣市) == 真值`。

### 結果

- desa 候選點（含 multipart 分列）：**104,470** 個。
- 模擬 GPS 樣本：**21,897** 個（rejection 全部成功，0 退用代表點）。
- **admin2 命中率（Albers centroid）：96.99%**。
- 先導簡化版對照：97.17%（差 0.18 個百分點）。

差異來源：全精度幾何邊界更細緻、海岸/邊界鋸齒更真實，使邊界鄰近樣本的
最近鄰判定略嚴格，導致命中率微幅下降，屬合理且可接受範圍。

### 分區域命中率

| 區域 | n | centroid | representative_point |
|------|---|----------|----------------------|
| 摩鹿加 | 708 | 98.31% | 97.88% |
| 峇里/努沙登加拉 | 1,470 | 97.76% | 97.96% |
| 爪哇 | 6,438 | 97.51% | 97.25% |
| 巴布亞 | 2,403 | 96.92% | 96.84% |
| 蘇拉威西 | 3,090 | 96.70% | 96.60% |
| 加里曼丹 | 1,878 | 96.59% | 96.01% |
| 蘇門答臘 | 5,910 | 96.38% | 96.24% |

各區域命中率穩定落在 96%~98%，無明顯弱區；僅峇里/努沙登加拉的
representative_point 略優於 centroid（+0.20 pp），其餘區域 centroid 均勝出。

## 實驗 C：座標策略對照（centroid vs representative_point）

### 背景

已確認 BIG schema **無官方代表點欄位**（屬性僅含行政碼、名稱、面積等），
故須由幾何自行決定代表座標。比較兩策略：

- (a) **等積投影幾何 centroid**：Albers 空間取 centroid 後轉回 WGS84。
- (b) **shapely `representative_point`**：Albers 空間取保證落在幾何內的點，
  再轉回 WGS84。

### 結果

| 策略 | admin2 命中率 |
|------|----------------|
| (a) Albers centroid | **96.99%** |
| (b) representative_point | 96.80% |

**整體 centroid 勝出 0.19 個百分點 → 採 centroid 策略。**

### centroid 落海 / 落 part 外分析

群島國家的隱憂：單一 polygon 的 centroid 在凹形（C 形、新月形、散島）下
可能落在該 part 幾何之外（含落海）。統計（嚴格 `contains`，含 1e-9 容差）：

- part 總數：104,470。
- centroid 落在 part 外：**2,311 個（2.21%）**。

**是否需 representative_point fallback？** 結論：**不需要**。理由：

1. 候選點僅用於**最近鄰匹配的代表座標**，非對外顯示座標；即使 centroid
   落在 part 外數十至數百公尺，對「最近的候選 part」判定通常無影響——
   該 part 的 centroid 仍是離其領域內樣本最近的點之一。
2. 整體命中率 centroid（96.99%）**高於** representative_point（96.80%），
   代表 representative_point 為了「保證落在幾何內」反而把代表點推向幾何
   邊緣（凹形的內凹邊），降低了對鄰近樣本的代表性。
3. 各區域對照（見實驗 B 表）也顯示 centroid 在 7 區中 6 區勝出。

因此**統一採 centroid，無需條件式 fallback**，pipeline 行為單純且可重現。

## 對 handler 實作的建議

1. **投影**：extract handler 對印尼 polygon 一律以上述 Albers proj4 投影後
   取 centroid，再轉回 WGS84 輸出 `latitude` / `longitude`。
2. **multipart**：沿用 production 慣例「每 part 一列」，每 part 取 Albers
   centroid。
3. **代表座標**：採 centroid，不引入 representative_point fallback。
4. 行政欄位對應：`WADMPR`→admin1（省）、`WADMKK`→admin2（縣市）、
   `WADMKD`→村名（admin3/admin4 視 schema 設計）。

## 可重現性

- 隨機種子：`seed=42`（全實驗）。
- 共用工具：`/tmp/idn_exp/formal/common.py`
- 實驗 A：`/tmp/idn_exp/formal/exp_a_projection.py` → `result_a.json`
- 實驗 B+C：`/tmp/idn_exp/formal/exp_bc_hitrate.py` → `result_bc.json`
- 依賴：shapely 2.1.2、numpy 2.4.6、scikit-learn、pyproj 3.7.2
  （venv：`/tmp/idn_exp/.venv`）。
