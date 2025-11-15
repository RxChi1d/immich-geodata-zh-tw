# Wikidata Translator 批次翻譯重構 PRP

## 1. 簡介
本文件彙整目前 Wikidata 批次翻譯流程的痛點、設計目標與開發計畫，作為後續實作與驗收的依據。範圍聚焦於 `core/utils/wikidata_translator.py` 與依賴此模組的韓國資料處理流程，JP/TW handler 不在此次調整之列。

## 2. 目標與成功指標
- **統一進度追蹤**：任何批次翻譯任務都能得知「總筆數 / 已處理」與百分比，並顯示清楚的階段性 log。
- **可維運的 log**：預設 INFO 僅顯示與生產相關的高層摘要；細節移至 DEBUG 才會輸出。
- **資料模型抽象**：建立可復用的資料集 (dataset) 與批次器 (dataloader)，分離 geodata handler 與翻譯器的責任。
- **介面一致性**：批次與單筆翻譯共用相同核心邏輯，單筆仍可簡潔呼叫，不強迫使用 dataset/dataloader。
- **最小變動原則**：保持現有對外 CLI 行為（參數、輸出檔案）不變，必要時才調整。

## 3. 現況痛點總結
1. Admin_2 依上級分組後逐批翻譯，導致無法得知全域進度，只能看到「正在翻譯 XX 下 n 筆」的零碎訊息。
2. Log 同時輸出 INFO/DEBUG 細節，包含候選 QID 過濾、手動對照等，造成生產執行時畫面極度雜亂。
3. Geodata handler 必須自己管理去重、分組、快取等細節，`WikidataTranslator` 無法重用這些邏輯。
4. 單筆翻譯 API 目前只是 batch translate 的包裝，缺乏明確契約；重構後若不整理容易產生技術債。

## 4. 範圍與出入
- **In scope**：KR handler 內所有透過 Wikidata 取得的 Admin_1、Admin_2 名稱；`core/utils/wikidata_translator.py` 的資料模型、批次流程、log 設定、快取使用方式。
- **Out of scope**：JP/TW 的手動翻譯流程、其它 ETL 階段、資料儲存格式、CLI 參數/輸出位置變動。

## 5. 系統設計
### 5.1 資料結構
```python
@dataclass(frozen=True, slots=True)
class TranslationItem:
    id: str                      # 以 level + parent_chain + 原名組成
    level: AdminLevel            # Enum: ADMIN_1 或 ADMIN_2
    original_name: str
    source_lang: str             # 例如 "ko"
    target_lang: str             # 例如 "zh-hant"
    parent_chain: tuple[str, ...]  # ("KR",) 或 ("KR", "京畿道")
    metadata: Mapping[str, Any]  # geoname_id、row_idx、manual flag 等
```
- `parent_chain` 為不可變 tuple，以國家碼起始，往下堆疊父層名稱。此結構對 key 與快取都足夠嚴謹，只需在建立 `TranslationItem` 時標準化元素（同語系、同格式）。
- `id` 建議為 `"KR/admin2/京畿道/始興市"` 形式，方便快取與 log。

### 5.2 TranslationDataset
```python
class TranslationDataset(Sequence[TranslationItem]):
    level: AdminLevel
    deduplicated: bool
    total: int
    stats(): DatasetStats
```
- 建構時使用 OrderedDict 以 `(level, parent_chain, original_name)` 為 key 控制去重；Admin_1 通常 `deduplicated=True`，Admin_2 預設 `False`（允許不同父層同名共存）。
- `stats()` 回報總筆數、獨特 parent 數、語言對等資訊，供 CLI/日誌使用。

### 5.3 TranslationDatasetBuilder
- 輸入：整理後的 DataFrame + `country_code`。
- 輸出：`build_admin1()` 與 `build_admin2()` 兩個 dataset，皆包含足夠 metadata（row index、admin codes、手動 override 標記）。
- Builder 只執行欄位驗證與去重，不負責 batching 或翻譯。

### 5.4 TranslationDataLoader
- 介面類似 PyTorch DataLoader 的同步版本。
- 參數：`batch_size`, `sorter`（optional，用來決定輸出順序），`progress_callback`（optional，但 batch translator 會強制提供）。
- 實作重點：初始化時計算總筆數，`__iter__` 每 yield 一個 list[TranslationItem] 後呼叫 callback 更新 `processed`。

### 5.5 批次翻譯流程
1. Handler 呼叫 builder 取得 dataset。兩層級各跑一次 `batch_translate(level)`。
2. `BatchTranslator`（`WikidataTranslator` 內的新協調類）負責：
   - 接收 dataset/dataloader
   - 管理三階段流程（搜尋 QID → 取 label → 處理結果）
   - 將進度、成功/失敗統計輸出到統一的 INFO log
   - 將候選過濾、特殊處理放到 DEBUG
3. Flow 結束後回傳結果 map，handler 再套用到 DataFrame。

### 5.6 單筆翻譯
- 新增 `TranslationItem.from_single(name, level, parent_chain, ...)` 或 helper，讓單筆翻譯可直接建構 item。
- `translate_one()` 內部 reuse `BatchTranslator` 核心邏輯，但跳過 dataloader（直接傳一筆 list）。

### 5.7 Log 與進度策略
- INFO：
  - dataset 構建摘要（筆數、dedup 狀態）
  - 各階段開始/完成（含耗時）
  - 統一進度列，例如 `Admin_2 翻譯 120/230 (52%)`
  - 結束統計（成功/失敗/跳過）
- DEBUG：候選過濾結果、特殊手動對照、HTTP 結果摘要。
- TRACE（optional）：需要時可保留極細資訊。
- 使用 loguru 既有 handler，僅調整訊息與層級，不強迫使用 rich/tqdm 視覺化。

## 6. 開發計畫與進度追蹤
| 編號 | 項目 | 說明 | 負責 | 狀態 |
| --- | --- | --- | --- | --- |
| T1 | 建立 `TranslationItem`、`AdminLevel` Enum | 定義 dataclass 與輔助函式 | Codex | ☐ |
| T2 | 實作 `TranslationDataset` 與 builder | 含 dedup 機制、stats | Codex | ☐ |
| T3 | 建立 `TranslationDataLoader` | 支援 batch_size、progress callback | Codex | ☐ |
| T4 | 重構 `WikidataTranslator.batch_translate` | 改用 dataset/dataloader，輸出統一 log | Codex | ☐ |
| T5 | 更新 KR handler | 產生 dataset、串接新 API、改善 log | Codex | ☐ |
| T6 | 重整 `translate_one` | 輕量封裝、沿用新資料模型 | Codex | ☐ |
| T7 | 測試與文件 | 補齊單元測試、README/PRP 更新 | Codex | ☐ |

> 註：狀態欄可在開發過程中以 `☑` / `☐` 更新；如需拆分任務再細分編號（例如 T5a/T5b）。

## 7. 驗收與測試
- 單元測試覆蓋：
  - Dataset builder 對同名不同父層的處理
  - DataLoader 進度計數
  - BatchTranslator 在 mock Wikidata API 下的成功與錯誤流程
- 整合測試：`python main.py extract --country KR` 可完成並輸出乾淨 log。
- 需更新 README 或相關文件，描述新的進度/log 行為（若 CLI 觀察到變化）。

## 8. 風險與緩解
1. **API 變動波及既有 handler**：採漸進式重構，先保留舊入口包裝新邏輯，確保外部呼叫不變。
2. **進度 log 與現有 tqdm 衝突**：新方案以 loguru 為主，若必要再評估移除 tqdm 依賴。
3. **快取鍵值變動**：`TranslationItem.id` 若與現有快取 key 不同，需要提供遷移或向下相容邏輯；計畫先保留原始 name 為 cache key，避免資料遺失。

## 9. 待確認 / 未決事項
- `metadata` 欄位標準：後續若需跨 handler 共用，可能要定義 schema；目前保持寬鬆。
- 是否保留 tqdm：若最終 INFO log 已足夠，可能可以完全移除，以減少輸出混亂。
- 如需記錄更細粒度的進度（例如每個行政區各自完成時間），可在後續迭代加入額外統計欄位。

