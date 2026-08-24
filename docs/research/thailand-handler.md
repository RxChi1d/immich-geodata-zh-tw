# 泰國 COD-AB 圖資 handler 評估

> **文件狀態：歷史紀錄**
> 本文記錄當時的研究與決策，不保證與目前實作同步。現行行為請參閱[泰國行政區處理](../zh-tw/thailand-admin-processing.md)。
> 部分實作後的變更未涵蓋於本文，例如 Admin 2 的 P131 行政隸屬驗證。

## 資料來源

- 資料集：Thailand - Subnational Administrative Boundaries (`cod-ab-tha`)
- 網址：https://data.humdata.org/dataset/cod-ab-tha
- 來源：Royal Thai Survey Department
- 授權：Creative Commons Attribution for Intergovernmental Organisations (CC BY-IGO)
- Metadata 修改時間：2026-04-24
- 圖資資源更新時間：2026-01-26
- 資料有效區間：2022-01-22 至 2025-10-30

## 可用格式

| 格式 | 檔案 | 壓縮大小 | 解壓後主要內容 |
|---|---:|---:|---|
| SHP | `tha_admin_boundaries.shp.zip` | 360 MB | `tha_admin0`、`tha_admin1`、`tha_admin2`、`tha_admin3`、`tha_adminlines`、`tha_adminpoints` |
| GeoJSON | `tha_admin_boundaries.geojson.zip` | 417 MB | `tha_admin0.geojson`、`tha_admin1.geojson`、`tha_admin2.geojson`、`tha_admin3.geojson`、`tha_adminlines.geojson`、`tha_adminpoints.geojson` |

建議以 SHP 作為主要輸入，理由是壓縮體積較小，且目前 Rust extract runtime 已支援 `.shp`。GeoJSON 可作為備援格式，但 `tha_admin3.geojson` 解壓後約 701 MB，讀取成本較高。

## 行政層級

HDX metadata 記錄的層級如下：

- Admin 1：77 Province
- Admin 2：928 District
- Admin 3：7425 Sub-district / Tambon

實測 `tha_admin3.shp`：

- 筆數：7425
- CRS：EPSG:4326
- Geometry：7314 Polygon、111 MultiPolygon
- 編碼：UTF-8
- `adm3_pcode` 無重複
- 主要欄位無缺值：`adm1_name`、`adm1_name1`、`adm2_name`、`adm2_name1`、`adm3_name`、`adm3_name1`、`center_lat`、`center_lon`

## 欄位映射建議

`tha_admin3` 的欄位同時提供英文與泰文：

| 輸出欄位 | 來源欄位 | 說明 |
|---|---|---|
| `latitude` | 幾何 centroid | 使用投影後 polygon centroid |
| `longitude` | 幾何 centroid | 使用投影後 polygon centroid |
| `country` | 固定 `泰國` | 專案使用 zh-tw 國名 |
| `admin_1` | Wikidata / `adm1_name` / `adm1_name1` | Province 繁中翻譯，缺少中文時回退官方英文與泰文 |
| `admin_2` | Wikidata / `adm2_name` / `adm2_name1` | District 繁中翻譯，缺少中文時回退官方英文與泰文 |
| `admin_3` | `adm3_name` / `adm3_name1` | 官方英文 sub-district / tambon 名稱，英文缺少時回退官方泰文 |
| `admin_4` | `None` | COD-AB 此資料集無 admin4 |

Admin 1 / Admin 2 使用 Rust Wikidata translator。名稱優先級為 `zh-tw`、`zh-hant`、`zh` + OpenCC、`zhwiki` title conversion、COD-AB 官方英文、COD-AB 官方泰文。這個順序不使用 Wikidata 英文或泰文 fallback，因為 COD-AB 已提供官方英文與泰文，缺少中文時應回到官方來源。

Admin 3 目前不建立 Wikidata cache。原因是 `tha_admin3` 有 7425 筆 sub-district / tambon，低層級地名在 Wikidata 中更容易出現同名錯配、缺漏或非行政區候選；在未有更完整驗證前，保留官方英文比大量自動翻譯更穩定。

## 座標策略

泰國資料已在 polygon layer 內提供 `center_lat` / `center_lon`，且 `tha_adminpoints` level 3 的 `x_coord` / `y_coord` 與這兩欄完全一致。

實測以現有 JP/KR 類似的動態 UTM polygon centroid 重新計算 `tha_admin3` 中心點，與來源中心點差異如下：

- 最小：約 5.25 公尺
- 中位數：約 583.66 公尺
- 95 百分位：約 2235.21 公尺
- 最大：約 36214.25 公尺

最大差異出現在離島或不規則區域，例如 Satun / Mueang Satun / Ko Sarai。這代表 `center_lat` / `center_lon` 與幾何 centroid 的語意不同，需要依 Immich 的最近點查詢模型另做驗證，而不能只看欄位來源判斷。

### Centroid 差異原因

差異不是因為泰國投影設定不佳。以多種方法重算 `tha_admin3` polygon centroid，與官方 `center_lat` / `center_lon` 的差距都幾乎相同：

| 方法 | 中位數 | 95 百分位 | 最大值 |
|---|---:|---:|---:|
| EPSG:4326 直接 centroid | 583.68 m | 2235.74 m | 36215.41 m |
| Web Mercator EPSG:3857 | 583.64 m | 2236.20 m | 36217.12 m |
| Thailand Albers | 583.68 m | 2235.23 m | 36211.72 m |
| South Asia / Thailand Albers | 583.68 m | 2235.23 m | 36211.75 m |
| 動態 UTM 47N/48N | 583.66 m | 2235.21 m | 36214.25 m |

各投影算出的幾何 centroid 彼此非常接近。以動態 UTM 為基準：

- Thailand Albers 與動態 UTM 的中位差約 0.06 m，最大約 4.94 m。
- South Asia / Thailand Albers 與動態 UTM 的中位差約 0.06 m，最大約 5.04 m。
- Web Mercator 與動態 UTM 的中位差約 0.41 m，最大約 50.98 m。

因此，`center_lat` / `center_lon` 和重算 centroid 的差距主要不是投影最佳化問題，而是欄位語意不同：COD-AB 的 `center_lat` / `center_lon` 更像是資料來源提供的行政區代表點 / label point，而不是純幾何 centroid。

另外，官方點位全部落在對應 polygon 內；動態 UTM 幾何 centroid 則有 83 筆落在 polygon 外。這在離島、多面、多凹形行政區中特別明顯，代表純幾何 centroid 並不一定適合反向地理編碼的代表座標。

### 最近點命中率驗證

Immich 使用 EXIF GPS 對 cities500 單點做最近點查詢，因此真正要驗證的是「哪一種單點能讓 polygon 內的 GPS 取樣點更常命中原行政區」。本次使用 Thailand Albers 投影座標，以 `tha_admin3` polygon 內的取樣點作為真實 GPS，分別比較：

- 官方 `center_lat` / `center_lon`
- 投影後 polygon 幾何 centroid
- Shapely `representative_point()`

固定每個 polygon 取樣 20 個隨機內點，並加入 1 個 `representative_point()`，共 155925 個測試點：

| 候選點策略 | 整體命中率 | 隨機內點命中率 | Representative point 命中率 |
|---|---:|---:|---:|
| 官方 `center_lat` / `center_lon` | 74.30% | 73.02% | 99.97% |
| 投影後幾何 centroid | 76.18% | 75.01% | 99.56% |
| Shapely `representative_point()` | 74.31% | 73.02% | 100.00% |

面積加權取樣約 20 萬點，讓大面積行政區取得更多測試點後：

| 候選點策略 | 命中率 |
|---|---:|
| 官方 `center_lat` / `center_lon` | 70.07% |
| 投影後幾何 centroid | 71.84% |
| Shapely `representative_point()` | 70.08% |

在這兩種取樣方式下，幾何 centroid 都比官方代表點更適合 Immich 的最近點模型。官方點全部落在 polygon 內，但它更接近 label point / 行政代表點；若目標是讓 polygon 內任意 GPS 以最近點回到原行政區，投影後幾何 centroid 的整體誤判率較低。

因此，Thailand handler 的預設座標策略建議使用適合泰國的投影流程計算 polygon centroid。`center_lat` / `center_lon` 可保留為替代策略、fixture 對照或日後若要支援「行政代表點」模式時使用。

### Dynamic UTM 效益驗證

為確認是否需要像 JP/KR 一樣做 dynamic UTM，再比較兩種 centroid 策略：

- **Thailand Albers 直接 centroid**：全國統一轉到 Thailand Albers 後計算 centroid。
- **Dynamic UTM centroid**：先用 Thailand Albers 判斷 polygon 中心經度，再分別轉到 UTM 47N / 48N 計算 centroid。

兩者產生的中心點非常接近：

- 中位差：0.064 m
- 95 百分位：0.425 m
- 最大差：4.941 m

最近點命中率比較如下：

| 取樣方式 | Thailand Albers 直接 centroid | Dynamic UTM centroid | 最近行政區不同筆數 |
|---|---:|---:|---:|
| 每區 20 點 + representative point，共 155925 點 | 76.1834% | 76.1821% | 5 |
| 面積加權約 20 萬點，共 200127 點 | 71.8379% | 71.8359% | 9 |

結論：Dynamic UTM 對泰國 `tha_admin3` 的最近點反向地理解析準確度沒有實質提升。若以精簡與效能為優先，Thailand handler 可直接使用 Thailand Albers centroid；若以和 JP/KR runtime 設計一致為優先，使用 Dynamic UTM 也不會造成明顯差異，但不能期待它帶來可觀準確度改善。

## Rust runtime 設計

現有 Rust extract runtime 的國家分派需要擴充：

- `Country::Thailand`
- `FeatureAttributes::Thailand`
- `Country::parse("TH")`
- `Country::code()`
- `Country::extract_attribute_keys()`
- `Country::load_context()`
- `Country::rows_from_features()`

目前 `apply_country_centroids()` 會依國家對所有 geospatial input 計算 centroid。Thailand 可以沿用這條路徑，但 benchmark 顯示 dynamic UTM 對泰國沒有實質準確度提升。建議新增可直接使用投影 CRS 的 centroid pipeline，或讓 Thailand 使用既有 `DynamicUtm` 但在文件中註明效益有限：

```rust
enum CentroidPipeline {
    ProjectedEpsg(i32),
    ProjectedProj4(&'static str),
    DynamicUtm(&'static str),
}
```

Thailand 的建議預設：

```rust
CentroidPipeline::ProjectedProj4(THAILAND_ALBERS_PROJ4)
```

若為了與 JP/KR 風格一致，也可使用：

```rust
CentroidPipeline::DynamicUtm(THAILAND_ALBERS_PROJ4)
```

建議的 Thailand Albers 設定：

```text
+proj=aea +lat_1=5 +lat_2=21 +lat_0=13 +lon_0=101 +x_0=0 +y_0=0 +datum=WGS84 +units=m +no_defs
```

若日後要支援官方代表點模式，可以再新增 `SourceFields` centroid pipeline，但不建議作為 Immich 最近點模型下的預設。

`extract` 可讀 `tha_admin3.shp` 或 `tha_admin3.geojson`，流程為：

1. 讀取來源圖資，並依 `Country::Thailand` 驗證必要欄位存在：`adm1_name`、`adm1_name1`、`adm2_name`、`adm2_name1`、`adm3_name`、`adm3_name1`。
2. 使用 Thailand Albers 計算 polygon centroid，產生 `latitude` / `longitude`。
3. 載入或建立 `geoname_data/TH_wikidata_cache.json`，將 Admin 1 / Admin 2 轉為繁中；若找不到中文結果，回退至 COD-AB 官方英文與泰文。
4. 將欄位轉為標準 geodata schema：`country` 固定 `泰國`，`admin_1` / `admin_2` 使用翻譯結果，`admin_3` 使用 COD-AB 官方英文，`admin_4` 留空。
5. 交由既有 normalized geodata 與 cities schema 流程產生後續 release 使用的資料。

cities500 的 `name` 目前會沿用既有 handler 行為，以 `admin_2` 作為主要顯示名稱，也就是 District。這與臺灣 handler 使用村里 polygon 提升定位密度、但以鄉鎮市區作為顯示名稱的策略一致。若未來希望 Immich 顯示 Tambon/Sub-district，才需要另行調整 cities schema 對 handler geodata 的名稱選擇規則。

## 建議實作順序

1. 新增 Rust `Country::Thailand`、欄位解析與 Thailand Albers centroid pipeline。
2. 建立小型 parity fixture，至少包含：
   - Bangkok 一般都會區。
   - Ko Sarai 這類官方中心點與 polygon centroid 差異大的離島/多面區。
   - 同英文名但泰文不同的 tambon，例如 `Mae Kha`。
3. 跑 extract fixture，確認 TH normalized CSV row count 與座標排序穩定。
4. 跑 Rust gates：`cargo fmt --check`、`cargo clippy -- -D warnings`、`cargo test`。

## 風險與待決策

- 名稱語言：Admin 1 / Admin 2 已接 Wikidata 繁中翻譯；Admin 3 仍使用官方英文，若未來要翻譯 Admin 3，需另做候選過濾與抽樣驗證。
- 顯示粒度：建議仍以 `admin_2` 作為 cities500 `name`。若改用 `admin_3`，需評估 Immich 顯示與 GeoNames schema 相容性。
- ZIP 輸入：現有 CLI 接收 `.shp` 或 `.geojson`，不直接處理 COD-AB ZIP。下載後仍需先解壓出 `tha_admin3.shp` 或 `tha_admin3.geojson`。
- 座標策略：若使用者照片高度集中在行政中心或聚落，官方代表點可能有另一種實務價值；但以 polygon 內取樣點的最近點命中率驗證，幾何 centroid 較適合目前 Immich 單點最近查詢模型。
- Admin1 mapping：目前專案會依 `admin_1` 名稱排序自動產生 `TH.01` 到 `TH.77`，不是使用 COD-AB `adm1_pcode`。若要保留官方 pcode，需要擴充 geodata schema 或覆寫 mapping 生成邏輯。
