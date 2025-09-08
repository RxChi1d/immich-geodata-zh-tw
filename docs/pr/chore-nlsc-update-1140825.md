# chore(nlsc): 更新台灣村里界與中心點至 NLSC 1140825

## 摘要
- 使用國土測繪中心（NLSC）1140825 版本之「村(里)界 (TWD97經緯度)」資料，更新專案內建地理資料。
- 同步調整說明與預設 shapefile 路徑，確保以最新資料為基準重新產生輸出。

## 變更內容
- core/taiwan_geodata.py
  - 說明文字與範例路徑由 `1140620` 更新為 `1140825`。
  - 預設 `--shapefile` 路徑改為 `geoname_data/VILLAGE_NLSC_1140825/VILLAGE_NLSC_1140825.shp`。
- meta_data/taiwan_geodata.csv
  - 以 1140825 版本重新計算（或同步）村里多邊形中心點（WGS84）與輸出。
  - 約 10,524 行差異（5,274 新增／5,274 刪除），主要為座標更新與排序調整；欄位結構不變。

## 資料來源與版本
- 來源：國土測繪中心（NLSC）開放資料
  - 開放資料集名稱：村(里)界 (TWD97經緯度)
  - 下載頁面：https://whgis-nlsc.moi.gov.tw/Opendata/Files.aspx
- 本次使用版本：`1140825`
- 目錄結構（範例）
  ```
  geoname_data/
    └── VILLAGE_NLSC_1140825/
        ├── 修正清單_1140825.xlsx
        ├── TW-07-301000100G-613995.xml
        ├── VILLAGE_NLSC_1140825.{CPG,dbf,prj,shp,shx}
        ├── Village_Sanhe.{CPG,dbf,prj,shp,shx}
        └── Village_Xinyi.{CPG,dbf,prj,shp,shx}
  ```

## 驗證方式（建議）
1. 檔案就緒
   - 確認 `geoname_data/VILLAGE_NLSC_1140825/` 已完整解壓，含 `.shp/.dbf/.shx/.prj/.cpg` 等檔案。
2. 重新產出 CSV
   - 使用預設路徑直接執行：
     - `python core/taiwan_geodata.py`
   - 或手動指定路徑：
     - `python core/taiwan_geodata.py --shapefile geoname_data/VILLAGE_NLSC_1140825/VILLAGE_NLSC_1140825.shp`
   - 產出後比對 `meta_data/taiwan_geodata.csv` 差異應與本 PR 內容一致。
3. 抽樣比對
   - 隨機抽查多個村里，以 GeoPandas 或 GIS 工具對多邊形求中心點，與 CSV 經緯度比對（誤差在數公尺內屬合理）。
4. 坐標與欄位確認
   - 確認輸出為 WGS84（EPSG:4326）。
   - CSV 欄位標頭與順序未變更：`longitude,latitude,admin_1,admin_2,admin_3,admin_4,country`。

## 向後相容性
- 僅更新資料內容與 `core/taiwan_geodata.py` 的預設輸入與文字說明，未更動輸出格式與欄位結構。
- 依據最新 NLSC 幾何，村里中心點可能發生微幅調整；若有依賴固定排序的流程，可能需同步調整。

## 風險與影響
- 資料更新造成部分中心點經緯度變動，可能影響以既有座標進行比對或快取的流程。
- 如外部流程依賴 CSV 行順序，需注意此次重排（若有）。

## 檢查清單
- [ ] `VILLAGE_NLSC_1140825` 資料已完整下載並解壓。
- [ ] 本機以 1140825 資料成功重新產生 CSV 並比對差異。
- [ ] 隨機抽查 ≥ 5 個村里中心點與 GIS 計算結果一致。
- [ ] 若文件需對外標示資料版號，已更新（README/CHANGELOG 等）。

## 其他
- 分支：`chore/nlsc-update-1140825`
- 建議合併方式：Squash & merge（建議提交訊息：`chore(nlsc): update Taiwan geodata to 1140825`）

