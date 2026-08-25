# Indonesia Administrative Division Processing

> This document explains how the project processes geographic information for Indonesia. It is the detailed version of [Supported Regions and Language Strategy](../../README.en.md#supported-regions-and-language-strategy) in the README.

## Data Sources

Indonesian processing is built around the official administrative boundary data published by the **Geospatial Information Agency of Indonesia (BIG)**:

- **Source**: official REST service of Badan Informasi Geospasial (BIG)
- **Provider**: BIG, published through the official ArcGIS REST service (`BATASWILAYAH/BATAS_DESAKEL_AR` MapServer)
- **Dataset layer**: desa (village) boundaries; the attributes carry province, regency/city, district, and village names
- **Download date**: 2026-08-24
- **Data version**: `TASWIL1000020260612DESAKEL_AR` (2026-06-12, recorded in each feature's `METADATA` attribute)
- **Download precision**: `geometryPrecision=6` (six decimal places, full-precision output with no geometry simplification)
- **Coordinate system**: attribute `SRS_ID = "4326"`, treated directly as WGS84 (EPSG:4326)

For the download steps and the error checks they require, see [Local Data Processing](development.md#indonesia).

### License Compliance Stance

BIG boundary data is official, publicly available Indonesian geospatial data. This project **uses it only as input for derivative processing** and publishes reverse-geocoding-optimized place names and representative points (cities500-style single-point metadata). It **does not distribute or re-publish BIG's original vector boundary data (polygons)**. Obtain the original boundary data directly from the official BIG service. Should a redistributable open dataset be needed as an alternative or for cross-validation, **GADM** and the **HDX (OCHA)** Indonesia COD-AB are known fallback sources.

## Administrative Levels

> [!NOTE]
> `admin_3` and `admin_4` exist only in this project's intermediate CSV, where they preserve the source administrative levels for tracing and debugging; they are never written to the cities500 file Immich consumes. The finest level Immich displays is `admin_2`. Representative point density depends on the granularity of the source features read during extract (desa for Indonesia, with multipart features split per part), not on these two columns.

The BIG desa attributes provide the following administrative levels:

- **Admin 1**: province (Provinsi, BIG field `WADMPR`)
- **Admin 2**: regency / city (Kabupaten / Kota, BIG field `WADMKK`)
- **Admin 3**: district (Kecamatan, BIG field `WADMKC`)
- **Admin 4**: village (Desa / Kelurahan, BIG field `WADMKD`)

This project extracts from the desa-level boundary data (village polygons increase positioning density) and emits the following columns:

| Output column | Source field | Description |
|---|---|---|
| `country` | Fixed value | `印尼` |
| `admin_1` | Wikidata / `WADMPR` | Province in Traditional Chinese; falls back to the official BIG Indonesian name when no Chinese exists |
| `admin_2` | Wikidata / `WADMKK` | Regency / city in Traditional Chinese; falls back to the official BIG Indonesian name when no Chinese exists |
| `admin_3` | `WADMKC` | District (Kecamatan), official Indonesian name |
| `admin_4` | `WADMKD` | Village (Desa / Kelurahan), official Indonesian name |

As with the TH / KR handlers, `admin_1` / `admin_2` carry translated Traditional Chinese while `admin_3` and below keep the original Indonesian.

Translation lookup for `admin_2` runs in three stages: first query the (province, regency/city) pair; if that misses, fall back to a global same-name table (a given `WADMKK` is listed only when every province agrees on its translation, and inconsistent ones are dropped to avoid mismatched names); only then fall back to the BIG original.

> **Undefined-area filtering**: features whose `WADMPR` or `WADMKK` is empty are mostly "Area tidak terdefinisi" (undefined administrative area). They cannot map to a province or regency/city and are skipped during extract.

> **Row counts**: this batch of desa boundary data holds 84,468 usable features (84,503 originally; 35 were filtered for a missing `WADMPR` / `WADMKK`). Extract emits more rows than there are administrative divisions, because when a single desa consists of several disjoint polygons (a multipart boundary), **each part gets its own Albers centroid and becomes a separate row**. After per-part splitting, this batch of desa yields 108,673 candidate points, matching the current row count of `meta_data/id_geodata.csv`. Row count and administrative-division count therefore differ, which is expected behavior. The feature count is an offline statistic for the 2026-08-24 download batch and is not recomputed on every release.

## Naming Strategy

The Indonesia handler reuses the Wikidata translator pipeline from the Thailand / South Korea handlers: **Admin 1 and Admin 2 both go through standard P131 chain validation, with instance-of (P31) class filtering of candidates**; when no reliable Chinese exists, names fall back to the official BIG Indonesian. Naming is decided in two tiers: first decide whether to trust the Wikidata result, then decide which language label to use.

### Tier 1: Whether to Trust the Wikidata Result (P131 Parent Validation)

| Level | Parent QID | P131 validation | When validation fails |
|---|---|---|---|
| **Admin 1** (province) | Indonesia (`Q252`) | Takes the first passing candidate | Falls back to official BIG Indonesian |
| **Admin 2** (regency / city) | QID of the parent province | Takes the first passing candidate | Falls back to official BIG Indonesian |

Candidates are validated one by one in search-ranking order, and the first entity that passes P131 wins; when none pass, the name falls back to the official BIG Indonesian. A failed WDQS query (timeout, unparsable response) counts as a candidate that did not pass and is not written to the cache, so a transient network problem only makes that entry fall back to the original name instead of freezing a wrong conclusion into the cache.

The Admin 2 parent QID comes from the province QID resolved by the Tier 1 Admin 1 translation. A full WDQS re-validation of all 38 provinces passed `(wdt:P131)+ → Q252` 38/38 (100%) and P31 containing `Q5098` 38/38 (100%), so the standard P131 rule does not make correct existing admin1 translations regress. This is an offline validation performed while building the handler; the record lives in the [Indonesia handler research report](../research/indonesia-handler.md) (in Traditional Chinese).

#### P31 Candidate Class Filter (Five Classes)

Entity lookups confirmed that a candidate must fall into one of the five instance-of classes below to count as a valid administrative division. Each level uses its own allow-set: Admin 1 permits only `Q5098`, Admin 2 permits the other four.

| QID | English label | Chinese | Use |
|---|---|---|---|
| `Q5098` | province of Indonesia | 印度尼西亞省 | Admin 1 (province) |
| `Q3191695` | regency of Indonesia | 縣 (kabupaten) | Admin 2 standard |
| `Q3199141` | city of Indonesia | 市 (kota) | Admin 2 standard |
| `Q4272761` | administrative city of Indonesia | 行政市 | Jakarta's five administrative cities |
| `Q11127777` | administrative regency of Indonesia | 行政縣 | Jakarta's Thousand Islands |

> [!IMPORTANT]
> The last two classes (`Q4272761` / `Q11127777`) were missed in the pilot experiment. Jakarta's five city districts and the Thousand Islands use these special "administrative city / administrative regency" classes rather than ordinary kota / kabupaten. Validating with only three classes hit Jakarta's six districts 0/6; adding the two classes raised it to 6/6. The handler's P31 filter set **must** contain all five classes.

#### Excluding Electoral Districts (dapil)

Indonesian electoral districts (daerah pemilihan / dapil) on Wikidata frequently share names with administrative divisions, covering both national (DPR / DPD) and local council (DPR-D) districts. Lookups returned the electoral-district classes `Q56072658` (electoral district in Indonesia) and `Q109540666` (DPR-D electoral district). Neither class belongs to the allow-sets above, so electoral districts are dropped during candidate filtering; labels containing tokens such as dapil / DPR / DPRD / DPD / pemilihan / electoral are additionally excluded by tokenized keyword matching. "Province name + Roman numeral" districts (such as `Jawa Barat V`, `DKI Jakarta II`, `Sumatera Utara II`) do collide with province names, but they too are dropped because their P31 is not in the allow-set.

### Search Language: Indonesian (id) Primary, English as Verification Fallback

Search uses Indonesian (`id`) as the primary language; English (`en`) serves only as a fallback for manual verification and never enters the automated pipeline.

> [!IMPORTANT]
> "Indonesian primary" is an experimentally validated choice.
>
> - **Admin 1 (all 38 provinces)**: id and en both hit 38/38 (100%) at rank-1. They tie, and id is chosen to stay consistent with admin2.
> - **Admin 2 (94-item test set)**: id hits **94/94 (100%)** at rank-1, en hits 90/94 (95.7%). All four en failures are `Kota X` city-level units (Kota Tegal, Kota Kediri, Kota Madiun, Kota Probolinggo): the en label prefers the identically named bare regency, pushing the city to rank-2. The same cases hit rank-1 with id.
>
> Test-set construction (`seed=42`, reproducible): all 52 structurally colliding names (26 pairs where a bare name and a `Kota`-prefixed name coexist, taking one of each), plus 50 random entries, deduplicated to 94. These hit rates come from an offline experiment run while building the handler and are not verified by the code; the raw record lives in the [Indonesia handler research report](../research/indonesia-handler.md) (in Traditional Chinese).

#### Search String Normalization

For regencies, BIG's `WADMKK` mostly stores only the place name after "Kabupaten", which collides with the identically named kota (city), so searching it directly picks the wrong entity. The search string is therefore decoupled from the lookup-table key:

- Entries starting with `Kota ` (city): kept as-is.
- Everything else (regency): the `Kabupaten ` prefix is prepended before searching.
- Jakarta city districts: expanded to generic names per the rules in the next section.

### Jakarta Special Capital Region Normalization

BIG stores Jakarta's five city districts and the Thousand Islands under their official full names, but Wikidata uses generic names as the primary label, so without normalization the correct entity is never found. Expansion rules and verification results (all rank-1, all P131 to `Q3630`):

| WADMKK (BIG original) | Normalized query | QID | P31 |
|---|---|---|---|
| Kota Adm. Jakarta Barat | Jakarta Barat | `Q10116` | `Q4272761` |
| Kota Adm. Jakarta Pusat | Jakarta Pusat | `Q10109` | `Q4272761` |
| Kota Adm. Jakarta Selatan | Jakarta Selatan | `Q10114` | `Q4272761` |
| Kota Adm. Jakarta Timur | Jakarta Timur | `Q10111` | `Q4272761` |
| Kota Adm. Jakarta Utara | Jakarta Utara | `Q10113` | `Q4272761` |
| Adm. Kep. Seribu | Kepulauan Seribu | `Q10107` | `Q11127777` |

#### DKI / DKJ Note

The Wikidata entity for Jakarta province is `Q3630`, whose primary label is still "Jakarta" (en="Jakarta", zh="雅加达", zh-tw="雅加達", id="Jakarta"), with P31 containing `Q5098` (still registered at province level). Jakarta was recently renamed from DKI (Daerah Khusus Ibukota, Special Capital Region) to **DKJ (Daerah Khusus Jakarta, Jakarta Special Region)**, but the rename is not yet reflected in the Wikidata primary label. The **admin1 search string therefore uses BIG's original `WADMPR` value "DKI Jakarta"** (rank-1 hit on `Q3630`) instead of relying on a primary label that can change. For the same reason, "Daerah Istimewa Yogyakarta" (Yogyakarta Special Region) is searched with its original `WADMPR` value as well.

### Tier 2: Language Label Priority

Once an item is set to adopt its Wikidata result, the name is chosen in this order:

1. Wikidata `zh-tw` label
2. Wikidata `zh-hant` label
3. Wikidata `zh` label, converted to Traditional Chinese with OpenCC (`cn2t` preset)
4. Title of the Wikidata Chinese Wikipedia (zhwiki) sitelink, converted to Traditional Chinese through the zh.wikipedia conversion API
5. Official BIG Indonesian original (`WADMPR` / `WADMKK`)

> **Why the fallback chain excludes English**: the handler sets the translator's `fallback_langs` explicitly to `["zh-hant", "zh"]` (mirroring `thailand_wikidata.rs`), excluding `en` and the source language (`id`) from the default chain. The BIG source has no official English field, so leaving `en` in would make regencies/cities without a Chinese label fall back to Wikidata English (for example Aceh Tengah → Central Aceh), contradicting the design rule of keeping the official Indonesian original when Chinese is missing.
>
> **Why simplified-to-traditional conversion is needed**: Traditional Chinese labels for Indonesian place names are scarce on Wikidata, and `zh` plus some `zh-hant` labels mix in Simplified characters (Papua's `zh-hant` label, for instance, is 巴布亚省). The translator converts `zh` labels with OpenCC; `zh-hant` labels are corrected in the handler's consumer layer using a safe character-level simplified-to-traditional whitelist.

> **Why not run full OpenCC conversion over `zh-hant`**: OpenCC's full simplified-to-traditional conversion over-converts characters that are already Traditional into variant forms (`里→裏`, `占→佔`, `岩→巖`, `干→乾`, `群→羣`). Applying the full conversion to real ID output broke 24 correct translations (for example 井里汶縣 → 井裏汶縣, 峇里巴板 → 峇裏巴板) while fixing only one (巴布亚省 → 巴布亞省). The handler therefore uses a whitelist of characters that are Simplified-only and unambiguous in Traditional (see `indonesia_normalize::fix_simplified_chars`), fixing genuine Simplified characters only and staying idempotent and regression-free for already-correct Traditional proper names.

Coverage of zh-family labels for admin1 is 38/38 (100%), and the four new Papua provinces are all **semantic translations** rather than transliterations. The table shows the final output after the consumer layer's safe simplified-to-traditional conversion and province-suffix completion; the raw `zh` label for Papua Barat Daya is the Simplified 西南巴布亚省:

| Province | QID | Final admin1 output |
|---|---|---|
| Papua Barat Daya | `Q115253263` | 西南巴布亞省 |
| Papua Pegunungan | `Q112810104` | 高地巴布亞省 |
| Papua Selatan | `Q61439296` | 南巴布亞省 |
| Papua Tengah | `Q12486766` | 中巴布亞省 |

The few entries that genuinely lack Chinese on Wikidata (no zh-family label at all) follow the fallback chain to the official BIG Indonesian original, never to English. For the actual fallback count and list, read `geoname_data/ID_wikidata_cache.json` after a release re-extract.

### Consumer-Layer Guarding

Before the handler adopts a Wikidata translation, it runs two more checks on the string itself:

- **Strip disambiguation parentheses**: labels of same-name Wikidata entities may carry a disambiguation suffix (such as 薩米縣 (巴布亞省)). Trailing parentheses are always removed, so Wikidata's internal disambiguation format never reaches users.
- **Pure-Chinese check**: a translation containing ASCII letters, or containing no Han characters at all, is treated as invalid and falls back to the official BIG Indonesian. This check catches pure Latin strings (such as `East Barito`) and half-translated mixtures (such as 西Kutai區).

Both checks treat live queries, the cache, and fixture stubs alike (see `label_sanitize`), so a wrong translation left over in an old cache cannot slip past them.

### Wikidata Cache

The Wikidata cache lives at:

```text
geoname_data/ID_wikidata_cache.json
```

Fixture tests use `ID_wikidata_stub.json` to avoid depending on live network queries.

## Coordinate Strategy

The BIG schema has **no official representative point field** (attributes only carry administrative codes, names, area, and the like), so the representative point must be derived from the geometry. This project uses the **geometric centroid under an Albers equal-area projection**, taking one centroid per MultiPolygon part (consistent with per-part splitting), and introduces no `representative_point` fallback.

The experiment used each desa part's Albers centroid as a candidate point and bbox rejection sampling inside each kecamatan as simulated GPS, matched by BallTree haversine nearest neighbor:

| Coordinate strategy | admin2 hit rate |
|---|---:|
| Albers centroid | **96.99%** |
| representative_point | 96.80% |

The centroid wins overall by 0.19 percentage points. For an archipelago, 2.21% of centroids (2,311 of 104,470 parts) do fall outside their own part geometry, but **no fallback is needed**: candidate points serve only as representative points for nearest-neighbor matching, not as display coordinates, and a centroid landing tens to hundreds of meters outside rarely changes which part is nearest. `representative_point`, by guaranteeing a point inside the geometry, instead pushes the representative point toward concave edges and reduces representativeness. The centroid wins in 6 of the 7 regional breakdowns, with every region stable between 96% and 98% and no weak region.

These hit rates come from an offline experiment run while building the handler and are not verified by the code; the full method and data live in [Indonesia Projection and Coordinate Experiment](../research/idn-handler-projection-coordinate-experiment.md) (in Traditional Chinese).

## Projection Strategy

The Indonesia handler computes centroids with a single Indonesia Albers equal-area projection:

```text
+proj=aea +lat_1=1 +lat_2=-8 +lat_0=-3 +lon_0=118 +x_0=0 +y_0=0 +ellps=GRS80 +towgs84=0,0,0,0,0,0,0 +units=m +no_defs
```

Design rationale: Indonesia spans roughly 6°N–11°S (about 17°), so the standard parallels sit about 1/6 and 5/6 of the way inside the north and south edges to minimize areal distortion — `lat_1` = +1° (pulled inward because there is little landmass north of the equator), `lat_2` = −8°, `lat_0` = −3° (latitude center), `lon_0` = 118° (longitude center of the archipelago), and the GRS80 ellipsoid (consistent with SRGI 2013 / WGS84).

This strategy skips the dynamic UTM flow used by the Japan and South Korea handlers, following Thailand's single-Albers precedent. Measured differences between Albers and dynamic UTM centroids are tiny:

| Sample group | n | Median | Mean | p99 | Max |
|---|---:|---:|---:|---:|---:|
| Full random sample | 5,000 | 0.0108 m | 0.1850 m | 3.79 m | 32.98 m |
| Top 100 by area | 100 | 2.49 m | 3.17 m | 11.53 m | 12.12 m |
| Top 200 by longitude span | 200 | 2.52 m | 3.25 m | 12.14 m | 32.98 m |

For ordinary desa the two methods differ at the centimeter scale (median 0.011 m); even the extreme scattered-island samples with a wide longitude span (max difference 32.98 m, at Nua Nea village, Maluku Tengah Regency, Maluku Province) differ by only tens of meters, far below the spatial granularity of a village-level division and with no effect on nearest-neighbor admin2 assignment. Weighing accuracy, performance, and implementation simplicity, Indonesia computes centroids directly with a single Albers projection. The table above is likewise an offline experiment result; its source is [Indonesia Projection and Coordinate Experiment](../research/idn-handler-projection-coordinate-experiment.md) (in Traditional Chinese).

## Time Zone Handling

Indonesia spans three time zones, resolved through a per-province table covering all 38 provinces:

| Time zone | IANA | UTC offset | Provinces | Provinces (BIG WADMPR original) |
|---|---|---|---:|---|
| WIB (Waktu Indonesia Barat) | `Asia/Jakarta` | UTC+7 | 18 | Aceh, Sumatera Utara, Sumatera Barat, Riau, Kepulauan Riau, Jambi, Sumatera Selatan, Kepulauan Bangka Belitung, Bengkulu, Lampung, DKI Jakarta, Banten, Jawa Barat, Jawa Tengah, Daerah Istimewa Yogyakarta, Jawa Timur, Kalimantan Barat, Kalimantan Tengah |
| WITA (Waktu Indonesia Tengah) | `Asia/Makassar` | UTC+8 | 12 | Kalimantan Selatan, Kalimantan Timur, Kalimantan Utara, Bali, Nusa Tenggara Barat, Nusa Tenggara Timur, Sulawesi Utara, Sulawesi Tengah, Sulawesi Selatan, Sulawesi Tenggara, Gorontalo, Sulawesi Barat |
| WIT (Waktu Indonesia Timur) | `Asia/Jayapura` | UTC+9 | 8 | Maluku, Maluku Utara, Papua, Papua Barat, Papua Selatan, Papua Tengah, Papua Pegunungan, Papua Barat Daya |

Time zones are resolved during the transform stage (cities500 schema). Assignment keys off the **original BIG WADMPR spelling** (`WIB/WITA/WIT_PROVINCES`) as the single authority: the original spelling is stable and language-independent, immune to translation forms and to future drift in Wikidata labels.

The transform stage only sees the handler's final province name (the Wikidata Traditional Chinese translation after safe simplified-to-traditional conversion and province-suffix completion, such as 中爪哇省, 巴釐省, 巴布亞省), because the canonical CSV schema carries no WADMPR column. The table therefore ships a companion "final province name → WADMPR original" mapping (`PROVINCE_ZH_TW`) as the lookup entry point. These Traditional Chinese keys **must match the handler's final admin1 output character for character**, which the `handler_admin1_outputs_resolve_timezone` test in `indonesia_timezone` asserts by checking that all 38 final province names hit the table, preventing drift between the two places. When a province name misses the table, transform reports an error and fails the release rather than silently applying WIB, so the problem surfaces before shipping.

## Notes

- Indonesian Admin 1 / Admin 2 use Wikidata Traditional Chinese translations, and both levels must pass P131 parent validation (Admin 1 against Indonesia `Q252`, Admin 2 against the QID of its province). On validation failure, or when Wikidata offers no reliable Chinese result, names fall back to the official BIG Indonesian.
- Indonesian Admin 3 (district) and Admin 4 (village) keep the official BIG Indonesian, avoiding the mismatches and unstable translations that affect large numbers of low-level place names on Wikidata.
- The original BIG vector boundary data is out of scope for this project's distribution; only the reverse-geocoding-optimized derivative metadata is distributed.
- Every coordinate decision, hit-rate measurement, and search-language experiment fixes the random seed at `seed=42`, so the results are reproducible.
- For the commands to reproduce the extraction locally, see [Local Data Processing](development.md#2-extract-raw-geographic-data).
