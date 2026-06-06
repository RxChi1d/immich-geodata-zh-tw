# Indonesia Administrative Processing Guide

This document captures the data source, administrative hierarchy, naming strategy, and coordinate/projection details for Indonesian geospatial data. It is the detailed companion to the Indonesia optimization section in the README.

## Table of Contents

- [Data Source](#data-source)
- [Administrative Levels](#administrative-levels)
- [Naming Strategy](#naming-strategy)
  - [Tier 1: Whether to Trust the Wikidata Result (P131 Validation)](#tier-1-whether-to-trust-the-wikidata-result-p131-validation)
  - [Search Language: Indonesian Primary, English Verification Fallback](#search-language-indonesian-primary-english-verification-fallback)
  - [Jakarta Special Capital Region Normalization](#jakarta-special-capital-region-normalization)
  - [Tier 2: Language Label Priority](#tier-2-language-label-priority)
  - [Wikidata Cache](#wikidata-cache)
- [Coordinate Strategy](#coordinate-strategy)
- [Projection Strategy](#projection-strategy)
- [Time Zone Handling](#time-zone-handling)
- [Extraction Workflow](#extraction-workflow)
- [Notes](#notes)

---

## Data Source

Indonesian geospatial processing is built around the official **Geospatial Information Agency of Indonesia (BIG)** administrative boundaries:

- **Source**: Badan Informasi Geospasial (BIG) official ArcGIS REST FeatureServer
- **Provider**: BIG, published via the official REST service
- **Dataset layer**: desa (village) boundaries; attributes carry province, regency/city, district, and village names
- **Download date**: 2026-06-06
- **Data version**: `TASWIL20230928` (BIG administrative-division version identifier)
- **Download precision**: `geometryPrecision=6` (six decimal places, full-precision output with no geometry simplification)
- **Coordinate system**: attribute `SRS_ID = "SRGI 2013"`, treated as EPSG:4326 (difference from WGS84 is negligible at the meter scale)

### License Compliance Stance

BIG data is official, publicly available Indonesian geospatial data. This project **uses it only as input for derivative processing**, emitting reverse-geocoding-optimized place names and representative coordinates (cities500-style single-point metadata). It **does not redistribute or re-publish BIG's original vector boundaries (polygons)**. Obtain the original boundaries directly from the BIG service. If a redistributable open-data source is needed as an alternative or for cross-validation, **GADM** and the **HDX (OCHA)** Indonesia COD-AB are known fallback sources.

---

## Administrative Levels

The BIG desa attributes provide the following levels:

- **Admin 1**: Province (Provinsi, BIG field `WADMPR`)
- **Admin 2**: Regency / City (Kabupaten / Kota, BIG field `WADMKK`)
- **Admin 3**: District (Kecamatan, BIG field `WADMKC`)
- **Admin 4**: Village (Desa / Kelurahan, BIG field `WADMKD`)

This project uses the desa-level layer as the extract source (village polygons increase positioning density). The output columns are:

| Output Column | Source Field | Description |
|---|---|---|
| `country` | Fixed value | `印尼` (Indonesia) |
| `admin_1` | Wikidata / `WADMPR` | Province in Traditional Chinese; falls back to official Indonesian when no Chinese is available |
| `admin_2` | Wikidata / `WADMKK` | Regency / City in Traditional Chinese; falls back to official Indonesian when no Chinese is available |
| `admin_3` | `WADMKC` | District (Kecamatan) official Indonesian name |
| `admin_4` | `WADMKD` | Village (Desa / Kelurahan) official Indonesian name |

As with the TH / KR handlers, `admin_1` / `admin_2` are translated Traditional Chinese while `admin_3` and below keep the original Indonesian.

> [!NOTE]
> **Undefined-area filtering**: rows where `WADMPR` or `WADMKK` is empty are usually "Area tidak terdefinisi" (undefined administrative area); they cannot map to a province / regency and are skipped during extract.

> [!NOTE]
> **Row count**: this batch of desa data has 83,461 usable features (out of 83,462; one is filtered for a missing `WADMPR` / `WADMKK`). The extract emits more rows than administrative units because when a single desa is made up of several disjoint polygons (a multipart boundary), **each part gets its own Albers centroid and is written as a separate row**. In the hit-rate experiment, these desa expand to 104,470 candidate points after multipart splitting. The row count therefore does not equal the administrative-unit count, which is expected behavior.

---

## Naming Strategy

The Indonesia handler reuses the Thailand / South Korea Wikidata translator pipeline: **both Admin 1 and Admin 2 go through standard P131 chain validation with instance-of (P31) class filtering of candidates**; when no reliable Chinese exists, names fall back to BIG official Indonesian. Naming is decided in two tiers: first decide whether to trust the Wikidata result, then decide which language label to use.

### Tier 1: Whether to Trust the Wikidata Result (P131 Validation)

| Level | Parent QID | P131 validation | When validation fails |
|---|---|---|---|
| **Admin 1** (Province) | Indonesia (`Q252`) | Every candidate must pass | Falls back to BIG official Indonesian |
| **Admin 2** (Regency / City) | The QID of its Province | Every candidate must pass | Falls back to BIG official Indonesian |

The Admin 2 parent QID comes from the Province QID resolved during Admin 1 translation. A full WDQS re-validation over the 38 provinces showed `(wdt:P131)+ → Q252` passing 38/38 (100%) and P31 containing `Q5098` passing 38/38 (100%); the standard P131 rule does not cause correct admin1 translations to regress.

#### P31 Candidate Class Filter (Five Classes)

Entity lookups confirmed that admin2 candidates must fall into one of the following five instance-of classes to be accepted as a valid administrative unit:

| QID | English label | Chinese | Use |
|---|---|---|---|
| `Q5098` | province of Indonesia | 印度尼西亞省 | Admin 1 (province) |
| `Q3191695` | regency of Indonesia | 縣 (kabupaten) | Admin 2 standard |
| `Q3199141` | city of Indonesia | 市 (kota) | Admin 2 standard |
| `Q4272761` | administrative city of Indonesia | 行政市 | Jakarta's five administrative cities |
| `Q11127777` | administrative regency of Indonesia | 行政縣 | Jakarta's Thousand Islands |

> [!IMPORTANT]
> The last two classes (`Q4272761` / `Q11127777`) were missed in the pilot. Jakarta's five city districts and the Thousand Islands use these special "administrative city / administrative regency" classes rather than ordinary kota / kabupaten. The initial three-class validation hit Jakarta's six districts 0/6; adding the two classes raised it to 6/6. The handler's P31 filter set **must** include all five classes.

#### Excluding Electoral Districts (dapil)

Indonesian electoral districts (daerah pemilihan / dapil) on Wikidata frequently share names with administrative areas, including national (DPR / DPD) and local council (DPR-D) electoral districts. Lookups returned the electoral-district classes `Q56072658` (electoral district in Indonesia) and `Q109540666` (DPR-D electoral district). These are excluded by the administrative P31 filter (e.g. requiring `Q5098`) and keyword matching. "Province name + Roman numeral" electoral districts (such as `Jawa Barat V`, `DKI Jakarta II`, `Sumatera Utara II`) collide with province names but are naturally excluded because their labels do not exactly equal the bare province search string.

### Search Language: Indonesian Primary, English Verification Fallback

Search uses Indonesian (`id`) as the primary language; English (`en`) is only a manual verification fallback and is not used in the automated pipeline.

> [!IMPORTANT]
> "Indonesian primary" is an experimentally validated choice.
>
> - **Admin 1 (all 38 provinces)**: both id and en hit 38/38 (100%) at rank-1. They tie, and id is chosen for consistency with admin2.
> - **Admin 2 (94-item test set)**: id is **94/94 (100%)** at rank-1, while en is 90/94 (95.7%). The four en failures are all `Kota X` city-level units (Kota Tegal, Kota Kediri, Kota Madiun, Kota Probolinggo), where the en label prefers the identically named bare regency, pushing the city to rank-2; the same cases hit rank-1 with id.
>
> Test-set construction (`seed=42`, reproducible): all 52 structurally name-colliding entities (26 "bare name + Kota prefix" pairs, taking both bare and Kota), plus a random 50, deduped to 94. When adding a new country, run the same sampling experiment before choosing the search language; see the new-country section in `CLAUDE.md`.

#### Search String Normalization

BIG's `WADMKK` usually stores only the place name after "Kabupaten" for regencies, which collides with the identically named kota (city). The search string is therefore decoupled from the lookup-table key:

- Entries starting with `Kota ` (city): kept as-is.
- Others (regency): the `Kabupaten ` prefix is prepended before searching.
- Jakarta city districts: expanded to generic names per the next section.

### Jakarta Special Capital Region Normalization

BIG stores Jakarta's five city districts and the Thousand Islands under official full names, but Wikidata uses generic names as the primary label; without normalization the correct entity is not found. Expansion rules and verification results (all rank-1, all P131 to `Q3630`):

| WADMKK (BIG original) | Normalized query | QID | P31 |
|---|---|---|---|
| Kota Adm. Jakarta Barat | Jakarta Barat | `Q10116` | `Q4272761` |
| Kota Adm. Jakarta Pusat | Jakarta Pusat | `Q10109` | `Q4272761` |
| Kota Adm. Jakarta Selatan | Jakarta Selatan | `Q10114` | `Q4272761` |
| Kota Adm. Jakarta Timur | Jakarta Timur | `Q10111` | `Q4272761` |
| Kota Adm. Jakarta Utara | Jakarta Utara | `Q10113` | `Q4272761` |
| Adm. Kep. Seribu | Kepulauan Seribu | `Q10107` | `Q11127777` |

#### DKI / DKJ Note

Jakarta province's Wikidata entity is `Q3630`, whose primary label is still "Jakarta" (en="Jakarta", zh="雅加达", zh-tw="雅加達", id="Jakarta"), with P31 containing `Q5098` (still registered at the province level). Jakarta was recently renamed from DKI (Daerah Khusus Ibukota, Special Capital Region) to **DKJ (Daerah Khusus Jakarta, Jakarta Special Region)**, but this rename is not yet reflected in the Wikidata primary label. The **admin1 search string therefore uses BIG's `WADMPR` original value "DKI Jakarta"** (which hits `Q3630` at rank-1), not the mutable primary label. Likewise, "Daerah Istimewa Yogyakarta" (Yogyakarta Special Region) is searched using its `WADMPR` original value.

### Tier 2: Language Label Priority

Once an item is set to adopt the Wikidata result, the name is chosen in this order:

1. Wikidata `zh-tw` label
2. Wikidata `zh-hant` label
3. Wikidata `zh` label, converted to Traditional Chinese via OpenCC s2t (Simplified → Traditional)
4. BIG official Indonesian (`WADMPR` / `WADMKK`)

> **Why OpenCC s2t is needed**: experiments show Traditional Chinese labels for Indonesian place names barely exist on Wikidata. Of 10 sampled verified regencies/cities, `zh` had a value 10/10 while `zh-tw` had only 1/10, and the `zh` labels mix Simplified and Traditional (e.g. "下罗干县", "双木丹县", "北鲁乌县" are Simplified). Across the 94-item admin2 hit set, coverage is: `zh` 89/94 (94.7%), `zh-hant` 56/94 (59.6%), `zh-tw` 4/94 (4.3%), any zh-family 90/94 (95.7%). The handler therefore mirrors `thailand_wikidata.rs`: the translator's primary language is `zh-tw` with fallback `["zh-hant", "zh"]`, and `translate.rs` applies OpenCC s2t to fill in Traditional.

Admin1 zh coverage is 38/38 (100%); the four new Papua provinces are all **semantic translations** rather than transliterations:

| Province | QID | zh label |
|---|---|---|
| Papua Barat Daya | `Q115253263` | 西南巴布亚省 (Simplified, converted by s2t) |
| Papua Pegunungan | `Q112810104` | 高地巴布亞省 |
| Papua Selatan | `Q61439296` | 南巴布亞省 |
| Papua Tengah | `Q12486766` | 中巴布亞省 |

A few entries genuinely missing Chinese on Wikidata (4 of the 515 unique regencies/cities in the pilot full set have no zh-family label: Pekalongan `Q10623`, Solok `Q6058`, Pasaman Barat `Q6103`, Kepahiang `Q7940`) fall back to BIG official Indonesian.

### Wikidata Cache

The Wikidata cache lives at:

```text
geoname_data/ID_wikidata_cache.json
```

Fixture tests use `ID_wikidata_stub.json` to avoid depending on live network queries.

---

## Coordinate Strategy

The BIG schema has **no official representative-point field** (attributes only carry administrative codes, names, area, etc.), so the representative coordinate must be derived from geometry. This project uses the **geometric centroid under an Albers equal-area projection**, taking one centroid per MultiPolygon part (consistent with multipart splitting), and introduces no `representative_point` fallback.

Using each desa part's Albers centroid as candidate points and bbox rejection sampling inside kecamatan as simulated GPS, with BallTree haversine nearest-neighbor matching:

| Coordinate strategy | admin2 hit rate |
|---|---:|
| Albers centroid | **96.99%** |
| representative_point | 96.80% |

The centroid wins overall by 0.19 percentage points. Although for an archipelago 2.21% of centroids (2,311 of 104,470 parts) fall outside their part geometry, **no fallback is needed**: candidate points serve only as nearest-neighbor representative coordinates (not display coordinates), so a centroid falling tens to hundreds of meters outside usually does not affect the "nearest part" decision; and `representative_point`, by guaranteeing a point inside the geometry, pushes the representative point toward concave edges, reducing representativeness. The centroid wins in 6 of 7 regions, with every region stable in the 96%–98% range and no weak region.

---

## Projection Strategy

The Indonesia handler computes centroids using a single Indonesia Albers equal-area projection:

```text
+proj=aea +lat_1=1 +lat_2=-8 +lat_0=-3 +lon_0=118 +x_0=0 +y_0=0 +ellps=GRS80 +towgs84=0,0,0,0,0,0,0 +units=m +no_defs
```

Design rationale: Indonesia spans roughly 6°N–11°S (about 17°). Standard parallels are placed about 1/6 and 5/6 inside the north/south edges to minimize areal distortion — `lat_1` = +1° (the northern landmass is sparse, so it is pulled inward), `lat_2` = −8°, `lat_0` = −3° (latitude center), `lon_0` = 118° (archipelago longitude center), GRS80 ellipsoid (consistent with SRGI 2013 / WGS84).

This does not use the dynamic UTM flow from the Japan and South Korea handlers, consistent with Thailand's single-Albers precedent. Measured differences between Albers and dynamic UTM centroids are tiny:

| Sample group | n | Median | Mean | p99 | Max |
|---|---:|---:|---:|---:|---:|
| Full sample | 5,000 | 0.0108 m | 0.1850 m | 3.79 m | 32.98 m |
| Top 100 by area | 100 | 2.49 m | 3.17 m | 11.53 m | 12.12 m |
| Top 200 by longitude span | 200 | 2.52 m | 3.25 m | 12.14 m | 32.98 m |

Ordinary desa differ at the centimeter scale (median 0.011 m); even the most extreme cross-longitude scattered-island samples (max 32.98 m, occurring at Nua Nea village, Maluku Tengah Regency, Maluku Province) differ by only tens of meters — far smaller than the spatial granularity of a village-level area, with no effect on nearest-neighbor admin2 assignment. Balancing accuracy, performance, and implementation simplicity, Indonesia computes centroids directly with a single Albers projection.

---

## Time Zone Handling

Indonesia spans three time zones, resolved via a per-province table over the 38 provinces:

| Time zone | IANA | UTC offset | Provinces | Provinces (BIG WADMPR original) |
|---|---|---|---:|---|
| WIB (Waktu Indonesia Barat) | `Asia/Jakarta` | UTC+7 | 18 | Aceh, Sumatera Utara, Sumatera Barat, Riau, Kepulauan Riau, Jambi, Sumatera Selatan, Kepulauan Bangka Belitung, Bengkulu, Lampung, DKI Jakarta, Banten, Jawa Barat, Jawa Tengah, Daerah Istimewa Yogyakarta, Jawa Timur, Kalimantan Barat, Kalimantan Tengah |
| WITA (Waktu Indonesia Tengah) | `Asia/Makassar` | UTC+8 | 12 | Kalimantan Selatan, Kalimantan Timur, Kalimantan Utara, Bali, Nusa Tenggara Barat, Nusa Tenggara Timur, Sulawesi Utara, Sulawesi Tengah, Sulawesi Selatan, Sulawesi Tenggara, Gorontalo, Sulawesi Barat |
| WIT (Waktu Indonesia Timur) | `Asia/Jayapura` | UTC+9 | 8 | Maluku, Maluku Utara, Papua, Papua Barat, Papua Selatan, Papua Tengah, Papua Pegunungan, Papua Barat Daya |

Time zones are resolved during the transform stage (cities500 schema). The table provides keys for both the "BIG WADMPR original" and the "Traditional Chinese translation" and tries both: the authoritative input list is the WADMPR original spelling, so time-zone assignment is anchored to it and does not drift with translation; the transform stage receives the Traditional Chinese name, so a Chinese key is added too. Even if the Chinese key temporarily misses due to Simplified/Traditional differences, the WADMPR original key and the default-fallback time zone keep the flow uninterrupted.

---

## Extraction Workflow

```bash
# 1. Download the desa layer from the BIG official REST service (geometryPrecision=6, version TASWIL20230928)
# 2. Feed GeoJSON / Shapefile into extract
cargo run --release -- extract --country ID \
  --shapefile path/to/idn_desa.geojson \
  --output meta_data/id_geodata.csv
```

---

## Notes

- Indonesian Admin 1 / Admin 2 use Wikidata Traditional Chinese translations; both levels must pass P131 parent validation (Admin 1 against Indonesia `Q252`, Admin 2 against its Province QID). On validation failure or when no reliable Chinese result exists, the name falls back to BIG official Indonesian.
- Indonesian Admin 3 (district) / Admin 4 (village) keep BIG official Indonesian to avoid mismatches or unstable translations for the large number of low-level place names on Wikidata.
- The original BIG vector boundaries are out of scope for this project's distribution; only the reverse-geocoding-optimized derivative metadata is distributed.
- All coordinate decisions and the hit-rate and search-language experiments use a fixed random seed of `seed=42`, making the results reproducible.

---

**Last Updated**: 2026-06-06
