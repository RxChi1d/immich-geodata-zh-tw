#!/usr/bin/env bash
# 從 CHANGELOG.md 抽取指定版本區段，轉換為 GitHub Release Notes 後輸出至 stdout。
#
# 用法：extract-release-notes.sh <version_tag> [changelog_path]
#   version_tag     例如 v2.3.0 或 v2.3.0rc1（PEP 440 預發布）
#   changelog_path  預設 CHANGELOG.md
#
# 行為：
#   - 正式版：CHANGELOG 缺對應版本區段時直接失敗（exit 1），避免發出空的
#     release notes；錯誤訊息提示先完成 CHANGELOG 版本切割。
#   - 預發布版（aN/bN/rcN）：缺區段時輸出簡化說明並指向 [未發佈版本] 區段
#     （業界慣例：預發布不重複撰寫完整 notes，精緻內容留給正式版）。
#   - 分類標題依 CLAUDE.md 的對應表轉換為 emoji 格式（### Added → ## 🚀 Added）。
#   - compare 連結優先取 CHANGELOG 底部的版本 link reference；缺少時以
#     git 最新 tag 與 GITHUB_REPOSITORY 推導。

set -euo pipefail

version_tag="${1:?用法: extract-release-notes.sh <version_tag> [changelog_path]}"
changelog="${2:-CHANGELOG.md}"
version="${version_tag#v}"

if [[ ! -f "${changelog}" ]]; then
  echo "錯誤：找不到 ${changelog}" >&2
  exit 1
fi

is_prerelease=0
if [[ "${version}" =~ ^[0-9]+\.[0-9]+\.[0-9]+(a|b|rc)[0-9]+$ ]]; then
  is_prerelease=1
fi

# --------------------------------------------------------------------------
# 抽取版本區段（## [X.Y.Z] 起、至下一個 ## [ 為止），並取得標題列的日期
# --------------------------------------------------------------------------
heading="$(grep -m1 "^## \[${version}\]" "${changelog}" || true)"
section=""
if [[ -n "${heading}" ]]; then
  # Reason: 版本字串含 '.'，避免 regex 詮釋，改用 index() 做字面比對。
  section="$(awk -v ver="${version}" '
    /^## \[/ {
      if (found) exit
      if (index($0, "## [" ver "]") == 1) { found = 1 }
      next
    }
    found { print }
  ' "${changelog}")"
fi

# --------------------------------------------------------------------------
# compare 連結：優先取 CHANGELOG link reference（[X.Y.Z]: <url>）
# --------------------------------------------------------------------------
compare_url="$(grep -m1 "^\[${version}\]: " "${changelog}" | sed 's/^[^ ]* //' || true)"
if [[ -z "${compare_url}" ]]; then
  repo_url="https://github.com/${GITHUB_REPOSITORY:-RxChi1d/immich-geodata-zh-tw}"
  prev_tag="$(git describe --tags --abbrev=0 2>/dev/null || true)"
  if [[ -n "${prev_tag}" && "${prev_tag}" != "${version_tag}" ]]; then
    compare_url="${repo_url}/compare/${prev_tag}...${version_tag}"
  else
    compare_url="${repo_url}/commits/${version_tag}"
  fi
fi
# 連結文字顯示比較範圍（如 v2.2.3...v2.2.4）；非 compare 連結時退回 tag 名稱
compare_label="${compare_url##*/compare/}"
if [[ "${compare_label}" == "${compare_url}" ]]; then
  compare_label="${version_tag}"
fi

# 發布日期：取 CHANGELOG 標題列的日期（## [X.Y.Z] - YYYY-MM-DD），缺少時用今天
release_date="$(sed -n 's/^## \[[^]]*\] - \([0-9-]*\).*/\1/p' <<< "${heading}" | head -1)"
release_date="${release_date:-$(date +%Y-%m-%d)}"

# --------------------------------------------------------------------------
# 組合輸出
# --------------------------------------------------------------------------
if [[ -z "${section//[[:space:]]/}" ]]; then
  if [[ "${is_prerelease}" -eq 0 ]]; then
    echo "錯誤：CHANGELOG.md 中找不到 [${version}] 的版本區段。" >&2
    echo "正式發版前請先完成 CHANGELOG 版本切割（[未發佈版本] → [${version}]）。" >&2
    exit 1
  fi
  # 預發布版缺區段：輸出簡化說明
  body="預發布版本（\`${version_tag}\`），用於正式發布前的驗證；完整變更說明將隨對應的正式版本發布。

詳細變更請參考 [CHANGELOG](https://github.com/${GITHUB_REPOSITORY:-RxChi1d/immich-geodata-zh-tw}/blob/main/CHANGELOG.md) 的「未發佈版本」區段。"
else
  # 分類標題轉換為 Release Notes 的 emoji 格式（對應表見 CLAUDE.md）
  body="$(sed \
    -e 's/^### Added$/## 🚀 Added/' \
    -e 's/^### Changed$/## 🔄 Changed/' \
    -e 's/^### Deprecated$/## ⚠️ Deprecated/' \
    -e 's/^### Removed$/## 🗑️ Removed/' \
    -e 's/^### Fixed$/## 🐛 Fixed/' \
    -e 's/^### Security$/## 🔒 Security/' \
    <<< "${section}")"
fi

cat <<EOF
# What's Changed

$(echo "${body}" | sed -e '/./,$!d' -e :a -e '/^\s*$/{$d;N;ba' -e '}')

---

**完整變更記錄**: [${compare_label}](${compare_url})

**發布日期**: ${release_date}
EOF
