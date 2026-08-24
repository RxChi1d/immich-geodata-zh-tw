# South Korea Administrative Processing Guide

> This document explains how the project handles geographic information for South Korea. It is the detailed version of [Supported Regions and Language Strategy](../../README.en.md#supported-regions-and-language-strategy) in the README.

## Table of Contents

- [Administrative Levels](#administrative-levels)
- [Metropolitan City and Province Types](#metropolitan-city-and-province-types)
- [Centroid Calculation Method](#centroid-calculation-method)
- [Traditional Chinese Translation Strategy](#traditional-chinese-translation-strategy)
  - [Translation Levels and Priority](#translation-levels-and-priority)
  - [Rust Wikidata Translation Source and Cache](#rust-wikidata-translation-source-and-cache)
- [Data Normalization](#data-normalization)
  - [Splitting Combined City-District Names](#splitting-combined-city-district-names)
  - [Handling Duplicate Place Names](#handling-duplicate-place-names)
- [Special Handling for Sejong Special Self-Governing City](#special-handling-for-sejong-special-self-governing-city)
- [Data Sources](#data-sources)

---

## Administrative Levels

> [!NOTE]
> `admin_3` and `admin_4` exist only in the project's intermediate CSV. They preserve the source administrative level for traceability and debugging, and are never written to the cities500 file Immich consumes. The finest level Immich displays is `admin_2`. Representative point density depends on the source feature granularity used during extract (administrative dong for South Korea) and is unrelated to these two columns.

South Korea uses a three-level administrative system (excluding the country level):

| Level | Korean Name | Type | Count | Source Field |
|-------|-------------|------|-------|--------------|
| **Admin 1** | Metropolitan City / Province | 광역자치단체 | 17 | `sidonm` |
| **Admin 2** | City / District / County | 기초자치단체 | 252 (231 distinct translations) | `sggnm` |
| **Admin 3** | Dong / Eup / Myeon | 행정동/법정동 | 3,534 rows (2,802 distinct names) | Parsed from `adm_nm` |
| **Admin 4** | Original Admin 3 | - | Only when a city-district split occurs (492 rows) | Preserves the original value |

> **Statistical baseline**: `meta_data/kr_geodata.csv` generated from the `HangJeongDong_ver20260401` boundary data (3,558 rows in total).

> **Note**: Admin 4 is used only when a city-district split occurs (for example, `성남시분당구` splits into `성남시` + `분당구`), preserving the original Admin 3 value. Immich's reverse geocoding ignores this column; it exists for data completeness and debugging.

### Admin 1 Typology

The 17 metropolitan cities and provinces fall into these categories:

- **Special City** (특별시): 1 – Seoul
- **Metropolitan City** (광역시): 6 – Busan, Daegu, Incheon, Gwangju, Daejeon, Ulsan
- **Special Self-Governing City** (특별자치시): 1 – Sejong
- **Province** (도): 6 – Gyeonggi-do, Chungcheongbuk-do, Chungcheongnam-do, Gyeongsangbuk-do, Gyeongsangnam-do, Jeollanam-do
- **Special Self-Governing Province** (특별자치도): 3 – Gangwon, Jeonbuk, Jeju

### Admin 2 Types

Basic local governments (기초자치단체) fall into three kinds:

- **City** (시): major urban areas
- **District** (구): subdivisions inside metropolitan cities
- **County** (군): rural areas

### Admin 3 Types

- **Dong** (동): base-level units in urban areas
- **Eup** (읍): town-level units
- **Myeon** (면): rural townships

---

## Metropolitan City and Province Types

The 17 metropolitan cities and provinces follow different naming conventions, and the translations must match what users in Taiwan expect.

### Naming Rules

1. **Special City / Metropolitan City / Special Self-Governing City**: always output as "X市".
   - Examples: 서울특별시 → 首爾市, 부산광역시 → 釜山市, 세종특별자치시 → 世宗市

2. **Province / Special Self-Governing Province**: always output as "X道", and special self-governing provinces revert to the traditional province names familiar in Taiwan.
   - Examples: 경기도 → 京畿道, 강원특별자치도 → 江原道, 전북특별자치도 → 全羅北道, 제주특별자치도 → 濟州道

---

## Centroid Calculation Method

### Challenge and Solution

South Korea spans UTM zones 51N and 52N (longitudes 124°–132°), so a fixed UTM zone introduces severe errors for administrative divisions near the zone boundary.

**The solution combines dynamic UTM zone selection with an Albers projection:**

1. **Albers equal-area projection**: compute an accurate geometric centroid longitude for the polygon (parameters `lat_1=33`, `lat_2=43`, `lat_0=37`, `lon_0=127.5`).
2. **Dynamic UTM zone selection**: pick UTM 51N or 52N based on that centroid longitude.
3. **UTM centroid computation**: compute the final centroid inside the selected UTM zone.
4. **Projection transform caching**: features are processed one by one, and transform objects are cached and reused per UTM zone. Multithreading only kicks in at 10,000 features or more, so South Korea's 3,558 rows run single-threaded.

---

## Traditional Chinese Translation Strategy

### Translation Levels and Priority

Translation of South Korean geographic data is layered, balancing translation quality against API request efficiency:

| Level | Method | Count | Wikidata Lookups |
|-------|--------|-------|------------------|
| **Admin 1** | Built-in mapping + Wikidata QID | 17 | 17 items |
| **Admin 2** | Wikidata batch translation | 228 | 228 items |
| **Admin 2 (Sejong)** | **Manual mapping** | 24 | **None** |
| **Admin 3** | Korean source text retained | 3,534 | None |

> [!NOTE]
> The number of lookup items is not the number of HTTP requests. Every item that misses the cache triggers one entity search, labels are fetched in batches of 32, and candidate entities get an extra P131 validation query. A fully warm cache issues no requests at all.

> [!NOTE]
> Admin 2 values for Sejong Special Self-Governing City come from a manual mapping and skip Wikidata entirely. See [Manual Mapping Translation](#2-manual-mapping-translation) for the reasoning.

**Design rationale:**

1. **Admin 1**: the built-in mapping supplies the concise names used in Taiwan (「首爾市」 rather than 「首爾特別市」), while still fetching the QID for Admin 2 validation.
2. **Admin 2**: Wikidata translation, at a manageable volume (228 items).
3. **Admin 2 (Sejong)**: the manual mapping guarantees 100% Traditional Chinese coverage.
4. **Admin 3**: keeping the Korean source text avoids 3,500+ API requests and barely affects the user experience.

### Admin 1 Translation Method

A **built-in mapping** supplies the names customary in Taiwan (not the official Wikidata labels) for all 17 metropolitan cities and provinces. The full table lives in `src/pipeline/extract/handlers.rs`:

| Official Name | Project Output | Official Name | Project Output |
| :--- | :--- | :--- | :--- |
| 서울특별시 | 首爾市 | 강원특별자치도 | 江原道 |
| 부산광역시 | 釜山市 | 충청북도 | 忠清北道 |
| 대구광역시 | 大邱市 | 충청남도 | 忠清南道 |
| 인천광역시 | 仁川市 | 전북특별자치도 | 全羅北道 |
| 광주광역시 | 光州市 | 전라남도 | 全羅南道 |
| 대전광역시 | 大田市 | 경상북도 | 慶尚北道 |
| 울산광역시 | 蔚山市 | 경상남도 | 慶尚南道 |
| 세종특별자치시 | 世宗市 | 제주특별자치도 | 濟州道 |
| 경기도 | 京畿道 | | |

### Rust Wikidata Translation Source and Cache

The Rust production handler uses the shared Wikidata translation cache pipeline to read and
update the local `geoname_data/KR_wikidata_cache.json`. When the cache is missing or lacks
entries, production extract queries Wikidata, fills in the context-aware cache, then applies
the built-in Admin 1 and Sejong mappings. Automated fixture validation still uses the supplied
`KR_wikidata_stub.json` and never calls the live Wikidata service, keeping release gates
independent of API quota, upstream data drift, and network conditions.

To refresh the South Korean translation source, rerun production extract against the real KR
GeoJSON. Fixture and parity validation that requires deterministic output should keep using the
matching stub or cache.

**Fallback chain**: translation tries these label sources in order.

1. zh-TW (Traditional Chinese – Taiwan)
2. zh-Hant (Traditional Chinese)
3. zh (Simplified Chinese, converted to Traditional via OpenCC)
4. en (English)
5. ko (Korean label)
6. Chinese Wikipedia article title (converted to Traditional)
7. Original Korean name (all sources failed)

> [!NOTE]
> English or Korean labels matched at steps 4 and 5 are rejected by the South Korea handler's
> `is_valid_chinese_translation` and fall back to the original Korean name, so they never reach
> the output. And because Wikidata entities almost always carry an `en` label, step 6's Chinese
> Wikipedia title is never reached in practice. Across the 245 translations currently in
> `geoname_data/KR_wikidata_cache.json`, only three sources appear: zh-TW (180), OpenCC
> conversion (46), and zh-Hant (19).

#### Candidate Filtering

When translating Admin 2 (city / district / county), candidate filtering removes government
institutions so the result is a real administrative division name.

**Filter rules:**

Wikidata candidate labels are checked and dropped when they contain any of these keywords:

- **Legislative bodies**: 의회, 議會, council, assembly, 委員會, legislature
- **Executive agencies**: 시청, 구청, 군청, 도청, 교육청, 廳, government

**Design considerations:**

- Match whole terms (`시청`, `도청`, `군청`) instead of single characters, avoiding false positives.
- Prevent government offices from being taken for administrative divisions (`세종특별자치시청` must not become one).
- Keep legitimate divisions intact (Cheongdo County is not dropped because of the character 「청」).

> [!NOTE]
> This filter applies only to Admin 2 translation for ordinary regions. Sejong Special Self-Governing City uses the manual mapping exclusively, never enters the Wikidata flow, and therefore never touches this filter.

---

## Data Normalization

This section covers the general normalization applied to South Korean geographic data, which holds for most of the country.

### Splitting Combined City-District Names

Some cities store `sggnm` as a combined string, `<city>시<district>구` (for example `성남시분당구`), which causes two problems:

1. **Translation failures**: the Wikidata entity is the standalone district name (`분당구`), so lookups on the combined name fail.
2. **Confusing display**: Immich puts the combined city-district name at the same level as dong / eup / myeon.

**Processing logic:**

1. **Detection**: when the name ends with `구` or `군`, split it after the first `시` into a city name and a district/county name (see `split_korea_city_district` in `src/pipeline/extract/handlers.rs`).
2. **Split**:
   - `sggnm` → city-level name (`성남시`)
   - `admin_3` → district/county name (`분당구`)
   - `admin_4` → original Admin 3 (`태평3동`)

**Effect:**
- Admin 2 now maps to the city-level name, raising the Wikidata match rate.
- Hierarchy: 京畿道 → 城南市 (`admin_2`, translated) → 분당구 (`admin_3`, Korean retained) → 태평3동 (`admin_4`, original value)
- The original value is preserved in `admin_4`.

### Handling Duplicate Place Names

South Korea has many repeated administrative division names, so P131 validation is required to pick the right entity.

#### Admin 2 Level

Seven names repeat, yet each is unique within its own Admin 1:

| Name | Occurrences | Parent Admin 1 |
|------|-------------|----------------|
| 중구 (中區) | 6 | 首爾市, 釜山市, 大邱市, 仁川市, 大田市, 蔚山市 |
| 동구 (東區) | 6 | 釜山市, 大邱市, 仁川市, 光州市, 大田市, 蔚山市 |
| 서구 (西區) | 5 | 釜山市, 大邱市, 仁川市, 光州市, 大田市 |
| 남구 (南區) | 4 | 釜山市, 大邱市, 光州市, 蔚山市 |
| 북구 (北區) | 4 | 釜山市, 大邱市, 光州市, 蔚山市 |
| 강서구 (江西區) | 2 | 首爾市, 釜山市 |
| 고성군 | 2 | 江原道, 慶尚南道 |

**Handling**: P131 validation ensures the correct Admin 2 is chosen (「中區」 must sit inside 「首爾」).

> **Exception**: the two 고성군 translate differently (高城郡 in 江原道, 固城郡 in 慶尚南道), so they do not collide at the Traditional Chinese layer.

#### Admin 3 Level

About 229 duplicate names (counted from the `HangJeongDong_ver20260401` boundary data). They do not affect what Immich shows, which normally stops at Admin 2.

**Strategy**: keep the Korean source text untranslated.

### Disambiguation Parentheses Removal

Some place names carry disambiguation parentheses in Wikidata (「東區 (光州)」). In this project's data structure `admin_1` already names the parent region, so `admin_2` does not need to repeat it.

#### Gwangju-Specific Processing

Gwangju's Dong-gu and Seo-gu carry disambiguation markers in their Traditional Chinese Wikidata labels:

- `동구` → 東區 (光州)
- `서구` → 西區 (光州)

**Problems:**
- The same-named districts in other cities (Busan, Daegu, Incheon, Daejeon, Ulsan) have no parentheses.
- Naming becomes inconsistent.
- Immich displays redundant information: 「光州 > 東區 (光州)」.

**Processing logic:**

After translation completes, the Rust handler strips trailing disambiguation parentheses only from Admin 2 records where `sidonm == "광주광역시"`. The implementation lives in `src/pipeline/extract/handlers.rs`, controlled by `strip_trailing_parenthetical`.

**Effect:**

| Before | After |
|--------|-------|
| 光州 > 東區 (光州) | 光州 > 東區 ✅ |
| 光州 > 西區 (光州) | 光州 > 西區 ✅ |
| 釜山 > 東區 | 釜山 > 東區 (unaffected) |

> [!NOTE]
> This rule targets Gwangju alone, keeping control precise. If other cities turn out to have the same problem, the logic can be extended.

---

## Special Handling for Sejong Special Self-Governing City

Sejong Special Self-Governing City (세종특별자치시) is South Korea's only **single-tier special self-governing city**. Its administrative structure differs from every other metropolitan city and needs a dedicated workflow, which can serve as a reference for similar structures in other countries.

**Structural difference:**

```
Standard structure: Metropolitan City/Province → City/District/County → Dong/Eup/Myeon
Sejong structure:   Sejong Special Self-Governing City → Dong/Eup/Myeon ← no middle level!
```

### Data Problem

In the source GeoJSON, the `sggnm` field (Admin 2) is always `세종시` for Sejong; earlier versions filled it with government institution names instead:

- `세종특별자치시광역자치의회` (the council)
- `세종특별자치시청` (city hall)

Neither is a real Admin 2. Used as-is, Immich would show 「世宗 > 世宗市」 or 「世宗 > 議會」 instead of 「世宗 > 鳥致院邑」.

### Processing

A **two-phase mechanism** resolves both Sejong's structure and its translation problem.

#### 1. Administrative Level Normalization

Promote Sejong's dong / eup / myeon records from Admin 3 to Admin 2, matching its single-tier structure:

- **Detection**: `sggnm` does not end with 읍/면/동 (which would indicate a real administrative division).
- **Normalization**: Admin 3 → Admin 2, then clear Admin 3.

**Example:**
```
Before: 세종특별자치시 → 세종시 → 대평동
After:  세종특별자치시 → 대평동 → (empty)
```

#### 2. Manual Mapping Translation

**Problem**: Sejong was founded in 2012, and most of its new dong have no Chinese label on Wikidata, so the early implementation fell back to romanization (`Boram-dong`) or even the Korean source text.

**Solution**: the Rust handler ships a 25-entry manual mapping (`sejong_admin2` in `src/pipeline/extract/handlers.rs`) covering all 24 Admin 2 names in the current boundary data, plus `합강동` (合江洞), which has not yet appeared in it.

**Mapping samples:**

| Korean | Traditional Chinese |
|--------|---------------------|
| 보람동 | 寶藍洞 |
| 대평동 | 大平洞 |
| 다정동 | 多情洞 |
| 도담동 | 陶潭洞 |
| 고운동 | 高雲洞 |
| 조치원읍 | 鳥致院邑 |
| 부강면 | 芙江面 |
| 장군면 | 將軍面 |

**Translation flow:**

```
1. Detect Sejong → 2. Look up the mapping → 3. Return Traditional Chinese
                                              (Wikidata skipped)
Example: 보람동 → 寶藍洞
```

**Results:**

| Original Korean | Wikidata Translation | Manual Mapping |
|-----------------|----------------------|----------------|
| 보람동 | Boram-dong ❌ | 寶藍洞 ✅ |
| 대평동 | Daepyeong-dong ❌ | 大平洞 ✅ |
| 어진동 | 어진동 ❌ | 汝珍洞 ✅ |
| 조치원읍 | 鳥致院邑 ✅ | 鳥致院邑 ✅ |

> [!NOTE]
> The "Wikidata Translation" column shows the historical behavior from before the manual mapping was introduced. The current Rust implementation rejects romanized and mixed Chinese-English results through `is_valid_chinese_translation` and falls back to the original Korean name, and Sejong never enters the Wikidata flow at all.

> [!NOTE]
> After both phases, Sejong records carry the Traditional Chinese dong / eup / myeon name in Admin 2 (「鳥致院邑」, 「燕岐面」, 「寶藍洞」) with Admin 3 left blank, so Immich correctly shows 「世宗 > 鳥致院邑」.

---

## Data Sources

### Primary Data Source

**admdongkor**
- Repository: https://github.com/vuski/admdongkor
- Description: South Korean administrative boundary data (GeoJSON)
- License: the boundary data is CC BY 4.0; the underlying source data is released under South Korea's KOGL Type 1 (공공누리 제1유형) and requires attribution (see [NOTICE.md](../../NOTICE.md))
- Usage: supplies boundaries and names for all three South Korean administrative levels

### Translation Data Sources

**Wikidata**
- API: https://www.wikidata.org/w/api.php
- SPARQL: https://query.wikidata.org/sparql
- License: CC0 1.0 Universal (Public Domain)
- Usage: multilingual place name translations and P131 relation validation

**Chinese Wikipedia**
- API: https://zh.wikipedia.org/w/api.php
- Usage: Simplified-to-Traditional conversion (converttitles API)

**OpenCC (Open Chinese Convert)**
- Repository: https://github.com/BYVoid/OpenCC
- Usage: Simplified Chinese to Traditional Chinese conversion

---

**Last Updated**: 2025-11-10
