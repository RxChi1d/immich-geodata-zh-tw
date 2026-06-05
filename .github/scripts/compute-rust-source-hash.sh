#!/usr/bin/env bash
set -euo pipefail

repo_root="${1:-.}"
cd "$repo_root"

# 只納入會影響 production binary 的 Rust 輸入，避免 DB 或文件更新讓 binary cache 失效。
mapfile -t source_files < <(
  git ls-files \
    'Cargo.toml' \
    'Cargo.lock' \
    'src/**' \
    | sort
)

if [[ "${#source_files[@]}" -eq 0 ]]; then
  echo "找不到可計算 hash 的 Rust source 檔案" >&2
  exit 1
fi

sha256sum "${source_files[@]}" | sha256sum | awk '{print $1}'
