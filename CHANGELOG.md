# 更新日誌

此檔案記錄專案的所有重要變更。

格式基於 [Keep a Changelog](https://keepachangelog.com/en/1.0.0/)，
專案遵循 [語義版本控制](https://semver.org/spec/v2.0.0.html)。

安裝說明與使用方式請參閱 [README](README.md)。

---

## [未發佈版本]

### Changed
- **臺灣圖資更新**：村里界資料更新至國土測繪中心 2026-06-24 版，新增數個村里並修正邊界。
- **印尼圖資更新**：行政區邊界更新至地理空間資訊局 2026-06-12 版，村級行政區的涵蓋範圍擴增約 4%。**建議重新提取中繼資料**，既有照片才會套用新的行政區劃。
- **南韓行政區改制**：資料更新至 2026-07-01 生效的最新行政區劃。全羅南道與光州廣域市合併為「全南光州市」，仁川的中區、東區、西區改制為濟物浦區、永宗區、西海區與黔丹區。**建議重新提取中繼資料**，既有照片才會顯示新的地名。
- **南韓地名改用韓國官方漢字**：地名改以韓國官方漢字表記為準，如「淸州市」「尙州市」「鎭川郡」。與日本地名保留日式漢字的作法一致，共 147 筆的用字有所調整。
- **文件重整**：README 聚焦安裝流程，腳本參數、非容器部署與開發說明移至 `docs/`，並新增文件索引、貢獻指南與安全性回報方式。
- **macOS 加速器說明**：補充 Homebrew services 環境下的正確重啟方式，以及分離式部署時 Docker 端的設定調整。
- **自行建置產物精簡**：LocationIQ 階段不再讀寫臺灣專用的行政區對照表，建置時也不再產生 `output/tw_admin1_map.csv`。臺灣自 1.2.2 起即改用國土測繪中心官方圖資處理，該對照表早已不影響任何輸出，發布內容與現行版本完全相同。
- **資料來源聲明**：`NOTICE.md` 補上 Natural Earth、Wikidata、中文維基百科、韓文維基百科與 i18n-iso-countries 的授權聲明，更正南韓行政區資料的授權敘述，並補齊五個地區的圖資版本。
- **LocationIQ 查詢結果改放獨立目錄**：自行建置時，LocationIQ 的逆地理查詢結果改寫入 `data/locationiq/`，不再與各地區官方圖資的中繼資料共用 `meta_data/`。兩者欄位相同但性質相反——前者刪除後重跑即可復原（需消耗 API 額度），後者不可任意重建——分開存放後不必再從檔名判斷哪些檔案可以清理。既有的查詢進度移至新目錄即可續用，或以 `--locationiq-folder` 指回原本的位置。使用官方發布資料者不受影響。
- **官方圖資中繼資料改放 `data/handler/`**：各地區官方圖資產生的中繼檔從 `meta_data/` 移至 `data/handler/`，與 LocationIQ 查詢結果（`data/locationiq/`）並列於同一結構下，依產生方式分類。自行建置者若有既有的 `meta_data/` 目錄，更新後改用新路徑即可。輸出的地理資料完全不變，使用官方發布資料者不受影響。
- **外部 vendored 資料改放 `data/vendor/`**：國教院譯名表與 i18n-iso-countries 從專案根目錄移至 `data/vendor/`，`data/` 至此依產生方式分為 handler 產物、LocationIQ 查詢結果與外部既有資料三類。發布內容的目錄結構完全不變，使用官方發布資料與更新腳本者不受影響。

### Fixed
- **自行建置的 LocationIQ 中文名損毀**：LocationIQ 回傳的中文地名會被存成無意義的字元序列，使該來源的譯名全數失效。僅影響自行建置且指定了 LocationIQ 國家的情況，官方發布資料不受影響。
- **Nightly 版本恢復更新**：修正 nightly 預發布版本自 2026-05-29 起停止發布的問題。發布與否原本取決於中繼資料檔案有無變動，在所有地區改用官方圖資處理後該條件已不再成立，導致每週的自動建置雖然成功卻不發布任何版本。現在每次排程都會重新建置並發布，nightly 版本恢復每週更新最新的上游地理資料。
- **南韓地名錯誤**：修正四處長期存在的錯誤地名——首爾市的冠岳區原本顯示為區內的「新林洞」、松坡區顯示為地鐵站「蠶室站」，京畿道的驪州市仍沿用升格前的「驪州郡」，全南光州市的咸平郡誤植為「鹹平郡」。共影響 69 筆地名。
- **安裝說明錯誤**：更正手動部署的下載目錄設定、指定版本時的腳本取得網址，以及 `docker-compose.yml` 的服務名稱。舊寫法會讓資料解壓到錯誤層級，或使容器因取不到安裝腳本而無法啟動。
- **安裝失敗提示**：安裝位置有誤時，一併提示可用 `IMMICH_BUILD_DATA` 指定 geodata 目錄。原本只提示 `IMMICH_SERVER_ROOT`，但該變數不影響 geodata 的安裝位置。
- **自行建置的翻譯階段不再讀取無用資料**：翻譯階段會把各地區官方圖資的中繼檔誤認成 LocationIQ 查詢結果載入，等於多讀約 25 萬列永遠用不到的資料，過程中也沒有任何訊息可循。現在只載入 LocationIQ 產生的檔案，並在記錄中列出實際載入與略過的檔案。地理資料輸出完全不變，自行建置時該階段的耗時與記憶體用量明顯下降。
- **自行建置不再強制要求 LocationIQ API key**：所有地區都改用官方圖資後，LocationIQ 查詢階段本來就會自動跳過，但建置指令仍會在開始前要求 API key 而中止。現在只有在指定了需要 LocationIQ 的國家時才會要求金鑰，自行重新建置資料不必再申請 LocationIQ 帳號。

### Security
- **相依套件更新**：`quinn-proto` 更新至 0.11.17，解除 QUIC 封包重組可造成記憶體耗盡的警示。本專案未啟用 HTTP/3，該套件未編譯進產物，行為不變。

---

## [3.2.0] - 2026-08-18

本版本讓部署腳本可在官方容器以外的環境安裝資料，並修正安裝流程中數個會靜默失敗或破壞現狀的問題。

### Added
- **非容器部署支援**：`update_data.sh` 可在 macOS 原生 worker、LXC 與裸機等非容器環境安裝資料。安裝位置改為自動偵測，需要時可用 `IMMICH_SERVER_ROOT` 指定 Immich server 根目錄，geodata 位置則沿用 Immich 自身的 `IMMICH_BUILD_DATA`。
- **安裝結果驗證**：安裝後確認資料確實寫入 Immich 會載入的位置，包含實際決定國名顯示的 `en.json`，不一致時直接報錯，不再出現「安裝成功但資料沒生效」的情況。
- **新增參數**：`--print-paths` 可在安裝前確認解析出的目標路徑；`--archive` 可直接以本機既有的 `release.tar.gz` 安裝，適用於離線環境。

### Changed
- **安裝路徑改以目錄結構判斷**：不再讀取 Immich 版本推斷路徑，因此 Immich 變更目錄結構時不需要再修改腳本，安裝流程也不再依賴 Node.js。
- **文件說明**：README 與 README.en 補充本專案同時支援 Immich v2 與 v3，並新增非容器部署的說明章節。
- **Release 頁面精簡**：發佈頁面不再附帶未壓縮 CLI binary、ldd 診斷清單與 build manifest 等工程性檔案；預編譯 CLI 仍以 tar.gz 與對應 sha256 校驗檔提供。

### Fixed
- **安裝失敗不再留下半套資料**：安裝過程中任何一步失敗，都會還原為安裝前的狀態並以錯誤結束，不再出現地理資料已更新但語系資料未更新、或下載中斷卻回報成功的情況。此前在 macOS 等非 root 環境會因調整檔案擁有者失敗而中斷於此狀態。
- **地理資料下載穩定性**：放寬大型資料檔的下載逾時，避免上游伺服器速率下降時，200 MB 以上的資料檔因逾時過短而下載失敗。

### Security
- **語系檔覆寫可能寫穿符號連結**：`langs` 下的檔案若為符號連結，原本的覆寫方式會寫入連結指向的檔案；以 root 執行安裝時，這讓可寫入該目錄的使用者得以影響其他位置的檔案。改為逐檔寫入隨機命名的暫存檔後 rename。

---

## [3.1.0] - 2026-06-06

本版本整合國教院官方譯名全面提升全球地名的翻譯品質，並新增印尼繁體中文在地化。

**升級提醒**：
- 本版本更新全球各地區與印尼的地名資料。若您已部署本專案，請在更新後於 Immich 執行「重新擷取照片中繼資料」，以套用最新資料。

### Added
- **國教院官方譯名整合**：全球地名翻譯新增國家教育研究院《外國地名譯名》資料來源，以信心分級補洞並升級為官方臺灣譯名，提升全球非特化地區的中文覆蓋率與譯名品質。
- **印尼繁體中文在地化**：新增印尼行政區支援，採用 BIG（印尼地理空間資訊局）官方村界圖資，省與縣市名稱提供繁體中文翻譯，並以村級資料提升定位精準度。

---

## [3.0.0] - 2026-06-05

本版本為重大更新：新增泰國繁體中文在地化，並將資料處理引擎全面遷移至 Rust。

**升級提醒**：
- 若媒體庫含有泰國地區照片，請在更新後於 Immich 執行「重新擷取照片中繼資料」，以套用最新的泰國資料。
- 若有自動化腳本下載 Rust binary 資產，請注意資產檔名已變更（見 Changed）。

### Added
- **泰國繁體中文在地化**：泰國行政區改採官方 COD-AB 邊界資料搭配 Wikidata 繁體中文翻譯，一級行政區（府）全數翻譯、二級行政區（縣）覆蓋率約 94%，取代先前以 LocationIQ 產生的簡體與未翻譯名稱。

### Changed
- **資料處理引擎全面遷移至 Rust**：正式發布流程改由 Rust pipeline 產生，提升處理效能與發布可重現性。
- **南韓地理資料更新**：同步 admdongkor 最新行政區界資料。
- **Release 資產檔名變更**：Rust binary 由 `immich-geodata-migration` 更名為 `immich-geodata`，Release 下載資產檔名同步更新為 `immich-geodata-x86_64-unknown-linux-gnu`。

### Removed
- **Python 資料處理工具鏈**：Python 實作正式退場，所有資料處理與發布流程改由 Rust CLI 提供。

### Fixed
- **地名翻譯正確性強化**：所有 Wikidata 翻譯逐級驗證行政隸屬關係（二級對一級、一級對國家），杜絕同名地名錯配（例如泰國難府不再被無關的同拼寫條目干擾）；跨行政區同名單位不會再取得錯誤翻譯，無法確認時保守顯示官方名稱。
- **資料更新流程穩定性**：Wikidata 查詢加入自適應速率控制與每端點節流，避免大量請求被拒導致更新緩慢或中斷；發布流程新增地理資料完整性檢查，防止任一國家資料靜默缺漏。

---

## [2.2.4] - 2026-05-25

### Changed
- **地理資料維護更新**：更新臺灣、日本、南韓與泰國反向地理編碼資料，讓 release 套件與最新可用來源資料保持同步。
- **地理資料處理架構**：整理國別 Handler 結構、註冊流程、共用 admin/geospatial 工具與 Polars 資料處理流程，維持既有輸出行為並提升後續維護性。
- **南韓資料處理**：拆分南韓翻譯與特殊規則資料，讓地名轉換邏輯更清楚、可測且易於調整。
- **測試覆蓋**：新增 geodata handler registry、admin1 mapping、南韓處理器與 geoname ID 掃描相關測試，保護架構整理後的既有行為。

### Fixed
- **Nightly workflow refspec 衝突**：將內部 nightly 建構分支改為 `nightly-build`，避免與公開 `nightly` tag 衝突，並維持既有 nightly release 行為不變。
- **Wikidata 翻譯穩定性**：保留語言回退順序，並改善進度與快取命中顯示。
- **Admin1 mapping cache**：避免同一程序中重新產生 CSV 後仍讀到舊 mapping。
- **資料正規化與檔案處理**：強化空值 sentinel、檔案讀取錯誤訊息，以及 malformed `geoname_id` 的處理。

## [2.2.3] - 2026-03-06

### Changed
- **泰國地理資料更新**：更新泰國反向地理編碼資料，讓 release 套件持續同步最新來源資料。
- **Release Notes 規範**：補充發布說明撰寫規範，統一分類、emoji 與比較連結格式。

### Fixed
- **自動更新變更偵測**：統一 release 與 auto-update workflow 的變更判斷方式，減少重複邏輯並避免不必要的發布流程。
- **Nightly 發布策略**：改用單一 `nightly` tag 與 pre-release，避免自動更新累積大量動態 tag。

## [2.2.2] - 2026-02-13

### Changed
- **泰國地理資料更新**：更新 2026-01-29 與 2026-02-13 的泰國反向地理編碼資料。

### Fixed
- **Nightly tag 與分支命名衝突**：調整動態 nightly tag 產生方式，避免與分支名稱衝突並確保自動更新流程穩定執行。

## [2.2.1] - 2026-01-29

### Changed
- **泰國地理資料更新**：匯入 2025-12-05 的泰國反向地理編碼資料。
- **Release 流程審核**：release 與 auto-update 變更改由 Pull Request 進入主分支，讓資料更新與發布流程更可追蹤。
- **Nightly 同步流程**：新增 main 到 nightly 的同步流程，確保穩定版本修正能自動帶入 nightly 建構。
- **資料來源聲明**：補充南韓資料來源 `admdongkor` 的授權與歸屬資訊。

### Fixed
- **Nightly 分支同步**：調整 release 分支命名與同步方式，避免 nightly 分支、tag 與自動同步流程互相衝突。
- **Release 套件內容**：移除 release 打包階段對 `update_data.sh` 的額外處理，讓壓縮檔內容與 repository 版本一致。

## [2.2.0] - 2025-11-22

### Added
- **Wikidata 翻譯工具**：通用地名翻譯引擎，支援 P131 驗證、P31 過濾、多語言回退與 OpenCC，並使用 Context-Aware Cache（`TranslationCacheStore`）將翻譯結果、搜尋結果依 `TranslationItem.id` 獨立儲存，確保同名不同父層的行政區在搜尋、驗證、快取各階段完全隔離。
- **翻譯可追溯性**：翻譯結果包含實際使用語言、QID、父層驗證狀態與時間戳記，提升除錯與品質分析能力。
- **南韓地理資料處理器**：從 admdongkor 專案提取官方行政區邊界並自動翻譯為繁體中文。廣域市統一加「市」字（首爾市、釜山市），濟州道區分濟州市，世宗市採用業界通用譯名（大平洞、賽倫洞等）。自動拆分「市＋區/郡」，支援特殊行政結構。
- **資料來源授權聲明**：新增 NOTICE.md 完整聲明第三方資料授權，符合 GeoNames (CC-BY 4.0) 與 OpenStreetMap (ODbL 1.0) 等授權要求。

### Changed
- **Release 套件一致性**：release 壓縮檔與 README/README.en 現在都改為指向 release 內的 `update_data.sh`，避免使用者拉取 main 分支腳本造成版本不一致。
- **LocationIQ QPS 預設值**：從 1 調整為 2，提升資料處理效率。
- **GeoData 欄位順序**：統一 GEODATA_SCHEMA、各國 Handler 與 LocationIQ 流程，並將欄位實際順序調整為 `latitude, longitude, country, admin_1, admin_2, admin_3, admin_4`，同時更新 meta_data CSV 與文件，確保所有 ETL 階段與翻譯腳本依此排列讀寫。
- **Admin 欄位缺值處理**：meta_data CSV 在產生時保留 Null，不再強制輸出空白字串，並於讀取階段透過共用的 `fill_admin_columns()` 將 `admin_1-4` 的缺值統一補為空字串，避免 Polars 將空欄解析成 `None` 造成翻譯流程異常。
- **Extract 儲存邏輯**：重構為共用方法，消除重複程式碼。
- **Extract CSV 排序策略**：擴充至全欄位排序（latitude, longitude, country, admin_1-4），優化版本追蹤效果，同一行政區資料聚集在一起，提升可讀性。
- **資料預覽多樣化**：Extract 完成後採用階層式去重策略，優先確保不同省/道/市（admin_1），資料不足時才顯示同一省內的不同市/區，最大化地理區域代表性。

---

## [2.1.0] - 2025-10-18

### Changed
- **日本政令市區名**：政令市記錄的 admin_3 欄位填入區名，保留來源資料的完整行政層級資訊
- **Admin2 資料處理簡化**：移除 admin2Codes.txt 翻譯流程，僅保留下載與複製，因該檔案在 Immich 的反向地理編碼中不會被使用
- **國家代碼過濾邏輯統一**：在 main.py 集中處理所有 --country-code 參數的過濾，移除分散在各模組的重複邏輯，提升程式碼可維護性

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

## [release-2025-04-05] - 2025-04-05

### Added
- **泰國支援**：泰國 (TH) 地區地理資料處理
- **國際擴展**：將在地化功能擴展至臺灣以外地區

## [release-2025-02-06] - 2025-02-06

### Changed
- **翻譯改善**：強化翻譯處理與準確度

## [release-2025-02-05] - 2025-02-05

### Added
- **韓國中繼資料**：支援韓國地區地理資料處理

### Fixed
- **翻譯處理**：解決翻譯腳本問題並改善可靠性

---

特定變更的詳細資訊請參閱 [提交歷史](https://github.com/RxChi1d/immich-geodata-zh-tw/commits/main) 或 [發佈頁面](https://github.com/RxChi1d/immich-geodata-zh-tw/releases)。

[未發佈版本]: https://github.com/RxChi1d/immich-geodata-zh-tw/compare/v3.2.0...HEAD
[3.2.0]: https://github.com/RxChi1d/immich-geodata-zh-tw/compare/v3.1.0...v3.2.0
[3.1.0]: https://github.com/RxChi1d/immich-geodata-zh-tw/compare/v3.0.0...v3.1.0
[3.0.0]: https://github.com/RxChi1d/immich-geodata-zh-tw/compare/v2.2.4...v3.0.0
[2.2.4]: https://github.com/RxChi1d/immich-geodata-zh-tw/compare/v2.2.3...v2.2.4
[2.2.3]: https://github.com/RxChi1d/immich-geodata-zh-tw/compare/v2.2.2...v2.2.3
[2.2.2]: https://github.com/RxChi1d/immich-geodata-zh-tw/compare/v2.2.1...v2.2.2
[2.2.1]: https://github.com/RxChi1d/immich-geodata-zh-tw/compare/v2.2.0...v2.2.1
[2.2.0]: https://github.com/RxChi1d/immich-geodata-zh-tw/compare/v2.1.0...v2.2.0
[2.1.0]: https://github.com/RxChi1d/immich-geodata-zh-tw/compare/v2.0.0...v2.1.0
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
