# City Level Selection Criteria

> This document defines which administrative level belongs in the `name` column of
> `cities500.txt` (the "city" Immich displays), and how to validate that choice when
> adding a new country. It applies to both the official-boundary handler path and the
> LocationIQ path.

## 1. Why an Explicit Criterion Is Needed

Immich's reverse geocoding query returns exactly three fields:

```sql
SELECT * FROM geodata_places
WHERE earth_box(ll_to_earth_public($1, $2), 25000) @> ll_to_earth_public(latitude, longitude)
ORDER BY earth_distance(...) LIMIT 1
```

It returns `{country, state: admin1Name, city: name}`
(`server/src/repositories/map.repository.ts`). Immich never reads `admin2_code` or any
level below it, so **choosing the administrative level is choosing what goes into
`name`** — there is no second place to correct it later.

Choose too coarse, and the three fields cannot locate the coordinate. Indonesia's
kabupaten averages 3,705 km²; a photo taken in Ubud would display
"Bali・Gianyar Regency", an area larger than a Taiwanese county, and not a name anyone
uses to refer to Ubud.

Choose too fine, and each unit gets too few points, so label boundaries become
inaccurate (see criterion 2).

## 2. The Four Criteria

Criteria 1 through 3 are hard requirements: if any of them fails, move up one level.
Criterion 4 is a trade-off to be measured, not a gate — low Chinese coverage means most
place names display in their original language, which is normal behaviour for the
fallback chain.

### Criterion 1: Recognizability

**Names at this level are what locals and visitors use to refer to the place.**

How to check: put the level's name into the three fields "country + first-level division
+ city" and ask whether those three fields let someone recognize where the coordinate is.

- Passes: Taiwan "Taiwan・Yilan County・Jiaoxi Township", Japan "Japan・Kanagawa・Hakone"
- Fails: Indonesia "Indonesia・Bali・Gianyar Regency" — the place is called Ubud, and the
  kabupaten name is not the one a user would say

Note that this criterion has nothing to do with Chinese. `Ubud` is Indonesian, yet it is
still more recognizable than "Gianyar Regency". Chinese is criterion 4's concern.

### Criterion 2: Density

**Each unit at this level needs enough representative points to hold its boundary.**

| Metric | Threshold |
| :--- | :--- |
| Median points per unit | ≥ 5 |
| Share of units with only 1 point | < 10% |

Reason: Immich takes the nearest neighbour within 25 km, so a label's effective boundary
is the Voronoi boundary of these points. When a unit has only one point, its border with
an adjacent unit is determined entirely by the perpendicular bisector of the two points
and bears no relation to the real administrative boundary — coordinates near the border
get assigned to the neighbouring division. The denser the points (especially along the
border), the closer the Voronoi boundary approximates the real one, and the lower the
misassignment rate.

The thresholds come from measurement: the sparsest level currently shipped is Thailand's
อำเภอ (median 7, 1% single-point), whose quality is acceptable; while the next level down
in Taiwan, South Korea and Thailand all drop to a median of 1 — one point per unit — which
is below the threshold.

**Pruning does not affect this judgement.** The prune stage only removes points whose
deletion cannot change the result of any coordinate query — that is, interior points fully
enclosed by same-label neighbours; points near a boundary are by definition not removable.
Density must therefore be measured **before** pruning.

### Criterion 3: Level Consistency

**A single country may use only one administrative level.**

Mixing levels within one administrative unit is worse than being one level too coarse
overall: users cannot predict what they will see, and inconsistent labels between
neighbouring points defeat pruning.

The counterexample is Malaysia before the LocationIQ path was corrected: `normalizecity=1`
promoted nine different levels into `city` in priority order, producing up to nine
different names inside a single daerah. See `data/locationiq/README.md` for details.

**This criterion governs the administrative level only, not the language.** Name language
falls back per name — Traditional Chinese → Chinese (converted with OpenCC) → English →
source original; see
[Global Translation Processing](global-translation-processing.md#translation-priority).
Having some names in Chinese and others in their original language within one country is
the normal outcome of this pipeline, not a defect; every non-handler country looks like
this.

### Criterion 4: Chinese Coverage (Measure It, but It Is Not a Gate)

**Measure what share of units at this level have an authoritative Chinese name, and accept
that it drops as the level gets finer.**

This is **not** a pass/fail gate. Name language falls back per name (criterion 3), so low
coverage means "most names display in their original language", not that something is
broken. It is measured because changing level usually moves Chinese coverage too, and that
is one side of the trade-off:

| Country | Level | Units with Chinese |
| :--- | :--- | ---: |
| Indonesia | kabupaten | 90% |
| Indonesia | kecamatan | 6.4% |

Acceptable sources: existing Chinese fields in the country's official boundary data;
Wikidata (subject to P131 parent validation, see
[Known Translation Failures on Wikidata](wikidata-translation.md)); and the NAER glossary
(**for non-handler countries only** — for handler countries the names come from official
boundary data and are authoritative, so `naer_lookup.rs:240` always skips them, preventing
name-based mismatches from overwriting correct names).

**Machine transliteration is not accepted.** With no authoritative source, keep the
original name rather than inventing a translation — a wrong translation is harder to
recognize than the original and contaminates every subsequent match.

**Low coverage is not a reason to reject translations.** Even 6% is worth taking: that 6%
is exactly the best-known places (Ubud, Seminyak, Kuta), where users are most likely to
take photos.

## 3. Per-Country Measurements

Density and Chinese coverage are measured **before pruning**
(`data/handler/*_geodata.csv`, `data/locationiq/MY.csv`). The Malaysia row comes from the
19,903-point dataset re-queried on `feat/locationiq-address-level`; before that branch was
merged, `data/locationiq/MY.csv` was still the 791-point version using the old fields, and
the numbers differ.

| Country | Chosen level | Units | Median pts/unit | Single-point units | Units with Chinese |
| :--- | :--- | ---: | ---: | ---: | ---: |
| Taiwan | township/district | 368 | 17 | 0% | 100% |
| Japan | 市区町村 | 1,754 | 1 | 62% | 100% |
| South Korea | 시군구 | 253 | 13 | 9% | 100% |
| Thailand | อำเภอ | 927 | 7 | 1% | 94% |
| Indonesia | kecamatan | 7,377 | 11 | 0% | 4.1% (see section 4) |
| Malaysia | daerah | 144 | 102 | 3% | 90% |

The same metrics one level down, showing why we do not go further:

| Country | Next level down | Units | Median | Single-point units | Units with Chinese | Fails on |
| :--- | :--- | ---: | ---: | ---: | ---: | :--- |
| Taiwan | village | 7,814 | 1 | 100% | 100% | Criterion 2 |
| South Korea | 읍면동 | 3,080 | 1 | 99% | 0% | Criteria 2, 4 |
| Thailand | ตำบล | 7,423 | 1 | 100% | 0% | Criteria 2, 4 |
| Indonesia | desa | 63,513 | 1 | — | 0% | Criteria 2, 4 |
| Malaysia | (no such level) | — | — | — | — | See below |

Two exceptions need explanation:

**Japan has the weakest density of the six, but there is no alternative.** The median is 1
and 62% of 市区町村 have only one point; the mean of 71.3 is pulled up entirely by
island-type municipalities (Ogasawara 4,812 points, Ishinomaki 3,293, Nagasaki 2,243 — one
centroid per polygon along coastlines and islands). `admin_3` exists only for wards of
government-designated cities (171 units, 1.3% coverage), which is not a nationwide level,
so going down would be *less* consistent. 市区町村 itself passes criteria 1, 3 and 4, so it
stays.

**Malaysia has no finer administrative level to choose.** LocationIQ's address object for
MY only goes down to daerah; the values in its `admin_3` / `admin_4` fields are house
numbers, mile markers and development names (`14300`, `16英里`, `Alam Perdana`,
`Amethyst 2`), not administrative divisions. No amount of additional querying will produce
mukim, because the data source does not have that level, and LocationIQ exposes neither
`admin_level` nor `place_rank` for dynamic detection.

## 4. Indonesia: The Cost of Changing Level

Indonesia's kecamatan passes criteria 1, 2 and 3. The gap on criterion 1 is decisive —
kabupaten averages 3,705 km² and 536,000 people per name, 37 times coarser than a
Taiwanese township, and the three fields simply cannot locate the coordinate:

| Coordinate | kabupaten (old) | kecamatan (current) |
| :--- | :--- | :--- |
| Ubud | Indonesia・Bali・Gianyar Regency | Indonesia・Bali・Ubud |
| Borobudur | Indonesia・Central Java・Magelang Regency | Indonesia・Central Java・Borobudur |
| Seminyak | Indonesia・Bali・Badung Regency | Indonesia・Bali・Kuta |

### Verifying Chinese Name Sources (2026-09-14)

All 6,908 kecamatan names were queried against Wikidata in batches
(`?x rdfs:label ?name` + `P17=Q252`, **without restricting `P31`**):

| Query | Kecamatan name coverage |
| :--- | ---: |
| Restricted to `P31=Q3700011` (kecamatan class) | ~310 / 6,579 (5%) |
| Unrestricted class, matched by name | **440 / 6,908 (6.4%)**, 7.7% of points |
| Chinese Wikipedia articles (kecamatan class only) | 148 / 6,579 (2%) |
| NAER glossary | Structurally inapplicable (see below) |

**Chinese labels frequently hang on a same-named settlement entity rather than on the
kecamatan entity**, so restricting by class misses them systematically:

| Place | Entity holding the Chinese name | That entity's `P31` |
| :--- | :--- | :--- |
| Ubud | 乌布 (`Q210654`) | town |
| Seminyak | 水明漾 (`Q1026424`) | kelurahan |
| Kuta | 庫塔 (`Q994499`) | kecamatan |
| Ubud's kecamatan | none (`Q3274172`) | kecamatan |

The price of relaxing the class filter is noise: of the 440 hits, 64 same-name matches map
to multiple Chinese names, including languages and ethnic groups
(`Adonara → 阿多纳拉语`, `Banjar → 班查語`/`班查人`) and the parent division
(`Bandung → 萬隆縣`). Adopting them requires filtering through P131 parent validation, the
same approach used by the existing admin1/admin2 translators.

NAER is always skipped for handler countries (`naer_lookup.rs:240`, with the list derived
from extract's `Country` enum), because handler names come from official boundary data and
are authoritative, while NAER matches by name — `Alas` would match "阿拉斯海峽" (Alas
Strait) rather than the same-named kecamatan.

What is actually used is a vendored lookup table consolidating three sources
(`data/vendor/indonesia/kecamatan_zh.csv`, priority NAER → Wikidata → OSM), covering 286
kecamatan (329 rows, one per distinct unit for same-named units):

| | Kecamatan with Chinese | Point coverage |
| :--- | ---: | ---: |
| Jakarta | 39/44 | 249/417 (59.7%) |
| Bali | 15/57 | 266/793 (33.5%) |
| West Kalimantan | 43/173 | 632/2,391 (26.4%) |
| Nationwide | 286/6,907 (4.1%) | 5,433/107,961 (5.0%) |

OSM is accepted only when the first two have nothing and the Chinese string is
corroborated character by character by Wikidata — Bali has a batch of machine-generated
artifacts that exist only in OSM (`Sidemen → 伴奏者` is the English musical term run
through a translator), and rejecting them drops Bali from 57 to 15, which is a deliberate
trade-off. Sources and rules are documented in `data/vendor/indonesia/README.md`.

### The Cost of Changing Level

Same GeoNames snapshot, same flags, only `city_level` switched:

| | kabupaten | kecamatan |
| :--- | ---: | ---: |
| Indonesia points before pruning | 108,673 | 108,558 |
| Indonesia points after pruning | 30,992 | **83,333** |
| Distinct Indonesian city names | 514 | **6,908** |
| Global rows after pruning | 305,088 | **358,447** (+17.5%) |
| Global pruning rate | 34.8% | 23.4% |
| Uncompressed size | 62 MB | 67 MB |

Labels became 14 times finer, so the redundancy pruning can prove shrinks sharply —
Indonesia's deletion rate falls from 71.5% to 23.2%, and the global output gains 53,359
rows. This is the direct cost of recognizability, and it has been accepted.

For implementation, province and timezone details, see
[Indonesia Administrative Division Processing](indonesia-admin-processing.md).

## 5. Checklist for Adding a Country

1. List the country's administrative levels and the number of units at each
2. Starting from the finest level and working up, check the four criteria level by level;
   the first level that passes all of them is the answer
3. Compute density from `data/handler/{cc}_geodata.csv` (or the LocationIQ CSV) **before
   pruning**: the median points per unit and the share of single-point units
4. Compute Chinese coverage as "share of units with a Chinese name", not point coverage —
   only unit coverage reflects how many distinct Chinese names a user will see
5. Add the results to the two tables in section 3 of this document
