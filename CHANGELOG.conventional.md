# Conventional Changelog

## [未發佈版本]

### Features
- **handler registry**: 建立 `register_handler` 與 `get_handler` 等函式，提供依國碼載入專用處理器的機制並整合 CLI `extract`、`enhance` 流程。
- **japan support**: 新增 `JapanGeoDataHandler` 與日本專屬 CSV，提供官方行政區 Shapefile 的 ETL 流程。
- **enhance workflow**: 引入 `update_geodata()`，同時更新 admin1 與 cities500，並導入全球 geoname ID 管理。

### Bug Fixes
- **enhance output**: 寫入前建立輸出資料夾，避免因路徑不存在造成輸出失敗。
- **japan data format**: 統一 admin_3 欄位空值表示方式（從 None 改為空字串），確保 Extract 階段輸出格式一致性。

### Refactors
- **handler base**: 將 `convert_to_cities_schema` 與 admin1 生成流程提升至 `GeoDataHandler`，透過鉤子方法保留前處理彈性與錯誤檢測。
- **geoname id**: `replace_in_dataset` 使用動態計算起始值，移除硬編碼的 geoname ID 常數以避免衝突。
- **schema sources**: 將 schema 定義集中於 `core/schemas.py`，常數整併至 `core/constants.py`，改善匯入依賴。
- **admin1 hooks**: 新增 admin1 前處理鉤子與快取機制，讓臺灣處理器改用共用實作並減少重複程式碼。
- **environment cleanup**: 移除多餘的 `SHAPE_RESTORE_SHX` 設定，簡化執行環境預設值。
- **cities conversion**: 透過鉤子統一城市資料轉換流程，提供穩定輸出與一致的空值處理。
- **coordinate precision**: `GeoDataHandler` 將經緯度固定為 8 位小數，避免重複計算時產生大量浮點誤差差異。
