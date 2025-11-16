# Wikidata Translator 重構 TODO List

> 用途：在實作過程中逐項檢查並勾選。若子任務需拆分，可在原項目下方新增縮排子項。

## Phase A：資料模型與資料集
- [x] A1. 定義 `AdminLevel` 與 `TranslationItem`，並提供 `from_row`/`build_id` helper。
- [x] A2. 建立 `TranslationDataset`、`DatasetStats`，確保 `(level, parent_chain, name)` key 唯一。
- [x] A3. 實作 `TranslationDatasetBuilder`（admin_1、admin_2），補齊最小單元測試。
- [x] A4. 完成 `TranslationDataLoader`（batch、sorter、progress callback）與測試。

## Phase B：翻譯流程整合
- [x] B1. 引入 `BatchTranslationRunner`（或等效協調類），拆分搜尋/抓標籤/處理三階段。
- [x] B2. 調整 `WikidataTranslator.batch_translate` 只接受 dataset/dataloader，移除舊 list 介面。
- [x] B3. 重新整理 INFO/DEBUG log：移除多餘細節，統一進度、統計輸出。
- [x] B4. 更新 `core/geodata/south_korea.py`：建構 dataset、呼叫新 API、套用結果。
- [x] B5. 更新 `translate_one` 走新的資料模型，確保對外介面簡潔。

## Phase C：測試與文件
- [ ] C1. 擴充/新增 pytest，涵蓋 dataset、dataloader、batch translator、KR handler（可用 fixture/mock）。
- [ ] C2. 執行 `python main.py extract --country KR` 驗證實際 log 與進度顯示。
- [ ] C3. 更新 `README.md`、`WIKIDATA_TRANSLATOR_PRP.md`、`WIKIDATA_TRANSLATOR_IMPLEMENTATION_PLAN.md` 等文檔狀態。
- [ ] C4. 最終 code review/self-check：確保 log 等級、命名、docstring 均符合規範。

> 註：勾選完成時請再次檢查對應測試、文件與 log 是否達預期，避免遺漏。
