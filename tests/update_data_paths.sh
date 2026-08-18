#!/bin/bash
# update_data.sh --print-paths 的路徑偵測自我檢查。
# 用法：bash tests/update_data_paths.sh
#
# 這裡以假造的目錄結構涵蓋各種版面。實機驗證（--install 全流程且安裝驗證通過）
# 已在下列環境完成，四種組合皆覆蓋：
#   immich-server v1.135.3  扁平版面 + npm    /usr/src/app/node_modules
#   immich-server v1.136.0  巢狀版面 + npm    /usr/src/app/server/node_modules
#   immich-server v3.1.0    巢狀版面 + pnpm   /usr/src/app/server/node_modules
#   macOS 原生 worker 3.1.0 扁平版面 + pnpm   ~/.immich-accelerator/server/3.1.0/node_modules
# 另以 v3.1.0 映像模擬非標準安裝位置（LXC / 裸機）：
#   應用搬到 /opt/immich，未設 IMMICH_SERVER_ROOT   -> 明確報錯並提示設定變數
#   設定 IMMICH_SERVER_ROOT                          -> 安裝並驗證通過
#   非 root 使用者 + IMMICH_BUILD_DATA 指向家目錄    -> 安裝並驗證通過，不嘗試 chown

set -e

SCRIPT="$(cd "$(dirname "$0")/.." && pwd)/update_data.sh"
TMP="$(mktemp -d -t update_data_paths_XXXXXX)"
trap 'rm -rf "$TMP"' EXIT

fail=0

check() {
  local name="$1" expected="$2" actual="$3"
  if [ "$expected" = "$actual" ]; then
    echo "ok   - $name"
  else
    echo "FAIL - $name"
    echo "       expected: $expected"
    echo "       actual:   $actual"
    fail=1
  fi
}

i18n_path_of() {
  bash "$SCRIPT" --print-paths | sed -n 's/^i18n-iso-countries: //p'
}

geodata_path_of() {
  bash "$SCRIPT" --print-paths | sed -n 's/^geodata: //p'
}

# 案例 1：巢狀版面 (Immich 1.136+ 容器)
mkdir -p "$TMP/nested/server/node_modules/i18n-iso-countries"
check "巢狀版面命中 server/node_modules" \
  "$TMP/nested/server/node_modules/i18n-iso-countries" \
  "$(IMMICH_SERVER_ROOT="$TMP/nested" i18n_path_of)"

# 案例 2：扁平版面 (Immich < 1.136 容器、macOS 原生 worker)
# 即使 Immich 版本是新版也應走扁平路徑，這是版本號推斷會判錯的情況
mkdir -p "$TMP/flat/node_modules/i18n-iso-countries"
check "扁平版面命中 node_modules" \
  "$TMP/flat/node_modules/i18n-iso-countries" \
  "$(IMMICH_SERVER_ROOT="$TMP/flat" i18n_path_of)"

# 案例 3：新舊版面並存 (搬遷後殘留舊目錄) 時優先取巢狀版面並提出警告
mkdir -p "$TMP/both/server/node_modules/i18n-iso-countries" "$TMP/both/node_modules/i18n-iso-countries"
check "新舊版面並存時優先取 server/node_modules" \
  "$TMP/both/server/node_modules/i18n-iso-countries" \
  "$(IMMICH_SERVER_ROOT="$TMP/both" i18n_path_of 2>/dev/null)"
if IMMICH_SERVER_ROOT="$TMP/both" bash "$SCRIPT" --print-paths 2>&1 >/dev/null | grep -q "多個"; then
  echo "ok   - 新舊版面並存時提出警告"
else
  echo "FAIL - 新舊版面並存時未提出警告"
  fail=1
fi

# 案例 4：套件搬到未知位置時改用掃描找回 (取代原本的版本號分界推斷)
mkdir -p "$TMP/moved/apps/server/node_modules/i18n-iso-countries"
check "套件搬家後仍能掃描找到" \
  "$TMP/moved/apps/server/node_modules/i18n-iso-countries" \
  "$(IMMICH_SERVER_ROOT="$TMP/moved" i18n_path_of 2>/dev/null)"

# 案例 4b：同一個實體目標被不同 root 找到時應去重，不得誤報歧義
mkdir -p "$TMP/dup/node_modules/i18n-iso-countries"
ln -sfn "$TMP/dup" "$TMP/dup/server"
if IMMICH_SERVER_ROOT="$TMP/dup" bash "$SCRIPT" --print-paths 2>&1 >/dev/null | grep -q "多個"; then
  echo "FAIL - 同一實體目標被誤判為多個候選"
  fail=1
else
  echo "ok   - 同一實體目標已去重"
fi

# 案例 4c：明確指定 IMMICH_SERVER_ROOT 後，不得回頭採用其他來源的路徑
mkdir -p "$TMP/fakehome/.immich-accelerator" "$TMP/accel/node_modules/i18n-iso-countries" "$TMP/wrongroot"
printf '{\n  "server_dir": "%s"\n}\n' "$TMP/accel" > "$TMP/fakehome/.immich-accelerator/config.json"
if HOME="$TMP/fakehome" bash "$SCRIPT" --print-paths 2>/dev/null | grep -q "$TMP/accel"; then
  echo "ok   - 未指定 root 時採用 accelerator 設定"
else
  echo "FAIL - 未指定 root 時未採用 accelerator 設定"
  fail=1
fi
if HOME="$TMP/fakehome" IMMICH_SERVER_ROOT="$TMP/wrongroot" bash "$SCRIPT" --print-paths >/dev/null 2>&1; then
  echo "FAIL - 明確指定的 root 沒有命中時仍回報成功"
  fail=1
else
  echo "ok   - 明確指定的 root 沒有命中時直接失敗"
fi

# 案例 5：只有 node_modules 而套件不存在時必須失敗，不能自行建立空目錄
mkdir -p "$TMP/empty/node_modules"
if HOME="$TMP/empty" IMMICH_SERVER_ROOT="$TMP/empty" bash "$SCRIPT" --print-paths >/dev/null 2>&1; then
  echo "FAIL - 套件不存在時不應回報成功"
  fail=1
else
  echo "ok   - 套件不存在時以非零狀態結束"
fi

# 案例 6：geodata 路徑跟隨 Immich 自己的 IMMICH_BUILD_DATA
check "IMMICH_BUILD_DATA 覆寫生效" \
  "$TMP/custom/geodata" \
  "$(IMMICH_SERVER_ROOT="$TMP/flat" IMMICH_BUILD_DATA="$TMP/custom" geodata_path_of)"

# 案例 7：geodata 預設值
check "geodata 預設為 /build/geodata" \
  "/build/geodata" \
  "$(IMMICH_SERVER_ROOT="$TMP/flat" geodata_path_of)"

# 案例 8：完全找不到時應失敗並提示 IMMICH_SERVER_ROOT
mkdir -p "$TMP/nothing"
if HOME="$TMP/nothing" IMMICH_SERVER_ROOT="$TMP/nothing" bash "$SCRIPT" --print-paths >/dev/null 2>"$TMP/err"; then
  echo "FAIL - 偵測失敗時應以非零狀態結束"
  fail=1
elif grep -q "IMMICH_SERVER_ROOT" "$TMP/err"; then
  echo "ok   - 偵測失敗時提示 IMMICH_SERVER_ROOT"
else
  echo "FAIL - 偵測失敗訊息未提示 IMMICH_SERVER_ROOT"
  cat "$TMP/err"
  fail=1
fi

exit $fail
