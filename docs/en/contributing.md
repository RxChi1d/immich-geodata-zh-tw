# Contributing Guide

[繁體中文](../../CONTRIBUTING.md) | English

Bug reports and pull requests are welcome. This guide covers the workflow and conventions you need to know.

## Reporting Issues

Open a new issue under [Issues](https://github.com/RxChi1d/immich-geodata-zh-tw/issues). For installation or update problems, include your Immich version, deployment method, and the relevant logs. For place-name errors, include the coordinates, the name currently shown, and the expected name.

Report security issues privately as described in the [Security Policy](security.md). Do not open a public issue for them.

## Development Environment

The data pipeline is written in Rust. For environment setup, data extraction, and release builds, see [Local Data Processing](development.md).

Before submitting, make sure these checks pass:

```bash
cargo fmt --check
cargo clippy -- -D warnings
cargo test
```

## Branches and Commits

- **Branch names**: Follow Conventional Branch Naming, such as `feat/add-vietnam-handler` or `fix/install-path`.
- **Commit messages**: Follow [Conventional Commits](https://www.conventionalcommits.org/), with a 50–72 character first line. The description may be in Traditional Chinese, Simplified Chinese, or English.
- **Pull requests**: Use the Conventional Commits format for the title as well, and fill in the [PR template](../../.github/pull_request_template.md).

PR titles drive automatic categorization and release notes. CI blocks the merge when a title does not follow Conventional Commits. Keep the first line within 72 characters where possible.

## Writing Conventions

- **Code**: Write comments and doc comments in Traditional Chinese (Taiwan usage); use English for function and variable names.
- **Documentation**: The Traditional Chinese (Taiwan usage) version lives in `docs/zh-tw/`, and the English version in `docs/en/`. Follow the Google writing style: simple, concise, easy to understand.
- **Communication**: Commit messages, PRs, and issues may be in Traditional Chinese, Simplified Chinese, or English.
- If a change affects how the project is used (installation steps, options, how data appears), update the corresponding documentation in the same PR.

## CHANGELOG

Maintainers compile `CHANGELOG.md` at release time, so **do not edit it in a PR**. Release notes are generated from it, and its entries are written from the user's perspective, which requires weighing changes across PRs.

Make sure your PR title describes the change accurately, since it is the basis for that compilation.

## Testing Requirements

New features and bug fixes need `cargo test` coverage for at least the normal case, boundary conditions, and failure cases. Put tests in `tests/` and use fixtures for test data.

## Data File Protection Rules

`meta_data/*_geodata.csv` holds production data generated from official boundary data and verified by the release workflow. It is not a regenerable build artifact. Do not delete, regenerate, or normalize these files outside of an explicit data update task.

When a PR changes these files, add the `data-update` label to confirm the data update is intentional. Otherwise CI blocks the merge.

## Adding Support for a Country

1. Add the country handler in `src/pipeline/extract/handlers.rs`, and update the CLI country parsing and handler routing to match.
2. Implement the data source reader, coordinate conversion, and administrative division field mapping, and emit a normalized CSV with the columns `latitude,longitude,country,admin_1..admin_4`. The handler does not produce `geoname_id`, the time zone, or `country_code` — `transform_cities_schema` fills those in later. A country spanning several time zones needs a province lookup table registered through `country_profile`.
3. If the country uses Wikidata translations, follow the P131 containment verification standard:
   - Look up the country's Wikidata QID manually, write it into a handler constant, and annotate it with the Chinese name. QIDs are never queried at runtime.
   - Set `country_qid` when building the `TranslationDataset`, and verify containment level by level: admin2 against admin1, admin1 against the country.
   - Before rollout, use WDQS to confirm that every admin1 in the country passes `(wdt:P131)+ <country QID>`, so existing correct translations do not regress.
   - Choose the search language by how well it discriminates at that administrative level on Wikidata, and confirm the choice with an actual sample (South Korea uses native Korean, Thailand uses English with a Thai fallback). Build the test set from the country's structural same-name categories plus a random sample, and fix the random seed so results are reproducible. See the experiment recorded in [Indonesia Administrative Division Processing](indonesia-admin-processing.md).
   - Wikidata translation errors usually raise nothing: selecting the wrong entity writes out a wrong name, and most safeguards live in individual country handlers rather than the shared layer. Read [Known Translation Failures on Wikidata](wikidata-translation.md) before you start.
4. Add fixtures, unit tests, and real-data verification, along with the corresponding documentation page in `docs/zh-tw/`.
