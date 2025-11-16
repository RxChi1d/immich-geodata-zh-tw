# WikidataTranslator 說明文檔

本文檔說明 WikidataTranslator 的設計理念、核心功能、翻譯策略與技術實作邏輯。

## 目錄

- [WikidataTranslator 說明文檔](#wikidatatranslator-說明文檔)
  - [目錄](#目錄)
  - [設計理念](#設計理念)
  - [核心功能](#核心功能)
  - [翻譯流程](#翻譯流程)
    - [單一翻譯流程](#單一翻譯流程)
    - [批次翻譯流程](#批次翻譯流程)
  - [翻譯策略](#翻譯策略)
    - [多層回退機制](#多層回退機制)
    - [P131 層級關係驗證](#p131-層級關係驗證)
    - [候選過濾機制](#候選過濾機制)
  - [快取機制](#快取機制)
    - [快取結構](#快取結構)
    - [快取版本遷移](#快取版本遷移)
  - [批次查詢優化](#批次查詢優化)
  - [錯誤處理與重試機制](#錯誤處理與重試機制)
    - [速率限制處理](#速率限制處理)
    - [重試機制](#重試機制)
    - [錯誤降級處理](#錯誤降級處理)
  - [使用範例](#使用範例)
    - [基本使用](#基本使用)
    - [使用 P131 驗證](#使用-p131-驗證)
    - [使用候選過濾器](#使用候選過濾器)
    - [不同語言翻譯](#不同語言翻譯)
  - [API 端點與速率限制](#api-端點與速率限制)

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
| **單一翻譯** | `translate(name, parent_qid)` - 翻譯單一地名 |
| **批次翻譯** | `batch_translate(dataset, parent_qids)` - 透過 dataset/dataloader 批次翻譯並驅動統一進度 |
| **P131 驗證** | 透過 Wikidata P131（located in）關係驗證地名層級關係 |
| **候選過濾** | 提供自訂過濾器排除不符合條件的候選實體 |
| **多層快取** | 分層快取搜尋結果、標籤、P131 驗證、翻譯結果 |
| **簡轉繁** | 透過 OpenCC 將簡體中文標籤轉換為繁體中文 |
| **維基百科標題轉換** | 使用中文維基百科 API 進行標題簡繁轉換 |
| **資料集＋進度控制** | 透過 `TranslationDataset` / `TranslationDataLoader` 封裝待翻譯項目，統一追蹤總筆數、進度條與行別快取 |

---

## 翻譯流程

### 單一翻譯流程

`translate()` 方法實際上是 `batch_translate()` 的包裝（會在內部臨時建立 `TranslationDataset`），因此所有邏輯統一走批次流程：

```
輸入地名 → 檢查快取 → 搜尋 Wikidata → 取得標籤 → 選擇最佳翻譯 → 快取結果
```

### 批次翻譯流程

`batch_translate()` 採用三階段處理，最大化批次查詢效率：

```
階段 1: 搜尋階段
  ├─ 逐一搜尋地名取得候選 QID（Wikidata API 不支援批次搜尋）
  ├─ 檢查翻譯快取，已快取的直接返回
  └─ 收集所有候選 QID

階段 1.5: 候選過濾（可選）
  ├─ 批次取得所有候選 QID 的標籤與 P31（instance of）
  ├─ 應用自訂過濾器函式
  └─ 過濾不符合條件的候選實體

階段 2: 批次取得標籤
  ├─ 收集所有候選 QID（去重）
  ├─ 批次查詢標籤（每批最多 50 個 QID）
  └─ 快取標籤結果

階段 3: 選擇最佳翻譯
  ├─ 根據 P131 驗證選擇正確的 QID
  ├─ 應用多層回退策略選擇最佳標籤
  ├─ 快取翻譯結果
  └─ 返回翻譯結果

#### 資料集與進度控制

批次翻譯以 `TranslationDatasetBuilder → TranslationDataset → TranslationDataLoader` 為骨架：

1. **Dataset Builder**：處理 handler 提供的 DataFrame，產生 `TranslationItem`（包含 `id`、原始名稱、admin level、parent chain 等 metadata）。Admin_1 與 Admin_2 各自轉成 dataset，以便獨立翻譯。  
2. **Dataset**：實作 `Sequence` 介面並保留統計資訊（總筆數、唯一 parent 數、語言對），方便 log 與進度輸出。  
3. **DataLoader**：依 `batch_size` 迭代 dataset，並對接 `progress_callback`。若啟用 `show_progress`，callback 會驅動 `tqdm` 進度條；否則改用 `ProgressLogger` 在 INFO 等級打印進度百分比。

此設計將翻譯邏輯與資料來源解耦：handler 只需負責構建 dataset，翻譯器則專注於批次查詢／回寫快取，進度與 batch 控制也統一集中於 `BatchTranslationRunner`。
```

**關鍵優化**：階段 2 使用批次 API（每次最多 50 個 QID），大幅減少請求次數。例如翻譯 250 個地名，使用批次查詢只需約 5 次 API 請求，而非 250 次。

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

WikidataTranslator 使用多層快取減少重複的 API 請求，大幅提升效能。

### 快取結構

快取檔案採用 JSON 格式，包含 metadata（元資料）、translations（翻譯結果）和 cache（中間查詢結果）三大區塊：

```json
{
  "metadata": {
    "source_lang": "ja",
    "target_lang": "zh-tw",
    "created_at": "2025-01-15T10:30:00",
    "version": "1.1"
  },
  "translations": {
    "東京都": {
      "translated": "東京都",
      "qid": "Q1490",
      "source": "wikidata",
      "used_lang": "zh-tw",
      "parent_verified": false,
      "cached_at": "2025-01-15T10:31:25"
    }
  },
  "cache": {
    "search": {
      "東京都": ["Q1490", "Q123456"]
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
  }
}
```

**快取層級說明**：

| 層級 | 用途 | 快取鍵 | 快取值 |
|------|------|--------|--------|
| **metadata** | 快取檔案元資料 | - | 來源/目標語言、建立時間、版本 |
| **translations** | 最終翻譯結果 | 地名 | 翻譯結果 + metadata |
| **cache.search** | Wikidata 搜尋結果 | 地名 | QID 列表 |
| **cache.labels** | 實體的多語言標籤 | QID | {語言: 標籤} |
| **cache.p131** | P131 驗證結果 | `{候選QID}_{父級QID}` | true/false |
| **cache.instance_of** | P31 屬性（實例類型） | QID | [P31 QID 列表] |

**快取查詢優先順序**：

1. **translations**：如果已有翻譯結果，直接返回（跳過所有查詢）
2. **cache.search**：如果已搜尋過該地名，使用快取的 QID 列表
3. **cache.labels**：如果已取得該 QID 的標籤，使用快取的標籤
4. **cache.p131**：如果已驗證過該層級關係，使用快取的驗證結果
5. **cache.instance_of**：如果已取得該 QID 的 P31，使用快取的類型資訊

### 快取同步策略

過去快取僅在整個批次翻譯結束時一次性寫入，長時間處理 Admin_2 時若中途中斷便會遺失結果。現在改為「隨寫隨沖」策略：

- 所有會寫入快取的步驟（搜尋、標籤、P31、P131、翻譯結果）都會在記憶體更新後呼叫 `_mark_cache_dirty()`。  
- `_mark_cache_dirty()` 會累計髒污筆數並透過 `_flush_cache_if_needed()` 判斷是否落盤：預設達到 20 筆或距離上次儲存超過 30 秒就自動 `_save_cache()`。  
- `BatchTranslationRunner` 在階段 3 完成時一律 `force=True` 進行最後一次 flush，確保批次結果全部寫入。  

如此即便在翻譯過程中遭遇網路中斷或手動終止，也只會損失最後極少數尚未 flush 的筆數，大幅改善長時間作業的可靠性。

---

## 批次查詢優化

批次翻譯透過 Wikidata API 的批次查詢功能大幅減少請求次數。

**批次查詢方法**：

| 方法 | 功能 | 批次大小 | API 端點 |
|------|------|----------|----------|
| `_batch_get_labels()` | 取得多個 QID 的標籤 | 50 個/批 | wbgetentities |
| `_batch_get_instance_of()` | 取得多個 QID 的 P31 | 50 個/批 | wbgetentities |

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

### 基本使用

```python
from core.utils.wikidata_translator import WikidataTranslator

# 範例 1: 日文 → 繁體中文
translator_ja = WikidataTranslator(
    source_lang="ja",
    target_lang="zh-tw",
    fallback_langs=["zh-hant", "zh", "en", "ja"],
    cache_path="geoname_data/JP_wikidata_cache.json",
    use_opencc=True
)

# 單一翻譯
result = translator_ja.translate("東京都")
# {'translated': '東京都', 'qid': 'Q1490', 'source': 'wikidata',
#  'used_lang': 'zh-tw', 'parent_verified': False}

# 批次翻譯
builder = TranslationDatasetBuilder(country_code="JP", source_lang="ja", target_lang="zh-tw")
records = [{"sidonm": name} for name in ["東京都", "大阪府", "京都府"]]
dataset = builder.build_admin1(records, name_field="sidonm")
results = translator_ja.batch_translate(dataset, batch_size=16)
# {'JP/admin_1/東京都': {...}, ...}
```

### 使用 P131 驗證

```python
# 範例 1: 翻譯日本的同名地名「中区」
translator_ja = WikidataTranslator(source_lang="ja", target_lang="zh-tw")

# 指定父級為「大阪市」（Q35765）
result = translator_ja.translate("中区", parent_qid="Q35765")
# → 選擇大阪市中區（Q54886752），而非橫濱市中區

# 範例 2: 批次翻譯時提供父級對照表
records = [{"sidonm": name} for name in ["中区", "西区"]]
dataset = TranslationDatasetBuilder(
    country_code="JP", source_lang="ja", target_lang="zh-tw"
).build_admin1(records, name_field="sidonm")
parent_qids = {
    item.id: "Q35765"  # 指向大阪市
    for item in dataset
}
results = translator_ja.batch_translate(dataset, parent_qids=parent_qids)
```

### 使用候選過濾器

```python
def filter_administrative_only(name: str, metadata: dict) -> bool:
    """僅保留行政區實體，排除政府機構、歷史地名等。"""
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
records = [{"sidonm": name} for name in ["Hà Nội", "Hồ Chí Minh", "Đà Nẵng"]]
dataset = TranslationDatasetBuilder(
    country_code="VN", source_lang="vi", target_lang="zh-tw"
).build_admin1(records, name_field="sidonm")
results = translator_vi.batch_translate(
    dataset,
    candidate_filter=filter_administrative_only,
)
```

### 不同語言翻譯

```python
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

**最後更新**：2025-11-11
