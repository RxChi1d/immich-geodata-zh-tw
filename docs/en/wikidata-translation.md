# Known Translation Failures on Wikidata

The South Korea, Thailand, and Indonesia handlers resolve administrative divisions to
Wikidata entities and source Traditional Chinese names from them. How far each handler
relies on Wikidata differs: TH and ID use the Chinese labels directly, while KR takes
admin1 names from a built-in table in the handler and admin2 names from the Chinese
characters in the Korean Wikipedia article, leaving Wikidata to identify the entity and
verify containment.

This page records the failure patterns seen on real data, how to spot each one, and how
far the current safeguards actually reach. Read it before adding a country that uses
Wikidata, or before refreshing an existing one.

## Failures Are Silent

Most problems do not interrupt the run. They flow quietly into the output, and the two
outcomes look nothing alike:

- **No entity is found, or no candidate passes containment verification**: the handler
  falls back to the source name, and the division stays untranslated in the output. This
  is the easier case, because it shows up in the untranslated list.
- **The wrong entity is selected**: as long as that entity passes P131 verification, the
  name is taken from that entity (possibly after simplified-to-traditional conversion, or
  from its Chinese Wikipedia article title). This is the hard case, because the output is
  a valid Chinese string that simply points at the wrong place. Seoul's `관악구` was once
  written out as 「新林洞」, which never appeared in any untranslated list.

The one thing that does stop the run is in the KR handler: when an admin1 cannot be
resolved to a QID it raises an error rather than letting lower divisions fall back to
verification against the country QID. No other country has that guard.

So when verifying data, **compare the full list of untranslated names, not just the
count**. The total stays the same when one name drops out while a different name starts
translating for the first time. This happened in practice: moving Indonesia to newer
boundary data kept the untranslated `admin_2` count at 51, and only a name-by-name
comparison revealed that `Kabupaten Ngawi` had dropped out while another name took its
place.

When the untranslated count goes up, find out why name by name. Do not assume upstream
added divisions — a new division upstream and a translation that stopped working look
exactly the same in the output.

## Six Patterns

These are observations from August 2026. Several cases were reported and fixed upstream,
so following the "How to spot it" column may no longer reproduce them.

| Pattern | Example | How to spot it |
| :--- | :--- | :--- |
| Search resolves to the wrong entity (a lower division, a facility, or an agency) | Seoul's `관악구` matched `신림동`, a neighborhood inside it; `송파구` matched the subway station `잠실역`; `전남광주통합특별시` matched a same-named education office, which in turn pointed the parent QID of 27 lower divisions at the wrong entity | The candidate's native label is not equal to the queried name |
| The upstream label predates a reorganization | The Chinese label for `여주시` was 「驪州郡」, its pre-2013 name; `검단구` was stuck on 「黔丹面」, its name before the district was created | The administrative suffix disagrees with the source name (시→市, 군→郡, 구→區) |
| A label declared as Traditional has the wrong glyphs | Two sub-cases: conversion picked the wrong character (the `zh-tw` label for `함평군` turned 「咸」 into 「鹹」), or no conversion happened at all (the `zh-hant` label for Indonesia's Papua still contained the simplified 「巴布亚」) | Manual sampling, or a scan against the simplified-character allowlist |
| The `P131` statement is missing | `영종구` had none at all for a while after the district was created | Containment verification fails and no statement exists |
| The `P131` statement is marked deprecated | Indonesia's `Kabupaten Ngawi` had its containment in East Java marked deprecated | The SPARQL `wdt:` prefix only matches best-rank statements, so a deprecated one is equivalent to a missing one |
| A filter in this project rejects the correct candidate | The keyword blocklist inspected each label fetched for a candidate, so the correct `관악구` was dropped because a bot had filled its `zh-hant` label with 「冠嶽區廳」, which contains 「廳」 | The correct entity is absent from the candidate set |

The last pattern is not an upstream problem but a safeguard backfiring. A blocklist can
never be complete, and it can reject a correct candidate over a label in an unrelated
language. KR now uses an allowlist-style rule instead — the native label must equal the
queried name — which drops agencies, officeholder positions, electoral districts, and
stations on its own.

> [!WARNING]
> **The same mechanism is still in place on the ID path.** Candidate filtering in
> `indonesia_wikidata.rs` walks every label in `metadata.labels` looking for excluded
> keywords, exactly as the filter that rejected `관악구` did. What that map holds depends
> on `target_lang` and `fallback_langs` — `zh-tw`, `zh-hant`, and `zh` for ID — and
> `parse_entity_labels` also puts the `zhwiki` and `kowiki` article titles into the same
> map, so article titles are scanned too. The label that rejected `관악구` was its
> `zh-hant` one, which falls squarely inside that range. No false rejection has been
> observed for ID so far, but it is a known risk to keep in mind when working on
> Indonesian translations.

## Current Safeguards and Their Reach

Most safeguards live in individual country handlers rather than the shared layer. Do not
assume a new country is already protected.

| Safeguard | Where it lives | Reach |
| :--- | :--- | :--- |
| Candidate filtering | Each country supplies its own `candidate_filter` | All three differ: KR requires the Korean label to equal the queried name, on both admin1 and admin2. ID applies an instance-of class allowlist from the first pass, plus a keyword blocklist matched against the Chinese labels it fetched and the article titles. TH does **no filtering at all** on its English first pass, and applies an instance-of allowlist only on the Thai retry that follows a failure |
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

The South Korea handler is the worked example: seven manual mappings were planned to work
around upstream errors, and that count dropped to zero once admin2 names were sourced from
the Chinese characters in the Korean Wikipedia article. Korean division names are
Chinese-character words to begin with, so the characters are the original name rather than
a translation — the same principle by which the Japan handler keeps the Japanese kanji
from its boundary data.

(The KR handler still holds two fixed tables unrelated to upstream errors: the built-in
admin1 names, and the admin2 names for Sejong Special Self-Governing City, whose lower
divisions bypass Wikidata entirely.)

## Caches

`geoname_data/{CC}_wikidata_cache.json` has several layers. `cache.search`,
`cache.labels`, `cache.instance_of`, and `cache.p131` hold the results of each lookup
stage, and `translations` holds the final translation decisions, keyed as
`<level>/<country>/<parent divisions…>/<source name>`.

An entry in `translations` is used directly only when two conditions hold: the parent QID
computed for this run matches the one recorded in the cache, and the entry either has
`qid = null` (translation failed) or `parent_verified = true`. When both hold,
`cache.p131` is never consulted, so **clearing only `cache.p131` does nothing for those
entries** — which in practice is nearly all of them.

- To re-verify one entity after fixing it upstream, clear both its `cache.p131` entry and
  its `translations` entry. Keys in `cache.p131` are `<entity QID>_<parent QID>` pairs;
  an entry whose translation failed carries no QID in `translations`, so read the
  candidate QIDs from that item's `cache.search` entry to find the right key.
- After changing candidate-filtering logic, or after editing upstream labels, rerun from
  an empty cache. Filtering also reads `cache.search` and `cache.labels`, so selective
  clearing is easy to get wrong.
