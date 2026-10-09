#!/bin/bash
# update_data.sh --install 的離線回歸測試。
# 以 --archive 餵入自製的 payload，在暫存目錄裡搭出假的 Immich 安裝，不需要網路。
# 用法：bash tests/update_data_install.sh

set -e

SCRIPT="$(cd "$(dirname "$0")/.." && pwd)/update_data.sh"
TMP="$(mktemp -d -t update_data_install_XXXXXX)"
trap 'chmod -R u+w "$TMP" 2>/dev/null; rm -rf "$TMP"' EXIT

fail=0
ok()   { echo "ok   - $1"; }
bad()  { echo "FAIL - $1"; fail=1; }

# 建立 payload 壓縮檔：內容與 release.tar.gz 相同版面
make_archive() {
  local dir="$1" date_text="$2" en_text="$3" ci_text="${4:-}"
  mkdir -p "$dir/payload/geodata" "$dir/payload/i18n-iso-countries/langs"
  echo "$date_text" > "$dir/payload/geodata/geodata-date.txt"
  echo "$en_text" > "$dir/payload/i18n-iso-countries/langs/en.json"
  echo '{"locale":"ja"}' > "$dir/payload/i18n-iso-countries/langs/ja.json"
  [ -n "$ci_text" ] && echo "$ci_text" > "$dir/payload/geodata/countryInfo.txt"
  tar czf "$dir/release.tar.gz" -C "$dir/payload" geodata i18n-iso-countries
  echo "$dir/release.tar.gz"
}

# 建立假的 Immich 安裝
make_install() {
  local dir="$1"
  mkdir -p "$dir/server/node_modules/i18n-iso-countries/langs" "$dir/build/geodata"
  echo '{"name":"i18n-iso-countries"}' > "$dir/server/node_modules/i18n-iso-countries/package.json"
  echo 'OLD-EN' > "$dir/server/node_modules/i18n-iso-countries/langs/en.json"
  echo 'UPSTREAM-ONLY' > "$dir/server/node_modules/i18n-iso-countries/langs/de.json"
  echo 'OLD-DATE' > "$dir/build/geodata/geodata-date.txt"
}

# Immich >= 3.3.0 的版面：沒有 i18n-iso-countries，geodata 內有 countryInfo.txt
make_install_v33() {
  local dir="$1"
  mkdir -p "$dir/server" "$dir/build/geodata"
  echo 'OLD-DATE' > "$dir/build/geodata/geodata-date.txt"
  echo 'OLD-CI' > "$dir/build/geodata/countryInfo.txt"
}

run_install() {
  local root="$1" build="$2" archive="$3"
  IMMICH_SERVER_ROOT="$root" IMMICH_BUILD_DATA="$build" \
    bash "$SCRIPT" --install --archive "$archive" 2>&1
}

# --- 案例 1：正常安裝 ---
C1="$TMP/c1"; mkdir -p "$C1"; make_install "$C1"
A1="$(make_archive "$C1" NEW-DATE NEW-EN)"
out="$(run_install "$C1" "$C1/build" "$A1")" || bad "正常安裝應成功"
LANGS="$C1/server/node_modules/i18n-iso-countries/langs"
[ "$(cat "$LANGS/en.json")" = "NEW-EN" ] && ok "en.json 已更新" || bad "en.json 未更新"
[ "$(cat "$C1/build/geodata/geodata-date.txt")" = "NEW-DATE" ] && ok "geodata 已更新" || bad "geodata 未更新"
[ "$(cat "$LANGS/de.json")" = "UPSTREAM-ONLY" ] && ok "payload 未包含的上游語系檔被保留" || bad "上游語系檔被刪除"
echo "$out" | grep -q "驗證通過" && ok "驗證通過" || bad "未輸出驗證通過"
ls "$LANGS" | grep -q "\.tmp\." && bad "留下暫存檔" || ok "沒有留下暫存檔"

# --- 案例 2：目的地是 symlink 時不得寫穿到 target ---
C2="$TMP/c2"; mkdir -p "$C2"; make_install "$C2"
echo 'VICTIM' > "$C2/victim.txt"
ln -sf "$C2/victim.txt" "$C2/server/node_modules/i18n-iso-countries/langs/en.json"
A2="$(make_archive "$C2" NEW-DATE NEW-EN)"
run_install "$C2" "$C2/build" "$A2" >/dev/null || bad "symlink 案例安裝應成功"
[ "$(cat "$C2/victim.txt")" = "VICTIM" ] && ok "symlink 指向的檔案未被覆寫" || bad "寫穿了 symlink"
[ -L "$C2/server/node_modules/i18n-iso-countries/langs/en.json" ] && bad "en.json 仍是 symlink" || ok "en.json 已被實體檔取代"

# --- 案例 3：安裝中途失敗必須完整復原 ---
C3="$TMP/c3"; mkdir -p "$C3"; make_install "$C3"
A3="$(make_archive "$C3" NEW-DATE NEW-EN)"
chmod a-w "$C3/server/node_modules/i18n-iso-countries/langs"
if run_install "$C3" "$C3/build" "$A3" >/dev/null 2>&1; then
  bad "無法寫入 langs 時不應回報成功"
else
  ok "無法寫入 langs 時以非零狀態結束"
fi
chmod u+w "$C3/server/node_modules/i18n-iso-countries/langs"
[ "$(cat "$C3/build/geodata/geodata-date.txt")" = "OLD-DATE" ] && ok "失敗後 geodata 已復原" || bad "失敗後 geodata 未復原"
[ "$(cat "$C3/server/node_modules/i18n-iso-countries/langs/en.json")" = "OLD-EN" ] && ok "失敗後 langs 已復原" || bad "失敗後 langs 未復原"
[ -d "$C3/server/node_modules/i18n-iso-countries/langs.bak" ] && bad "復原後仍留著 langs.bak" || ok "復原後沒有殘留 langs.bak"

# --- 案例 4：payload 缺 geodata 時不得動到系統檔案 ---
C4="$TMP/c4"; mkdir -p "$C4/payload/i18n-iso-countries/langs"; make_install "$C4"
echo '{}' > "$C4/payload/i18n-iso-countries/langs/en.json"
tar czf "$C4/release.tar.gz" -C "$C4/payload" i18n-iso-countries
if run_install "$C4" "$C4/build" "$C4/release.tar.gz" >/dev/null 2>&1; then
  bad "payload 缺 geodata 時不應回報成功"
else
  ok "payload 缺 geodata 時以非零狀態結束"
fi
[ "$(cat "$C4/build/geodata/geodata-date.txt")" = "OLD-DATE" ] && ok "前置檢查失敗時系統檔案未被動過" || bad "前置檢查前就動了系統檔案"

# --- 案例 5：geodata 原本不存在，失敗時不應留下半套 ---
C5="$TMP/c5"; mkdir -p "$C5"; make_install "$C5"; rm -rf "$C5/build/geodata"
A5="$(make_archive "$C5" NEW-DATE NEW-EN)"
chmod a-w "$C5/server/node_modules/i18n-iso-countries/langs"
run_install "$C5" "$C5/build" "$A5" >/dev/null 2>&1 || true
chmod u+w "$C5/server/node_modules/i18n-iso-countries/langs"
[ -d "$C5/build/geodata" ] && bad "失敗後留下本次新建的 geodata" || ok "失敗後移除本次新建的 geodata"

# --- 案例 6：安裝後內容被竄改時，驗證必須擋下並復原 ---
# 以 PATH 上的 mv 包裝器在 rename 完成後污染 en.json，模擬「bytes 沒有正確落地」。
# 這是恆真式驗證回歸時唯一會失敗的測試。
C6="$TMP/c6"; mkdir -p "$C6/bin"; make_install "$C6"
A6="$(make_archive "$C6" NEW-DATE NEW-EN)"
cat > "$C6/bin/mv" <<'SHIM'
#!/bin/bash
/bin/mv "$@"
rc=$?
for last; do :; done
case "$last" in
  */langs/en.json) echo TAMPERED >> "$last" ;;
esac
exit $rc
SHIM
chmod +x "$C6/bin/mv"
if PATH="$C6/bin:$PATH" run_install "$C6" "$C6/build" "$A6" >/dev/null 2>&1; then
  bad "內容與下載資料不符時不應回報成功"
else
  ok "內容與下載資料不符時驗證擋下並以非零狀態結束"
fi
[ "$(cat "$C6/build/geodata/geodata-date.txt")" = "OLD-DATE" ] && ok "驗證失敗後 geodata 已復原" || bad "驗證失敗後 geodata 未復原"
[ "$(cat "$C6/server/node_modules/i18n-iso-countries/langs/en.json")" = "OLD-EN" ] && ok "驗證失敗後 langs 已復原" || bad "驗證失敗後 langs 未復原"

# --- 案例 7：事先放置的可預測暫存檔 symlink 不得被寫穿 ---
C7="$TMP/c7"; mkdir -p "$C7"; make_install "$C7"
echo 'VICTIM7' > "$C7/victim.txt"
LANGS7="$C7/server/node_modules/i18n-iso-countries/langs"
# 涵蓋舊實作可預測的 .$name.tmp.$$ 形式（PID 未知，佈滿一段範圍）
for pid in $(seq 100 400); do ln -sf "$C7/victim.txt" "$LANGS7/.en.json.tmp.$pid"; done
A7="$(make_archive "$C7" NEW-DATE NEW-EN)"
run_install "$C7" "$C7/build" "$A7" >/dev/null 2>&1 || bad "暫存 symlink 案例安裝應成功"
[ "$(cat "$C7/victim.txt")" = "VICTIM7" ] && ok "事先放置的暫存 symlink 未被寫穿" || bad "暫存路徑被寫穿"

# --- 案例 8：Immich >= 3.3.0 版面（無 i18n、有 countryInfo.txt）只裝 geodata ---
C8="$TMP/c8"; mkdir -p "$C8"; make_install_v33 "$C8"
A8="$(make_archive "$C8" NEW-DATE NEW-EN NEW-CI)"
out="$(run_install "$C8" "$C8/build" "$A8")" || bad "3.3.0 版面安裝應成功"
[ "$(cat "$C8/build/geodata/countryInfo.txt")" = "NEW-CI" ] && ok "countryInfo.txt 已更新" || bad "countryInfo.txt 未更新"
[ "$(cat "$C8/build/geodata/geodata-date.txt")" = "NEW-DATE" ] && ok "3.3.0 版面 geodata 已更新" || bad "3.3.0 版面 geodata 未更新"
[ -e "$C8/server/node_modules" ] && bad "3.3.0 版面不應建立 node_modules" || ok "3.3.0 版面沒有建立 i18n 目錄"

# --- 案例 9：i18n 與 countryInfo.txt 都不存在時必須報錯且不動任何檔案 ---
C9="$TMP/c9"; mkdir -p "$C9/server" "$C9/build/geodata"; echo OLD-DATE > "$C9/build/geodata/geodata-date.txt"
A9="$(make_archive "$C9" NEW-DATE NEW-EN NEW-CI)"
if run_install "$C9" "$C9/build" "$A9" >/dev/null 2>&1; then
  bad "兩個國名來源都不存在時不應回報成功"
else
  ok "兩個國名來源都不存在時以非零狀態結束"
fi
[ "$(cat "$C9/build/geodata/geodata-date.txt")" = "OLD-DATE" ] && ok "判斷失敗時系統檔案未被動過" || bad "判斷失敗時動了系統檔案"

# --- 案例 10：目標需要 countryInfo.txt 但 release 沒帶時，不得替換 geodata ---
C10="$TMP/c10"; mkdir -p "$C10"; make_install_v33 "$C10"
A10="$(make_archive "$C10" NEW-DATE NEW-EN)"
if run_install "$C10" "$C10/build" "$A10" >/dev/null 2>&1; then
  bad "release 缺 countryInfo.txt 時不應回報成功"
else
  ok "release 缺 countryInfo.txt 時以非零狀態結束"
fi
[ "$(cat "$C10/build/geodata/countryInfo.txt")" = "OLD-CI" ] && ok "countryInfo.txt 未被刪除" || bad "countryInfo.txt 被刪除"

# --- 案例 11：兩者並存（過渡期映像）兩邊都裝 ---
C11="$TMP/c11"; mkdir -p "$C11"; make_install "$C11"; echo OLD-CI > "$C11/build/geodata/countryInfo.txt"
A11="$(make_archive "$C11" NEW-DATE NEW-EN NEW-CI)"
run_install "$C11" "$C11/build" "$A11" >/dev/null || bad "兩者並存時安裝應成功"
[ "$(cat "$C11/build/geodata/countryInfo.txt")" = "NEW-CI" ] && ok "並存時 countryInfo.txt 已更新" || bad "並存時 countryInfo.txt 未更新"
[ "$(cat "$C11/server/node_modules/i18n-iso-countries/langs/en.json")" = "NEW-EN" ] && ok "並存時 en.json 已更新" || bad "並存時 en.json 未更新"

# --- 案例 12：舊版 Immich 安裝後不得留下 countryInfo.txt 當作「新版」標記 ---
# 否則 i18n 偵測日後失敗（例如加速器升版後路徑變了）時，第二次執行會誤判為
# Immich >= 3.3.0 而靜默略過 i18n，國名悄悄退回英文。
C12="$TMP/c12"; mkdir -p "$C12"; make_install "$C12"
A12="$(make_archive "$C12" NEW-DATE NEW-EN NEW-CI)"
run_install "$C12" "$C12/build" "$A12" >/dev/null || bad "舊版版面安裝應成功"
[ -e "$C12/build/geodata/countryInfo.txt" ] && bad "舊版版面留下了 countryInfo.txt" || ok "舊版版面沒有留下 countryInfo.txt"
rm -rf "$C12/server/node_modules"
if run_install "$C12" "$C12/build" "$A12" >/dev/null 2>&1; then
  bad "i18n 消失後第二次執行不應回報成功"
else
  ok "i18n 消失後第二次執行以非零狀態結束"
fi

exit $fail
