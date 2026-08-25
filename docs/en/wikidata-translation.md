# Known Translation Failures on Wikidata

The South Korea, Thailand, and Indonesia handlers source Traditional Chinese names for
administrative divisions from Wikidata. This page records the failure patterns seen on
real data, how to spot each one, and how far the current safeguards actually reach. Read
it before adding a country that uses Wikidata, or before refreshing an existing one.

## Failures Are Silent

When an entity is not found, the wrong entity is selected, or containment verification
fails, the handler falls back to the source name without raising an error. A wrong
translation raises no error either — it is a valid Chinese string that simply points at
the wrong place.

So when verifying data, **compare the full list of untranslated names, not just the
count**. The total stays the same when one name drops out while a different name starts
translating for the first time. This happened in practice: moving Indonesia to newer
boundary data kept the untranslated `admin_2` count at 51, and only a name-by-name
comparison revealed that `Kabupaten Ngawi` had dropped out while another name took its
place.

## Six Patterns

These are observations from August 2026. Several cases were reported and fixed upstream,
so following the "How to spot it" column may no longer reproduce them.

| Pattern | Example | How to spot it |
| :--- | :--- | :--- |
| Search resolves to the wrong entity (a lower division, a facility, or an agency) | Seoul's `관악구` matched `신림동`, a neighborhood inside it; `송파구` matched the subway station `잠실역`; `전남광주통합특별시` matched a same-named education office, which in turn pointed the parent QID of 27 lower divisions at the wrong entity | The candidate's native label is not equal to the queried name |
| The upstream label predates a reorganization | The Chinese label for `여주시` was 「驪州郡」, its pre-2013 name; `검단구` was stuck on 「黔丹面」, its name before the district was created | The administrative suffix disagrees with the source name (시→市, 군→郡, 구→區) |
| Simplified-to-traditional conversion picks the wrong character | The `zh-tw` label for `함평군` converted 「咸」 into 「鹹」; the `zh-hant` label for Indonesia's Papua still contained the simplified 「巴布亚」 | Manual sampling, or a scan against the simplified-character allowlist |
| The `P131` statement is missing | `영종구` had none at all for a while after the district was created | Containment verification fails and no statement exists |
| The `P131` statement is marked deprecated | Indonesia's `Kabupaten Ngawi` had its containment in East Java marked deprecated | The SPARQL `wdt:` prefix only matches best-rank statements, so a deprecated one is equivalent to a missing one |
| A filter in this project rejects the correct candidate | The keyword blocklist inspected **every language** label of a candidate, so the correct `관악구` was dropped because a bot had filled its `zh-hant` label with 「冠嶽區廳」, which contains 「廳」 | The correct entity is absent from the candidate set |

The last pattern is not an upstream problem but a safeguard backfiring. A blocklist can
never be complete, and it can reject a correct candidate over a label in an unrelated
language. An allowlist-style rule — the native label must equal the queried name — drops
agencies, officeholder positions, electoral districts, and stations on its own.

## Current Safeguards and Their Reach

Most safeguards live in individual country handlers rather than the shared layer. Do not
assume a new country is already protected.

| Safeguard | Where it lives | Reach |
| :--- | :--- | :--- |
| Candidate filtering | Each country supplies its own `candidate_filter` | For KR, the Korean label must equal the queried name (applied to both admin1 and admin2). TH and ID each have their own exclusion rules |
| A missing parent QID is an error | `korea_admin2_parent_qids` | **KR only.** The shared `resolve_parent_qid` still falls back to the country QID when no parent is found, and TH and ID skip missing entries when building their parent tables |
| Administrative-suffix consistency | `korea_admin2_level_matches` | **KR admin2 only.** A mismatch falls back to the Korean source name, which then appears in the untranslated list in the output CSV |
| Simplified-character detection | `WikidataTranslator::warn_if_simplified` | Shared, but it **only warns and never rewrites** — converting automatically in the shared layer would affect output that is already correct |
| Simplified-character conversion | `indonesia_normalize::fix_simplified_chars` | **ID only.** It uses the allowlist in `src/wikidata/simplified.rs`, which covers only characters that exist solely in simplified Chinese and map to a single traditional form |

Why not run a full OpenCC s2t pass: measured against real Indonesian output, a full pass
broke 24 correct translations through over-conversion (`里→裏`, `占→佔`, and similar) and
fixed only one. The allowlist trades coverage for never introducing a new error.

## Fix Upstream First

When a translation is wrong because of an error in Wikidata itself, **fix Wikidata rather
than adding a mapping table here**. Fixing upstream helps everyone; a mapping table helps
only this project and has to be maintained indefinitely.

The South Korea handler is the worked example: seven manual mappings were planned, and
the count dropped to zero once admin2 names were sourced from the Chinese characters in
the Korean Wikipedia article. Korean division names are Chinese-character words to begin
with, so the characters are the original name rather than a translation — the same
principle by which the Japan handler keeps the Japanese kanji from its boundary data.

## Caches

`geoname_data/{CC}_wikidata_cache.json` has several layers. `cache.search`,
`cache.labels`, `cache.instance_of`, and `cache.p131` hold the results of each lookup
stage, and `translations` holds the final translation decisions, keyed as
`<level>/<country>/<parent divisions…>/<source name>`.

When `translations` has an entry and the parent context is unchanged, that entry is used
directly and `cache.p131` is never consulted. **Clearing only `cache.p131` therefore has
no effect.**

- To re-verify one entity after fixing it upstream, clear both its `cache.p131` entry and
  its `translations` entry.
- After changing candidate-filtering logic, or after editing upstream labels, rerun from
  an empty cache. Filtering also reads `cache.search` and `cache.labels`, so selective
  clearing is easy to get wrong.
