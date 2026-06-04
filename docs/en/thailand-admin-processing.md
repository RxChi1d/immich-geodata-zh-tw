# Thailand Administrative Processing Guide

This document captures the data source, administrative hierarchy, naming strategy, and coordinate/projection details for Thai geospatial data. It is the detailed companion to the Thailand optimization section in the README.

## Table of Contents

- [Data Source](#data-source)
- [Administrative Levels](#administrative-levels)
- [Naming Strategy](#naming-strategy)
  - [Tier 1: Whether to Trust the Wikidata Result (P131 Validation)](#tier-1-whether-to-trust-the-wikidata-result-p131-validation)
  - [Search Language: English Primary, Thai Fallback](#search-language-english-primary-thai-fallback)
  - [Tier 2: Language Label Priority](#tier-2-language-label-priority)
  - [Admin 3 Names](#admin-3-names)
  - [Wikidata Cache](#wikidata-cache)
- [Coordinate Strategy](#coordinate-strategy)
- [Projection Strategy](#projection-strategy)
- [Extraction Workflow](#extraction-workflow)
- [Notes](#notes)

---

## Data Source

Thai geospatial processing is built around the official **Thailand COD-AB** administrative boundaries:

- **Source**: [HDX Thailand - Subnational Administrative Boundaries](https://data.humdata.org/dataset/cod-ab-tha)
- **Provider**: Royal Thai Survey Department, published via OCHA / HDX
- **Dataset**: Thailand administrative level 0-3 boundaries (COD-AB)
- **License**: Creative Commons Attribution for Intergovernmental Organisations (CC BY-IGO)
- **Usage**: primary source for Thai administrative boundaries and names

---

## Administrative Levels

COD-AB Thailand provides the following levels:

- **Admin 1**: Province
- **Admin 2**: District
- **Admin 3**: Sub-district / Tambon

This project uses `tha_admin3` as the extract source. The output columns are:

| Output Column | Source Field | Description |
|---|---|---|
| `country` | Fixed value | `泰國` (Thailand) |
| `admin_1` | Wikidata / `adm1_name` / `adm1_name1` | Province in Traditional Chinese; falls back to official English then official Thai when no Chinese is available |
| `admin_2` | Wikidata / `adm2_name` / `adm2_name1` | District in Traditional Chinese; falls back to official English then official Thai when no Chinese is available |
| `admin_3` | `adm3_name` / `adm3_name1` | Sub-district / Tambon official English; falls back to official Thai when English is missing |
| `admin_4` | Empty | This COD-AB dataset does not provide admin4 |

> [!NOTE]
> `tha_admin3` contains roughly 7,425 sub-districts / tambons, but the extract usually emits more rows than that. When a single tambon is made up of several disjoint polygons (a multipart boundary), each polygon gets its own centroid and is written as a separate row. The row count therefore does not equal the administrative-unit count, which is expected behavior.

---

## Naming Strategy

The Thai handler reuses the South Korea handler's Wikidata translator pipeline. Because COD-AB ships both official English and official Thai names, every translation item additionally keeps those official names as a fallback. Naming is decided in two tiers: **first decide whether to trust the Wikidata result, then decide which language label to use.**

### Tier 1: Whether to Trust the Wikidata Result (P131 Validation)

Following the Wikidata translator's standard rule, **both Admin 1 and Admin 2 must pass P131 (`located in the administrative territorial entity`) chain validation**, resolving name ambiguity level by level against the most specific known parent:

| Level | Parent QID | P131 validation | When validation fails |
|---|---|---|---|
| **Admin 1** (Province) | Thailand (`Q869`) | Every candidate must pass | Enters the Thai fallback search; if that also fails, falls back to official English / Thai |
| **Admin 2** (District) | The QID of its Province | Every candidate must pass | Enters the Thai fallback search; if that also fails, falls back to official English / Thai |

The Admin 2 parent QID comes from the Province QID resolved during Admin 1 translation:

- **No candidate passes validation** → no Wikidata candidate is adopted (there is no "take the first candidate" path); the item proceeds to the Thai fallback search or the official-name fallback.
- **Cached entries** → trusted only when the cached parent QID matches the current one and the entry was verified (or had already given up under the same context); a changed parent context (e.g., a corrected parent QID) triggers a re-query, preventing wrong translations from becoming permanently cached.

> [!NOTE]
> This validation prevents mismatches against identically or similarly named entities. For example, an English search for `Nan` returns seven unrelated entities (Nantes, Nancy, Southern Min, ...) — P131 validation rejects them all instead of blindly picking the first; likewise `Fang` in Chiang Mai Province will not be wrongly linked to an unrelated entity and rendered as "方". When the relationship cannot be confirmed, the handler conservatively falls back — preferring an English name over an incorrect Chinese one.

### Search Language: English Primary, Thai Fallback

Search runs in two passes:

1. **First pass (English)**: searches with the COD-AB official English name (`adm1_name` / `adm2_name`); each candidate is P131-validated.
2. **Second pass (Thai fallback)**: items that failed the first pass are re-searched with the official Thai name (`adm1_name1` / `adm2_name1`), with instance-of class filtering (Admin 1 restricted to province of Thailand `Q50198`; Admin 2 restricted to amphoe `Q475061` and khet of Bangkok `Q15634531`); a candidate is adopted only after passing P131 validation.

> [!IMPORTANT]
> "English primary" is an experimentally validated choice, not a default habit. A controlled experiment over 125 verified districts showed the correct entity appears in the top-7 search results 100% of the time with English search, versus only 4% (75 Mueang capital districts) to 12% (50 random districts) with Thai search — the English labels of Thai districts (the `X District` form) are far more discriminative on Wikidata than bare Thai names. Conversely, Thai search at the province level is highly discriminative (5/5 ambiguous province names hit the correct entity as the first result), which makes Thai well suited as a verified fallback rather than the primary language. When adding a new country, run the same sampling experiment before choosing the search language; see the new-country section in `CLAUDE.md`.

### Tier 2: Language Label Priority

Once an item is set to adopt the Wikidata result (Admin 2 must first pass Tier 1 validation), the name is chosen in this order:

1. Wikidata `zh-tw` label
2. Wikidata `zh-hant` label
3. Wikidata `zh` label, converted to Traditional Chinese via OpenCC
4. `zhwiki` title conversion
5. COD-AB official English field: `adm1_name` or `adm2_name`
6. COD-AB official Thai field: `adm1_name1` or `adm2_name1`

This order intentionally excludes Wikidata English and Thai labels from the fallback chain. Because COD-AB already supplies official English and Thai, when no Chinese is available the handler returns to the official source rather than using a potentially inconsistent English or Thai alias from Wikidata.

### Admin 3 Names

Admin 3 currently builds no Wikidata cache, primarily because `tha_admin3` has a large number of sub-districts / tambons where translation cost and ambiguity risk are high. Admin 3 uses official English `adm3_name` first, falling back to official Thai `adm3_name1` only when English is missing.

### Wikidata Cache

The Wikidata cache lives at:

```text
geoname_data/TH_wikidata_cache.json
```

Fixture tests use `TH_wikidata_stub.json` to avoid depending on live network queries.

---

## Coordinate Strategy

Thailand COD-AB exposes official representative points through the `tha_adminpoints` layer (mirrored by the polygon attributes `center_lat` / `center_lon`, which agree with each other). However, this project **does not use the official representative point as the default coordinate.**

The reason is that Immich's reverse geocoding uses a single-point nearest-distance model. Sampling points inside each `tha_admin3` polygon as ground-truth GPS and comparing the official representative point against the geometric centroid, the geometric centroid achieves a higher overall hit rate:

| Sampling Method | Official Representative Point | Geometric Centroid |
|---|---:|---:|
| 20 points per area + representative point, 155,925 points total | 74.30% | 76.18% |
| Area-weighted ~200k points, 200,127 points total | 70.07% | 71.84% |

The Thai handler therefore defaults to the polygon geometric centroid, keeping the single cities500 point closer to Immich's nearest-distance query model.

---

## Projection Strategy

The Thai handler computes centroids using a Thailand Albers projection:

```text
+proj=aea +lat_1=5 +lat_2=21 +lat_0=13 +lon_0=101 +x_0=0 +y_0=0 +datum=WGS84 +units=m +no_defs
```

This does not use the dynamic UTM flow from the Japan and South Korea handlers. Testing against Thailand's `tha_admin3` showed dynamic UTM yields no meaningful accuracy gain:

| Sampling Method | Thailand Albers | Dynamic UTM |
|---|---:|---:|
| 20 points per area + representative point, 155,925 points total | 76.1834% | 76.1821% |
| Area-weighted ~200k points, 200,127 points total | 71.8379% | 71.8359% |

The two centroids differ by a median of ~0.064 m, a 95th percentile of ~0.425 m, and a maximum of ~4.941 m. Balancing accuracy, performance, and implementation simplicity, Thailand computes centroids directly with Thailand Albers.

---

## Extraction Workflow

```bash
# 1. Download tha_admin_boundaries.shp.zip from HDX
# 2. Unzip and use tha_admin3.shp
cargo run --release --manifest-path rust/Cargo.toml -- extract --country TH \
  --shapefile path/to/tha_admin3.shp \
  --output meta_data/th_geodata.csv
```

GeoJSON input is also supported (`--shapefile` accepts both `.shp` and `.geojson` / `.json`):

```bash
cargo run --release --manifest-path rust/Cargo.toml -- extract --country TH \
  --shapefile path/to/tha_admin3.geojson \
  --output meta_data/th_geodata.csv
```

---

## Notes

- `center_lat` / `center_lon` (official representative points) are kept as source reference only and are not used as the default coordinate.
- If a representative-point mode is needed in the future, add an explicit coordinate-strategy option rather than overriding the current nearest-distance optimization.
- Thai Admin 1 / Admin 2 use Wikidata Traditional Chinese translations; both levels must pass P131 parent validation (Admin 1 against Thailand `Q869`, Admin 2 against its Province QID). On validation failure the Thai fallback search is attempted first; if that also fails or no reliable Chinese result exists, the name falls back to COD-AB official English and official Thai.
- Thai Admin 3 currently keeps COD-AB official English to avoid mismatches or unstable translations for the large number of low-level place names on Wikidata.

---

**Last Updated**: 2026-06-03
