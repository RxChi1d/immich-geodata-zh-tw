# CLAUDE.md

此檔案提供 Claude Code (claude.ai/code) 在此儲存庫中工作時的指引。

## 語言規則

**重要：請嚴格遵循以下語言規則**

1. **Claude.md 內容**：使用zh-tw
2. **對話語言**：使用zh-tw
3. **程式碼註解**：使用zh-tw
4. **函數/變數命名**：使用en
5. **Git commit 訊息**：使用zh-tw
6. **文件字串 (docstrings)**：使用zh-tw
7. **專案文檔**：使用zh-tw
8. **其他發布用文件**：使用zh-tw

## 撰寫風格與格式

- **程式碼**（Python, Bash, etc.）、**配置檔案**（YAML, TOML, etc.）：遵循 ruff 規範。如果有不同會在後續 `uv run ruff check .` 檢查出異常。
- **專案文檔、說明文字、文件模板**：遵循 Google 風格。
- **Commit 與 PR 訊息**：遵循 Conventional Commit 格式與 Google 風格。
- **Changelog**：遵循 Keep a Changelog 格式。
- **分支名稱**：遵循 Conventional Branch Naming。

### Commit 撰寫規範

- **格式**：遵循 **Conventional Commits** 規範
- **風格**：Google 風格

#### 格式要求

```
<type>(<scope>): <description>    ← 第一行（50-72 字符）

[optional body]                   ← 詳細說明（72 字符換行）

[optional footer(s)]              ← 破壞性變更、問題參考
```

**重要說明**：
- **第一行**：GitHub 自動生成 release notes 使用
- **內容主體**：複雜變更的詳細解釋（不會出現在 release notes 中）
- **腳註**：破壞性變更和問題參考

### Pull Request 撰寫規範

**重要**：建立 PR 時必須遵循以下規範：

#### PR 標題格式
- 必須遵循約定式提交格式：`<type>(<scope>): <description>`
- 範例：`feat: add async operations with progress callbacks`

#### PR 內容格式
- 參考 `.github/pull_request_template.md` 中的模板
- 包含完整的變更說明、測試資訊、檢查清單

#### PR 標籤
- 根據 PR 標題自動分類（Release Drafter 自動處理）
- 確保選擇正確的變更類型

#### PR 描述要求
- 清楚描述變更內容和原因
- 列出相關的測試項目
- 確認所有檢查清單項目

**模板位置**：`.github/pull_request_template.md`
**風格**：Google 風格

### CHANGELOG 撰寫規範

**重要**：CHANGELOG.md 必須遵循 [Keep a Changelog](https://keepachangelog.com/en/1.0.0/) 格式、業界最佳實務與 **Google 風格**。

#### 基本原則
1. **面向用戶**：描述功能影響，而非技術實現細節
2. **語義化描述**：歸納整理變更，避免直接複製 commit 訊息
3. **完整版本記錄**：包含所有版本（包括 pre-release）
4. **標準分類**：僅使用 Keep a Changelog 的六個類別
   - **Added**：新功能
   - **Changed**：現有功能變更
   - **Deprecated**：即將移除的功能
   - **Removed**：已移除的功能
   - **Fixed**：錯誤修復
   - **Security**：安全性修復

**注意**：不使用 Conventional Commit 的分類（feat, docs, refactor 等），因為 CHANGELOG 面向最終用戶。

#### 版本處理策略
- **Stable Release (1.0.0)**：詳細描述所有重要變更，面向最終用戶
- **Pre-release (alpha, beta, rc)**：簡化描述，重點說明階段目標和主要改進
- **版本順序**：最新版本在上，按時間倒序排列
- **日期格式**：使用 YYYY-MM-DD 格式

#### 內容撰寫要求
- **用戶導向**：描述用戶能感受到的變化和價值
- **簡潔明確**：每項變更一行，易於掃讀
- **粗體標題**：使用 **功能名稱** 突出重要特性
- **避免技術細節**：不包含 commit hash、內部重構、開發工具變更等

#### 範例格式
```markdown
## [1.0.0] - 2025-08-01

### Added
- **Windows Filename Compatibility**: Automatic sanitization of problematic filenames
- **Enhanced Security Features**: Built-in protection against ZIP bombs

### Changed
- **API Architecture**: Redesigned for better performance and maintainability

### Fixed
- **Cross-Platform Compatibility**: Resolved Windows-specific path issues
```

## 專案概述

本專案為 Immich 提供反向地理編碼功能的臺灣特化優化，旨在提升地理資訊的準確性及使用體驗。主要功能包括：

中文化處理：將國內外地理名稱轉換為符合臺灣用語的繁體中文。
行政區優化：解決臺灣直轄市與省轄縣市僅顯示地區名稱的問題。
提升臺灣資料準確性：利用中華民國國土測繪中心 (NLSC) 的官方圖資處理臺灣地區的地理名稱與邊界資料，確保數據來源的權威性。

- **Python 支援版本**：Python >= 3.13
- **Python 套件管理工具**：uv
- **uv 管理的虛擬環境**：`.venv/`

## 架構說明

### 地理資料處理流程（ETL 模式）

本專案採用 **Extract-Transform-Load (ETL)** 模式處理地理資料：

```
core/geodata/
├── base.py         # GeoDataHandler 抽象基類
├── taiwan.py       # TaiwanGeoDataHandler
└── japan.py        # JapanGeoDataHandler
```

每個國家的 Handler 包含三個階段的方法：

#### 1. Extract（提取）
- **方法**：`extract_from_shapefile(shapefile_path, output_csv)`
- **功能**：從 Shapefile 提取資料並轉換為標準化 CSV
- **輸入**：原始 Shapefile 檔案
- **輸出**：`meta_data/{country}_geodata.csv`
- **處理內容**：
  - 讀取 Shapefile（使用 geopandas）
  - 計算多邊形中心點（使用適當的投影）
  - 轉換為 WGS84 座標
  - 輸出標準化欄位：latitude, longitude, country, admin_1-4

#### 2. Transform（轉換）
- `convert_to_cities_schema(csv_path)`: 將標準 CSV 轉成 CITIES_SCHEMA，負責生成 geoname_id、對應行政區與補齊時區、國家代碼。

#### 3. Load（載入）
- `replace_in_dataset(input_df, base_geoname_id)`: 以新資料覆蓋 cities500 中對應國家的紀錄並回傳更新結果。國家代碼由 Handler 實例的 `COUNTRY_CODE` 類別變數自動決定。

### 常用指令

```bash
python main.py extract --country TW --shapefile <path_to_tw_shapefile>
python main.py extract --country JP --shapefile <path_to_jp_shapefile>
python main.py enhance --country-code <country_code>
```

### 在程式中使用

```python
from core.geodata import get_handler

handler = get_handler("TW")()
handler.extract_from_shapefile("path/to/file.shp", "output.csv")
df = handler.convert_to_cities_schema("output.csv", base_geoname_id=92_000_000)
updated_df, max_id = handler.replace_in_dataset(cities500_df, base_geoname_id=92_000_000)
```

### 擴充新國家

1. 建立 `core/geodata/<country>.py`，繼承 `GeoDataHandler` 並加上 `@register_handler("<CC>")`。
2. 實作 `extract_from_shapefile` 與 `convert_to_cities_schema`：
   - `geoname_id` 從 `92_000_000` 起算。
   - 填寫正確時區與 `country_code`。
   - 輸出需符合 `CITIES_SCHEMA`。
3. 執行 `python main.py extract --country <CC> ...` 與 `python main.py enhance --country-code <CC>` 驗證流程。

Registry 會自動載入新處理器，毋須調整其他檔案。

### 核心開發循環

#### Git Hooks（pre-commit）

專案使用 **pre-commit** 工具進行自動化品質檢查，配置檔案：`.pre-commit-config.yaml`。

可以手動執行所有檢查：
```bash
uv run pre-commit run --all-files
```

開發過程中也可以使用以下命令做基本的檢查與測試：
```bash
# 使用 uv（推薦）- 按照 CI 執行順序
uv run ruff format .        # 1. 格式化程式碼
uv run ruff check --fix .   # 2. 風格檢查並自動修正
uv run mypy .               # 3. 類型檢查
uv run pytest              # 4. 執行單元測試
```

## 開發注意事項

### 模組化設計原則
- **單一檔案不得超過 500 行程式碼**
- **每個模組都有清楚的職責分工**
- **使用相對匯入**（`from .utils import compression`）
- **每個函式都需要 Google 格式的 docstring**

### 測試要求
- **為所有新功能撰寫 Pytest 單元測試**
- **至少包含：正常情境、邊界情況、失敗情況**
- **測試應位於 `/tests` 資料夾中**
- **使用 fixtures 提供測試資料**

### 錯誤處理
- **所有檔案操作都要有適當的錯誤處理**
- **使用具體的例外類型而非通用 Exception**
- **提供有用的錯誤訊息和解決建議**
- **記錄重要操作的日誌資訊**

## 文件撰寫與可解釋性
- **當新增功能、依賴變更或安裝步驟修改時，請更新 `README.md`。**
- **為不明顯的程式碼加上註解，並確保所有內容中階開發者都能理解。**
- 撰寫複雜邏輯時，**請加入行內 `# Reason:` 註解，說明「為什麼」這麼做，而不只是「做了什麼」。**

## AI 行為規範
- **絕不假設缺漏的上下文，如有疑問務必提出問題確認。**
- **嚴禁臆造不存在的函式或套件** —— 只能使用已知、驗證過的 Python 套件。
- **在程式碼或測試中引用檔案路徑或模組名稱前，務必確認其存在。**
- **除非有明確指示，或任務需求（見 `TASK.md`），**否則**不得刪除或覆蓋現有程式碼。**
- **需要分析或拆解問題，通過 sequential thinking 進行更深度思考**
- **與 GitHub 互動需使用 gh CLI**
- **不准在未經允許的情況下，擅自在任何的文檔、訊息等文字中，包含 AI 編輯器或是 AI 模型的名稱**，例如:
  - Generated with [Claude Code]
  - Co-Authored-By: Claude
