# South Korea Administrative Processing Guide

This document captures the processing strategy, administrative hierarchy, translation rules, and implementation details for South Korean geospatial data.

## Table of Contents

- [Administrative Levels](#administrative-levels)
- [Metropolitan City and Province Types](#metropolitan-city-and-province-types)
- [Centroid Calculation Method](#centroid-calculation-method)
- [Traditional Chinese Translation Strategy](#traditional-chinese-translation-strategy)
  - [Priority per Level](#priority-per-level)
  - [WikidataTranslator Configuration](#wikidatatranslator-configuration)
- [Data Normalization](#data-normalization)
  - [Split City-District Composite Names](#split-city-district-composite-names)
  - [Handling Duplicate Names](#handling-duplicate-names)
- [Special Handling for Sejong Special Self-Governing City](#special-handling-for-sejong-special-self-governing-city)
- [Data Sources](#data-sources)

---

## Administrative Levels

South Korea uses a three-level system beneath the country level:

| Level | Korean Name | Type | Count | Source Field |
|-------|-------------|------|-------|--------------|
| **Admin 1** | Metropolitan City / Province | 광역자치단체 | 17 | `sidonm` |
| **Admin 2** | City / District / County | 기초자치단체 | ~250 | `sggnm` |
| **Admin 3** | Dong / Eup / Myeon | 행정동/법정동 | ~3,500 | Parsed from `adm_nm` |
| **Admin 4** | Original Admin 3 | - | Only when a city-district split occurs | Stored for provenance |

> **Note**: Admin 4 is only used while splitting combined city-district names (for example, `성남시분당구` becomes `성남시` + `분당구`). Immich ignores this column; it is preserved for auditing and debugging.

### Admin 1 Typology

The 17 metropolitan cities and provinces fall into the following categories:

- **Special City** (특별시): 1 – Seoul
- **Metropolitan City** (광역시): 6 – Busan, Daegu, Incheon, Gwangju, Daejeon, Ulsan
- **Special Self-Governing City** (특별자치시): 1 – Sejong
- **Province** (도): 6 – Gyeonggi-do, Chungcheongbuk-do, Chungcheongnam-do, Gyeongsangbuk-do, Gyeongsangnam-do, Jeollanam-do
- **Special Self-Governing Province** (특별자치도): 3 – Gangwon Special Self-Governing Province, Jeonbuk Special Self-Governing Province, Jeju Special Self-Governing Province

### Admin 2 Types

Basic local governments (기초자치단체) mainly consist of:

- **City (시)**: urban areas
- **District (구)**: subdivisions inside metropolitan cities
- **County (군)**: rural regions

### Admin 3 Types

- **Dong (동)**: base units within cities
- **Eup (읍)**: township-level units
- **Myeon (면)**: rural townships

---

## Metropolitan City and Province Types

The 17 metropolitan cities and provinces follow different naming conventions that must align with Taiwanese usage.

### Naming Rules

1. **Special/Metropolitan/Special Self-Governing Cities**: drop the suffix and use the short form.
   - Example: `서울특별시` → Seoul, `부산광역시` → Busan.
2. **Provinces and Special Self-Governing Provinces**: keep or drop "-do" depending on user familiarity.
   - Traditional provinces keep the suffix: `경기도` → Gyeonggi-do.
   - Special self-governing provinces drop it: `제주특별자치도` → Jeju.

---

## Centroid Calculation Method

### Challenge and Solution

South Korea spans UTM zones 51N and 52N (longitudes 124°–132°). Fixing the projection to a single zone causes severe centroid errors near zone boundaries.

**Adopt a dynamic UTM selection combined with an Albers projection:**

1. **Albers equal-area projection**: compute accurate polygon centroid longitude.
2. **Dynamic UTM choice**: select UTM 51N or 52N based on that longitude.
3. **UTM centroid computation**: compute the final centroid within the chosen UTM zone.
4. **Vectorized batches**: process geometries in bulk per zone for performance.

---

## Traditional Chinese Translation Strategy

### Priority per Level

The translation pipeline balances quality against API usage by applying different strategies per level:

| Level | Method | Count | API Calls |
|-------|--------|-------|-----------|
| **Admin 1** | Built-in mapping + Wikidata QID lookup | 17 | ~17 |
| **Admin 2** | Wikidata batch translation | ~250 | ~250 |
| **Admin 2 (Sejong)** | **Manual mapping** | 24 | **0** |
| **Admin 3** | Retain Korean source | ~3,500 | 0 |

> [!NOTE]
> Sejong's Admin 2 values use only the manual mapping, skipping Wikidata entirely. See [Manual Mapping for Sejong](#2-manual-mapping).

**Design rationale:**

1. **Admin 1**: a built-in lookup returns concise Taiwanese names ("Seoul" instead of "Seoul Special City"), while storing the QID for Admin 2 validation.
2. **Admin 2**: Wikidata delivers ~250 translations with acceptable latency.
3. **Admin 2 (Sejong)**: the manual mapping guarantees 100% Traditional Chinese coverage.
4. **Admin 3**: Korean labels avoid 3,500+ API calls with minimal UX impact.

### Admin 1 Translation

Use a **built-in table** with Taiwanese short names (not the canonical Wikidata labels) for all 17 metropolitan cities/provinces:

- `서울특별시` → Seoul (not "Seoul Special City")
- `부산광역시` → Busan (not "Busan Metropolitan City")
- `경기도` → Gyeonggi-do (retain "-do")
- `제주특별자치도` → Jeju (drop the suffix)

### WikidataTranslator Configuration

`WikidataTranslator` performs the name resolution. See its documentation for search behavior, P131 validation, batching, cache mechanics, and more.

#### Initialization

```python
translator = WikidataTranslator(
    source_lang="ko",
    target_lang="zh-tw",
    fallback_langs=["zh-hant", "zh", "en", "ko"],
    cache_path="geoname_data/KR_wikidata_cache.json",
    use_opencc=True,
)
```

**Fallback order**:

1. zh-TW (Traditional Chinese - Taiwan)
2. zh-Hant (Traditional Chinese)
3. zh (Simplified Chinese, converted to Traditional via OpenCC)
4. en (English)
5. ko (Korean)
6. Original name (if everything fails)

#### Candidate Filtering

While translating Admin 2 names, candidate filtering removes government institutions to avoid confusing them with geographic entities.

**Exclusion keywords** (matched as whole terms):

- **Legislative bodies**: 의회, 議會, council, assembly, 委員會, legislature
- **Executive agencies**: 시청, 구청, 군청, 도청, 교육청, 廳, government

**Why this matters**:

- Prevents government offices (e.g., `세종특별자치시청`) from being returned as administrative units.
- Avoids false positives from partial matches ("청" in valid names).
- Keeps legitimate regions such as Cheongdo County untouched.

> [!NOTE]
> This filter applies only to standard Admin 2 translations. Sejong records bypass Wikidata entirely and therefore skip the filter.

---

## Data Normalization

These routines apply to most regions nationwide.

### Split City-District Composite Names

Some records store `sggnm` as a concatenated city + district string (`<city>시<district>구`, e.g., `성남시분당구`). These cause two issues:

1. **Translation failures**: Wikidata expects standalone district names (e.g., `분당구`).
2. **Messy display**: Immich would flatten city-district names into the same level as dong/eup/myeon.

**Processing steps**:

1. **Detection**: regex `^(?P<city>.+?시)(?P<district>.+?(?:구|군))$` catches composite patterns.
2. **Split**:
   - `sggnm` → city name (`성남시`).
   - `admin_3` → district/county (`분당구`).
   - `admin_4` → original Admin 3 (`태평3동`).
3. **Logging**: record how many rows were split for auditing.

**Impact**:

- Higher Wikidata match rate.
- Clear hierarchy: Gyeonggi-do → Bundang District → Tae-pyeong 3-dong.
- Original data preserved in `admin_4`.

### Handling Duplicate Names

Many administrative units share the same Korean names. P131 validation ensures the correct entity is selected.

#### Admin 2 Level

Seven names repeat yet remain unique within their Admin 1 parent:

| Name | Occurrences | Example |
|------|-------------|---------|
| 중구 (Jung District) | 2 | Jung-gu, Seoul; Jung-gu, Incheon |
| 남구 (Nam District) | 3 | Nam-gu, Busan; Nam-gu, Daegu; Nam-gu, Gwangju |
| 북구 (Buk District) | 3 | Buk-gu, Daegu; Buk-gu, Gwangju; Buk-gu, Ulsan |

**Approach**: enforce P131 validation so "Jung District" under Seoul cannot be mistaken for the one in Incheon.

#### Admin 3 Level

Roughly 239 duplicates exist, but Immich usually displays up to Admin 2.

**Strategy**: keep the Korean labels.

---

## Special Handling for Sejong Special Self-Governing City

Sejong (세종특별자치시) is the only **single-tier special self-governing city**. Its structure differs from other metropolitan cities and needs a bespoke workflow that can serve as a template for similar cases elsewhere.

**Structural difference**:

```
Standard: Metropolitan/Province → City/District/County → Dong/Eup/Myeon
Sejong: Sejong Special Self-Governing City → Dong/Eup/Myeon (no middle tier)
```

### Data Issues

The shapefile incorrectly fills `sggnm` (Admin 2) with government agencies:

- `세종특별자치시광역자치의회` (city council)
- `세종특별자치시청` (city hall)

If left untouched, Immich would display "Sejong > City Hall" instead of "Sejong > Jochiwon-eup".

### Two-Phase Solution

1. **Normalize the hierarchy**: promote Sejong's dong/eup/myeon records from Admin 3 to Admin 2 and clear Admin 3.
2. **Manual translation table**: translate all Admin 2 names via `SEJONG_ADMIN2_MAP` rather than Wikidata.

#### 1. Hierarchy Normalization

- **Detection**: if `sggnm` does not end with 읍/면/동, treat it as a mislabeled agency.
- **Normalization**: move `adm_nm` into Admin 2, empty Admin 3, keep Admin 4 blank.

**Example**:
```
Before: Sejong → Sejong City Hall → 대평동
After:  Sejong → 대평동 → (empty)
```

#### 2. Manual Mapping

**Problem**: founded in 2012, Sejong's new neighborhoods often lack Traditional Chinese labels on Wikidata, forcing fallbacks to romanization ("Boram-dong") or Korean.

**Solution**: `SEJONG_ADMIN2_MAP` in `core/geodata/south_korea.py` covers all 24 Admin 2 names.

```python
SEJONG_ADMIN2_MAP = {
    "보람동": "寶藍洞",
    "대평동": "大坪洞",
    "다정동": "多情洞",
    "도담동": "嶋潭洞",
    "고운동": "高運洞",
    "종촌동": "鍾村洞",
    "새롬동": "新羅洞",
    "소담동": "素潭洞",
    "어진동": "御珍洞",
    "반곡동": "盤谷洞",
    "해밀동": "海密洞",
    "조치원읍": "鳥致院邑",
    "부강면": "芙江面",
    "장군면": "將軍面",
    # ...remaining entries...
}
```

**Translation flow**:

```
1. Detect Sejong → 2. Lookup in map → 3. Return Traditional Chinese
                                (skip Wikidata)
Example: 보람동 → 寶藍洞
```

**Results**:

| Korean | Wikidata Result | Manual Result |
|--------|-----------------|---------------|
| 보람동 | Boram-dong ❌ | 寶藍洞 ✅ |
| 대평동 | Daepyeong-dong ❌ | 大坪洞 ✅ |
| 어진동 | 어진동 ❌ | 御珍洞 ✅ |
| 조치원읍 | 鳥致院邑 ✅ | 鳥致院邑 ✅ |

> [!NOTE]
> After both steps, Sejong Admin 2 fields display the Traditional Chinese dong/eup/myeon names (e.g., "鳥致院邑", "燕岐面", "寶藍洞"), while Admin 3 remains empty. Immich can then show "Sejong > 鳥致院邑" correctly.

---

## Data Sources

### Primary Dataset

**admdongkor**
- Repository: https://github.com/vuski/admdongkor
- Description: South Korean administrative boundaries (GeoJSON)
- License: MIT
- Usage: provides polygons and names for all three administrative levels

### Translation Data

**Wikidata**
- API: https://www.wikidata.org/w/api.php
- SPARQL: https://query.wikidata.org/sparql
- License: CC0 1.0 Universal (Public Domain)
- Usage: multi-language translations and P131 validation

**Chinese Wikipedia**
- API: https://zh.wikipedia.org/w/api.php
- Usage: title conversion for simplified ↔ traditional

**OpenCC (Open Chinese Convert)**
- Repository: https://github.com/BYVoid/OpenCC
- Usage: automatic simplified-to-traditional conversion

---

**Last Updated**: 2025-11-10
