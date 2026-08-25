# 貢獻指南

繁體中文 | [English](docs/en/contributing.md)

歡迎回報問題與提交修改。本文說明參與開發需要知道的流程與規範。

## 回報問題

到 [Issues](https://github.com/RxChi1d/immich-geodata-zh-tw/issues) 開新議題。安裝或更新問題請附上 Immich 版本、部署方式與相關日誌；地名錯誤請附上座標、目前顯示的名稱與預期的名稱。

安全性問題請依 [SECURITY.md](SECURITY.md) 私下回報，不要開公開 issue。

## 開發環境

資料處理工具鏈以 Rust 撰寫。環境準備、資料提取與 release 產生的操作說明見[本地資料處理](docs/zh-tw/development.md)。

提交前請確認以下檢查通過：

```bash
cargo fmt --check
cargo clippy -- -D warnings
cargo test
```

## 分支與提交

- **分支命名**：遵循 Conventional Branch Naming，例如 `feat/add-vietnam-handler`、`fix/install-path`。
- **Commit 訊息**：遵循 [Conventional Commits](https://www.conventionalcommits.org/)，第一行 50–72 字元。描述可使用繁體中文、簡體中文或英文。
- **Pull Request**：標題同樣使用 Conventional Commits 格式，內容依 [PR 模板](.github/pull_request_template.md)填寫。

PR 標題會用於自動分類與產生發布說明，格式不符合 Conventional Commits 時 CI 會擋下合併。第一行請盡量控制在 72 字元內。

## 撰寫規範

- **程式碼**：註解與文件字串使用繁體中文（臺灣用語），函式與變數命名使用英文。
- **文件**：繁體中文（臺灣用語）版本位於 `docs/zh-tw/`，英文版本位於 `docs/en/`。遵循 Google 寫作風格：簡單、不冗餘、易於理解。
- **溝通**：commit 訊息、PR 與 issue 可使用繁體中文、簡體中文或英文。
- 變更若影響使用方式（安裝步驟、參數、資料呈現結果），請一併更新對應的說明文件。

## CHANGELOG

`CHANGELOG.md` 由維護者在發版時彙整，**不需要在 PR 中修改**。發布說明會依此產生，而條目是以使用者的角度撰寫，因此需要跨 PR 統一取捨。

請確保 PR 標題準確描述變更，它是彙整時的依據。

## 測試要求

新功能與錯誤修復都應附上 `cargo test` 測試，至少涵蓋正常情境、邊界情況與失敗情況。測試置於 `tests/`，測試資料使用 fixtures。

## 資料檔案的保護規則

`meta_data/*_geodata.csv` 是由各國官方圖資產生、並由 release workflow 驗證的正式資料，不是可重新產生的建置產物。除了明確的資料更新任務外，請勿刪除、重新產生或正規化這些檔案。

PR 變更到這些檔案時，需加上 `data-update` 標籤以確認這是刻意的資料更新，否則 CI 會擋下合併。

## 新增支援的國家

1. 在 `src/pipeline/extract/handlers.rs` 新增該國 handler，並同步更新 CLI 的國家解析與 handler routing。
2. 實作資料來源讀取、座標轉換與行政區欄位對應，輸出符合 `CITIES_SCHEMA` 的 CSV：`geoname_id` 自 `92_000_000` 起算，並填入正確的時區與 `country_code`。
3. 若該國使用 Wikidata 翻譯，須遵循 P131 隸屬驗證標準：
   - 人工查證該國的 Wikidata QID 並寫入 handler 常數，附上中文名註解；QID 不做執行期查詢。
   - 建構 `TranslationDataset` 時必填 `country_qid`，逐級驗證 admin2 對 admin1、admin1 對國家的隸屬關係。
   - 推行前以 WDQS 確認該國全部 admin1 都能通過 `(wdt:P131)+ <國家QID>`，避免既有正確譯名回退。
   - 搜尋語言依該層級在 Wikidata 上的鑑別度選擇，並以實際抽樣驗證後決定（例如南韓使用韓文原文、泰國以英文為主並以泰文後備）。測試集建議涵蓋該國的結構性同名類別，再加上隨機樣本，並固定亂數種子以便重現；作法可參考[印尼行政區處理](docs/zh-tw/indonesia-admin-processing.md)記錄的實驗。
   - Wikidata 譯名的失敗是無聲的，且多數防線只在個別國家的 handler 內。動手前請先讀[Wikidata 譯名的已知失效形態](docs/zh-tw/wikidata-translation.md)。
4. 補上 fixture、單元測試與真實資料驗證，並新增 `docs/zh-tw/` 的對應說明文件。
