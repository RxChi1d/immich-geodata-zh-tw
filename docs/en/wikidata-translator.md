# WikidataTranslator Documentation

This document details the design philosophy, core capabilities, translation strategy, and implementation mechanics of WikidataTranslator.

## Table of Contents

- [WikidataTranslator Documentation](#wikidatatranslator-documentation)
  - [Table of Contents](#table-of-contents)
  - [Quick Start](#quick-start)
  - [Design Philosophy](#design-philosophy)
  - [Core Features](#core-features)
  - [Translation Flow](#translation-flow)
    - [Batch Translation Flow](#batch-translation-flow)
      - [Datasets and Progress Control](#datasets-and-progress-control)
    - [Single Translation Interface](#single-translation-interface)
  - [Translation Strategy](#translation-strategy)
    - [Multi-Layer Fallback](#multi-layer-fallback)
    - [P131 Hierarchy Verification](#p131-hierarchy-verification)
    - [Candidate Filtering](#candidate-filtering)
  - [Caching](#caching)
    - [Cache Structure](#cache-structure)
    - [Cache Sync Strategy](#cache-sync-strategy)
  - [Batch Query Optimization](#batch-query-optimization)
  - [Error Handling and Retries](#error-handling-and-retries)
    - [Rate-Limit Handling](#rate-limit-handling)
    - [Retry Strategy](#retry-strategy)
    - [Graceful Degradation](#graceful-degradation)
  - [Usage Examples](#usage-examples)
    - [Batch Translation](#batch-translation)
    - [Single Translation](#single-translation)
    - [Using P131 Verification](#using-p131-verification)
    - [Using Candidate Filters](#using-candidate-filters)
    - [Admin 2 Batch Translation](#admin-2-batch-translation)
    - [Translating Different Languages](#translating-different-languages)
  - [API Endpoints and Rate Limits](#api-endpoints-and-rate-limits)

---

## Quick Start

```python
from core.utils.wikidata_translator import WikidataTranslator

translator = WikidataTranslator(source_lang="ko", target_lang="zh-tw")
result = translator.translate("서울특별시")
print(result["translated"])  # Output: 首爾特別市
```

See the [Usage Examples](#usage-examples) section for the full workflow.

---

## Design Philosophy

WikidataTranslator is a generalized toponym translator that uses Wikidata to translate place names from any language into Traditional Chinese (or other target languages).

**Core design goals**:

1. **Generality**: Support translations between arbitrary source and target languages.
2. **Accuracy**: Use P131 relationship verification to pick the correct location when names collide.
3. **Performance**: Reduce API calls with batch queries and multi-layer caching.
4. **Reliability**: Rate-limit handling, retries, and error management keep the system stable.
5. **Flexibility**: Support candidate filtering and layered fallback strategies.

---

## Core Features

WikidataTranslator provides the following core capabilities:

| Feature | Description |
|--------|-------------|
| **Batch translation** | `batch_translate(dataset, parent_qids)` – translate with a `TranslationDataset` and unified progress tracking. |
| **Dataset and progress control** | `TranslationDataset` / `TranslationDataLoader` encapsulate translation items, track totals, surface progress bars, and manage per-row caching. |
| **Single translation interface** | `translate(name, parent_qid)` – internally builds a single-item dataset before calling the batch engine, suitable for quick checks. |
| **P131 verification** | Validate administrative hierarchies via the Wikidata P131 (located in) relationship. |
| **Candidate filtering** | Provide custom filters to drop candidate entities that do not meet requirements. |
| **Multi-layer cache** | Cache search results, labels, P131 verification, and translations at different layers. |
| **Automatic cache sync** | “Write-through with deferred flush” – flush every **20 entries** or **30 seconds** to avoid data loss during long runs. |
| **Simplified-to-Traditional conversion** | Use OpenCC to convert Simplified Chinese labels to Traditional Chinese. |
| **Wikipedia title conversion** | Use the Chinese Wikipedia API to convert page titles between Simplified and Traditional scripts. |

---

## Translation Flow

### Batch Translation Flow

`batch_translate()` is the core implementation and uses a three-phase pipeline that maximizes batch-query efficiency:

```
Phase 1: Search
  ├─ Iterate over the dataset via DataLoader with batch_size
  ├─ Search each toponym for candidate QIDs (Wikidata API lacks batch search)
  ├─ Check the translation cache and short-circuit hits
  ├─ Collect all candidate QIDs
  └─ Update progress via progress_callback

Phase 1.5: Candidate filtering (optional)
  ├─ Batch-fetch labels and P31 (instance of) for all candidates
  ├─ Apply the custom filter function
  └─ Drop candidates that fail the filter

Phase 2: Batch label retrieval
  ├─ Collect unique candidate QIDs
  ├─ Fetch labels in batches (max 50 QIDs each)
  └─ Cache label responses

Phase 3: Pick the best translation and persist cache
  ├─ Verify P131 hierarchy to select the correct QID
  ├─ Apply fallback strategy to choose the best label
  ├─ Cache the translation result (each write marks the cache dirty)
  ├─ Auto-sync the cache when the dirty counter or timer fires
  └─ Return translation results
```

**Key optimization**: Phase 2 uses the batch API (up to **50 QIDs** per call) to dramatically cut requests. Translating 250 names only needs about 5 label API calls, versus 250 if done individually.

#### Datasets and Progress Control

Batch translation relies on the `TranslationDatasetBuilder → TranslationDataset → TranslationDataLoader → BatchTranslationRunner` pipeline:

1. **TranslationDatasetBuilder**: Processes handler-provided DataFrames to produce `TranslationItem` objects (with `id`, original name, admin level, parent chain, and metadata). Admin_1 and Admin_2 data become separate datasets for independent translation.
2. **TranslationDataset**: Implements `Sequence` and keeps stats (record count, unique parents, language pair) for logging and progress display. Exposes `stats()` for quick summaries.
3. **TranslationDataLoader**: Iterates the dataset by `batch_size` and reports progress via `progress_callback`. Supports custom ordering via the `sorter` parameter.
4. **BatchTranslationRunner**: Coordinates the three phases and progress display:
   - `show_progress=True` uses a `tqdm` bar and preserves the final results.
   - `show_progress=False` relies on `ProgressLogger`, emitting INFO-level progress percentages (0%, 5%, 10%, …, 100%).

**Advantages**:
- Separates translation logic from data ingestion: handlers focus on dataset construction while the translator manages batch queries and caching.
- Unified progress control: both progress bars and logs reuse the same callback pipeline.
- Adjustable batch size: tune `batch_size` for network conditions or API limits.

### Single Translation Interface

`translate()` exposes a simplified single-entry interface by creating a temporary one-item `TranslationDataset` and delegating to the batch core:

```
translate(name, parent_qid)
  ↓
Build a temporary single-item TranslationDataset
  ↓
Call batch_translate(dataset, parent_qids)
  ↓
Return the single result
```

**Rationale**:
- **Unified logic**: single and batch translations share the same core implementation.
- **Full feature parity**: P131 checks, candidate filtering, and other batch features work for single translations.
- **Maintainability**: only the batch logic needs maintenance; single translations automatically inherit improvements.
- **Ease of use**: quick single-entry translation without manually creating datasets.

**When to use**:
- Rapidly translate a single toponym.
- Interactive testing or debugging.
- Small (<10) on-demand translation tasks.

**Recommendation**:
- For larger batches (>10), use `batch_translate()` with `TranslationDataset` for better performance and progress tracking.

---

## Translation Strategy

### Multi-Layer Fallback

The translator uses a multi-layer fallback strategy to maximize success. The order is controlled by `fallback_langs`, which defaults to `["zh-hant", "zh", "en", source_lang]`:

```
1. Target language (target_lang)       ← Highest priority (e.g., zh-tw)
   ↓ Missing label
2. Fallback language 1 (zh-hant)       ← Generic Traditional Chinese
   ↓ Missing label
3. Fallback language 2 (zh)            ← Simplified Chinese + OpenCC to Traditional
   ↓ Missing label
4. Fallback language 3 (en)            ← English
   ↓ Missing label
5. Fallback language 4 (source_lang)   ← Original language (ja, ko, vi, th, ...)
   ↓ Missing label
6. Chinese Wikipedia title conversion  ← Use converttitles API to get Traditional
   ↓ Missing label
7. Original input name                 ← Last resort
```

> **Note**: You can customize the fallback list, but the default order is tuned for Traditional Chinese by prioritizing Chinese labels, then English, and finally the original name.

**Example 1**: Wikidata already has the target-language label (best-case scenario)

```
Translate “Tokyo” (Japanese: 東京)
1. zh-tw: 東京 ✅ → Return “東京”
```

**Example 2**: Only Simplified Chinese label exists (convert via OpenCC)

```
Translate a location
1. zh-tw: (none) ❌
2. zh-hant: (none) ❌
3. zh: 伦敦 → OpenCC → 倫敦 ✅
```

**Example 3**: Only English label exists (fallback to English)

```
Translate a small town
1. zh-tw: (none) ❌
2. zh-hant: (none) ❌
3. zh: (none) ❌
4. en: Springfield ✅ → Return “Springfield”
```

**Example 4**: Only the source-language label exists (fallback to source)

```
Translate Japanese “〇〇町”
1. zh-tw: (none) ❌
2. zh-hant: (none) ❌
3. zh: (none) ❌
4. en: (none) ❌
5. ja: 〇〇町 ✅ → Return “〇〇町” (original Japanese)
```

### P131 Hierarchy Verification

Wikidata’s P131 (located in) property defines administrative hierarchies. When search returns multiple identically named locations, P131 verification picks the correct entity.

**Challenge**: Many administrative regions share the same name worldwide

- **Springfield**: 30+ instances in the United States.
- **中區** (Chūō, “Central District”): Multiple occurrences in Japan, Korea, and China.
- **San José**: Numerous cities across Spain and Latin America.

**Solution**: Supply the parent QID to verify the hierarchy.

```python
# Example: translate “中区” with parent “Osaka” (Q35765)
translator.translate("中区", parent_qid="Q35765")
# → Picks Osaka’s Naka-ku (Q54886752), not Chūō in Tokyo or Yokohama.
```

**Verification logic** (SPARQL):

```sparql
ASK { wd:Q54886752 (wdt:P131)+ wd:Q35765 . }
# Check whether candidate Q54886752 lies within parent Q35765 (Osaka)
# (wdt:P131)+ walks the hierarchy recursively across multiple levels
```

**Cache optimization**: P131 results are cached with keys like `{candidate_qid}_{parent_qid}` so repeated checks reuse stored answers.

### Candidate Filtering

Batch translation supports custom filter functions that run in Phase 1.5 to drop invalid candidates. Filters receive candidate metadata and return `True` (keep) or `False` (drop).

**Filter signature**:

```python
def candidate_filter(name: str, metadata: dict) -> bool:
    """
    Args:
        name: Original place name.
        metadata: {
            "qid": Candidate QID,
            "labels": {language: label},
            "instance_of": [P31 QID list]
        }
    Returns:
        True to keep the candidate, False to discard it.
    """
```

**Example**: filter out government institutions

```python
def filter_administrative_divisions_only(name: str, metadata: dict) -> bool:
    """Keep administrative divisions and remove councils, offices, etc."""
    labels = metadata.get("labels", {})

    government_keywords = [
        "council", "assembly", "government", "office", "department",
        "議會", "政府", "辦公室", "部門", "廳",
        "conseil", "gobierno", "правительство",
    ]

    for label in labels.values():
        if any(keyword in label.lower() for keyword in government_keywords):
            return False

    return True

translator.batch_translate(
    names=["Tokyo", "Paris", "Berlin"],
    candidate_filter=filter_administrative_divisions_only,
)
```

**Use cases**:

- Remove museums, schools, companies, or other non-geographic entities.
- Skip historical (abolished) administrative divisions.
- Filter by P31 types (instance of) to only keep desired entity classes.

**Performance note**: Filters run after batch-fetching labels and P31 values so they do not require per-candidate network calls.

---

## Caching

`TranslationCacheStore` manages the cache for WikidataTranslator. All cached results use context-aware keys (`TranslationItem.id = level/parent_chain/name`) so that identically named places with different parents stay isolated. The cache is stored as JSON (schema v1.0). When an older schema is detected, the store auto-backs up the file and rebuilds it.

> **Version note**: The cache schema version tracks the data format and is independent of the project release version. Only incompatible cache data changes trigger a schema bump.

### Cache Structure

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

> **Reminder**: The context-aware design requires every cache key to include `level + parent_chain + original_name`. Upgrading to schema v1.0 rebuilds the entire cache (the old data is backed up as `.bak`).

**Cache layers**:

| Layer | Purpose | Key | Value |
|-------|---------|-----|-------|
| **metadata** | Cache file metadata | – | Source/target languages, timestamps, version, compaction history. |
| **translations** | Final translation results | `TranslationItem.id` | Translation output plus parent chain/QID and timestamps. |
| **cache.search** | Wikidata search results | `TranslationItem.id` | Candidate QID list (split by parent context). |
| **cache.labels** | Entity labels | QID | `{language: label}` pairs. |
| **cache.p131** | P131 verification results | `{candidate_qid}_{parent_qid}` | Boolean. |
| **cache.instance_of** | P31 attributes | QID | `[P31 QID, ...]`. |
| **indexes.by_name** | Debug index | Place name | Array of context-aware keys. |

**Lookup order**:

1. **translations**: Hit means we bypass search and verification entirely.
2. **cache.search**: Keeps per-context candidate lists.
3. **cache.labels**: Reuses stored labels and avoids extra `wbgetentities` calls.
4. **cache.p131**: Stores `{candidate → parent}` verification results.
5. **cache.instance_of**: Supplies P31 metadata for candidate filters.

### Cache Sync Strategy

To prevent losing earlier work during long translations (for example, Admin 2 runs), `TranslationCacheStore` uses a “write-through with deferred flush” strategy:

**Trigger mechanism**:

1. Any cache write (search, labels, P31/P131, translations) updates memory and calls `mark_dirty()`.
2. `mark_dirty()` increments the dirty counter and checks whether to flush.
3. The store automatically calls `save()` whenever **either** condition is met:
   - The dirty counter reaches **20** entries.
   - More than **30 seconds** have elapsed since the last save.
4. `BatchTranslationRunner` forces a final flush after Phase 3 completes.

**Benefits**:
- **Better fault tolerance**: Even if the process stops mid-run, only a handful of unsaved entries are lost.
- **Balanced performance**: Avoids writing every entry (which hurts I/O) yet does not wait until the very end.
- **Transparent**: Developers do not need to call save manually; the translator manages persistence.

**Atomic writes**:
- Writes use temporary `.tmp` files plus `rename()` to avoid corruption if a flush is interrupted.
- If saving fails, the original cache file remains intact.

---

## Batch Query Optimization

Batch translation leverages Wikidata API batch functions to reduce request counts.

**Batch helpers**:

| Method | Purpose | Batch size | Endpoint |
|--------|---------|------------|----------|
| `_batch_get_labels()` | Fetch labels for multiple QIDs | **50 per batch** | `wbgetentities` |
| `_batch_get_instance_of()` | Fetch P31 lists for multiple QIDs | **50 per batch** | `wbgetentities` |

**Workflow**:

```
Input: [Q8684, Q41164, Q515, ...]

Step 1: Deduplicate and skip cached QIDs
Step 2: Split into 50-item batches
  ├─ Batch 1: Q8684|Q41164|Q515|... (50 QIDs)
  ├─ Batch 2: Q12345|Q67890|... (50 QIDs)
  └─ ...
Step 3: Parse responses and store them in cache.labels / cache.instance_of
Step 4: Return data as {qid: labels} or {qid: [P31]}
```

**Performance comparison** (translating 250 names):

| Method | Label queries | Total requests |
|--------|---------------|----------------|
| Per-item | 250 | ~500 |
| Batch (50/Q) | 5 | ~260 |

**Savings**: ~48% fewer API calls.

---

## Error Handling and Retries

The translator implements layered error handling to stay resilient.

### Rate-Limit Handling

**Problem**: Wikidata enforces rate limits and returns HTTP 429 when exceeded.

**Solution**:

1. **Proactive throttling**: Delay after each request.
   - SPARQL: 0.8 seconds.
   - Wikidata API: 0.2 seconds.
   - Chinese Wikipedia API: 0.2 seconds.

2. **Reactive throttling**: When a 429 occurs, read the `Retry-After` header and wait accordingly.

```python
if response.status_code == 429:
    retry_after = int(response.headers.get("Retry-After", 5))
    time.sleep(retry_after)
```

### Retry Strategy

**Approach**: Exponential backoff with jitter.

```python
for attempt in range(MAX_RETRIES):  # Up to 5 tries
    try:
        response = self.session.get(url, params=params, timeout=30)
        response.raise_for_status()
        return response.json()
    except RequestException:
        base_wait = 2 * (attempt + 1)        # 2, 4, 6, 8, 10 seconds
        jitter = random.uniform(-0.2, 0.2)   # ±20% jitter
        wait_time = base_wait * (1 + jitter)
        time.sleep(wait_time)
```

**Why jitter**: Prevents thundering-herd retries when multiple requests fail simultaneously.

### Graceful Degradation

Each phase of translation has dedicated error handling:

| Phase | Error handling | Degradation |
|-------|----------------|-------------|
| Search failure | Log a warning | Return an empty candidate list. |
| Label fetch failure | Log a warning | Return empty labels. |
| P131 verification failure | Log a warning | Return `False` (treat as unverifiable). |
| OpenCC failure | Log a warning | Use the original Simplified label. |
| Wikipedia conversion failure | Log a warning | Use the original title. |

**Design principle**: Failure on one place name must not stop the rest of the batch.

---

## Usage Examples

### Batch Translation

```python
from core.utils.wikidata_translator import (
    WikidataTranslator,
    TranslationDatasetBuilder,
)

translator_ja = WikidataTranslator(
    source_lang="ja",
    target_lang="zh-tw",
    fallback_langs=["zh-hant", "zh", "en", "ja"],
    cache_path="geoname_data/JP_wikidata_cache.json",
    use_opencc=True,
)

builder = TranslationDatasetBuilder(
    country_code="JP",
    source_lang="ja",
    target_lang="zh-tw",
)

records = [{"sidonm": name} for name in ["東京都", "大阪府", "京都府"]]
dataset = builder.build_admin1(records, name_field="sidonm")

results = translator_ja.batch_translate(
    dataset,
    batch_size=16,
    show_progress=True,
)
# Returns: {"JP/admin_1/東京都": {...}, ...}
```

### Single Translation

```python
from core.utils.wikidata_translator import WikidataTranslator

translator_ja = WikidataTranslator(
    source_lang="ja",
    target_lang="zh-tw",
    fallback_langs=["zh-hant", "zh", "en", "ja"],
    cache_path="geoname_data/JP_wikidata_cache.json",
    use_opencc=True,
)

result = translator_ja.translate("東京都")
# {'translated': '東京都', 'qid': 'Q1490', 'source': 'wikidata',
#  'used_lang': 'zh-tw', 'parent_verified': False}
```

### Using P131 Verification

```python
from core.utils.wikidata_translator import (
    WikidataTranslator,
    TranslationDatasetBuilder,
)

translator_ja = WikidataTranslator(source_lang="ja", target_lang="zh-tw")

result = translator_ja.translate("中区", parent_qid="Q35765")
# → Picks Osaka's Naka-ku (Q54886752)

builder = TranslationDatasetBuilder(
    country_code="JP",
    source_lang="ja",
    target_lang="zh-tw",
)
records = [{"sidonm": name} for name in ["中区", "西区"]]
dataset = builder.build_admin1(records, name_field="sidonm")

parent_qids = {
    item.id: "Q35765"
    for item in dataset
}

results = translator_ja.batch_translate(
    dataset,
    parent_qids=parent_qids,
    show_progress=True,
)
```

### Using Candidate Filters

```python
from core.utils.wikidata_translator import (
    WikidataTranslator,
    TranslationDatasetBuilder,
)

def filter_administrative_only(name: str, metadata: dict) -> bool:
    """Keep administrative divisions and drop government agencies, historical areas, etc."""
    labels = metadata.get("labels", {})
    instance_of = metadata.get("instance_of", [])

    gov_keywords = ["council", "assembly", "government", "office"]
    for label in labels.values():
        if any(k in label.lower() for k in gov_keywords):
            return False

    if "Q19953632" in instance_of:  # Historical administrative division
        return False

    return True

translator_vi = WikidataTranslator(source_lang="vi", target_lang="zh-tw")
builder = TranslationDatasetBuilder(
    country_code="VN",
    source_lang="vi",
    target_lang="zh-tw",
)
records = [{"name": city} for city in ["Hà Nội", "Hồ Chí Minh", "Đà Nẵng"]]
dataset = builder.build_admin1(records, name_field="name")

results = translator_vi.batch_translate(
    dataset,
    candidate_filter=filter_administrative_only,
    show_progress=True,
)
```

### Admin 2 Batch Translation

```python
from core.utils.wikidata_translator import (
    WikidataTranslator,
    TranslationDatasetBuilder,
)

translator_ja = WikidataTranslator(
    source_lang="ja",
    target_lang="zh-tw",
    cache_path="geoname_data/JP_wikidata_cache.json",
)

builder = TranslationDatasetBuilder(
    country_code="JP",
    source_lang="ja",
    target_lang="zh-tw",
)

records = [
    {"parent": "東京都", "name": "千代田区"},
    {"parent": "東京都", "name": "中央区"},
    {"parent": "大阪府", "name": "大阪市"},
]

dataset = builder.build_admin2(
    records,
    parent_field="parent",
    name_field="name",
    deduplicate=True,
)

results = translator_ja.batch_translate(
    dataset,
    batch_size=16,
    show_progress=False,
)
# Returns: {'JP/admin_2/東京都/千代田区': {...}, ...}
```

### Translating Different Languages

```python
from core.utils.wikidata_translator import WikidataTranslator

translator_vi = WikidataTranslator(
    source_lang="vi",
    target_lang="zh-tw",
    fallback_langs=["zh-hant", "zh", "en", "vi"],
    cache_path="geoname_data/VN_wikidata_cache.json",
)
result = translator_vi.translate("Hà Nội")
# {'translated': '河內', 'qid': 'Q1858', ...}

translator_th = WikidataTranslator(
    source_lang="th",
    target_lang="zh-tw",
    fallback_langs=["zh-hant", "zh", "en", "th"],
    cache_path="geoname_data/TH_wikidata_cache.json",
)
result = translator_th.translate("กรุงเทพมหานคร")
# {'translated': '曼谷', 'qid': 'Q1861', ...}

translator_ko = WikidataTranslator(
    source_lang="ko",
    target_lang="zh-tw",
    fallback_langs=["zh-hant", "zh", "en", "ko"],
    cache_path="geoname_data/KR_wikidata_cache.json",
)
result = translator_ko.translate("서울특별시")
# {'translated': '首爾特別市', 'qid': 'Q8684', ...}
```

---

## API Endpoints and Rate Limits

WikidataTranslator hits the following APIs:

| API | Purpose | Endpoint | Rate limit |
|-----|---------|----------|------------|
| **Wikidata SPARQL** | P131 verification | `https://query.wikidata.org/sparql` | 0.8 s/call |
| **Wikidata API** | Search entities, fetch labels | `https://www.wikidata.org/w/api.php` | 0.2 s/call |
| **Chinese Wikipedia API** | Title conversion (Simplified/Traditional) | `https://zh.wikipedia.org/w/api.php` | 0.2 s/call |

**User-Agent**:

```
immich-geodata-zh-tw/1.0 (Wikidata Translation Tool)
```

**Rate-limit considerations**:

- Wikidata does not publish strict rate limits but discourages excessive querying.
- SPARQL queries are heavier, so the delay is longer (0.8 s).
- API queries are lighter, so delays are shorter (0.2 s).
- Batch optimization further reduces real-world request counts (e.g., 250 names need only ~5 label calls).

---

**Last updated**: 2025-11-16
