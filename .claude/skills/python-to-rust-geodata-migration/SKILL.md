---
name: python-to-rust-geodata-migration
description: Use when migrating immich-geodata-zh-tw from Python to Rust with stage-by-stage golden-output parity, Codex goals, Claude Code skills, and Rust compiler/LSP verification.
version: 1.0.0
author: Hermes Agent
license: MIT
metadata:
  hermes:
    tags: [python, rust, migration, geodata, parity, codex-goals]
    related_skills: [language-migration-modernization, test-driven-development, systematic-debugging, subagent-driven-development]
---

# Python → Rust Geodata Migration

## Overview

本 skill 定義 `immich-geodata-zh-tw` 的 Python→Rust 漸進式遷移規則。核心策略是：

```text
Python reference implementation
+ deterministic fixtures
+ per-stage golden outputs
+ Rust target implementation
+ canonical comparator
+ stage-by-stage parity gates
```

Python 實作在遷移期間是 oracle。Rust 每次只補上一個 pipeline stage，並且必須通過該 stage 對所有 fixtures 的 parity 檢查後才可標記完成。

## When to Use

使用於下列任務：

- 研究、規劃或執行 `immich-geodata-zh-tw` 的 Python→Rust 遷移。
- 建立 Python stage output、golden files、fixtures、comparator 或 parity runner。
- 使用 `codex exec`、Codex `/goal`、Claude Code `/batch` 或 Claude Code skills 執行分階段遷移。
- 審查 Rust 實作是否忠實保留 Python geodata pipeline 行為。

不要用於：

- 沒有 parity harness 的一次性整包 rewrite。
- 為了讓 Rust 通過而修改 Python golden output 或放寬 comparator。
- 與 geodata pipeline 無關的一般功能開發。

## Non-negotiable Rules

1. **Python 是 source of truth**：遷移期間不得刪除 Python 實作。
2. **不要 blind translation**：Rust 應使用 idiomatic design，不做逐行翻譯。
3. **先產生 golden output，再寫 Rust 邏輯**。
4. **一次只遷移一個 stage**，除非使用者明確要求 batch mode。
5. **不可偷改 `golden/python/`**：除非明確發現 Python baseline 有 bug，並先記錄原因。
6. **不可弱化 `tools/compare_outputs.py` 或 `docs/migration/parity-contract.md`** 來讓 Rust 通過。
7. **Parity fail 時先找 root cause**：判斷是 Rust bug、Python instrumentation bug、fixture 不足，或 contract 未定義。
8. **每個 stage 完成前必須更新 `docs/migration/parity-matrix.md`**。
9. **所有文件、註解、docstrings 依專案規則使用 zh-tw；函式/變數命名使用英文。**
10. **不要在文件或 commit message 中加入 AI 模型/編輯器署名。**

## Required Project Artifacts

建立或維護下列檔案：

```text
docs/migration/research.md
docs/migration/pipeline-map.md
docs/migration/parity-contract.md
docs/migration/parity-matrix.md
docs/migration/risk-register.md
docs/migration/rust-architecture.md
fixtures/parity/
golden/python/
golden/rust/
tools/export_python_golden.py
tools/compare_outputs.py
tools/run_parity.py
goals/
```

## Initial Pipeline Map

先以現有 Python 架構作為 stage 邊界：

| Stage | Source of truth | Output candidate | Main parity risks |
|---|---|---|---|
| `extract` | `GeoDataHandler.extract_from_shapefile()` and country handlers | normalized geodata CSV/JSON | CRS transform, centroid, admin fields, coordinate precision, sort order |
| `transform_cities_schema` | `GeoDataHandler.convert_to_cities_schema()` | GeoNames `CITIES_SCHEMA` rows | `geoname_id`, admin1 mapping, dtype/date/null handling |
| `admin1_load` | `update_admin1_data()` | optimized `admin1CodesASCII` rows | ID ranges, replacement rules, row order |
| `cities500_load` | `merge_extra_data()` and `replace_with_handler_data()` | optimized `cities500` rows | duplicate coordinate tie-breaker, population filter, handler replacement |
| `translate` | `translate.translate_cities500()` and `translate.translate_admin1()` | translated city/admin files | Traditional Chinese names, alternate names, empty/null handling |
| `pack` | `pack_release.pack()` | release artifacts | filenames, archive content, deterministic checksums if feasible |

建議先遷移 deterministic DataFrame stages（`transform_cities_schema`、`admin1_load`、`cities500_load`），再處理 geospatial extract stack。

## Canonical Parity Contract

`docs/migration/parity-contract.md` 至少定義：

- JSON key sorting and stable CSV/TSV column order。
- row ordering：若 output schema 無自然順序，先依 stable keys 排序。
- coordinate precision：沿用 `GeoDataHandler.COORD_DECIMAL_PLACES = 8` 作為初始標準。
- float tolerance：若不能用固定小數，明確定義 tolerance，例如 `1e-8`。
- date format：固定為 ISO `YYYY-MM-DD`。
- null/empty handling：明確區分 `null`、空字串、缺欄位；不要在 comparator 裡臨時猜測。
- runtime metadata：timestamp、temporary path、log text 不可參與 parity。
- diff reporting：第一個 mismatch stage 必須輸出可讀 diff，包含 fixture、stage、row key、field、Python value、Rust value。

## Goal Execution Pattern

每個 Codex/Claude goal 都要包含：

```text
Goal: Migrate <stage-name> from Python to Rust with parity verification.

Must read first:
- AGENTS.md
- docs/agent-skills/python-to-rust-geodata-migration/SKILL.md
- docs/migration/pipeline-map.md
- docs/migration/parity-contract.md
- docs/migration/parity-matrix.md

Scope:
- Only migrate <stage-name>.
- Do not modify unrelated stages.
- Do not modify golden/python unless explicitly instructed.

Verification:
- uv run pytest
- cargo fmt --check
- cargo clippy -- -D warnings
- cargo test
- uv run python tools/run_parity.py --stage <stage-name>

Stopping condition:
- <stage-name> passes parity for all fixtures.
- docs/migration/parity-matrix.md is updated.
```

## Suggested `codex exec` Usage

將 goal 存成檔案，再執行：

```bash
codex exec --cd /path/to/immich-geodata-zh-tw "$(cat goals/03-migrate-stage-transform-cities-schema.md)"
```

如果使用 Codex CLI high-reasoning/no-approval 模式，只有在使用者明確要求時才加：

```bash
codex exec -c model_reasoning_effort="high" --dangerously-bypass-approvals-and-sandbox --cd /path/to/immich-geodata-zh-tw "$(cat goals/03-migrate-stage-transform-cities-schema.md)"
```

## Rust Target Conventions

初始建議：遷移期間先把 Rust code 放在 `rust/` 子目錄，避免打亂 Python package：

```text
rust/
  Cargo.toml
  src/
    main.rs
    lib.rs
    pipeline/
      mod.rs
      extract.rs
      transform_cities_schema.rs
      admin1_load.rs
      cities500_load.rs
      translate.rs
    models/
      mod.rs
      geodata.rs
      geonames.rs
```

常用 mapping：

| Python | Rust |
|---|---|
| `argparse` | `clap` |
| `polars` | Rust `polars` crate or explicit `serde` structs, choose per stage |
| `loguru` | `tracing` |
| exceptions | `Result<T, E>`, `thiserror`, `anyhow` |
| CSV/TSV I/O | `csv`, `serde`, or Rust `polars` |
| JSON golden output | `serde_json` with deterministic formatting |
| tests | `cargo test`, `insta`, `proptest` where useful |

## Debugging Parity Failures

遇到 parity fail 時：

1. 找出第一個 failing fixture/stage。
2. 用 comparator diff 定位 row key 與欄位。
3. 回讀 Python stage source 與 golden output。
4. 判斷 root cause：Rust logic、serialization、sorting、precision、fixture、或 contract ambiguity。
5. 只修 root cause；不要同時修多個問題。
6. 重新跑該 stage parity，再跑全體已遷移 stages parity。

三次修復仍失敗時，停止並更新 `docs/migration/risk-register.md`，不要繼續猜。

## Completion Checklist

- [ ] `docs/migration/research.md` 已記錄遷移研究與決策。
- [ ] `pipeline-map.md` 已列出所有 stages、input/output、side effects、risks。
- [ ] `parity-contract.md` 已定義 canonical comparison。
- [ ] Python stage outputs 可 deterministic regenerate。
- [ ] `golden/python/` 已凍結代表性 fixtures。
- [ ] Rust skeleton CLI 可執行單一 stage 與 full pipeline。
- [ ] 每個 stage 都在 `parity-matrix.md` 中標記 pass/fail。
- [ ] `uv run pytest` 通過。
- [ ] `cargo fmt --check`、`cargo clippy -- -D warnings`、`cargo test` 通過。
- [ ] final pipeline parity 通過。
- [ ] `docs/migration/final-report.md` 記錄完成狀態、已知差異與剩餘風險。
