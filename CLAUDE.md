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

- **程式碼**（Rust, Bash, YAML, TOML, etc.）：遵循各語言既有格式化工具與專案慣例。
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

### Release Notes 撰寫規範

**重要**：GitHub Release Notes 與 CHANGELOG 共用 [Keep a Changelog](https://keepachangelog.com/en/1.0.0/) 分類規範。

#### 分類與 Emoji 對應

| 類別 | Emoji | 說明 |
|------|-------|------|
| Added | 🚀 | 新功能 |
| Changed | 🔄 | 現有功能變更、資料更新 |
| Deprecated | ⚠️ | 即將移除的功能 |
| Removed | 🗑️ | 已移除的功能 |
| Fixed | 🐛 | 錯誤修復 |
| Security | 🔒 | 安全性修復 |

**注意**：不使用 Conventional Commit 的分類（feat, fix, chore 等），因為 Release Notes 面向最終用戶。

#### 範例格式
```markdown
# What's Changed

## 🐛 Fixed

- 修正 nightly 動態 tag 與分支命名衝突問題，確保自動更新流程穩定執行

## 🔄 Changed

- 更新泰國地理資料（2026-01-29、2026-02-13）

---

**完整變更記錄**: [v2.2.1...v2.2.2](https://github.com/RxChi1d/immich-geodata-zh-tw/compare/v2.2.1...v2.2.2)

**發布日期**: 2026-02-13
```

## 專案概述

本專案為 Immich 提供反向地理編碼功能的臺灣特化優化，旨在提升地理資訊的準確性及使用體驗。主要功能包括：

中文化處理：將國內外地理名稱轉換為符合臺灣用語的繁體中文。
行政區優化：解決臺灣直轄市與省轄縣市僅顯示地區名稱的問題。
提升臺灣資料準確性：利用中華民國國土測繪中心 (NLSC) 的官方圖資處理臺灣地區的地理名稱與邊界資料，確保數據來源的權威性。

- **正式資料處理工具鏈**：Rust CLI（專案根目錄 crate）
- **Rust 主要檢查**：`cargo fmt`、`cargo clippy`、`cargo test`

## 版本管理

本專案使用兩種獨立的版本號：

- **專案版本**（如 0.1.0）：遵循語義化版本規範，代表功能、API、錯誤修復的變更。專案版本變更時需更新發布說明、CHANGELOG 等面向用戶的文檔。
- **快取 Schema 版本**（如 1.0）：定義於 `TranslationCacheStore.VERSION`，僅在快取資料結構不相容時升級。快取版本幾乎不變動（可能整個 1.x 專案生命週期都維持 schema 1.0）。

**規則**：兩者獨立管理，專案版本升級不會影響快取 schema 版本，避免不必要的快取重建。

## 架構說明

### 地理資料處理流程（ETL 模式）

本專案採用 **Extract-Transform-Load (ETL)** 模式處理地理資料。正式
production path 已遷移至 Rust：

```
src/pipeline/
├── extract.rs              # TW/JP/KR 圖資讀取、座標轉換與 normalized CSV 輸出
├── extract/handlers.rs     # 各國 extract handler 與行政區欄位規則
├── prepare.rs              # GeoNames、Natural Earth 等來源下載與前處理
├── admin1_load.rs          # admin1 replacement
├── cities500_load.rs       # cities500 merge / handler replacement
├── locationiq.rs           # LocationIQ metadata 產生與續跑
├── translate.rs            # 繁中翻譯、OpenCC 與 alternate names
└── pack.rs                 # release tree、zip 與 tar.gz 打包
```

各國 Handler 仍保留相同 ETL 概念，但以 Rust enum/static dispatch 與型別化
資料列實作，而不是 Python registry。

#### 1. Extract（提取）
- **入口**：`cargo run --release -- extract`
- **功能**：從 Shapefile、GeoJSON 或已正規化 CSV 提取資料並轉換為標準化 CSV。
- **輸入**：原始 Shapefile、GeoJSON 或 normalized geodata CSV
- **輸出**：`meta_data/{country}_geodata.csv`
- **處理內容**：
  - 讀取 Shapefile/GeoJSON
  - 計算多邊形中心點（使用適當的投影）
  - 轉換為 WGS84 座標
  - 輸出標準化欄位：latitude, longitude, country, admin_1-4

#### 2. Transform（轉換）
- `transform_cities_schema`：將標準 CSV 轉成 CITIES_SCHEMA，負責生成
  geoname_id、對應行政區與補齊時區、國家代碼。

#### 3. Load（載入）
- `admin1_load` 與 `cities500_load`：以 handler 生成資料覆蓋 GeoNames 中對應國家的
  admin1/cities500 紀錄，並保留固定排序、ID 範圍與 schema。

### 常用指令

```bash
cargo run --release -- extract \
  --country TW \
  --shapefile <path_to_tw_shapefile> \
  --output meta_data/tw_geodata.csv

cargo run --release -- release \
  --locationiq-api-key "<api_key>" \
  --country-code KR TH

cargo run --release -- release \
  --fixture-mode \
  --pass-locationiq \
  --output-folder /tmp/rust-release-smoke
```

### 擴充新國家

1. 在 `src/pipeline/extract/handlers.rs` 新增或拆分該國 handler。
2. 實作該國資料來源讀取、座標轉換、行政區欄位對應與 normalized CSV 輸出：
   - `geoname_id` 從 `92_000_000` 起算。
   - 填寫正確時區與 `country_code`。
   - 輸出需符合 `CITIES_SCHEMA`。
3. 若該國使用 Wikidata 翻譯，遵循 P131 parent 驗證標準：
   - 人工查證該國的 Wikidata QID（以即時查詢確認 label），寫入 handler
     常數並附中文名註解；QID 為 Wikidata 永久識別碼，不做執行期查詢。
   - 建構 `TranslationDataset` 時必填 `country_qid`——admin2 對 admin1
     的 QID 驗證、admin1 對國家 QID 驗證，逐級裁決同名歧義。
   - 推行前以 WDQS 驗證該國全部 admin1 可通過 `(wdt:P131)+ <國家QID>`，
     避免標準規則使既有正確翻譯回退。
   - 搜尋語言依該層級在 Wikidata 上的鑑別度選擇（參考：KR 用韓文原文、
     TH 用英文為主＋泰文後備），以實際抽樣搜尋驗證後決定。
4. 補上 Rust fixture、單元測試與真實資料驗證。
5. 執行 Rust gates：
   `cargo fmt --check`、`cargo clippy -- -D warnings`、`cargo test`。

Rust 版本目前採用明確註冊/dispatch，新增國家時需同步更新 CLI country parsing 與
handler routing，避免 runtime magic 造成 release 行為不透明。

### 核心開發循環

開發過程中也可以使用以下命令做基本的檢查與測試：
```bash
cargo fmt --check
cargo clippy -- -D warnings
cargo test
```

## 開發注意事項

- **Rust logging**：production CLI 使用 Rust logging/tracing 慣例，新增 pipeline
  訊息需避免輸出 API key、暫存路徑中敏感資訊或不可重現的 runtime metadata。

### 模組化設計原則
- **單一檔案不得超過 500 行程式碼**
- **每個模組都有清楚的職責分工**
- **Rust public function 需有清楚 rustdoc 或註解**

### 測試要求
- **為所有 Rust production 新功能撰寫 `cargo test` 測試**
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
- **嚴禁臆造不存在的函式或套件** —— 只能使用已知、驗證過的套件。
- **在程式碼或測試中引用檔案路徑或模組名稱前，務必確認其存在。**
- **除非有明確指示，或任務需求（見 `TASK.md`），**否則**不得刪除或覆蓋現有程式碼。**
- **需要分析或拆解問題，通過 sequential thinking 進行更深度思考**
- **與 GitHub 互動需使用 gh CLI**
- **不准在未經允許的情況下，擅自在任何的文檔、訊息等文字中，包含 AI 編輯器或是 AI 模型的名稱**，例如:
  - Generated with [Claude Code]
  - Co-Authored-By: Claude

## Shell 工具使用指引

⚠️ **重要**：使用以下專業工具替代傳統 Unix 指令（若缺少請安裝）：

| 任務類型 | 必須使用 | 禁止使用 |
|---------|---------|---------|
| 檔案搜尋 | `fd` | `find`, `ls -R` |
| 文字搜尋 | `rg` (ripgrep) | `grep`, `ag` |
| 程式碼結構分析 | `ast-grep` | `grep`, `sed` |
| 互動式選擇 | `fzf` | 手動篩選 |
| 處理 JSON | `jq` | 手動解析 |
| 處理 YAML/XML | `yq` | 手動解析 |

## Rust Migration 完成狀態

本專案 production pipeline 已完成 Rust 遷移，Python 實作已退場。後續功能開發、
修正與驗證以 Rust CLI、Rust tests、fixture release smoke 與真實資料 release
gate 為準。遷移過程紀錄見 `docs/history/python-to-rust-migration.md`。
