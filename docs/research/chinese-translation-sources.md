# 中文地名翻譯來源替代方案評估

> **文件定位**：本文件為翻譯來源強化**前**的研究與實測紀錄（snapshot），記錄
> 成文時（2026-06）的評估數據與決策依據，不隨後續實作同步更新。現行翻譯邏輯
> 請以 `src/pipeline/translate.rs` 與 `src/wikidata/` 實作為準。

## 研究背景

現行翻譯分兩條路徑：

- **handler 國家（TW/JP/KR/TH）**：以 Wikidata translator 為核心——以
  名稱搜尋 Wikidata（`wbsearchentities`）、取得 zh / zh-tw / zh-hant
  label（`wbgetentities`）、以 P131 上級行政區鏈驗證消歧，再經 OpenCC
  做簡繁與臺灣用語轉換。
- **全球其餘國家（translate 階段）**：僅以 GeoNames alternateNames 中文
  列與 cities500 內嵌中文翻譯，經 OpenCC 轉換；無 Wikidata 參與
  （LocationIQ 流程不在 production release 中）。

Wikidata 的優點是 CC0 授權、可程式化查詢、無使用限制；缺點是不保證
所有地名都有中文 label，且部分 label 為大陸譯名或簡體。

本研究目標：盤點並**實測**其他開放的「外國地名 → 繁體中文（臺灣用語）」
翻譯來源，評估是否有來源可取代或補強 Wikidata。

## 評估對象

| 來源 | 授權 | 初步篩選結果 |
|---|---|---|
| Wikidata labels（現行） | CC0 | 基準線 |
| 國教院《外國地名譯名》（data.gov.tw dataset 15211） | OGDL 1.0（相容 CC BY 4.0） | 進入實測 |
| GeoNames alternateNamesV2 | CC BY 4.0 | 進入實測 |
| OpenStreetMap `name:zh-Hant` | ODbL | 授權排除（見下） |
| Unicode CLDR | Unicode License | 範圍排除（見下） |
| 樂詞網（terms.naer.edu.tw） | 版權所有，未開放 | 授權排除 |

**OSM 排除理由**：ODbL 的 share-alike 條款會使批次萃取的譯名構成
Derivative Database，發佈時被迫採用同授權，與本專案發佈模式衝突。且 OSM
對國外地名的 `name:zh-Hant` 填充率稀疏，效益低而授權風險高。

**CLDR 排除理由**：僅涵蓋國家／地區層級顯示名稱（約 250 個 ISO territory
碼），無城市級 gazetteer，無法用於 admin2 / city 層級。

## 實測方法

所有數據為 2026-06 實測：

- **國教院（NAER）資料**：下載 dataset 15211 現行版全量解析（64,487 筆）。
- **Wikidata 覆蓋率**：仿照專案 translator 查詢路徑——`wbsearchentities`
  英文名搜尋 top-3 候選、`wbgetentities` 取 zh 系 label、P625 座標距樣本
  ≤25 km 才視為同一地點。8 國 × 各隨機抽 100 個 GeoNames cities500 條目
  （共 800 樣本）。
- **GeoNames alternateNamesV2**：全量 dump（740 MB）統計 cities500 範圍內
  zh 系語言碼分布，並在同批 800 樣本上交叉比對。
- **NAER 匹配**：名稱正規化（去除括號註記、逗號倒裝、變音符號）後比對，
  並以座標距離 ≤15 km 驗證（NAER 座標精度為 ±1 角分，約 2 km）。

## 實測結果

### NAER 資料品質

| 項目 | 實測值 | 說明 |
|---|---|---|
| 筆數 | 64,487 | 涵蓋 700 國家／地區 |
| 座標可解析率 | 99.4% | 原始格式髒（HTML entities、撇號變體、度分制），需 robust parser；54% 可直接解析；失敗 0.6% = 解析失敗 112 筆（0.2%）+ 座標空值 269 筆（0.4%） |
| 座標精度 | ±1 角分（約 2 km） | 匹配容差需 ≥15 km 等級 |
| 疑似簡體字 | 0.03% | 多為誤判（音譯用「云」「后」等），實質為臺灣慣用譯名 |
| 中文名含括號註記 | 3.5% | 如 `科魯涅(科倫納)`，需程式清除 |
| 自然地物比例 | 約 19.6% | 河／灣／島／山等，與聚落對應時需座標驗證排除 |
| 國內同名歧義 | 3.6% | 必須以座標驗證消歧 |

座標驗證的必要性：美國 10,693 筆名稱匹配中，僅 4,072 筆通過 ≤15 km 座標
驗證——同名城鎮極多，名稱匹配不可單獨使用。

### 同批 800 樣本的覆蓋率交叉比較

| 國家 | Wikidata 任何 zh | Wikidata zh-tw/hant | NAER | NAER 獨有救回 |
|---|---:|---:|---:|---:|
| 法國 | 96% | 59% | 11% | +0 pp |
| 巴西 | 93% | 12% | 13% | +1 pp |
| 泰國 | 85% | 77% | 4% | +0 pp |
| 美國 | 67% | 48% | 16% | +3 pp |
| 英國 | 62% | 48% | 32% | +6 pp |
| 菲律賓 | 24% | 18% | 2% | +2 pp |
| 墨西哥 | 16% | 11% | 6% | +4 pp |
| 印尼 | 14% | 5% | 4% | +1 pp |
| **全體** | **57.1%** | **34.8%** | **11.0%** | **+2.1 pp** |

另外兩個關鍵交叉數據：

- **品質覆寫潛力**：+2.9% 的樣本 Wikidata 只有通用 zh label（常為大陸譯名
  ／簡體）而 NAER 有官方臺灣譯名，可作品質覆寫。
- **GeoNames alternateNames**：同批樣本命中 10.2%，但獨有救回僅 +0.9 pp，
  與 Wikidata 高度重疊且多為簡體。

### GeoNames alternateNames 的 zh 系分布（cities500 全量）

| 語言碼 | 筆數 | 備註 |
|---|---:|---|
| `zh` | 49,038 | 簡繁混雜，與 Wikidata 高度重疊 |
| `zh-Hant` | 1,338 | 全球僅此數量，絕大多數國家 <1% |
| `zh-TW` | 283 | 幾乎不存在 |

各國 cities500 的 `zh-Hant` + `zh-TW` 填充率：美國 3.3% 已是最高，
其餘多數國家低於 1%。**作為繁體補洞來源的假設不成立。**

### NAER 對 handler 國家的適用性

| 測試 | 結果 |
|---|---|
| 泰國 5,665 個未翻譯（fallback 英文）tambon 級地名 | 僅救回 1.0% |
| 日本／韓國 geodata 未翻譯地名 | 0 筆（無洞可補） |

NAER 對 handler 國家的細層級行政區無效，且存在 feature type 錯配風險
（如 `Ban Don → 班洞灣`，譯名實為海灣而非城鎮）。**NAER 僅適用於
非 handler 國家的城市級資料。**

### NAER 的 admin1 層級覆蓋

美國 96%、墨西哥 97%、巴西 85%、泰國 73%；但法國 31%、菲律賓 12%
（漏失多因 GeoNames 使用英語外名如 Tuscany / Saxony，而 NAER 收錄
當地語名）。admin1 本來就是 Wikidata 覆蓋最強的層級，NAER 在此層為
輔助性質。

### 最重要的發現：全球非 handler 資料的實際基線

審視 `translate_cities_rows` / `translate_admin1_rows` 與 release
workflow 後確認：**正式 release 只跑 4 個 handler 國家（TW/JP/KR/TH），
LocationIQ 流程不在 production release 中**。全球其餘所有國家的
cities500 與 admin1 記錄，中文名僅來自 GeoNames alternateNames
（priority：zh-Hant > zh-TW > zh-HK > zh > zh-Hans > zh-CN > zh-SG）
與 cities500 內嵌中文，再經 OpenCC 轉換——**完全沒有 Wikidata 參與**。

因此對這批資料（佔 release 絕大多數），比較基線不是 Wikidata 的
57.1%，而是 GeoNames alternateNames 的實際覆蓋。全量實測
（非抽樣）：

| 層級 | 記錄數 | 現行有中文名 | NAER 獨有救回 | 救回後覆蓋 |
|---|---:|---:|---:|---:|
| cities500（非 handler 全球） | 229,760 | 46,290（20.1%） | **+16,380（+7.1 pp）** | 27.3%（相對 +35%） |
| admin1（非 handler 全球） | 3,720 | 2,211（59.4%） | **+494（+13.3 pp）** | 72.7% |

cities500 救回幅度最大的國家：英國 +18.2 pp、荷蘭 +14.0 pp、西班牙
+13.7 pp、加拿大 +12.3 pp、義大利 +10.7 pp、美國 +9.8 pp。admin1
救回包含高曝光條目（如 `Dubai → 杜拜`、`Andorra la Vella →
老安道爾`——現行 release 中這些 admin1 無中文名）。

另有 9,415 筆 cities500 記錄現行已有中文（多來自 `zh` 簡體列＋OpenCC
機械轉換）且 NAER 也有官方臺灣譯名，可作品質覆寫。

注意：admin1 救回估算為名稱比對（`admin1CodesASCII.txt` 無座標），
其中 90 筆有多譯名歧義，實作時需以國家對應或 geonameid 主表座標
消歧。

## 結論

依資料路徑分別評估：

1. **handler 國家（TW/JP/KR/TH）**：Wikidata translator 不可取代
   （實證確認），且 NAER 對細層級行政區無效（泰國 tambon 救回僅
   1.0%）。**此路徑維持現狀。**
2. **全球非 handler 資料（release 絕大多數記錄）**：這是 NAER 價值
   最大的地方。現行基線僅 GeoNames alternateNames（cities 20.1%、
   admin1 59.4%），NAER 可帶來 cities +7.1 pp（相對 +35%）、admin1
   +13.3 pp 的覆蓋提升，外加 9,415 筆品質覆寫。**建議將 NAER 接入
   translate 階段**，優先序：`NAER 官方譯名 > GeoNames alternateNames
   zh 系 + OpenCC > 既有 fallback`。
3. **若未來以 Wikidata 擴大全球翻譯**，NAER 仍有價值但幅度縮小：
   對比 Wikidata 57.1% 基線，NAER 補洞 +2.1 pp、品質覆寫 +2.9 pp。
   兩者可疊加（NAER 覆寫 > Wikidata > GeoNames）。
4. **GeoNames alternateNames 已是現行來源，無需額外動作**；其 zh-Hant
   列近乎不存在（全球僅 1,338 筆），不能期待其作為繁體補洞來源。
5. **OSM、CLDR、樂詞網均排除**（授權或範圍不符）。
6. **NAER 授權**：OGDL 1.0 與 CC BY 4.0 相容，發佈物需保留
   attribution。
7. **低覆蓋國家在現有開放資料生態中無解**：墨西哥、印尼、菲律賓等
   區域，Wikidata＋NAER＋GeoNames 聯集仍僅 15–26%。未來唯一可行方向
   為 LLM 批次翻譯＋人工審核，或接受現行 fallback。

## NAER 覆寫層實作要件（供後續規劃參考）

實測驗證過的工程前提：

- **座標 parser**：需處理 HTML entities（`&deg;`）、多種撇號字元、度分制
  轉十進位；可達 99.4% 解析率。
- **名稱正規化**：去除 `[...]` 與 `(...)` 註記、逗號倒裝（`Paris, Ville
  de`）、變音符號折疊。
- **座標驗證**：名稱匹配後必須以 ≤15 km 容差驗證，消除同名歧義與
  feature type 錯配。
- **譯名清理**：3.5% 譯名含括號註記需剝離；自然地物後綴（河／灣／島）
  條目需排除或降權。
- **適用範圍**：僅非 handler 國家；handler 國家（研究當時為
  TW/JP/KR/TH，清單由 extract handler 單一來源決定，後續新增者自動
  跳過）不適用。

## 資料來源

- 國教院《外國地名譯名》：https://data.gov.tw/dataset/15211
  （CSV 下載：opendata.naer.edu.tw，OGDL 1.0）
- GeoNames dumps：https://download.geonames.org/export/dump/
  （cities500.zip、alternateNamesV2.zip、admin1CodesASCII.txt，CC BY 4.0）
- Wikidata API：https://www.wikidata.org/w/api.php（CC0）
- 業界混合策略參考：Who's On First 多來源 concordance 模式
  （https://whosonfirst.org/blog/2017/08/22/summer-2017-wof/）
