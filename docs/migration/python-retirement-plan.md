# Python Retirement Plan

更新日期：2026-06-01

本文件定義 migration PR 前的 Python retirement 邊界。目標是讓此分支成為
Rust-only production PR：production release、nightly、extract、prepare、translate、
pack 與 workflow 均不再依賴 Python。

## 盤點結果

截至本文件建立時，版控內仍存在下列 Python surface：

| 類別 | 檔案 | 處置 |
|---|---|---|
| Python production entrypoint | `main.py` | 刪除。Rust CLI 已覆蓋 production command contract。 |
| Python production package | `core/**` | 刪除。對應邏輯已由 `rust/src/pipeline/**` 取代。 |
| Python production/parity tests | `tests/**` | 刪除。Python implementation 移除後不再執行 pytest gate。 |
| Python parity tools | `tools/compare_outputs.py`、`tools/export_*.py`、`tools/run_*.py`、`tools/parity_*.py` | 刪除可執行工具；保留 `docs/migration/**` 的歷史驗證摘要。 |
| Python package metadata | `pyproject.toml`、`uv.lock`、`.python-version` | 刪除。Rust-only repo 不再需要 Python dependency lock。 |
| Python golden/reference output | `golden/python/**` | 刪除。最終 PR 不保留可執行 Python baseline；歷史結果改由 migration docs 摘要保存。 |
| Rust golden output | `golden/rust/**` | 刪除或歸檔為歷史資料；production 驗證改由 Rust tests、fixture smoke 與 real release gate 負責。 |
| Parity fixtures | `fixtures/parity/**` | 保留。Rust tests 與 release smoke 可繼續使用固定 fixture。 |
| Source download helper | `.agents/skills/immich-geodata-source-release/scripts/immich_geodata_download_latest_sources.py` | 改寫成非 Python helper 或移除該 helper；skill 文件同步更新。 |
| Workflow Python usage | `.github/workflows/build-rust-binary.yaml` 內 manifest 產生用 `python3` | 改成 shell/Node-free 的 POSIX heredoc 或其他已安裝 CLI，避免 workflow 仍顯示 Python dependency。 |

## 不可誤動的資料庫邊界

以下檔案是 canonical DB 或 production metadata，不屬於 Python retirement：

- `meta_data/tw_geodata.csv`
- `meta_data/jp_geodata.csv`
- `meta_data/kr_geodata.csv`
- `meta_data/TH.csv`

清理時不得刪除或重新產生上述檔案，除非另有資料更新任務。

## 階段驗證

每個階段完成後需至少執行：

1. `git diff --check`
2. `rg -n "python main\\.py|uv run python|pytest|ruff|mypy|pyproject|uv\\.lock|core/|from core" README.md README.en.md AGENTS.md CLAUDE.md docs/zh-tw docs/en .github .agents`
3. `cargo fmt --manifest-path rust/Cargo.toml --check`
4. `cargo clippy --manifest-path rust/Cargo.toml -- -D warnings`
5. `cargo test --manifest-path rust/Cargo.toml`

移除完成後還需執行 Rust release smoke 與真實資料 release gate，確認輸出品質未因清理
Python surface 受到影響。
