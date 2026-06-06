# Global (Non-Handler Region) Translation Processing

> This document explains how the project translates place names for
> "non-handler countries" (every region except Taiwan, Japan, South Korea,
> and Thailand). It is the detailed version of the "Other regions" section
> in the README.

## Background: Two Translation Paths

Place-name translation is split into two independent paths based on the data
source:

- **Handler countries (TW / JP / KR / TH)**: Built around official
  boundary handlers and the Wikidata translator (name search → zh / zh-tw /
  zh-hant labels → P131 administrative-parent chain validation for
  disambiguation → OpenCC simplified-to-traditional and Taiwan-usage
  conversion). This path is out of scope for this document.
- **Global non-handler regions (translate stage)**: The production release
  only runs the four handler countries above; the **LocationIQ flow is not
  part of the production release**. As a result, the cities500 and admin1
  records for every other country derive their Chinese names solely from the
  zh-family language rows in GeoNames alternateNames and the embedded
  Chinese names in cities500, converted with OpenCC. **Wikidata is not
  involved at all.**

### The Actual Baseline for Non-Handler Regions

Because of the above, translation coverage for non-handler regions is far
lower than intuition suggests. The full (non-sampled) measurements taken in
2026-06 are:

| Level | Records | Currently has Chinese name (baseline) |
|---|---:|---:|
| cities500 (non-handler, global) | 229,760 | 46,290 (20.1%) |
| admin1 (non-handler, global) | 3,720 | 2,211 (59.4%) |

This means the vast majority of global city records fall back to English
without an additional translation source. That is exactly why this project
introduces the NAER official translations as a reinforcement layer. See the
full research and measurements in
[Evaluation of Alternative Chinese Place-Name Translation Sources](../research/chinese-translation-sources.md)
(in Traditional Chinese).

## Translation Priority

For non-handler regions, the translate stage decides the translation source
by the priority below. NAER's insertion point depends on its **confidence
tier**: high confidence may overwrite existing translations, medium
confidence only fills gaps when no other source yields a result, and low
confidence is not used at all.

### cities500 (City Level)

1. **NAER official translation (high confidence)**: inserted at the head of
   the priority chain as an overwrite layer
2. meta_data metadata (existing; the production `meta_data/` only contains
   handler-country files, so this is a no-op for non-handler records)
3. GeoNames alternateNames zh-family + OpenCC (existing)
4. Embedded Chinese alternatenames (existing)
5. **NAER official translation (medium confidence)**: fills gaps only when
   steps 2–4 all yield nothing
6. Fallback to the original name (existing)

### admin1 (First-Level Administrative Division)

The first version takes a conservative approach: NAER **only fills gaps and
never overwrites**:

1. GeoNames alternateNames (existing; priority unchanged)
2. **NAER lookup**: used only when no existing source provides a Chinese name
3. Fallback (existing)

### Confidence Tiers

After a NAER hit, the confidence tier is determined by the table below
(transcribed from design spec section 4.3):

| Tier | Conditions (all must hold) | Permission |
|---|---|---|
| High | Matching country code, distance ≤ 15 km, `feature_hint=false`, no ambiguity (unique translation within tolerance, or nearest vs. second-nearest differ by ≥ 5 km) | Overwrite existing translation + fill gaps |
| Medium | A hit with any weakening signal: empty country code, `feature_hint=true`, near-distance ambiguity | Fill gaps only (used only when no existing source has a Chinese name) |
| Low | Mismatching country code, distance > 15 km, malformed input | Reject |

- For the first version, admin1 is always capped at "medium" (fill gaps
  only); overwrite will be reconsidered once the quality report quantifies
  the mismatch rate.
- Rationale for the tolerance constant `NAER_CITY_DISTANCE_KM = 15.0`: NAER
  coordinate precision is ±1 arc-minute (about 2 km), plus a buffer for
  city-centroid offset; in measurements the U.S. mismatch rate approaches
  zero under this tolerance.

## NAER Data Source and License

- **Source**: National Academy for Educational Research (NAER),
  *Translations of Foreign Place Names*, Open Government Data Platform
  [dataset 15211](https://data.gov.tw/dataset/15211)
- **License**: Open Government Data License, Version 1.0 (OGDL 1.0),
  compatible with CC BY 4.0; distributions must retain attribution (see
  [NOTICE.md](../../NOTICE.md))
- **Data scale**: 64,487 records covering 700 countries/regions
- **Vendored file**: the cleaned 6-column CSV is stored at
  `naer/naer_place_names.csv`; field descriptions are in
  [`naer/README.md`](../../naer/README.md)

## Why NAER

For non-handler regions (the majority of release records), NAER is the
reinforcement source with the largest benefit. Relative to the GeoNames
alternateNames baseline, NAER provides:

- **cities500**: +16,380 gap fills (+7.1 pp, +35% relative), raising
  coverage from 20.1% to 27.3%
- **admin1**: +494 gap fills (+13.3 pp), raising coverage from 59.4% to
  72.7%; recoveries include high-visibility entries such as
  `Dubai → 杜拜` and `Andorra la Vella → 老安道爾`
- **Quality-overwrite potential**: another 9,415 cities500 records already
  have a Chinese name (mostly from `zh` simplified rows mechanically
  converted by OpenCC) for which NAER also has an official Taiwan
  translation, available as a quality overwrite

Reasons for choosing NAER over the other candidate sources:

- **GeoNames `zh-Hant` is insufficient as a traditional-Chinese gap-fill
  source**: within global cities500 there are only 1,338 `zh-Hant` rows and
  283 `zh-TW` rows; even the best-filled country (the U.S.) reaches only
  3.3%.
- **OSM `name:zh-Hant` excluded for licensing**: the ODbL share-alike
  clause would make bulk-extracted translations a Derivative Database,
  forcing the same license on distribution, which conflicts with this
  project's distribution model; furthermore, its `name:zh-Hant` fill rate
  for foreign place names is sparse.
- **Unicode CLDR excluded for scope**: it only covers country/region-level
  display names with no city-level gazetteer.
- **NAER Terminology Net (樂詞網) excluded for licensing**: all rights
  reserved, not open.

NAER's core advantage is that it is an official, peer-reviewed translation
standard (vetted by the former National Institute for Compilation and
Translation across more than 220 review meetings), with quality superior to
GeoNames' mechanical simplified-to-traditional conversion: suspected
simplified characters account for only 0.03%, and the Chinese-name column
has no empty values. See the full evaluation in the
[research document](../research/chinese-translation-sources.md) (in
Traditional Chinese).

## Why Runtime Join Instead of a Precomputed Crosswalk

NAER translations are joined at the translate stage via a **runtime dynamic
join** (name normalization + coordinate disambiguation), rather than a
precomputed NAER ↔ GeoNames geonameid crosswalk file. Reasons:

- Under this project's nightly auto-update, GeoNames cities500 can change
  daily; a precomputed crosswalk would continuously go stale and require an
  extra synchronization mechanism.
- A runtime join re-matches against the current cities500 on every release,
  automatically adapting to newly added or changed cities.
- The vendored `naer_place_names.csv` is zero-coupled to GeoNames (it
  contains no GeoNames data), is reviewable via git diff, and requires no
  GeoNames-version dependency when updated.

## Why admin1 Only Fills Gaps

admin1 is a high-visibility level (every photo's administrative-region
display relies on it), and the project has **not yet quantified** NAER's
overwrite mismatch rate at admin1. Enabling overwrite without a measured
mismatch rate carries more risk than benefit: if NAER overwrites a correct
GeoNames translation and causes a regression, the impact is broad and hard
to notice.

In addition, admin1 coordinate disambiguation can only approximate a
centroid from the average coordinates of the cities500 cities under that
admin1 (`admin1CodesASCII.txt` itself has no coordinates), and this
approximation is affected by uneven city distribution (islands, crossing
the date line). In gap-fill mode, even a failed disambiguation merely skips
the fill and never corrupts an existing translation, so the mismatch cost is
limited.

For these reasons the first version always fills gaps only and never
overwrites; high-confidence overwrite will be reconsidered after the first
quality report quantifies the mismatch rate.

## Vendored File Update Flow

Downloading and cleaning the raw NAER data is an **offline path** that is
not on the release path; it is run manually only when the data source is
updated:

```bash
# 1. Manually download the latest CSV for dataset 15211 from
#    opendata.naer.edu.tw
# 2. Run the naer-prepare subcommand to clean and emit the vendored file
cargo run --release -- naer-prepare \
  --input <path-to-raw-CSV> \
  --output naer/naer_place_names.csv

# 3. Review the statistics report and the git diff, then commit once no
#    anomalies are found
```

`naer-prepare` performs the following preprocessing:

- **Coordinate parsing chain**: HTML entity decoding → tag removal →
  apostrophe unification → degree-minute to decimal → range validation
  (|lat| ≤ 90, |lon| ≤ 180). Measured success rate is about 99.4%.
- **Name normalization** (`name_norm`, the match key): strip `[...]` /
  `(...)` annotations, take the segment before the comma, NFKD diacritic
  folding, lowercase.
- **Chinese-name cleanup** (`name_zh`): strip parenthetical annotations
  (`科魯涅(科倫納)` → `科魯涅`).
- **Country-code mapping** (`country_code`): mapped to ISO 3166-1 alpha-2
  via the i18n-iso-countries zh-tw table plus an alias table (e.g.
  `韓國→KR`, `剛果{金夏沙}→CD`); unmapped values are left empty (downgraded
  to medium confidence per the tiers, gap-fill only).
- **Natural-feature heuristic** (`feature_hint`): set to `true` when the
  English name contains a feature marker (`R.`, `Bay`, `Mt.`, `Cape`,
  `Island`, etc.) or the Chinese translation ends with a landform suffix
  (river / bay / island / mountain / lake / cape / strait, etc.). It is a
  down-weighting signal only and never drops a row, to avoid mistakenly
  killing suffix-collision cities such as `San Francisco → 舊金山`.

Failed-row handling: rows with unparseable coordinates are **dropped** (a
row that cannot take part in coordinate disambiguation cannot be used
safely); rows with an unmapped country are **kept** (country_code left
empty).

## Reading the Quality Report

The translate stage emits a single-line NAER statistics log (one of the
acceptance gates), rendered as space-separated `key=value` pairs for easy
grep/awk parsing. The full set of fields:

**Adoption counts**

- `city_fill`: cities with no existing Chinese name, filled by NAER.
- `city_override`: cities with an existing name, overwritten by a
  high-confidence NAER match.
- `city_demoted_kept_existing`: cities with an existing name where the NAER
  match was medium-confidence, so the existing name was kept.
- `admin1_fill`: admin1 units with no existing Chinese name, filled by NAER.

**Rejection counts (categorized by reason)**

- `city_rejected_distance`: candidates existed but all exceeded the 15 km
  tolerance.
- `city_rejected_country`: candidates existed but none matched the country
  code, and no empty-country candidate was available to demote to.
- `admin1_rejected_no_centroid`: candidates existed but the admin1 has no
  centroid (no member cities) to validate against.
- `admin1_rejected_distance`: candidates existed but centroid validation
  exceeded the 300 km threshold for all of them.
- `admin1_rejected_ambiguous`: distance passed, but a near-distance
  candidate carried a distinct translation, so the centroid could not
  disambiguate.

> Note: handler-country skips and "name with no candidate at all" are
> normal and are not counted as rejections.

**Distance-distribution summary (disambiguation distance of adopted
matches, in km)**

- city: `city_dist_0_1km` ([0,1)), `city_dist_1_5km` ([1,5)),
  `city_dist_5_15km` ([5,15]).
- admin1: `admin1_dist_0_1km` ([0,1)), `admin1_dist_1_5km` ([1,5)),
  `admin1_dist_5km_plus` (>=5; admin1 centroids are approximations with a
  wider tolerance, so anything beyond 5 km falls into this bucket to share
  the same summary structure).

During acceptance, compare against the expected order of magnitude from
measurements:

- cities gap fills ≈ 16,380
- cities overwrites, upper bound ≈ 9,415 (the actual value after
  confidence-tier downgrades is lower; the first quality report establishes
  the baseline)
- admin1 gap fills ≤ 494 (name-matching estimate; lower after country-code
  and centroid validation)

If the actual numbers deviate from the orders of magnitude above (for
example, gap fills far below expectation or overwrites exploding), that is a
quality warning. Inspect the vendored file, normalization logic, or
confidence-tier conditions for anomalies instead of passing it through.
