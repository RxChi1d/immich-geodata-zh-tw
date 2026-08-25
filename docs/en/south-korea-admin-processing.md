# South Korea Administrative Processing Guide

> This document explains how the project handles geographic information for South Korea. It is the detailed version of [Supported Regions and Language Strategy](../../README.en.md#supported-regions-and-language-strategy) in the README.

## Table of Contents

- [2026 Administrative Reorganization](#2026-administrative-reorganization)
- [Administrative Levels](#administrative-levels)
- [Metropolitan City and Province Types](#metropolitan-city-and-province-types)
- [Centroid Calculation Method](#centroid-calculation-method)
- [Traditional Chinese Translation Strategy](#traditional-chinese-translation-strategy)
  - [Translation Levels and Priority](#translation-levels-and-priority)
  - [Rust Wikidata Translation Source and Cache](#rust-wikidata-translation-source-and-cache)
  - [Admin 2 Name Source: Korean Hanja](#admin-2-name-source-korean-hanja)
- [Data Normalization](#data-normalization)
  - [Splitting Combined City-District Names](#splitting-combined-city-district-names)
  - [Handling Duplicate Place Names](#handling-duplicate-place-names)
  - [Disambiguation Parentheses](#disambiguation-parentheses)
- [Special Handling for Sejong Special Self-Governing City](#special-handling-for-sejong-special-self-governing-city)
- [Data Sources](#data-sources)

---

## 2026 Administrative Reorganization

`HangJeongDong_ver20260701` reflects two statutory reorganizations that took effect on 2026-07-01. Both are administrative changes the project must follow, not data errors.

### Jeonnam-Gwangju Integrated Special City

Under the *Special Act on the Establishment of Jeonnam-Gwangju Integrated Special City* (법률 제21446호, promulgated 2026-03-05, in force 2026-07-01), Jeollanam-do and Gwangju Metropolitan City were abolished and merged into 전남광주통합특별시, granted a legal status comparable to Seoul Special City.

- 27 basic local governments: 5 cities (Mokpo, Yeosu, Suncheon, Naju, Gwangyang), the 5 former Gwangju districts (Dong, Seo, Nam, Buk, Gwangsan), and 17 counties
- Affects 393 rows; administrative codes were renumbered wholesale (`46110` / `29110` → `12110`)
- The project outputs 「全南光州市」, following the existing Admin 1 convention of dropping the administrative-class modifier rather than using the full official hanja 「全南光州統合特別市」

### Incheon District Reorganization

Under the *Act on the Establishment of Jemulpo-gu, Yeongjong-gu and Geomdan-gu in Incheon Metropolitan City* (법률 제20161호, promulgated 2024-01-30, in force 2026-07-01) and the *Act on Renaming Seo-gu of Incheon Metropolitan City* (법률 제21734호):

| Before | After |
| :--- | :--- |
| Jung-gu (inland) + Dong-gu | 제물포구 (Jemulpo District) |
| Jung-gu (Yeongjong Island area) | 영종구 (Yeongjong District) |
| Seo-gu (north of the Gyeongin Ara Waterway) | 검단구 (Geomdan District) |
| Seo-gu (remainder) | 서해구 (Seohae District) |

Incheon went from 2 counties and 8 districts to 2 counties and 9 districts.

---

## Administrative Levels

> [!NOTE]
> `admin_3` and `admin_4` exist only in the project's intermediate CSV. They preserve the source administrative level for traceability and debugging, and are never written to the cities500 file Immich consumes. The finest level Immich displays is `admin_2`. Representative point density depends on the source feature granularity used during extract (administrative dong for South Korea) and is unrelated to these two columns.

South Korea uses a three-level administrative system (excluding the country level):

| Level | Korean Name | Type | Count | Source Field |
|-------|-------------|------|-------|--------------|
| **Admin 1** | Metropolitan City / Province / Integrated Special City | 광역자치단체 | 16 | `sidonm` |
| **Admin 2** | City / District / County | 기초자치단체 | 253 (235 distinct translations) | `sggnm` |
| **Admin 3** | Dong / Eup / Myeon | 행정동/법정동 | 3,534 rows (2,802 distinct names) | Parsed from `adm_nm` |
| **Admin 4** | Original Admin 3 | - | Only when a city-district split occurs (492 rows) | Preserves the original value |

> **Statistical baseline**: `meta_data/kr_geodata.csv` generated from the `HangJeongDong_ver20260701` boundary data (3,558 rows in total).

> **Note**: Admin 4 is used only when a city-district split occurs (for example, `성남시분당구` splits into `성남시` + `분당구`), preserving the original Admin 3 value. Immich's reverse geocoding ignores this column; it exists for data completeness and debugging.

### Admin 1 Typology

The 16 first-level divisions fall into these categories:

- **Special City** (특별시): 1 – Seoul
- **Metropolitan City** (광역시): 5 – Busan, Daegu, Incheon, Daejeon, Ulsan
- **Integrated Special City** (통합특별시): 1 – Jeonnam-Gwangju
- **Special Self-Governing City** (특별자치시): 1 – Sejong
- **Province** (도): 5 – Gyeonggi-do, Chungcheongbuk-do, Chungcheongnam-do, Gyeongsangbuk-do, Gyeongsangnam-do
- **Special Self-Governing Province** (특별자치도): 3 – Gangwon, Jeonbuk, Jeju

> [!NOTE]
> Since 2026-07-01, Jeollanam-do and Gwangju Metropolitan City have been merged into 전남광주통합특별시 under 법률 제21446호, reducing the number of first-level divisions from 17 to 16. See [2026 Administrative Reorganization](#2026-administrative-reorganization).

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

The 16 first-level divisions follow different naming conventions, and the translations must match what users in Taiwan expect.

### Naming Rules

1. **Special City / Metropolitan City / Special Self-Governing City / Integrated Special City**: always output as "X市".
   - Examples: 서울특별시 → 首爾市, 부산광역시 → 釜山市, 세종특별자치시 → 世宗市, 전남광주통합특별시 → 全南光州市

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
| **Admin 1** | Built-in mapping + Wikidata QID | 16 | 16 items |
| **Admin 2** | Wikidata entity lookup + Korean hanja | 229 | 229 items |
| **Admin 2 (Sejong)** | **Manual mapping** | 24 | **None** |
| **Admin 3** | Korean source text retained | 3,534 | None |

> [!NOTE]
> The number of lookup items is not the number of HTTP requests. Every item that misses the cache triggers one entity search, labels are fetched in batches of 32, and candidate entities get an extra P131 validation query. A fully warm cache issues no requests at all.

> [!NOTE]
> Admin 2 values for Sejong Special Self-Governing City come from a manual mapping and skip Wikidata entirely. See [Manual Mapping Translation](#2-manual-mapping-translation) for the reasoning.

**Design rationale:**

1. **Admin 1**: the built-in mapping supplies the concise names used in Taiwan (「首爾市」 rather than 「首爾特別市」), while still fetching the QID for Admin 2 validation.
2. **Admin 2**: Wikidata identifies the entity; the name itself comes from the Korean Wikipedia hanja (see [Admin 2 Name Source: Korean Hanja](#admin-2-name-source-korean-hanja)), at a manageable volume (229 items).
3. **Admin 2 (Sejong)**: the manual mapping guarantees 100% Traditional Chinese coverage.
4. **Admin 3**: keeping the Korean source text avoids 3,500+ API requests and barely affects the user experience.

### Admin 1 Translation Method

A **built-in mapping** supplies the names customary in Taiwan (not the official Wikidata labels) for all 16 first-level divisions. The table also keeps the pre-merger `광주광역시` and `전라남도` entries so older boundary data can still be re-extracted. The full table lives in `src/pipeline/extract/handlers.rs`:

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
| 경기도 | 京畿道 | 전남광주통합특별시 | 全南光州市 |

### Rust Wikidata Translation Source and Cache

The Rust production handler uses the shared Wikidata translation cache pipeline to read and
update the local `geoname_data/KR_wikidata_cache.json`. When the cache is missing or lacks
entries, production extract queries Wikidata, fills in the context-aware cache, then applies
the built-in Admin 1 and Sejong mappings, and finally overwrites Admin 2 names with the Korean
Wikipedia hanja (see the next section). Automated fixture validation still uses the supplied
`KR_wikidata_stub.json` and never calls the live Wikidata service, keeping release gates
independent of API quota, upstream data drift, and network conditions.

To refresh the South Korean translation source, rerun production extract against the real KR
GeoJSON. Fixture and parity validation that requires deterministic output should keep using the
matching stub or cache.

### Admin 2 Name Source: Korean Hanja

Admin 2 names in Chinese are **taken from the hanja spelling in the Korean Wikipedia article**, not from Wikidata's Chinese labels.

Korean administrative names are hanja words to begin with, so the hanja is the *original*, not a translation — the same principle by which the Japan handler reuses the Japanese kanji straight from the source boundary data. Wikidata's Chinese labels are a second-hand product and carry three systematic error classes:

| Error class | Example | Cause |
| :--- | :--- | :--- |
| Institution name imported by mistake | `관악구` has `zh-hant` = 「冠嶽區**廳**」 | A bot imported the `governing_body` field from the Chinese Wikipedia infobox, which names the district *office*, not the district |
| Wrong character from script conversion | `함평군` → 「**鹹**平郡」 (should be 咸平郡); `관악구` → 「冠**嶽**區」 (should be 冠岳區) | `zh-tw` / `zh-hk` labels were generated in bulk from `zh` by a naive Simplified-to-Traditional converter that hit one-to-many mappings |
| Stale after a level change | `여주시` → 「驪州**郡**」 (promoted to a city in 2013); `검단구` → 「黔丹**面**」 (became a district in 2026) | Upstream labels were never updated |

Using the hanja eliminates all three at once and requires **no manual override table**.

**Flow:**

1. Wikidata identifies *which entity* this is — search, candidate filtering, and P131 containment verification.
2. Follow the entity's `kowiki` sitelink and fetch the article's lead paragraph as plain text.
3. Extract the hanja from the parentheses in the first sentence, e.g. `함평군(咸平郡)은 …` → `咸平郡`.
4. Append the administrative-level suffix when the hanja lacks it (시→市, 군→郡, 구→區). A few articles open with a name that omits the level — Mokpo's lead reads `목포(木浦, Mokpo)`, which becomes `木浦市`.
5. If no hanja can be extracted, keep the Wikidata label so the result never gets worse.

Extracted hanja is stored in the cache under `labels[<QID>].kohanja`, so reruns do not call Korean Wikipedia again.

> [!NOTE]
> Korean hanja uses the older Kangxi character forms, which differ from Taiwan's standard forms — `淸州市` (not 清), `尙州市` (not 尚), `鎭川郡` (not 鎮), `靑陽郡` (not 青), `鷄龍市` (not 雞). The source country's forms are **kept deliberately**, consistent with how the Japanese data keeps shinjitai forms such as `県`, `沢`, `塩`, and `浜`.

**Wikidata label fallback chain** (used only when no hanja can be extracted):

1. zh-TW (Traditional Chinese – Taiwan)
2. zh-Hant (Traditional Chinese)
3. zh (Simplified Chinese, converted to Traditional via OpenCC)
4. en (English)
5. ko (Korean label)
6. Chinese Wikipedia article title (converted to Traditional)
7. Original Korean name (all sources failed)

> [!NOTE]
> English or Korean labels matched at steps 4 and 5 are rejected by `is_valid_chinese_translation`
> and fall back to the original Korean name, so they never reach the output. Across the 245
> translations currently in `geoname_data/KR_wikidata_cache.json`, all 229 Admin 2 entries come
> from hanja (`kowiki-hanja`); the remaining 16 are Admin 1, whose output comes from the built-in
> mapping table.

#### Candidate Filtering

When translating Admin 1 and Admin 2, **a candidate's Korean label must match the queried name exactly**, or it is not considered.

**Design rationale:**

The previous approach dropped candidates whose labels contained keywords such as 의회, 구청, 교육청, 廳, or *government*. That blacklist had two fatal problems:

- **It killed correct candidates.** It inspected *every* language label and dropped the candidate if any one of them matched. `관악구` (whose `zh-hant` had been filled in as 「冠嶽區廳」) and `송파구` (「鬆坡區廳」) were both eliminated, so the pipeline selected a dong inside the district (신림동) and a subway station (잠실역) instead — 48 rows of wrong place names in the published data.
- **It could never be complete.** Of the seven candidates returned for 「전남광주통합특별시」, the superintendent of education (교육감), the election commission (선거관리위원회), and the city bus service (시내버스) were all absent from the blacklist.

Exact Korean-label matching drops institutions, offices, electoral districts, and stations naturally, with zero false positives across the 245 current entries.

> [!IMPORTANT]
> This filter **must also be applied to Admin 1**. After the 2026 reorganization, a search for 「전남광주통합특별시」 also returns the identically prefixed office of education (`…교육청`), whose P131 chain likewise reaches South Korea and passes containment verification. Without filtering at Admin 1, the pipeline picks the institution and every one of the 27 divisions beneath it inherits the wrong parent QID.

> [!NOTE]
> Sejong Special Self-Governing City uses the manual mapping exclusively, never enters the Wikidata flow, and therefore neither this filter nor the hanja override applies to it.

#### Failing Loudly on Missing Containment Parents

Verifying an Admin 2 entity's P131 containment requires its Admin 1 QID. **If any Admin 1 fails to resolve to a QID, extract aborts with an error** instead of falling back to the shared layer's default of using the country QID.

That default is far too permissive for Admin 2: verifying against the country QID turns "is this 동구 inside *this* city?" into "is this 동구 anywhere in South Korea?", which Busan's and Daegu's Dong-gu both satisfy. During the 2026 reorganization this silent degradation is exactly what amplified a single wrong Admin 1 lookup into 393 + 27 wrong output rows, with no error message anywhere along the way.

Failing outright means the next reorganization stops the pipeline during extract rather than quietly shipping bad data.

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

Six names repeat, yet each is unique within its own Admin 1:

| Name | Occurrences | Parent Admin 1 |
|------|-------------|----------------|
| 중구 (中區) | 5 | 首爾市, 釜山市, 大邱市, 大田市, 蔚山市 |
| 동구 (東區) | 5 | 釜山市, 大邱市, 大田市, 蔚山市, 全南光州市 |
| 서구 (西區) | 4 | 釜山市, 大邱市, 大田市, 全南光州市 |
| 남구 (南區) | 4 | 釜山市, 大邱市, 蔚山市, 全南光州市 |
| 북구 (北區) | 4 | 釜山市, 大邱市, 蔚山市, 全南光州市 |
| 강서구 (江西區) | 2 | 首爾市, 釜山市 |

> **Note**: the two 고성군 translate differently (高城郡 in 江原道, 固城郡 in 慶尚南道), so they never collide in Chinese. Incheon's 중구 / 동구 / 서구 became 제물포구 / 영종구 / 서해구 / 검단구 on 2026-07-01 and no longer take part in these collisions.

**Handling**: P131 validation ensures the correct Admin 2 is chosen (「中區」 must sit inside 「首爾」).

#### Admin 3 Level

About 229 duplicate names (counted from the `HangJeongDong_ver20260701` boundary data). They do not affect what Immich shows, which normally stops at Admin 2.

**Strategy**: keep the Korean source text untranslated.

### Disambiguation Parentheses Removal

Some place names carry disambiguation parentheses in Wikidata (「東區 (光州)」). In this project's data structure `admin_1` already names the parent region, so `admin_2` does not need to repeat it.

**Processing logic:**

The Rust handler strips trailing disambiguation parentheses from **all** Admin 2 records, via `strip_trailing_parenthetical` in `src/pipeline/extract/handlers.rs`.

The rule previously applied only when `sidonm == "광주광역시"`. Once Gwangju merged into 전남광주통합특별시 on 2026-07-01 that condition could never hold again, and the stripping **failed silently** — `東區 (光州)` would have gone straight to output with no error anywhere. A whitelist that breaks on the next reorganization should not exist, so the rule now applies unconditionally: official Chinese names of Korean administrative divisions contain no parentheses, so there is nothing to damage.

Now that Korean hanja is the primary name source this rule is no longer exercised on the main path (hanja carries no disambiguation suffix), but it remains in place for the Wikidata label fallback chain.

**Guard rails**: `strip_trailing_parenthetical` only recognises "a closing bracket at the end plus the last opening bracket of the same type", so nested or unbalanced brackets would produce a broken string. The original value is therefore kept whenever stripping would yield an empty string or leave a bracket behind:

| Input | Output | Reason |
| :--- | :--- | :--- |
| `東區 (光州)` | `東區` | Normal strip |
| `甲（乙（丙））` | `甲（乙（丙））` | Stripping would leave the unbalanced `甲（乙` |
| `（甲）` | `（甲）` | Stripping yields an empty string, i.e. the name is lost |

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
- Usage: identifying administrative entities, P131 containment validation, and resolving the `kowiki` sitelink

**Korean Wikipedia**
- API: https://ko.wikipedia.org/w/api.php
- License: CC BY-SA 4.0
- Usage: supplies the hanja spelling for Admin 2, taken from the first sentence of the article

**Chinese Wikipedia**
- API: https://zh.wikipedia.org/w/api.php
- Usage: Simplified-to-Traditional conversion (converttitles API)

**OpenCC (Open Chinese Convert)**
- Repository: https://github.com/BYVoid/OpenCC
- Usage: Simplified Chinese to Traditional Chinese conversion

---

**Last Updated**: 2025-11-10
