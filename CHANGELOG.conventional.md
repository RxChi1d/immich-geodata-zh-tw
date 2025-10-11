# Conventional Changelog

## [未發佈版本]

### Features
- **處理器註冊**：`register_handler` 與 `get_handler` 讓 CLI 依國碼載入對應處理器並整合 `extract` 與 `enhance`。
- **日本支援**：`JapanGeoDataHandler` 與 `meta_data/jp_geodata.csv` 導入官方行政區資料的完整 ETL。
- **Enhance 工作流程**：`update_geodata()` 同步編排 admin1 與 cities500 更新，集中 geoname ID 追蹤。
- **Enhance 覆蓋範圍**：`enhance` 指令自動略過已支援國家，減少重複處理與錯誤風險。
- **Geoname ID 管理**：動態分配 geoname ID，避免新資料與既有編號衝突。

### Bug Fixes
- **Enhance 輸出**：寫入前建立輸出資料夾，避免路徑缺失造成失敗。
- **日本資料格式**：將 admin_3 空值改為空字串，維持 Extract 輸出一致。

### Refactors
- **Handler 基底**：將 `convert_to_cities_schema` 與 admin1 流程提升至基底類別，加入鉤子與快取以支援國別差異。
- **Handler 整合**：既有國家處理器改採新基底類別，擴充流程與錯誤訊息更一致。
- **Schema 與常數**：整併 schema 與常數至 `core/schemas.py` 與 `core/constants.py`，降低匯入依賴。
- **環境設定清理**：移除 `SHAPE_RESTORE_SHX` 預設值，簡化執行環境設定。
- **城市轉換流程**：透過鉤子統一城市資料轉換流程，提升輸出穩定度。
- **座標精度**：統一經緯度為 8 位小數，減少重複匯出差異。
- **Admin1 鉤子**：共用行政區前處理實作，降低國別程式碼重複。
- **工具模組重構**：拆分 `core/utils.py` 為獨立套件，包含 `logging`、`filesystem`、`alternate_names`、`geoname_ids` 子模組並移除 `sys.path` hack。

### Documentation
- **README（中文）**：重寫專案概覽與設計理念，新增語言策略表格與指引連結。
- **README（英文）**：同步更新英文版導覽內容與跨語言文檔連結。
- **行政區處理文檔**：新增臺日行政區處理說明的中英文版本，並建立 `docs/zh-tw/` 文檔結構。
