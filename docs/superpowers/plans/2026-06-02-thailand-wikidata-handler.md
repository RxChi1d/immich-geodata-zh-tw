# Thailand Wikidata Handler Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 為泰國 extract handler 接上 Wikidata 繁中翻譯，並以官方英文、官方泰文作為最後 fallback。

**Architecture:** 沿用 KR 的 extract context 與 Wikidata cache builder 模式，新增 TH 專用 cache builder 與 stub loader。Translator 本身只新增通用的 metadata fallback，讓 TH 可控制語言優先級而不影響 KR。

**Tech Stack:** Rust CLI、GeoJSON/Shapefile extract、Wikidata translator cache、Rust integration tests。

---

### Task 1: Translator Source Fallback

**Files:**
- Modify: `rust/src/wikidata/types.rs`
- Modify: `rust/src/wikidata/translator.rs`
- Modify: `rust/src/wikidata/translator_tests.rs`

- [ ] **Step 1: Add metadata keys**

在 `types.rs` 新增 `METADATA_OFFICIAL_EN` 與 `METADATA_OFFICIAL_TH`，供 TH dataset item 記錄官方來源名稱。

- [ ] **Step 2: Add failing tests**

新增測試確認：

```rust
assert_eq!(result.translated, "Bangkok");
assert_eq!(result.source, "metadata");
assert_eq!(result.used_lang, "official_en");
```

以及當 Wikidata 僅有英文 label、TH options 未把 `en` 放入 fallback languages 時，結果仍使用官方英文 fallback。

- [ ] **Step 3: Implement fallback**

將 `select_best_label` 改為可讀取 `TranslationItem`，優先保留既有 `zh-tw`、`zh-hant`、`zh`、`zhwiki` 邏輯，最後才讀 metadata 中的官方英文與官方泰文。

### Task 2: Thailand Translation Context

**Files:**
- Modify: `rust/src/pipeline/extract/types.rs`
- Modify: `rust/src/pipeline/extract/handlers.rs`
- Create: `rust/src/pipeline/extract/thailand_wikidata.rs`
- Modify: `rust/src/pipeline/extract.rs`

- [ ] **Step 1: Add TH translation type**

新增 `ThailandTranslations`，結構與 `KoreaTranslations` 一致，並擴充 `ExtractContext`。

- [ ] **Step 2: Add TH source attributes**

讓 `FeatureAttributes::Thailand` 讀取 `adm1_name1`、`adm2_name1`、`adm3_name1`，作為官方泰文 fallback。

- [ ] **Step 3: Build TH Wikidata cache**

新增 `build_thailand_wikidata_cache(features, cache_path)`：

```rust
let mut options = WikidataClientOptions::new("en", "zh-tw");
options.fallback_langs = vec!["zh-hant".to_string(), "zh".to_string()];
```

Admin1/Admin2 使用官方英文查詢；metadata 記錄官方英文與官方泰文。Admin2 使用 Admin1 QID 做 P131 驗證。

- [ ] **Step 4: Route TH context**

`load_context` 對 TH 優先讀 `TH_wikidata_stub.json`，否則寫入 `geoname_data/TH_wikidata_cache.json`。

### Task 3: Fixtures, Docs, Verification

**Files:**
- Create: `fixtures/parity/geospatial_extract/extract_sources/TH_wikidata_stub.json`
- Modify: `rust/tests/geospatial_extract.rs`
- Modify: `docs/zh-tw/thailand-admin-processing.md`
- Modify: `docs/research/thailand-handler.md`
- Modify: `README.md`

- [ ] **Step 1: Add fixture stub**

新增 TH fixture stub，覆蓋曼谷、清邁、沙敦等 Admin1/Admin2 翻譯，避免 fixture 測試依賴網路。

- [ ] **Step 2: Update expected extract output**

Integration test 預期 TH Admin1/Admin2 為繁中；Admin3 保留官方英文，因為目前只為 Admin1/Admin2 建 Wikidata cache。

- [ ] **Step 3: Document decisions**

文件需說明不使用官方中心點、使用 Thailand Albers 而非 Dynamic UTM、以及 TH 名稱優先級：
`zh-tw`、`zh-hant`、`zh`、`zhwiki`、官方英文、官方泰文。

- [ ] **Step 4: Verify**

執行：

```bash
cargo fmt --manifest-path rust/Cargo.toml --check
cargo test --manifest-path rust/Cargo.toml wikidata
cargo test --manifest-path rust/Cargo.toml thailand
cargo test --manifest-path rust/Cargo.toml
cargo clippy --manifest-path rust/Cargo.toml -- -D warnings
git diff --check
```
