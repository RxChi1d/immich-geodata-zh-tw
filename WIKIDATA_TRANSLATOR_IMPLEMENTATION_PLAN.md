# Wikidata Translator 批次翻譯重構實作計畫

> 本計畫書延伸自 `WIKIDATA_TRANSLATOR_PRP.md`，聚焦「如何做」。文件採任務導向，讓未參與前期討論的開發者也能直接著手實作與驗收。

## 1. 重要檔案與模組
| 類別 | 路徑 | 說明 |
| --- | --- | --- |
| 翻譯器 | `core/utils/wikidata_translator.py` | 主要重構目標，將新增資料模型、dataloader 與 batch translator。 |
| KR Handler | `core/geodata/south_korea.py` | 驗證新 API 的主要呼叫端，需修改 extract/translate 流程。 |
| 測試 | `tests/utils/test_wikidata_translator.py`（若不存在則新增） | 單元測試集中地。 |
| 文件 | `README.md`, `WIKIDATA_TRANSLATOR_PRP.md`, 本文件 | 說明與追蹤。 |

## 2. 開發階段與時程建議
| 階段 | 對應 PRP 任務 | 預估工時 | 產出 |
| --- | --- | --- | --- |
| Phase A | T1 ~ T3 | 1.5 日 | 新資料模型、dataset builder、dataloader 及單元測試。 |
| Phase B | T4 ~ T6 | 1.5 日 | 改寫 batch/單筆翻譯流程 + KR handler 串接與 smoke test。 |
| Phase C | T7 | 0.5 日 | 文件更新、完整測試與 log 驗證。 |

## 3. 工作分解與指引
以下依 PRP 任務詳列實作步驟、檔案、完成條件。

### T1. `TranslationItem` 與 `AdminLevel`
1. 在 `core/utils/wikidata_translator.py` 上方（class 定義之前）新增：
   - `class AdminLevel(enum.Enum): ADMIN_1 = "admin_1"; ADMIN_2 = "admin_2"`
   - `@dataclass(frozen=True, slots=True) class TranslationItem: ...`
2. 提供 `@classmethod from_row(...)` 幫助 handler 快速從 DataFrame row 建立 item。
3. 實作 `build_item_id(level, parent_chain, name)` helper。需確保：
   - parent_chain 維持 tuple[str, ...]
   - name/parent 於建立時進行 strip、標準化語言代碼。
4. 加入最小單元測試：
   - 同名不同 parent 產生不同 `id`
   - `parent_chain` 順序與輸入相符

**完成條件**：`TranslationItem` 可直接用於 dict key、log；測試通過。

### T2. `TranslationDataset` 與 Builder
1. 在同檔案內建立 `DatasetStats`（NamedTuple 或 dataclass）以輸出總筆數、unique parent、語言對。
2. `class TranslationDataset(Sequence[TranslationItem])`：
   - 保存 `_items`, `_index`（OrderedDict 基於 `(level, parent_chain, name)`）
   - 方法：`__len__`, `__getitem__`, `__iter__`, `stats()`。
3. `class TranslationDatasetBuilder`：
   - `build_admin1(df, country_code, source_lang, target_lang)`：
     - 以 admin_1 欄位為主；去除完全重複 key。
   - `build_admin2(df, country_code, source_lang, target_lang)`：
     - key：`(level, parent_chain=(country_code, admin_1_translated or raw), admin_2_original)`。
     - 允許 `deduplicated=False` 但仍保證 key 唯一。
   - 針對缺失欄位拋出 `ValueError`，並在 log INFO 紀錄 dataset 摘要。
4. 單元測試：
   - fixture 建立 minimal DataFrame，驗證 unique key 與 stats。

**完成條件**：兩個 dataset builder 皆可輸出正確 `TranslationDataset`，測試覆蓋。

### T3. `TranslationDataLoader`
1. 需求：同步 iterator，支援 `batch_size`、`sorter`（callable，對 item 回傳排序 key）。
2. 結構範例：
   ```python
   class TranslationDataLoader:
       def __init__(self, dataset, batch_size, *, sorter=None, progress=None):
           ...
       def __iter__(self):
           items = self.dataset.items_sorted(self.sorter)
           for start in range(0, len(items), self.batch_size):
               batch = items[start:start+self.batch_size]
               if self.progress:
                   self.progress(step=len(batch))
               yield batch
   ```
3. `progress` 參數：
   - 格式：`Callable[[int, int], None]`（processed, total）
   - 若呼叫端未傳入則不輸出。
4. 單元測試：
   - 不同 batch_size 時 processed 應累計到 total。
   - `sorter` 生效（例如依 parent_chain 排序）。

**完成條件**：DataLoader 可在離線測試中提供 deterministic batch，progress callback 正確觸發。

### T4. 重構 `WikidataTranslator.batch_translate`
1. 新增 `class BatchTranslationRunner`（可為內部私有類），職責：
   - 接受 dataset/dataloader
   - 將三階段流程拆成私有方法 `_stage_search`, `_stage_fetch_labels`, `_stage_process`
   - 每階段開始/結束輸出 INFO log（含耗時與進度）
2. 介面：
   ```python
   def batch_translate_dataset(
       self,
       dataset: TranslationDataset,
       *,
       batch_size: int = 20,
       filters: list[CandidateFilter] | None = None,
   ) -> dict[str, TranslationResult]
   ```
3. 與現有 `batch_translate(list[str])` 保持相容：
   - Deprecated pathway：若仍傳入 list/tuple，轉換為臨時 dataset（level = UNKNOWN, parent_chain=(country,)）。
   - 在 log 中加警告提示未來版本僅接受 dataset。
4. INFO log 模板：
   - `logger.info("Admin_2 批次翻譯開始，筆數: {dataset.total}, batch_size={batch_size}")`
   - 進度：`logger.info("Admin_2 進度 {processed}/{total} ({percent:.1f}%)")`
   - 結束：輸出成功/失敗/跳過統計。
5. DEBUG log 保留：候選過濾、HTTP Response、Manually override events。
6. 單元測試：mock Wikidata API，確保 dataset 內多筆資料能完整走完各階段且 log level 正確（可用 caplog）。

**完成條件**：外部呼叫 `batch_translate_dataset` 得到 dict 結果；舊 API 仍可使用但印出提示；測試覆蓋核心邏輯。

### T5. 更新 `core/geodata/south_korea.py`
1. 在 extract 流程中：
   - 建立 `TranslationDatasetBuilder` 實例。
   - 分別呼叫 `build_admin1`, `build_admin2`。
   - 以清楚 log 告知 dataset 規模（沿用 builder stats）。
2. 呼叫新 translator API：
   - `admin1_results = translator.batch_translate_dataset(admin1_dataset, batch_size=32)`
   - `admin2_results = translator.batch_translate_dataset(admin2_dataset, batch_size=32, filters=[custom_filter])`
3. 資料套用：保留既有 `apply_translations` 邏輯，只是結果改讀 `TranslationItem.id` 或 `(parent_chain, original_name)`。
4. 移除舊有按 admin_1 分組 loop 與 tqdm；DEBUG log 改由 batch translator 自行處理。
5. 驗證：
   - 本地使用 KR GeoJSON 跑一次 `python main.py extract --country KR`。
   - 確認 log 乾淨（僅 INFO）且進度顯示正確百分比。

**完成條件**：KR handler 成功產出 CSV，log 無重複與噪音。

### T6. `translate_one` 精簡化
1. 新增 helper：
   ```python
   def translate_one(self, name: str, *, level: AdminLevel, parent_chain: tuple[str, ...], metadata=None):
       item = TranslationItem(...)
       dataset = TranslationDataset([item], level=level, deduplicated=True)
       return self.batch_translate_dataset(dataset, batch_size=1)[item.id]
   ```
2. 對外 API 保持 signature 盡量不變；若舊簽名為 `translate(name, **kwargs)`，則在內部組裝必要參數。
3. 補測試：
   - 成功翻譯回傳 dict
   - 失敗情境（例如搜尋不到 QID）能回傳 fallback 或 raise 既定例外

### T7. 測試、文件、驗收
1. 單元測試：
   - 新增/更新 pytest 檔案，涵蓋 dataset/dataloader/batch translator。
   - 若需 mock requests，使用 `responses` 或 `requests_mock`。
2. 文件：
   - `README.md` 更新「開發循環」或「翻譯流程」段落，描述新進度條與 log 行為。
   - `WIKIDATA_TRANSLATOR_PRP.md` 進度表勾選完成項目。
   - 本實作計畫如有調整也需同步更新。
3. Smoke test：實際跑 KR extract，並截圖或紀錄 log 段落作為驗證附件（可貼到 PR）。

## 4. 驗收清單
- [ ] `python -m pytest tests/utils/test_wikidata_translator.py` 全數通過。
- [ ] `python main.py extract --country KR --shapefile <sample>` 成功；log INFO 僅顯示 summary、進度、結果。
- [ ] 無破壞性 API 變更（舊 CLI 使用方式仍可運作）。
- [ ] 文件同步更新且描述行為一致。

## 5. 溝通與協作建議
- 在 PR 中附加此計畫書連結與 log 範例，供 reviewer 快速理解。
- 若在 Phase B 碰到與 KR handler 綁太緊的情形，可在 PR 分支再拆子任務（例如 `refactor/wikidata-dataloader` → `refactor/kr-handler-adapter`）。
- 若日後其他國家也要使用 Wikidata 翻譯，建議再起一份子計畫，引用此文件的公共模組設計部分。

