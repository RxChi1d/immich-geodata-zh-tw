# 泰國行政區處理邏輯

> 本文件說明本專案如何處理泰國地區的地理資訊，是 README 中泰國優化章節的詳細版本。

## 資料來源

本專案針對泰國地區的地理資訊處理，以 **Thailand COD-AB** 的官方行政區邊界資料為核心：

- **來源**：[HDX Thailand - Subnational Administrative Boundaries](https://data.humdata.org/dataset/cod-ab-tha)
- **資料提供**：Royal Thai Survey Department，經 OCHA / HDX 發布
- **資料集**：Thailand administrative level 0-3 boundaries (COD-AB)
- **授權**：Creative Commons Attribution for Intergovernmental Organisations (CC BY-IGO)
- **用途**：作為泰國地區行政區邊界與名稱的主要資料來源

## 行政區層級

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

## 名稱策略

泰國 handler 沿用南韓 handler 的 Wikidata translator 流程，但因 COD-AB 同時提供官方英文與官方泰文名稱，因此每筆翻譯 item 會額外保存官方 fallback。Admin 1 與 Admin 2 的名稱優先級如下：

1. Wikidata `zh-tw` label
2. Wikidata `zh-hant` label
3. Wikidata `zh` label，並透過 OpenCC 轉為繁體中文
4. `zhwiki` title conversion
5. COD-AB 官方英文欄位：`adm1_name` 或 `adm2_name`
6. COD-AB 官方泰文欄位：`adm1_name1` 或 `adm2_name1`

此順序刻意不把 Wikidata 英文或泰文 label 放入 fallback。原因是 COD-AB 已提供官方英文與泰文，若中文資料不存在，應優先回到官方來源，而不是使用 Wikidata 上可能不一致的英文或泰文別名。

目前 Admin 3 不建立 Wikidata cache，主要原因是 `tha_admin3` 有 7425 筆 sub-district / tambon，翻譯成本與歧義風險較高。Admin 3 會先使用官方英文 `adm3_name`，缺少時才使用官方泰文 `adm3_name1`。

Wikidata cache 位置為：

```text
geoname_data/TH_wikidata_cache.json
```

Fixture 測試會使用 `TH_wikidata_stub.json`，避免測試依賴即時網路查詢。

## 座標策略

泰國 COD-AB 的 polygon layer 內提供 `center_lat` / `center_lon`，且這兩欄與 `tha_adminpoints` 的官方點位一致。不過本專案**不使用這兩欄作為預設座標**。

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

## 資料提取流程

```bash
# 1. 從 HDX 下載 tha_admin_boundaries.shp.zip
# 2. 解壓縮並使用 tha_admin3.shp
cargo run --release --manifest-path rust/Cargo.toml -- extract --country TH \
  --shapefile path/to/tha_admin3.shp \
  --output meta_data/th_geodata.csv
```

GeoJSON 格式也可使用：

```bash
cargo run --release --manifest-path rust/Cargo.toml -- extract --country TH \
  --shapefile path/to/tha_admin3.geojson \
  --output meta_data/th_geodata.csv
```

## 注意事項

- `center_lat` / `center_lon` 保留為資料來源參考，不作為預設座標來源。
- 若未來需要改成行政代表點模式，應新增明確的座標策略選項，而不是覆蓋目前的最近距離最佳化策略。
- 泰國 Admin 1 / Admin 2 會使用 Wikidata 繁中翻譯；若 Wikidata 沒有可靠中文結果，會回退至 COD-AB 官方英文與官方泰文。
- 泰國 Admin 3 目前保留 COD-AB 官方英文，避免大量低層級地名在 Wikidata 中出現錯配或不穩定翻譯。
