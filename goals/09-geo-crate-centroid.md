# Goal: 在 Rust extract.rs 中使用 geo crate 實作真正的多邊形 centroid 計算

## Context

目前 `rust/src/pipeline/extract.rs` 的 `centroid_from_coordinates` 函數只做了 bounding box 平均（對多邊形取 min/max 再除以 2），這不是真正的幾何 centroid。

而 Python 端的 `core/geodata/geospatial.py` 透過 Shapely（底層 GEOS）計算真正的多邊形 centroid。

目標：在 Rust 端用 `geo` crate 替代 Shapely 的 centroid 行為，讓 GeoJSON Polygon/MultiPolygon geometry 能算出正確的幾何 centroid。

## Must Read First

- `AGENTS.md`（專案語言規則與開發規範）
- `.codex/skills/python-to-rust-geodata-migration/SKILL.md`（遷移規則）
- `docs/migration/parity-contract.md`（parity 比較規則，座標精度 8 位小數）
- `docs/migration/parity-matrix.md`（現有 parity 狀態）
- `docs/migration/risk-register.md`（已知風險）

## Scope

**只會動這些檔案：**

1. `rust/Cargo.toml` — 加入 `geo` crate 依賴
2. `rust/src/pipeline/extract.rs` — 改寫 `centroid_from_coordinates` 函數以及座標 parsing 邏輯，支援：
   - Point geometry（目前行為，直接回傳座標）
   - Polygon geometry（用 `geo` crate Centroid trait 計算）
   - MultiPolygon geometry（用 `geo` crate Centroid trait 計算）
3. `rust/src/pipeline/extract.rs` 的 GeoJSON coordinates 解析 — 從目前的 raw number scanning 改為正確解析 GeoJSON coordinate array 巢狀結構（Point 是 `[lon, lat]`，Polygon 是 `[[[lon, lat], ...]]`）

**不會動的檔案：**
- `golden/python/` 底下的任何 golden output
- `fixtures/parity/` 底下的任何 fixture
- Python 端任何 `.py` 檔案
- `tools/compare_outputs.py` 或 `tools/run_parity.py`

## Implementation Details

### Centroid 計算邏輯

對於 GeoJSON geometry：

1. **Point** (`[lon, lat]`): 直接回傳 `(lon, lat)` — 不變
2. **Polygon** (`[[[lon, lat], ...]]`): 
   - 解析外環座標
   - 用 `geo::Polygon::new(exterior, vec![])` 建立多邊形
   - 用 `polygon.centroid()` 計算 centroid
   - 回傳 centroid 的 x/y 作為 lon/lat
3. **MultiPolygon** (`[[[[lon, lat], ...]]]`):
   - 對每個 polygon 解析並計算面積
   - 取面積加權平均的 centroid（或用 geo 的 MultiPolygon centroid）

### GeoJSON Coordinates 解析

目前的 `scan_numbers` 會把所有數字打平掃描，這對多邊形不夠用。需要：

1. 解析 GeoJSON coordinate array 的巢狀結構
2. Point: `[lon, lat]` → 取第一個 pair
3. Polygon: `[[ring]]` → 第一個 ring 是外環
4. MultiPolygon: `[[[ring]]]` → 多個 polygon

### Cargo.toml 依賴

加上 `geo = "0.30"`（穩定版，不需要額外 features）。

### 座標精度

沿用專案標準：8 位小數 (`format_coordinate` 函數會處理)。

## Success Criteria（驗證方式）

完成後必須通過以下所有檢查：

1. `cargo fmt --check` — 格式化通過
2. `cargo clippy -- -D warnings` — 無 clippy 警告
3. `cargo test` — 所有 Rust tests 通過
4. `uv run python tools/run_parity.py --mode rust --stage extract` — extract stage parity 通過

**注意：** 現有的 `geospatial_extract` fixture 使用 Point geometry，所以 parity 測試應該仍然通過（Point geometry 行為不變）。

## Stopping Condition

當以下條件全部滿足時停止：
1. `cargo build` 成功
2. `cargo test` 通過
3. `cargo fmt --check` 通過
4. `cargo clippy -- -D warnings` 通過
5. `uv run python tools/run_parity.py --mode rust --stage extract` 通過
6. `git diff` 只改了 `rust/Cargo.toml` 和 `rust/src/pipeline/extract.rs`

## Edge Cases to Handle

- 空的 coordinates array → 回傳錯誤
- Point coordinate 只有 2 個數字 → 直接回傳（目前行為）
- Polygon 外環少於 3 個點 → 退化情況，用已有的 bounding box fallback
- 無效的 GeoJSON geometry type → 回傳有意義的錯誤訊息

## Commit 訊息

使用 zh-tw Conventional Commit 格式：

```
feat(extract): 以 geo crate 實作多邊形 centroid 計算

- 加入 geo crate 依賴
- 改寫 centroid_from_coordinates 支援 Point / Polygon / MultiPolygon
- 重構 coordinate 解析邏輯支援巢狀 GeoJSON array
- 通過 extract stage parity 檢查

Co-Authored-By: Codex
```
