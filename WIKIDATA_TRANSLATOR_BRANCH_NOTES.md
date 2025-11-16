# Wikidata Translator 開發分支紀錄

本文檔彙整 `refactor/wikidata-translator-progress` 分支自建立以來的重要增修，方便快速掌握目前未發布版本的狀態。

## 1. 資料集與進度控制

- 新增 `TranslationDatasetBuilder / TranslationDataset / TranslationDataLoader`：
  - Builder 由 handler 的 DataFrame 產出 `TranslationItem`（內含 `id`, level, parent_chain, metadata）。
  - Dataset 實作 `Sequence` 並保存統計資訊（總筆數、唯一 parent 數、語言對）。
  - DataLoader 依 `batch_size` 交付批次，並透過 `progress_callback` 對接 `tqdm` 或 `ProgressLogger`。
- `BatchTranslationRunner` 使用上述骨架，統一控制 Admin_1/2 批次翻譯流程與進度條輸出。

## 2. 批次翻譯流程優化

- 保留三階段流程：搜尋 → 候選過濾 → 批次抓標籤 → 處理結果。
- `show_progress=True` 時改用 `tqdm` 進度列，完成後保留結果；`show_progress=False` 則輸出 INFO 級進度百分比。
- 翻譯結果（含 cache 命中、fallback 統計）會在階段三結束統一彙總。

## 3. P131 驗證與測試

- 維持逐筆 `_verify_p131`（`ASK { wd:child (wdt:P131)+ wd:parent }`）策略；新增單元測試覆蓋 `translate()` 與 dataset 版 `batch_translate()` 在有 `parent_qid` 時皆會驗證並設置 `parent_verified`。

## 4. 快取機制改進

- 快取層級：`translations`, `cache.search`, `cache.labels`, `cache.p131`, `cache.instance_of`。
- 新增「隨寫隨沖」策略：
  - 任何快取寫入都呼叫 `_mark_cache_dirty()`，累計達 20 筆或 30 秒就自動 `_save_cache()`。
  - `BatchTranslationRunner` 在批次結束時 `force=True` 再 flush 一次。
  - 目的是避免長時間處理（例如 Admin_2）中途被中斷時，前幾百筆查詢成果遺失。
