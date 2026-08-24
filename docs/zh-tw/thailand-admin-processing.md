# 泰國行政區處理邏輯

> 本文件說明本專案如何處理泰國地區的地理資訊，是 [README 支援地區與語言策略](../../README.md#支援地區與語言策略) 的詳細版本。

## 資料來源

本專案針對泰國地區的地理資訊處理，以 **Thailand COD-AB** 的官方行政區邊界資料為核心：

- **來源**：[HDX Thailand - Subnational Administrative Boundaries](https://data.humdata.org/dataset/cod-ab-tha)
- **資料提供**：Royal Thai Survey Department，經 OCHA / HDX 發布
- **資料集**：Thailand administrative level 0-3 boundaries (COD-AB)
- **授權**：Creative Commons Attribution for Intergovernmental Organisations (CC BY-IGO)
- **用途**：作為泰國地區行政區邊界與名稱的主要資料來源

## 行政區層級

> [!NOTE]
> `admin_3` 與 `admin_4` 只存在於本專案的中介 CSV，用於保留來源行政層級供追溯與除錯，不會輸出到 Immich 使用的 cities500。Immich 顯示的最細層級是 `admin_2`。代表點密度取決於 extract 的來源 feature 顆粒度（泰國為 tambon），與這兩個欄位無關。


COD-AB Thailand 提供下列行政層級：

- **Admin 1**：Province
- **Admin 2**：District
- **Admin 3**：Sub-district / Tambon

本專案使用 `tha_admin3` 作為 extract 來源，輸出欄位如下：

| 輸出欄位 | 來源欄位 | 說明 |
|---|---|---|
| `country` | 固定值 | `泰國` |
| `admin_1` | Wikidata / `adm1_name` / `adm1_name1` | Province 繁中翻譯；缺少中文時回退官方英文、官方泰文 |
| `admin_2` | Wikidata / `adm2_name` / `adm2_name1` | District 繁中翻譯；缺少中文時回退官方英文、官方泰文 |
| `admin_3` | `adm3_name` / `adm3_name1` | Sub-district / Tambon 官方英文；英文缺少時回退官方泰文 |
| `admin_4` | 空值 | COD-AB 此資料集未提供 admin4 |

> **筆數說明**：泰國 extract 為每個 feature 輸出一列，`meta_data/th_geodata.csv` 的列數與 `tha_admin3` 的 tambon 數一致。multipart polygon 會合併計算單一中心點，不會拆成多列（逐 part 拆列只在印尼啟用，詳見[印尼行政區處理](indonesia-admin-processing.md)）。

## 名稱策略

泰國 handler 沿用南韓 handler 的 Wikidata translator 流程，但因 COD-AB 同時提供官方英文與官方泰文名稱，因此每筆翻譯 item 會額外保存官方 fallback。名稱決策分為兩個層次：**先決定是否信任 Wikidata 結果，再決定採用哪一個語言的 label。**

### 第一層：是否信任 Wikidata 結果（P131 行政隸屬驗證）

依 Wikidata translator 的標準規則，**Admin 1 與 Admin 2 都必須通過 P131
（`located in the administrative territorial entity`）鏈驗證**，逐級對
「已知最特定的上層」裁決同名歧義：

| 層級 | parent QID | P131 驗證 | 無法通過驗證時 |
|---|---|---|---|
| **Admin 1**（Province） | 泰國（`Q869`） | 每個候選都需通過 | 進入泰文後備搜尋；仍失敗則回退官方英文 / 泰文 |
| **Admin 2**（District） | 所屬 Province 的 QID | 每個候選都需通過 | 進入泰文後備搜尋；仍失敗則回退官方英文 / 泰文 |

Admin 2 的 parent QID 來自第一層 Admin 1 翻譯所解析出的 Province QID：

- **沒有任何候選通過驗證** → 不採用任何 Wikidata 候選（不存在「退而求其次拿第一個候選」的路徑），改走泰文後備搜尋或官方名稱回退。
- **快取中的舊結果** → 僅在「parent QID 與本次一致且已驗證（或當時已放棄）」時沿用；parent 上下文變更（例如修正了上層 QID）會觸發重新查詢，避免錯誤翻譯被快取固化。
- **上層 QID 未解析** → 該府底下的 Admin 2 第一輪改以泰國 `Q869` 作為驗證樓地板，並且不進入泰文後備搜尋，直接回退官方名稱，避免跨府同名縣誤配。以目前資料，77 個府全數解析出 QID，此路徑不會被觸發。

> [!NOTE]
> 此驗證避免了同名或近似名的錯配。例如英文搜尋 `Nan` 的前 7 名候選全是無關實體（南特、南錫、閩南語等），P131 驗證會全數拒絕而非盲選第一名；Chiang Mai 府下的 `Fang` 也不會被誤接到無關實體而翻成「方」。當驗證無法確認隸屬關係時，保守回退，寧可顯示英文也不顯示錯誤中文。

### 搜尋語言：英文為主、泰文為後備

搜尋分兩輪進行：

1. **第一輪（英文）**：以 COD-AB 官方英文名（`adm1_name` / `adm2_name`）搜尋，候選逐一做 P131 驗證。
2. **第二輪（泰文後備）**：第一輪驗證失敗的 item，改以官方泰文名（`adm1_name1` / `adm2_name1`）搜尋，並加上 instance-of 類別過濾（Admin 1 限「泰國府」`Q50198`；Admin 2 限「縣」`Q475061` 與「曼谷轄區」`Q15634531`），通過 P131 驗證才採用。

> [!IMPORTANT]
> 「英文為主」是經實驗驗證的選擇，不是預設慣性。以 125 個已驗證縣為樣本
> 的對照實驗顯示：正確實體出現在搜尋前 7 名的比率，英文搜尋為 100%，泰文
> 搜尋僅 4%（75 個 เมืองX 首府縣）至 12%（50 個隨機縣）——泰國縣級實體的
> 英文標籤（`X District` 形式）在 Wikidata 上的鑑別度遠高於泰文裸名稱。
> 反之，府級的泰文搜尋鑑別度極佳（5/5 歧義府名均以第一名命中正確實體），
> 因此泰文適合作為驗證後備而非主要語言。上述數字取自開發期間的離線對照實
> 驗，樣本與腳本未隨程式碼保存（`docs/research/thailand-handler.md` 只收錄
> 座標與投影實驗）。

### 第二層：語言 label 優先序

當一筆 item 決定採用 Wikidata 結果後（Admin 2 須先通過第一層驗證），依下列順序挑選名稱：

1. Wikidata `zh-tw` label
2. Wikidata `zh-hant` label
3. Wikidata `zh` label，並透過 OpenCC 轉為繁體中文
4. `zhwiki` title conversion
5. COD-AB 官方英文欄位：`adm1_name` 或 `adm2_name`
6. COD-AB 官方泰文欄位：`adm1_name1` 或 `adm2_name1`

此順序刻意不把 Wikidata 英文或泰文 label 放入 fallback。原因是 COD-AB 已提供官方英文與泰文，若中文資料不存在，應優先回到官方來源，而不是使用 Wikidata 上可能不一致的英文或泰文別名。

採用譯名前另有形態守門：若 Wikidata 譯名為中英夾雜的半翻譯（同時含漢字與拉丁字母，例如「西Kutai區」形式），視為髒資料而不採用，直接回退 COD-AB 官方英文。純英文譯名則是泰國設計內的合法輸出，不受此規則影響。

### Admin 3 名稱

目前 Admin 3 不建立 Wikidata cache，主要原因是 `tha_admin3` 有大量 sub-district / tambon，翻譯成本與歧義風險較高。Admin 3 會先使用官方英文 `adm3_name`，缺少時才使用官方泰文 `adm3_name1`。

### Wikidata cache

Wikidata cache 位置為：

```text
geoname_data/TH_wikidata_cache.json
```

Fixture 測試會使用 `TH_wikidata_stub.json`，避免測試依賴即時網路查詢。

## 座標策略

泰國 COD-AB 的官方代表點位由 `tha_adminpoints` layer 提供（對應 polygon 屬性中的 `center_lat` / `center_lon`，兩者一致）。不過本專案**不使用官方代表點作為預設座標**。

原因是 Immich 的反向地理解析使用單點最近距離模型。實測以 `tha_admin3` polygon 內取樣點作為真實 GPS，分別比較官方代表點與幾何中心點後，幾何中心點的整體命中率較高：

| 取樣方式 | 官方代表點 | 幾何中心點 |
|---|---:|---:|
| 每區 20 點 + representative point，共 155925 點 | 74.30% | 76.18% |
| 面積加權約 20 萬點，共 200127 點 | 70.07% | 71.84% |

因此，泰國 handler 預設使用 polygon 幾何中心點，讓 cities500 單點更接近 Immich 最近距離查詢模型。

## 投影策略

泰國 handler 使用 Thailand Albers 投影計算 centroid：

```text
+proj=aea +lat_1=5 +lat_2=21 +lat_0=13 +lon_0=101 +x_0=0 +y_0=0 +datum=WGS84 +units=m +no_defs
```

此策略沒有使用日本與南韓 handler 的 dynamic UTM 流程。原因是針對泰國 `tha_admin3` 實測後，dynamic UTM 不會帶來實質準確度提升：

| 取樣方式 | Thailand Albers | Dynamic UTM |
|---|---:|---:|
| 每區 20 點 + representative point，共 155925 點 | 76.1834% | 76.1821% |
| 面積加權約 20 萬點，共 200127 點 | 71.8379% | 71.8359% |

兩種 centroid 本身的中位差約 0.064 公尺，95 百分位約 0.425 公尺，最大差約 4.941 公尺。基於準確度、效能與實作簡潔性，泰國採用 Thailand Albers 直接計算 centroid。

## 注意事項

- `center_lat` / `center_lon`（官方代表點）保留為資料來源參考，不作為預設座標來源。
- 若未來需要改成行政代表點模式，應新增明確的座標策略選項，而不是覆蓋目前的最近距離最佳化策略。
- 泰國 Admin 1 / Admin 2 會使用 Wikidata 繁中翻譯；兩級都須通過 P131 行政隸屬驗證（Admin 1 對泰國 `Q869`、Admin 2 對所屬府的 QID），驗證失敗會先嘗試泰文後備搜尋，仍失敗或 Wikidata 沒有可靠中文結果時，回退至 COD-AB 官方英文與官方泰文。
- 泰國 Admin 3 目前保留 COD-AB 官方英文，避免大量低層級地名在 Wikidata 中出現錯配或不穩定翻譯。
- 在本機重現提取流程的指令請見[本地資料處理](development.md#2-提取原始地理資料)；`--shapefile` 同時支援 `.shp` 與 `.geojson` / `.json`。
