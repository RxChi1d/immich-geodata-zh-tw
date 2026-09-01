# Global (Non-Handler Region) Translation Processing

> This document explains how the project translates place names for
> "non-handler countries" — every region except Taiwan, Japan, South Korea,
> Thailand, and Indonesia. It is the detailed version of the "Other regions"
> optimization section in the README.

## Background: Two Translation Paths

Place-name translation splits into two independent paths based on the data
source:

- **Handler countries (TW / JP / KR / TH / ID; the extract handlers are the
  single source of truth for this list)**: Built around official boundary
  data handlers, with the Wikidata translator (name search → zh / zh-tw /
  zh-hant label → P131 administrative-parent chain validation for
  disambiguation → OpenCC character-level simplified-to-traditional
  conversion) producing high-quality Traditional Chinese translations.
  Taiwan usage comes from Wikidata zh-tw labels and the handler's built-in
  mapping tables; OpenCC only handles character-level conversion. This path
  is out of scope for this document.
- **Global non-handler regions (translate stage)**: The production release
  runs only the handler countries above — the **LocationIQ flow is not part
  of the production release**. As a result, the cities500 and admin1 records
  for every other country originally derive their Chinese names solely from
  the zh-family language rows in GeoNames alternateNames and the embedded
  Chinese names in cities500, converted with OpenCC. **Wikidata plays no
  part at all.**

### The Actual Baseline for Non-Handler Regions

Because of the above, translation coverage for non-handler regions is far
lower than intuition suggests. Full (non-sampled) measurements from 2026-06:

| Level | Records | Currently has a Chinese name (baseline) |
|---|---:|---:|
| cities500 (non-handler, global) | 229,760 | 46,290 (20.1%) |
| admin1 (non-handler, global) | 3,720 | 2,211 (59.4%) |

This means the vast majority of global city records fall back to English
without an additional translation source, which is exactly why this project
introduces the official NAER translations as a reinforcement layer. The
table records an offline measurement and is not verified automatically by
the code; recomputing it requires re-running the prepare and translate
stages. For the full research and measurement process, see
[Evaluation of Alternative Chinese Place-Name Translation Sources](../research/chinese-translation-sources.md)
(Chinese).

## Translation Priority

For non-handler regions, the translate stage picks a translation source by
the priority below. NAER enters at a different point depending on its
**confidence tier**: high confidence may override an existing translation,
medium confidence only fills gaps when no other source yields a result, and
low confidence is not used at all.

### cities500 (City Level)

1. **NAER official translation (high confidence)**: inserted at the head of
   the priority chain as an override layer
2. LocationIQ metadata (existing; `cache/locationiq/{CC}.csv` is loaded.
   Handler countries' `meta_data/{cc}_geodata.csv` lives in a separate
   directory and is consumed by the enhance stage under its explicit
   filename, never taking part in this lookup. The production release
   currently has no non-handler country, so this layer is a no-op)
3. GeoNames alternateNames zh-family + OpenCC (existing)
4. Embedded Chinese alternatenames (existing)
5. **NAER official translation (medium confidence)**: fills gaps only when
   steps 2–4 all yield nothing
6. Fallback to the original name (existing)

### admin1 (First-Level Administrative Division)

The first version of admin1 takes a conservative approach: NAER **only fills
gaps and never overrides**:

1. GeoNames alternateNames (existing; priority unchanged)
2. **NAER lookup**: used only when no existing source provides a Chinese name
3. Fallback (existing)

admin1 matching conditions differ from the city path, and the confidence
tier table below does not apply here:

- A candidate's country code must exactly match the country prefix of the
  admin1 code; candidates with an empty country code are not accepted.
- The average coordinates of the cities500 records under the admin1 serve as
  an approximate centroid, and the distance threshold is relaxed to 300 km.
- Within tolerance, if the second-nearest candidate carries a different
  translation than the nearest one and their distances to the query point
  differ by less than 5 km, the
  gap-fill is abandoned outright.
- `feature_hint` is not read and no confidence tier is computed — this path
  only ever fills gaps, so down-weighting would make no difference to the
  result.
- Finding no country-matching candidate is normal (most admin1 units are
  absent from the NAER dictionary) and is not counted as a rejection.

### Confidence Tiers

After a NAER hit on cities500, the confidence tier is determined by the
table below (implemented in `src/pipeline/naer_lookup.rs`):

| Tier | Conditions (all must hold) | Permission |
|---|---|---|
| High | Matching country code, distance ≤ 15 km, `feature_hint=false`, no ambiguity (a unique translation within tolerance, or the nearest and second-nearest distances differ by at least 5 km) | Override an existing translation + fill gaps |
| Medium | A hit with any weakening signal: no country-matching candidate so an empty-country candidate was used instead, `feature_hint=true`, near-distance ambiguity | Fill gaps only (used only when no existing source has a Chinese name) |
| Low | Country code mismatch with no empty-country candidate to demote to, no candidate within tolerance (distance > 15 km), or malformed unparseable coordinates | Reject |

- The first version of admin1 always fills gaps only; overriding will be
  reconsidered once a quality report quantifies the mismatch rate.
- Rationale for the tolerance constant `NAER_CITY_DISTANCE_KM = 15.0`: NAER
  coordinate precision is ±1 arc-minute (about 2 km), plus a buffer for
  city-centroid offset; measurements show the U.S. mismatch rate approaching
  zero under this tolerance.

## NAER Data Source and License

- **Source**: National Academy for Educational Research (NAER),
  *Translations of Foreign Place Names*, Open Government Data Platform
  [dataset 15211](https://data.gov.tw/dataset/15211)
- **License**: Open Government Data License, Version 1.0 (OGDL 1.0),
  compatible with CC BY 4.0; distributions must retain attribution (see
  [NOTICE.md](../../NOTICE.md))
- **Data scale**: the raw dataset holds 64,487 records covering 700
  countries/regions (offline measurement; see the
  [research document](../research/chinese-translation-sources.md)
  (Chinese)); after `naer-prepare` cleanup, the vendored file holds 64,075
  records. The 412-record difference consists of rows dropped for
  unparseable coordinates, an empty coordinate column, or an unusable name
- **Vendored file**: the cleaned 6-column CSV lives at
  `naer/naer_place_names.csv`, covering 192 ISO 3166-1 alpha-2 country
  codes, plus 1,798 records whose country name could not be mapped
  (`country_code` left empty); field descriptions are in
  [`naer/README.md`](../../naer/README.md)

## Why NAER (Research Summary)

For non-handler regions — the vast majority of release records — NAER is the
reinforcement source with the largest benefit. Relative to the GeoNames
alternateNames baseline, NAER provides (2026-06 offline measurement, not
verified by the code):

- **cities500**: +16,380 gap fills (+7.1 pp, +35% relative), raising
  coverage from 20.1% to 27.3%
- **admin1**: +494 gap fills (+13.3 pp), raising coverage from 59.4% to
  72.7%; recoveries include high-visibility entries such as
  `Dubai → 杜拜` and `Andorra la Vella → 老安道爾`
- **Quality-override potential**: another 9,415 cities500 records already
  have a Chinese name (mostly from `zh` simplified rows mechanically
  converted by OpenCC) for which NAER also has an official Taiwan
  translation, available as a quality override

Reasons for choosing NAER over the other candidate sources:

- **GeoNames `zh-Hant` is insufficient as a Traditional Chinese gap-fill
  source**: within global cities500 there are only 1,338 `zh-Hant` rows and
  283 `zh-TW` rows; even the best-filled country, the U.S., reaches only
  3.3%.
- **OSM `name:zh-Hant` excluded for licensing**: the ODbL share-alike clause
  would make bulk-extracted translations a Derivative Database, forcing the
  same license on distribution, which conflicts with this project's
  distribution model; its `name:zh-Hant` fill rate for foreign place names
  is also sparse.
- **Unicode CLDR excluded for scope**: it covers only country/region-level
  display names, with no city-level gazetteer.
- **NAER Terminology Net (樂詞網) excluded for licensing**: all rights
  reserved, not open.

NAER's core advantage is that it is an official reviewed translation
standard — vetted by the former National Institute for Compilation and
Translation across more than 220 review meetings — and its quality beats
GeoNames' mechanical simplified-to-traditional conversion: suspected
simplified characters account for only 0.03%, and the Chinese-name column
has no empty values. For the full evaluation, see the
[research document](../research/chinese-translation-sources.md) (Chinese).

## Why a Runtime Join Instead of a Precomputed Crosswalk

NAER translations join in at the translate stage through a **runtime dynamic
join** (name normalization + coordinate disambiguation), rather than a
precomputed NAER ↔ GeoNames geonameid crosswalk file. Reasons:

- Under this project's nightly auto-update, GeoNames cities500 can change
  daily; a precomputed crosswalk would go stale continuously and would need
  an extra synchronization mechanism.
- A runtime join re-matches against the current cities500 on every release,
  adapting automatically to new or changed cities.
- The vendored `naer_place_names.csv` is zero-coupled to GeoNames (it
  contains no GeoNames data), is reviewable via git diff, and needs no
  GeoNames-version dependency when updated.

## Why admin1 Only Fills Gaps

admin1 is a high-visibility level — every photo's administrative division
display uses it — and this project has **not yet quantified** NAER's
override mismatch rate at admin1. Enabling overrides without a measured
mismatch rate carries more risk than benefit: if NAER overrides a correct
GeoNames translation and causes a regression, the impact is broad and hard
to notice.

Furthermore, admin1 coordinate disambiguation can only approximate a
centroid from the average coordinates of the cities500 cities under that
admin1 (`admin1CodesASCII.txt` itself carries no coordinates), and that
approximation suffers from uneven city distribution (outlying islands,
crossing the date line). In gap-fill mode, a failed disambiguation merely
abandons the fill and never damages an existing translation, so the cost of
a mismatch is limited.

The first version therefore always fills gaps and never overrides;
high-confidence override will be reconsidered after the first quality report
quantifies the mismatch rate.

## Vendored File Update Flow

Downloading and cleaning the raw NAER data is an **offline path**, not part
of the release path; run it manually only when the data source is updated:

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
- **Name normalization** (`name_norm`, the match key): strip parenthetical
  annotations (round brackets, full-width brackets, and square brackets
  including `〔〕`), take the segment before the comma, NFKD diacritic
  folding, lowercase, collapse runs of whitespace.
- **Chinese-name cleanup** (`name_zh`): strip the same set of parenthetical
  annotations (`科魯涅(科倫納)` → `科魯涅`).
- **Country-code mapping** (`country_code`): mapped to ISO 3166-1 alpha-2
  via the i18n-iso-countries zh-tw table plus an alias table (e.g.
  `韓國→KR`, `剛果{金夏沙}→CD`); unmapped values are left empty (demoted to
  medium confidence per the tiers, gap-fill only).
- **Natural-feature heuristic** (`feature_hint`): set to `true` when the
  English name contains a feature marker (`R.`, `Bay`, `Mt.`, `Cape`,
  `Island`, etc.) or the Chinese translation ends with a landform suffix
  (river / bay / island / mountain / lake / cape / strait, etc.). It only
  drives down-weighting and never drops a row: a city whose suffix collides
  with a landform, such as `San Francisco → 舊金山`, is still marked `true`
  but is merely demoted to medium confidence (gap-fill only) rather than
  disappearing from the dictionary.

Failed-row handling:

- **Dropped**: unparseable coordinates (`coordinate_failures`), an empty
  coordinate column (`coordinate_empty`), and unusable names — `name_norm`
  or `name_zh` empty after normalization, or `name_zh` containing a comma
  (both count toward `name_failures`).
- **Kept**: rows whose country name could not be mapped (`country_code` left
  empty); rows whose coordinates parsed successfully but resolve to (0,0)
  only count toward `suspicious_zero_coordinates` and are not dropped.

Output is sorted by column to ease git diff review, and rows sharing a
`(name_norm, country_code)` that sit less than 5 km apart yet carry
different translations are detected and counted in the report's `conflicts`.

## Reading the Quality Report

The translate stage emits a single-line NAER statistics log — one of the
acceptance gates — as space-separated `key=value` pairs for easy grep/awk
parsing. The full set of fields:

**Adoption counts**

- `city_fill`: cities with no existing Chinese name, filled by NAER.
- `city_override`: cities with an existing Chinese name, overridden by a
  high-confidence NAER match.
- `city_demoted_kept_existing`: cities with an existing Chinese name where
  the NAER match was medium confidence, so the existing name was kept.
- `admin1_fill`: admin1 units with no existing Chinese name, filled by NAER.

**Rejection counts (categorized by reason)**

- `city_rejected_distance`: candidates existed but all fell outside the
  15 km tolerance.
- `city_rejected_country`: candidates existed but none matched the country
  code, and no empty-country candidate was available to demote to.
- `admin1_rejected_no_centroid`: candidates existed but the admin1 has no
  centroid (no member cities) to validate against.
- `admin1_rejected_distance`: candidates existed but centroid validation
  exceeded the 300 km threshold for all of them.
- `admin1_rejected_ambiguous`: distance passed, but a near-distance
  candidate carried a different translation and the centroid could not
  disambiguate.

> Note: handler-country skips and "name with no candidate at all" are
> normal and are not counted as rejections.

**Distance-distribution summary (disambiguation distance of adopted
matches, in km)**

- city: `city_dist_0_1km` ([0,1)), `city_dist_1_5km` ([1,5)),
  `city_dist_5_15km` ([5,15]).
- admin1: `admin1_dist_0_1km` ([0,1)), `admin1_dist_1_5km` ([1,5)),
  `admin1_dist_5km_plus` (≥5; admin1 centroids are approximations with a
  wider tolerance, so anything beyond 5 km falls into this bucket to share
  the same summary structure).

For acceptance, compare against the expected order of magnitude from the
offline measurements (same source as the section above; not yet calibrated
against a quality report log — switch to actual values once the first report
exists):

- cities gap fills ≈ 16,380
- cities overrides, upper bound ≈ 9,415 (the actual value after
  confidence-tier demotion is lower; the first quality report establishes the
  baseline)
- admin1 gap fills ≤ 494 (name-matching estimate; lower after country-code
  and centroid validation)

If the actual numbers deviate from these magnitudes — gap fills far below
expectation, or overrides exploding — treat it as a quality warning and
inspect the vendored file, the normalization logic, or the confidence-tier
conditions for anomalies instead of passing the release through.
