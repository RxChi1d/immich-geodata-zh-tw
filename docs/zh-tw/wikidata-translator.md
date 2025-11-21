# WikidataTranslator 說明文檔

本文檔說明 WikidataTranslator 的設計理念、核心功能、翻譯策略與技術實作邏輯。

## 目錄

- [WikidataTranslator 說明文檔](#wikidatatranslator-說明文檔)
  - [目錄](#目錄)
  - [快速開始](#快速開始)
  - [設計理念](#設計理念)
  - [核心功能](#核心功能)
  - [翻譯流程](#翻譯流程)
    - [批次翻譯流程](#批次翻譯流程)
      - [資料集與進度控制](#資料集與進度控制)
    - [單一翻譯介面](#單一翻譯介面)
  - [翻譯策略](#翻譯策略)
    - [多層回退機制](#多層回退機制)
    - [P131 層級關係驗證](#p131-層級關係驗證)
    - [候選過濾機制](#候選過濾機制)
  - [快取機制](#快取機制)
    - [快取結構](#快取結構)
    - [快取同步策略](#快取同步策略)
  - [批次查詢優化](#批次查詢優化)
  - [錯誤處理與重試機制](#錯誤處理與重試機制)
    - [速率限制處理](#速率限制處理)
    - [重試機制](#重試機制)
    - [錯誤降級處理](#錯誤降級處理)
  - [使用範例](#使用範例)
    - [批次翻譯](#批次翻譯)
    - [單一翻譯](#單一翻譯)
    - [使用 P131 驗證](#使用-p131-驗證)
    - [使用候選過濾器](#使用候選過濾器)
    - [Admin 2 批次翻譯](#admin-2-批次翻譯)
    - [不同語言翻譯](#不同語言翻譯)
  - [API 端點與速率限制](#api-端點與速率限制)

---

## 快速開始

```python
from core.utils.wikidata_translator import WikidataTranslator

translator = WikidataTranslator(source_lang="ko", target_lang="zh-tw")
result = translator.translate("서울특별시")
print(result["translated"])  # 輸出: 首爾特別市
```

完整使用方式請參閱 [使用範例](#使用範例) 章節。

---

## 設計理念

WikidataTranslator 是一個通用的地名翻譯工具，透過 Wikidata 將任意語言的地名翻譯為繁體中文（或其他目標語言）。

**核心設計目標**：

1. **通用性**：支援任意來源語言到目標語言的翻譯
2. **準確性**：透過 P131 關係驗證選擇正確的同名地名
3. **效能**：批次查詢與多層快取減少 API 請求次數
4. **可靠性**：速率限制、重試機制與錯誤處理確保穩定運作
5. **靈活性**：支援候選過濾、多層回退策略

---

## 核心功能

WikidataTranslator 提供以下核心功能：

| 功能 | 說明 |
|------|------|
| **批次翻譯** | `batch_translate(dataset, parent_qids)` - 透過 `TranslationDataset` 批次翻譯並驅動統一進度 |
| **資料集與進度控制** | 透過 `TranslationDataset` / `TranslationDataLoader` 封裝待翻譯項目，統一追蹤總筆數、進度條與逐列快取 |
| **單一翻譯介面** | `translate(name, parent_qid)` - 內部建立單筆 dataset 後呼叫批次翻譯，適合快速測試與少量翻譯 |
| **P131 驗證** | 透過 Wikidata P131（located in）關係驗證地名層級關係 |
| **候選過濾** | 提供自訂過濾器排除不符合條件的候選實體 |
| **多層快取** | 分層快取搜尋結果、標籤、P131 驗證、翻譯結果 |
| **快取自動同步** | 採用「隨寫隨沖」策略，達 **20 筆**或 **30 秒**自動同步快取，避免長時間處理中斷時資料遺失 |
| **簡轉繁** | 透過 OpenCC 將簡體中文標籤轉換為繁體中文 |
| **維基百科標題轉換** | 使用中文維基百科 API 進行標題簡繁轉換 |

---

## 翻譯流程

### 批次翻譯流程

`batch_translate()` 是翻譯器的核心實作，採用三階段處理，最大化批次查詢效率：

```
階段 1: 搜尋階段
  ├─ 透過 DataLoader 依 batch_size 迭代 dataset
  ├─ 逐一搜尋地名取得候選 QID（Wikidata API 不支援批次搜尋）
  ├─ 檢查翻譯快取，已快取的直接返回
  ├─ 收集所有候選 QID
  └─ 更新進度（透過 progress_callback）

階段 1.5: 候選過濾（可選）
  ├─ 批次取得所有候選 QID 的標籤與 P31（instance of）
  ├─ 應用自訂過濾器函式
  └─ 過濾不符合條件的候選實體

階段 2: 批次取得標籤
  ├─ 收集所有候選 QID（去重）
  ├─ 批次查詢標籤（每批最多 50 個 QID）
  └─ 快取標籤結果

階段 3: 選擇最佳翻譯與快取寫入
  ├─ 根據 P131 驗證選擇正確的 QID
  ├─ 應用多層回退策略選擇最佳標籤
  ├─ 快取翻譯結果（每筆寫入都觸發 _mark_cache_dirty）
  ├─ 依照髒污次數或時間間隔自動同步快取
  └─ 返回翻譯結果
```

**關鍵優化**：階段 2 使用批次 API（每次最多 **50 個 QID**），大幅減少請求次數。例如翻譯 250 個地名，使用批次查詢只需約 5 次 API 請求，而非 250 次。

#### 資料集與進度控制

批次翻譯以 `TranslationDatasetBuilder → TranslationDataset → TranslationDataLoader → BatchTranslationRunner` 為骨架：

1. **TranslationDatasetBuilder**：處理 handler 提供的 DataFrame，產生 `TranslationItem`（包含 `id`、原始名稱、admin level、parent chain 等 metadata）。Admin_1 與 Admin_2 各自轉成 dataset，以便獨立翻譯。

2. **TranslationDataset**：實作 `Sequence` 介面並保留統計資訊（總筆數、唯一 parent 數、語言對），方便 log 與進度輸出。提供 `stats()` 方法取得資料集摘要。

3. **TranslationDataLoader**：依 `batch_size` 迭代 dataset，並透過 `progress_callback` 回報進度。支援自訂排序策略（`sorter` 參數）。

4. **BatchTranslationRunner**：協調三階段翻譯流程並控制進度顯示：
   - `show_progress=True` 時使用 `tqdm` 進度條，完成後保留結果
   - `show_progress=False` 時使用 `ProgressLogger`，在 INFO 級別輸出進度百分比（0%, 5%, 10%, ..., 100%）

**設計優勢**：
- 將翻譯邏輯與資料來源解耦：handler 只需負責構建 dataset，翻譯器專注於批次查詢與快取管理
- 統一進度控制介面：不論是進度條或日誌輸出，都透過相同的 callback 機制
- 靈活的批次大小控制：可依據網路狀況或 API 限制調整 batch_size

### 單一翻譯介面

`translate()` 方法提供簡化的單筆翻譯介面，內部實作為建立臨時單筆 `TranslationDataset` 後呼叫批次翻譯核心：

```
translate(name, parent_qid)
  ↓
建立臨時單筆 TranslationDataset
  ↓
呼叫 batch_translate(dataset, parent_qids)
  ↓
返回單筆結果
```

**設計理念**：
- **統一邏輯**：單一翻譯與批次翻譯共用相同的核心邏輯
- **功能完整**：P131 驗證、候選過濾等所有批次翻譯功能在單一翻譯也可使用
- **易於維護**：只需維護批次翻譯的邏輯，單一翻譯自動繼承所有改進
- **使用便利**：提供簡單的單筆翻譯介面，無需手動建立 dataset

**何時使用**：
- 快速翻譯單一地名
- 互動式測試與除錯
- 少量（< 10 筆）即時翻譯需求

**建議**：
- 大量翻譯（> 10 筆）建議直接使用 `batch_translate()` 並搭配 `TranslationDataset`，可獲得更好的效能與進度追蹤

---

## 翻譯策略

### 多層回退機制

翻譯器使用多層回退策略確保翻譯成功率。回退順序由 `fallback_langs` 參數控制，預設為 `["zh-hant", "zh", "en", source_lang]`：

```
1. 目標語言（target_lang）   ← 最優先（如 zh-tw）
   ↓ 無標籤
2. 回退語言 1（zh-hant）     ← 繁體中文通用
   ↓ 無標籤
3. 回退語言 2（zh）          ← 簡體中文 + OpenCC 轉繁體
   ↓ 無標籤
4. 回退語言 3（en）          ← 英文
   ↓ 無標籤
5. 回退語言 4（source_lang） ← 來源語言原文（如 ja, ko, vi, th）
   ↓ 無標籤
6. 中文維基百科標題          ← 使用 converttitles API 轉繁體
   ↓ 無標籤
7. 原始輸入名稱              ← 最終備案
```

> **說明**：回退語言列表可自訂，但預設順序已針對繁體中文翻譯優化（優先中文系標籤，次要英文，最後保留原文）。

**範例 1**：Wikidata 有目標語言標籤（優先使用）

```
翻譯「Tokyo」（日文：東京）
1. zh-tw: 東京 ✅ → 返回「東京」
```

**範例 2**：Wikidata 僅有簡體中文標籤（透過 OpenCC 轉換）

```
翻譯某個地名
1. zh-tw: (無) ❌
2. zh-hant: (無) ❌
3. zh: 伦敦 → OpenCC 轉換 → 倫敦 ✅
```

**範例 3**：Wikidata 僅有英文標籤（回退至英文）

```
翻譯某個小城鎮
1. zh-tw: (無) ❌
2. zh-hant: (無) ❌
3. zh: (無) ❌
4. en: Springfield ✅ → 返回「Springfield」
```

**範例 4**：Wikidata 僅有來源語言標籤（回退至原文）

```
翻譯日文地名「〇〇町」
1. zh-tw: (無) ❌
2. zh-hant: (無) ❌
3. zh: (無) ❌
4. en: (無) ❌
5. ja: 〇〇町 ✅ → 返回「〇〇町」（保留日文原文）
```

### P131 層級關係驗證

Wikidata 的 P131（located in）屬性定義行政區的層級關係。當搜尋結果包含多個同名地名時，透過 P131 驗證選擇正確的實體。

**問題範例**：世界各地有許多同名的行政區

- **Springfield**：美國至少有 30 個以上的 Springfield
- **中區**：東亞地區（日本、韓國、中國）有多個同名「中區」
- **San José**：西班牙、拉丁美洲有多個同名城市

**解決方案**：提供父級 QID 進行層級驗證

```python
# 範例：翻譯「中區」，指定父級為「大阪」（Q35765）
translator.translate("中区", parent_qid="Q35765")
# → 選擇大阪市中區（Q54886752），而非東京中央區或橫濱市中區
```

**驗證邏輯**（SPARQL）：

```sparql
ASK { wd:Q54886752 (wdt:P131)+ wd:Q35765 . }
# 檢查 Q54886752（候選中區）是否位於 Q35765（大阪市）之內
# (wdt:P131)+ 表示遞迴檢查（可跨越多層行政區）
```

**快取優化**：P131 驗證結果會被快取（格式：`{候選QID}_{父級QID}`），相同的層級關係查詢不會重複請求。

### 候選過濾機制

批次翻譯支援自訂過濾器函式，在階段 1.5 排除不符合條件的候選實體。過濾器接收候選的 metadata 並回傳 `True`（保留）或 `False`（排除）。

**過濾器函式簽名**：

```python
def candidate_filter(name: str, metadata: dict) -> bool:
    """
    Args:
        name: 原始地名
        metadata: {
            'qid': 候選 QID,
            'labels': {語言: 標籤},
            'instance_of': [P31 QID 列表]
        }
    Returns:
        True = 保留候選，False = 排除候選
    """
```

**使用範例**：排除政府機構類實體

```python
def filter_administrative_divisions_only(name: str, metadata: dict) -> bool:
    """僅保留行政區，排除政府機構（議會、辦公室、政府部門等）。"""
    labels = metadata.get("labels", {})

    # 定義政府機構關鍵字（多語言）
    government_keywords = [
        "council", "assembly", "government", "office", "department",  # 英文
        "議會", "政府", "辦公室", "部門", "廳",                        # 中文
        "conseil", "gobierno", "правительство"                        # 其他語言
    ]

    # 檢查所有語言的標籤
    for label in labels.values():
        if any(keyword in label.lower() for keyword in government_keywords):
            return False  # 排除政府機構

    return True  # 保留行政區

# 應用過濾器
translator.batch_translate(
    names=["Tokyo", "Paris", "Berlin"],
    candidate_filter=filter_administrative_divisions_only
)
```

**應用場景**：

- 排除博物館、學校、公司等非地理實體
- 排除歷史行政區（已廢除的行政區劃）
- 根據 P31（instance of）過濾特定類型實體

**效能考量**：過濾器在階段 1.5 執行，使用批次查詢取得所有候選的標籤與 P31，避免逐一查詢。

---

## 快取機制

WikidataTranslator 由 `TranslationCacheStore` 集中管理快取。所有翻譯結果與搜尋結果都採 **context-aware key**（`TranslationItem.id = level/parent_chain/name`），確保同名但不同父層的行政區擁有獨立快取。快取以 JSON 儲存（cache schema v1.0），若偵測到舊版會自動備份並重新建立。

> **版本說明**：快取 schema 版本是資料格式版本，與專案發布版本獨立。只有在快取資料結構不相容時才會升級 schema 版本。

### 快取結構

```jsonc
{
  "metadata": {
    "version": "1.0",
    "source_lang": "ja",
    "target_lang": "zh-tw",
    "created_at": "2025-11-15T10:30:00",
    "last_compacted_at": null
  },
  "translations": {
    "admin_2/KR/首爾/城東區": {
      "original_name": "城東區",
      "translated": "城東區",
      "qid": "Q1490",
      "source": "wikidata",
      "used_lang": "zh-tw",
      "level": "admin_2",
      "parent_chain": ["KR", "首爾"],
      "parent_qid": "PARENT1",
      "parent_verified": true,
      "context_hash": "cf82b8c7",
      "cached_at": "2025-11-15T10:31:25",
      "ttl": null
    }
  },
  "cache": {
    "search": {
      "admin_2/KR/首爾/城東區": ["Q1490", "Q123456"]
    },
    "labels": {
      "Q1490": {
        "zh-tw": "東京都",
        "en": "Tokyo",
        "ja": "東京都"
      }
    },
    "p131": {
      "Q54886752_Q35765": true
    },
    "instance_of": {
      "Q1490": ["Q50337", "Q515"]
    }
  },
  "indexes": {
    "by_name": {
      "城東區": ["admin_2/KR/首爾/城東區", "admin_2/KR/京畿道/城東區"]
    }
  }
}
```

> **注意**：context-aware 設計要求所有快取鍵均包含 `level + parent_chain + original_name`。升級至 v1.0 後會重新填滿整份快取（舊資料僅以 `.bak` 備份）。

**快取層級說明**：

| 層級 | 用途 | 快取鍵 | 快取值 |
|------|------|--------|--------|
| **metadata** | 快取檔案元資料 | - | 來源/目標語言、建立時間、版本、壓縮紀錄 |
| **translations** | 最終翻譯結果 | `TranslationItem.id` | 翻譯結果 + parent_chain + parent_qid + cached_at |
| **cache.search** | Wikidata 搜尋結果 | `TranslationItem.id` | 候選 QID 列表（依父層切割） |
| **cache.labels** | 實體的多語言標籤 | QID | {語言: 標籤} |
| **cache.p131** | P131 驗證結果 | `{候選QID}_{父級QID}` | true/false |
| **cache.instance_of** | P31 屬性（實例類型） | QID | [P31 QID 列表] |
| **indexes.by_name** | 偵錯索引 | 地名 | 對應的 context-aware key 陣列 |

**快取查詢優先順序**：

1. **translations**：命中即返回，避免重複查詢與驗證
2. **cache.search**：同名不同父層擁有獨立候選列表
3. **cache.labels**：沿用既有 QID 標籤，減少 `wbgetentities` 請求
4. **cache.p131**：記錄 `候選QID → 父層 QID` 驗證結果
5. **cache.instance_of**：提供候選過濾器使用的 P31 類型資訊

### 快取同步策略

為了避免長時間處理（例如 Admin 2）中途被中斷時，前幾百筆查詢成果遺失，`TranslationCacheStore` 採用「隨寫隨沖」（Write-Through with Deferred Flush）策略：

**觸發機制**：

1. 任何快取寫入（搜尋、標籤、P31、P131、翻譯結果）都會在記憶體更新後呼叫 store 的 `mark_dirty()`
2. `mark_dirty()` 累計髒污筆數並自動檢查是否需要同步
3. 達到以下**任一條件**時自動執行 `save()`：
   - 累計達 **20 筆**髒污資料
   - 距離上次儲存超過 **30 秒**
4. `BatchTranslationRunner` 在階段 3 完成時請求 store 強制執行最後一次同步

**優勢**：
- **容錯性提升**：即便翻譯過程中遭遇網路中斷或手動終止，也只會損失最後極少數尚未 flush 的筆數
- **效能平衡**：不會每筆都寫入（避免過度 I/O），也不會等到全部完成才寫入（避免中斷損失）
- **透明化**：開發者無需手動呼叫儲存，翻譯器自動管理快取同步

**原子寫入保護**：
- 使用臨時檔案（`.tmp`）+ `rename()` 確保快取檔案不會因寫入過程中斷而損毀
- 即使同步過程失敗，原快取檔案仍保持完整

---

## 批次查詢優化

批次翻譯透過 Wikidata API 的批次查詢功能大幅減少請求次數。

**批次查詢方法**：

| 方法 | 功能 | 批次大小 | API 端點 |
|------|------|----------|----------|
| `_batch_get_labels()` | 取得多個 QID 的標籤 | **50 個/批** | wbgetentities |
| `_batch_get_instance_of()` | 取得多個 QID 的 P31 | **50 個/批** | wbgetentities |

**批次查詢流程**：

```
輸入: [Q8684, Q41164, Q515, ...]

步驟 1: 去重與快取過濾
  └─ 過濾已快取的 QID

步驟 2: 分批查詢（每批 50 個）
  ├─ 批次 1: Q8684|Q41164|Q515|...（50 個）
  ├─ 批次 2: Q12345|Q67890|...（50 個）
  └─ ...

步驟 3: 解析回應並快取
  └─ 將標籤/P31 儲存到 cache.labels / cache.instance_of

步驟 4: 返回結果
  └─ {qid: labels} 或 {qid: [P31_qids]}
```

**效能比較**（翻譯 250 個地名）：

| 方法 | 標籤查詢次數 | 總請求數 |
|------|-------------|---------|
| 逐一查詢 | 250 次 | ~500 次 |
| 批次查詢（50 個/批） | 5 次 | ~260 次 |

**節省比例**：約 48% 的 API 請求數

---

## 錯誤處理與重試機制

翻譯器實作多層錯誤處理確保穩定性：

### 速率限制處理

**問題**：Wikidata 對 API 請求有速率限制，超過限制會返回 HTTP 429。

**解決方案**：

1. **主動速率限制**：每次請求後延遲固定時間
   - SPARQL 查詢：0.8 秒
   - Wikidata API：0.2 秒
   - 中文維基百科 API：0.2 秒

2. **被動速率限制**：收到 429 回應時，讀取 `Retry-After` 標頭並等待

```python
if response.status_code == 429:
    retry_after = int(response.headers.get("Retry-After", 5))
    time.sleep(retry_after)
```

### 重試機制

**策略**：指數退避（Exponential Backoff）+ 抖動（Jitter）

```python
for attempt in range(MAX_RETRIES):  # 最多重試 5 次
    try:
        response = self.session.get(url, params=params, timeout=30)
        response.raise_for_status()
        return response.json()
    except RequestException:
        base_wait = 2 * (attempt + 1)        # 2, 4, 6, 8, 10 秒
        jitter = random.uniform(-0.2, 0.2)   # ±20% 抖動
        wait_time = base_wait * (1 + jitter)
        time.sleep(wait_time)
```

**抖動目的**：避免多個請求同時失敗、同時重試造成的羊群效應（Thundering Herd）。

### 錯誤降級處理

各個翻譯階段的錯誤處理策略：

| 階段 | 錯誤處理 | 降級策略 |
|------|----------|----------|
| 搜尋失敗 | 記錄警告 | 返回空候選列表 |
| 標籤取得失敗 | 記錄警告 | 返回空標籤 |
| P131 驗證失敗 | 記錄警告 | 返回 False（視為未驗證） |
| OpenCC 轉換失敗 | 記錄警告 | 使用原始簡體標籤 |
| 維基百科轉換失敗 | 記錄警告 | 使用原始標題 |

**設計理念**：單一地名翻譯失敗不應影響批次翻譯的其他地名。

---

## 使用範例

### 批次翻譯

```python
from core.utils.wikidata_translator import (
    WikidataTranslator,
    TranslationDatasetBuilder,
)

# 建立翻譯器（日文 → 繁體中文）
translator_ja = WikidataTranslator(
    source_lang="ja",
    target_lang="zh-tw",
    fallback_langs=["zh-hant", "zh", "en", "ja"],
    cache_path="geoname_data/JP_wikidata_cache.json",
    use_opencc=True
)

# 建立 dataset builder
builder = TranslationDatasetBuilder(
    country_code="JP",
    source_lang="ja",
    target_lang="zh-tw"
)

# 準備資料並建立 dataset
records = [{"sidonm": name} for name in ["東京都", "大阪府", "京都府"]]
dataset = builder.build_admin1(records, name_field="sidonm")

# 批次翻譯（show_progress=True 會顯示 tqdm 進度條）
results = translator_ja.batch_translate(
    dataset,
    batch_size=16,
    show_progress=True
)
# 返回: {'JP/admin_1/東京都': {...}, 'JP/admin_1/大阪府': {...}, ...}
```

### 單一翻譯

```python
from core.utils.wikidata_translator import WikidataTranslator

# 建立翻譯器
translator_ja = WikidataTranslator(
    source_lang="ja",
    target_lang="zh-tw",
    fallback_langs=["zh-hant", "zh", "en", "ja"],
    cache_path="geoname_data/JP_wikidata_cache.json",
    use_opencc=True
)

# 單一翻譯（內部會建立臨時 dataset 後呼叫 batch_translate）
result = translator_ja.translate("東京都")
# {'translated': '東京都', 'qid': 'Q1490', 'source': 'wikidata',
#  'used_lang': 'zh-tw', 'parent_verified': False}

# 注意：大量翻譯（> 10 筆）建議使用 batch_translate() 以獲得更好的效能
```

### 使用 P131 驗證

```python
from core.utils.wikidata_translator import (
    WikidataTranslator,
    TranslationDatasetBuilder,
)

# 範例 1: 翻譯日本的同名地名「中区」
translator_ja = WikidataTranslator(source_lang="ja", target_lang="zh-tw")

# 指定父級為「大阪市」（Q35765）
result = translator_ja.translate("中区", parent_qid="Q35765")
# → 選擇大阪市中區（Q54886752），而非橫濱市中區

# 範例 2: 批次翻譯時提供父級對照表
builder = TranslationDatasetBuilder(
    country_code="JP",
    source_lang="ja",
    target_lang="zh-tw"
)
records = [{"sidonm": name} for name in ["中区", "西区"]]
dataset = builder.build_admin1(records, name_field="sidonm")

# 提供父級 QID 對照表（可用 item.id 或 item.original_name 作為鍵）
parent_qids = {
    item.id: "Q35765"  # 使用 item.id（如 "JP/admin_1/中区"）
    for item in dataset
}
# 或者
parent_qids = {
    item.original_name: "Q35765"  # 使用原始名稱（如 "中区"）
    for item in dataset
}

results = translator_ja.batch_translate(
    dataset,
    parent_qids=parent_qids,
    show_progress=True
)
```

### 使用候選過濾器

```python
from core.utils.wikidata_translator import (
    WikidataTranslator,
    TranslationDatasetBuilder,
)

def filter_administrative_only(name: str, metadata: dict) -> bool:
    """僅保留行政區實體，排除政府機構、歷史地名等。

    Args:
        name: 原始地名
        metadata: {
            'qid': 候選 QID,
            'labels': {語言: 標籤},
            'instance_of': [P31 QID 列表]
        }

    Returns:
        True = 保留候選，False = 排除候選
    """
    labels = metadata.get("labels", {})
    instance_of = metadata.get("instance_of", [])

    # 排除政府機構類關鍵字
    gov_keywords = ["council", "assembly", "government", "office"]
    for label in labels.values():
        if any(k in label.lower() for k in gov_keywords):
            return False

    # 根據 P31 (instance of) 過濾
    # 例如：排除歷史行政區 (Q19953632)
    if "Q19953632" in instance_of:
        return False

    return True

# 範例：翻譯越南省份並應用過濾器
translator_vi = WikidataTranslator(source_lang="vi", target_lang="zh-tw")
builder = TranslationDatasetBuilder(
    country_code="VN",
    source_lang="vi",
    target_lang="zh-tw"
)
records = [{"name": city} for city in ["Hà Nội", "Hồ Chí Minh", "Đà Nẵng"]]
dataset = builder.build_admin1(records, name_field="name")

results = translator_vi.batch_translate(
    dataset,
    candidate_filter=filter_administrative_only,
    show_progress=True
)
```

### Admin 2 批次翻譯

```python
from core.utils.wikidata_translator import (
    WikidataTranslator,
    TranslationDatasetBuilder,
)

# 範例：翻譯日本 Admin_2（市區町村）
translator_ja = WikidataTranslator(
    source_lang="ja",
    target_lang="zh-tw",
    cache_path="geoname_data/JP_wikidata_cache.json"
)

builder = TranslationDatasetBuilder(
    country_code="JP",
    source_lang="ja",
    target_lang="zh-tw"
)

# Admin_2 資料需包含 parent 欄位（所屬 Admin_1）
records = [
    {"parent": "東京都", "name": "千代田区"},
    {"parent": "東京都", "name": "中央区"},
    {"parent": "大阪府", "name": "大阪市"},
]

# 建立 Admin_2 dataset（需指定 parent_field 和 name_field）
dataset = builder.build_admin2(
    records,
    parent_field="parent",
    name_field="name",
    deduplicate=True  # 自動去重
)

# show_progress=False 時會使用 ProgressLogger（INFO 級別輸出）
results = translator_ja.batch_translate(
    dataset,
    batch_size=16,
    show_progress=False  # 使用日誌輸出進度
)

# 返回格式: {'JP/admin_2/東京都/千代田区': {...}, ...}
```

### 不同語言翻譯

```python
from core.utils.wikidata_translator import WikidataTranslator

# 範例 1: 越南文 → 繁體中文
translator_vi = WikidataTranslator(
    source_lang="vi",
    target_lang="zh-tw",
    fallback_langs=["zh-hant", "zh", "en", "vi"],
    cache_path="geoname_data/VN_wikidata_cache.json"
)
result = translator_vi.translate("Hà Nội")
# {'translated': '河內', 'qid': 'Q1858', ...}

# 範例 2: 泰文 → 繁體中文
translator_th = WikidataTranslator(
    source_lang="th",
    target_lang="zh-tw",
    fallback_langs=["zh-hant", "zh", "en", "th"],
    cache_path="geoname_data/TH_wikidata_cache.json"
)
result = translator_th.translate("กรุงเทพมหานคร")
# {'translated': '曼谷', 'qid': 'Q1861', ...}

# 範例 3: 韓文 → 繁體中文
translator_ko = WikidataTranslator(
    source_lang="ko",
    target_lang="zh-tw",
    fallback_langs=["zh-hant", "zh", "en", "ko"],
    cache_path="geoname_data/KR_wikidata_cache.json"
)
result = translator_ko.translate("서울특별시")
# {'translated': '首爾特別市', 'qid': 'Q8684', ...}
```

---

## API 端點與速率限制

WikidataTranslator 使用以下 API 端點：

| API | 用途 | 端點 | 速率限制 |
|-----|------|------|----------|
| **Wikidata SPARQL** | P131 驗證查詢 | `https://query.wikidata.org/sparql` | 0.8 秒/次 |
| **Wikidata API** | 搜尋實體、取得標籤 | `https://www.wikidata.org/w/api.php` | 0.2 秒/次 |
| **中文維基百科 API** | 簡繁標題轉換 | `https://zh.wikipedia.org/w/api.php` | 0.2 秒/次 |

**User-Agent 設定**：

```
immich-geodata-zh-tw/1.0 (Wikidata Translation Tool)
```

**速率限制設計考量**：

- Wikidata 官方無明確速率限制文件，但建議避免過度頻繁請求
- SPARQL 查詢較重量級，延遲較長（0.8 秒）
- API 查詢較輕量，延遲較短（0.2 秒）
- 實際速率會因批次查詢優化而更低（例如 250 個地名只需 5 次標籤請求）

---

**最後更新**：2025-11-16
