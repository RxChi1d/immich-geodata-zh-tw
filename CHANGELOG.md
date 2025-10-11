# 更新日誌

此檔案記錄專案的所有重要變更。

格式基於 [Keep a Changelog](https://keepachangelog.com/en/1.0.0/)，
專案遵循 [語義版本控制](https://semver.org/spec/v2.0.0.html)。

## 關於此專案

本專案為 Immich 提供反向地理編碼功能的臺灣特化優化，旨在透過中文在地化、行政區劃最佳化，以及使用國土測繪中心 (NLSC) 官方資料提升臺灣資料準確性，改善地理資訊準確度與使用體驗。

## 版本類型

- **穩定版本** (v1.x.x)：經過完整測試的手動發佈版本
- **夜間建構**：包含最新地理資料更新的自動建構，標記為 `nightly`
- **預發佈版本**：歷史開發快照 (release-YYYY-MM-DD 格式)

安裝說明與使用方式請參閱 [README](README.md)。

## 發佈連結

- [最新版本](https://github.com/RxChi1d/immich-geodata-zh-tw/releases/latest)
- [所有版本](https://github.com/RxChi1d/immich-geodata-zh-tw/releases)
- [版本比較](https://github.com/RxChi1d/immich-geodata-zh-tw/compare)

---

## [2.0.0] - 2025-10-11

### Added
- **Handler Registry**：`register_handler` 與 `get_handler` 讓 `extract` 與 `enhance` 指令依國碼載入專用處理器，減少手動切換設定。
- **GeoData Handler 架構**：建立共用基底類別與鉤子機制，整合原本分散的流程並提供一致的擴充介面。
- **Enhance 工作流程整合**：`update_geodata()` 同步更新 admin1 與 cities500，集中管理 geoname ID 範圍與處理日誌。
- **Geoname ID 管理**：以動態計算方式分配 geoname ID，確保新資料集不與既有編號衝突。
- **行政區處理指南**：新增臺灣與日本行政區處理文件的中英文版本，說明資料來源、層級對應與轉換策略。
- **Japan GeoData 處理器**：引入 `JapanGeoDataHandler` 與 `meta_data/jp_geodata.csv`，使用官方行政區資料提供日本地區全自動 ETL。

### Changed
- **Schema 與常數來源**：將資料表 schema 與常數集中於 `core/schemas.py` 與 `core/constants.py`，降低重複定義與匯入依賴。
- **國別處理器整合**：既有國家處理器改為透過新基底類別運行，擴充流程與錯誤訊息更一致。
- **CLI 行為**：`enhance` 指令自動略過已由 Handler 支援的國家並整合原有 modify 流程，降低重複執行風險。
- **經緯度精度**：`GeoDataHandler` 統一輸出經緯度為 8 位小數，確保重複匯出時差異最小化。
- **執行環境設定**：移除多餘的 `SHAPE_RESTORE_SHX` 參數，簡化預設值與除錯流程。
- **專案文件結構**：重寫 README 與 README.en 導覽章節，並建立 `docs/zh-tw/` 與英文對應路徑以統一文件架構。
- **工具模組架構**：將 `core/utils.py` 重構為模組化套件，拆分為 `logging`、`filesystem`、`alternate_names`、`geoname_ids` 四個子模組，改善程式碼組織與可維護性，並移除所有 `sys.path` 操作改用標準相對匯入。

### Fixed
- **輸出路徑處理**：建立輸出資料夾後再寫入，避免因路徑不存在而失敗。

## [1.2.2] - 2025-09-19

### Changed
- **Geodata Refresh**: 自動資料管線匯入 2025-09-12 的反向地理編碼資料集，確保 Nightly 建構持續同步最新地理資訊。
- **Deployment Guidance**: README 說明更新為使用 `exec start.sh`，避免 Immich 1.142.0+ 在整合式部署時因路徑誤判而發生重啟循環。

## [1.2.1] - 2025-09-08

### Changed
- **發佈流程**：推送以 v 為前綴的標籤自動觸發 Release，並在建置前依 PEP 440 驗證版本格式。
- **NLSC 圖資更新**：村(里)界資料更新至 `1140825`，同步調整 `meta_data/taiwan_geodata.csv` 之中心點座標（WGS84）

## [1.2.0] - 2025-09-04

### Added
- **Immich 版本自動偵測**：部署腳本支援自動識別不同版本的 Immich 容器結構，確保新舊版本相容性
- **可靠的版本比較機制**：部署腳本提升版本號判斷的準確性，避免因版本比較錯誤導致的路徑選擇問題
- **CLI 參數支援**：Taiwan geodata 處理工具新增命令列參數 (`--shapefile`, `--output`)

### Changed
- **依賴管理系統**：遷移至 uv 現代化套件管理，提升安裝效能並簡化專案維護
- **CI/CD 管線**：GitHub Actions 升級至 Python 3.13 並採用 uv 快速安裝流程
- **安裝方式**：本地開發安裝改為 `uv sync`，執行命令更新為 `uv run python main.py`
- **容器路徑更新**：調整 i18n-iso-countries 路徑以支援 Immich 1.136.0+，並新增版本相容性說明
- **NLSC 圖資更新**：更新至版本 1140620，提升臺灣地理資料準確性
- **依賴結構調整**：將 geopandas 提升為運行時依賴，並移除未使用的 scipy 開發依賴；升級核心套件至最新穩定版本

### Fixed
- **Immich 版本判斷邏輯**：修正容器路徑變更的版本分界點從 1.139.4 改為 1.136.0，確保版本判斷的準確性
- **版本發布邏輯**：Release workflow 自動識別預發布版本，確保只有穩定版本被標記為 Latest
- **指令範例調整**：統一文檔中的國家代碼參數為 `JP KR TH`，符合 CI/CD 流程

### Removed
- **舊式依賴管理**：移除 requirements.txt，統一使用 pyproject.toml 管理套件依賴
- **文檔結構優化**：移除過時的版本遷移警告
- **過時地理資料**：移除基於 LocationIQ 的臺灣地理資料檔案，統一使用 NLSC 官方資料

## [1.1.4] - 2025-08-11

### Added
- **AI 協作文件**：完整的 CLAUDE.md 檔案，包含專案指引、編碼規範與 AI 協作說明，改善開發工作流程
- **強化開發指引**：完整的編碼慣例、提交標準與語言使用規則，提升程式碼品質

### Changed
- **專案依賴套件**：更新核心依賴 (polars 1.32.2、regex 2025.7.34、requests 2.32.4)，提升效能與安全性
- **開發環境**：更新開發依賴套件 (geopandas 1.1.1、ruff 0.12.8、scipy 1.16.1)，改善程式碼品質與分析工具
- **專案維護**：改善 .gitignore 設定，排除暫存檔案與開發產物
- **資料更新**：更新反向地理編碼資料

## [1.1.3] - 2025-07-19

### Changed
- **強化資料追蹤**：改善中繼資料 CSV 檔案追蹤功能，提供更好的資料管理與監控

### Fixed
- 解決自動化資料處理工作流程中 CSV 檔案處理問題

## [1.1.2] - 2025-06-10

### Added
- **英文文件**：為國際使用者與貢獻者提供完整英文 README
- **雙語支援**：提供繁體中文與英文雙語完整文件

### Changed
- **文件結構**：改善安裝與使用說明的組織架構與清晰度
- **使用者體驗**：提升非中文使用者的可及性

## [1.1.1] - 2025-05-30

### Fixed
- **發佈自動化**：解決夜間建構系統中日期排序問題
- **CI/CD 流水線**：改善自動化發佈重建流程的可靠性

## [1.1.0] - 2025-04-12

### Added
- **NLSC 整合**：使用國土測繪中心 (NLSC) Shapefile 資料進行官方臺灣地理資料處理
- **強化臺灣準確度**：提供臺灣地區權威邊界與行政資料

### Changed
- **文件更新**：同步依賴套件版本並改善專案文件
- **地理資料品質**：大幅提升臺灣地理資訊準確度

## [1.0.0] - 2025-04-09

### Added
- **核心臺灣在地化**：完整的臺灣地區反向地理編碼最佳化
- **中文翻譯**：國內外地點的繁體中文名稱
- **行政區劃最佳化**：修正臺灣直轄市與縣市顯示問題
- **自動化更新**：簡化發佈系統與自動化資料更新
- **Docker 整合**：容器化部署與整合/手動部署選項

### Changed
- **發佈系統**：重構並簡化發佈自動化流程
- **腳本強化**：改善更新腳本的標籤驗證與錯誤處理

## 預發佈版本

### [release-2025-04-05] - 2025-04-05

### Added
- **泰國支援**：泰國 (TH) 地區地理資料處理
- **國際擴展**：將在地化功能擴展至臺灣以外地區

### [release-2025-02-06] - 2025-02-06

### Changed
- **翻譯改善**：強化翻譯處理與準確度

### [release-2025-02-05] - 2025-02-05  

### Added
- **韓國中繼資料**：支援韓國地區地理資料處理

### Fixed
- **翻譯處理**：解決翻譯腳本問題並改善可靠性

## 夜間建構

`nightly` 標籤提供包含最新地理資料的持續更新建構。這些自動化發佈包含：

- **自動化資料更新**：定期提取最新的反向地理編碼資料
- **最新改善**：最近的錯誤修正與功能增強
- **開發功能**：搶先使用穩定版本前的新功能

**注意**：建議希望取得最新地理資料的使用者使用夜間建構，但可能包含實驗性功能。生產環境建議使用穩定版本 (v1.x.x)。

## 歷史開發

### 早期開發 (2025-01-01 至 2025-03-31)

- **專案初始化**：首次提交與專案結構建立
- **核心開發**：實作臺灣在地化演算法
- **CI/CD 設定**：自動化發佈與資料更新工作流程
- **文件撰寫**：初始 README 與使用說明
- **測試**：品質保證與功能驗證

---

特定變更的詳細資訊請參閱 [提交歷史](https://github.com/RxChi1d/immich-geodata-zh-tw/commits/main) 或 [發佈頁面](https://github.com/RxChi1d/immich-geodata-zh-tw/releases)。

[未發佈版本]: https://github.com/RxChi1d/immich-geodata-zh-tw/compare/v2.0.0...HEAD
[2.0.0]: https://github.com/RxChi1d/immich-geodata-zh-tw/compare/v1.2.2...v2.0.0
[1.2.2]: https://github.com/RxChi1d/immich-geodata-zh-tw/compare/v1.2.1...v1.2.2
[1.2.1]: https://github.com/RxChi1d/immich-geodata-zh-tw/compare/v1.2.0...v1.2.1
[1.2.0]: https://github.com/RxChi1d/immich-geodata-zh-tw/compare/v1.1.4...v1.2.0
[1.1.4]: https://github.com/RxChi1d/immich-geodata-zh-tw/compare/v1.1.3...v1.1.4
[1.1.3]: https://github.com/RxChi1d/immich-geodata-zh-tw/compare/v1.1.2...v1.1.3
[1.1.2]: https://github.com/RxChi1d/immich-geodata-zh-tw/compare/v1.1.1...v1.1.2
[1.1.1]: https://github.com/RxChi1d/immich-geodata-zh-tw/compare/v1.1.0...v1.1.1
[1.1.0]: https://github.com/RxChi1d/immich-geodata-zh-tw/compare/v1.0.0...v1.1.0
[1.0.0]: https://github.com/RxChi1d/immich-geodata-zh-tw/compare/cb70535...v1.0.0
[release-2025-04-05]: https://github.com/RxChi1d/immich-geodata-zh-tw/releases/tag/release-2025-04-05
[release-2025-02-06]: https://github.com/RxChi1d/immich-geodata-zh-tw/releases/tag/release-2025-02-06
[release-2025-02-05]: https://github.com/RxChi1d/immich-geodata-zh-tw/releases/tag/release-2025-02-05
