#!/bin/bash

# 這個腳本用於下載和安裝最新的 geodata 和 i18n-iso-countries 資料夾
# 下載的檔案會被解壓縮到指定的目錄 (DOWNLOAD_DIR)
# 如果指定了 --install 參數，則會將檔案安裝到 Immich 的系統目錄
#
# 支援的部署型態：
#   1. 官方 Docker 容器 (immich-server)
#   2. 非容器部署，例如 macOS 原生 worker (epheterson/immich-apple-silicon)
#      或自行編譯的 LXC / 裸機部署
#
# 安裝路徑會自動偵測；若偵測不到，可用以下環境變數覆寫：
#   IMMICH_SERVER_ROOT   Immich server 根目錄 (其下應有 node_modules/)。
#                        一旦設定即為唯一搜尋範圍，不會再回頭找其他位置。
#   IMMICH_BUILD_DATA    Immich 自身的變數，geodata 會裝到其下的 geodata/ (預設 /build)


set -e

# 開場白刻意放在 main() 外面：腳本被截斷時 main 不會被呼叫，什麼都不會執行，
# 但日誌上會留下「開始了卻沒有完成」的落差，比完全沒有輸出容易察覺。
echo "[immich-geodata-zh-tw] update_data.sh 開始執行"

# 用戶可修改的配置
DOWNLOAD_DIR="./temp" # 普通模式下的下載目錄

# 預設值
RELEASE_TAG="latest"
INSTALL_MODE=false
PRINT_PATHS_MODE=false
ARCHIVE_FILE=""

# 安裝狀態 (供 EXIT trap 判斷是否需要復原)
INSTALL_IN_PROGRESS=false
GEODATA_BACKUP_STATE=""
LANGS_BACKED_UP=false

# --- 系統路徑偵測 ---
# 從 immich-accelerator 的設定檔讀出 server 目錄 (非容器部署常見來源)
read_accelerator_server_dir() {
  local config="$HOME/.immich-accelerator/config.json"
  [ -f "$config" ] || return 1
  sed -n 's/.*"server_dir"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' "$config" | head -n 1
}

# 收集候選的 Immich server 根目錄，順序即優先序。
# 使用者明確指定 IMMICH_SERVER_ROOT 時，該值就是唯一範圍：若指定的位置沒有套件，
# 應該讓使用者知道指定錯了，而不是安靜地改裝到別處。
collect_server_roots() {
  local accelerator_dir
  if [ -n "$IMMICH_SERVER_ROOT" ]; then
    # 兩種寫法都接受：指向 app 根目錄或直接指向 server 子目錄
    echo "$IMMICH_SERVER_ROOT/server"
    echo "$IMMICH_SERVER_ROOT"
    return 0
  fi
  # Immich 1.136+ 的容器版面 (server 子目錄)，以及更早的扁平版面
  echo "/usr/src/app/server"
  echo "/usr/src/app"
  accelerator_dir="$(read_accelerator_server_dir)" && [ -n "$accelerator_dir" ] && echo "$accelerator_dir"
  return 0
}

canonical_path() {
  (cd "$1" 2>/dev/null && pwd -P)
}

# 候選清單以 canonical path 去重，但保留原始 logical path。
# pnpm 版面下 canonical path 會指進 .pnpm 實體目錄，拿它當安裝路徑會改變後續語意。
_CANDIDATES=()
_CANDIDATE_KEYS=""
reset_candidates() {
  _CANDIDATES=()
  _CANDIDATE_KEYS=""
}
add_candidate() {
  local path="$1" key
  key="$(canonical_path "$path")" || key="$path"
  case "$_CANDIDATE_KEYS" in
    *"|$key|"*) return 0 ;;
  esac
  _CANDIDATE_KEYS="$_CANDIDATE_KEYS|$key|"
  _CANDIDATES+=("$path")
}

# 掃描候選根目錄底下的 node_modules，找出 Immich 遷移後的新位置。
# Immich 曾在 1.136.0 把套件從 /usr/src/app 移到 /usr/src/app/server，
# 這是為了讓下一次類似的搬遷不必再改腳本才能安裝到正確位置。
# -prune 讓 find 不進入 node_modules 內部，避免掃描整棵相依樹。
scan_for_candidates() {
  local root output modules
  while IFS= read -r root; do
    [ -d "$root" ] || continue
    if ! output="$(find "$root" -maxdepth 5 -type d -name node_modules -prune)"; then
      echo "警告：掃描 $root 時 find 回報錯誤，結果可能不完整。" >&2
    fi
    while IFS= read -r modules; do
      [ -n "$modules" ] || continue
      [ -d "$modules/i18n-iso-countries" ] && add_candidate "$modules/i18n-iso-countries"
    done <<< "$output"
  done < <(collect_server_roots)
  return 0
}

# 由實際目錄結構決定 i18n-iso-countries 的安裝位置。
# 直接偵測版面而非依版本號推斷：版本號只是版面的代理指標，Immich 的路徑分界點
# 本身就修正過兩次，而非容器部署即使是新版 Immich 也可能維持扁平版面。
# 只接受「套件確實存在」的路徑；若找不到就報錯，不自行建立空目錄，
# 否則 langs 會被裝進 Immich 根本不會讀取的位置而毫無徵兆。
detect_i18n_path() {
  local root
  reset_candidates

  while IFS= read -r root; do
    [ -d "$root/node_modules/i18n-iso-countries" ] && add_candidate "$root/node_modules/i18n-iso-countries"
  done < <(collect_server_roots)

  # 已知路徑都沒命中：可能又搬家了，改用掃描
  if [ ${#_CANDIDATES[@]} -eq 0 ]; then
    scan_for_candidates
    [ ${#_CANDIDATES[@]} -gt 0 ] && echo "提示：在預期路徑外找到 i18n-iso-countries，Immich 可能已變更目錄結構。" >&2
  fi

  [ ${#_CANDIDATES[@]} -eq 0 ] && return 1

  # 真正不同的目標才算歧義。這裡只警告不中斷：整合式部署的 entrypoint 是
  # `update_data.sh --install && exec start.sh`，為了在地化問題讓 Immich 起不來並不划算。
  if [ ${#_CANDIDATES[@]} -gt 1 ]; then
    echo "警告：找到多個 i18n-iso-countries，將使用第一個：" >&2
    printf '  - %s\n' "${_CANDIDATES[@]}" >&2
  fi
  echo "${_CANDIDATES[0]}"
}

resolve_system_paths() {
  # 沿用 Immich 自己的 IMMICH_BUILD_DATA：Immich 以 join(IMMICH_BUILD_DATA || "/build", "geodata")
  # 決定 geodata 位置，直接讀同一個變數就不會有另一套設定要同步。
  SYSTEM_GEODATA_PATH="${IMMICH_BUILD_DATA:-/build}/geodata"

  SYSTEM_I18N_PATH="$(detect_i18n_path)" || {
    echo "錯誤：找不到已安裝的 i18n-iso-countries 套件。" >&2
    echo "已檢查的位置：" >&2
    collect_server_roots | sed 's|^|  - |;s|$|/node_modules/i18n-iso-countries|' >&2
    if [ -n "$IMMICH_SERVER_ROOT" ]; then
      echo "已設定 IMMICH_SERVER_ROOT=$IMMICH_SERVER_ROOT，僅在該範圍內搜尋。請確認路徑是否正確。" >&2
    else
      echo "請確認此腳本在 Immich 環境中執行，或設定 IMMICH_SERVER_ROOT 指向 Immich server 根目錄。" >&2
    fi
    exit 1
  }
}
# --- 系統路徑偵測結束 ---

# --- 安裝結果驗證 ---
# 安裝後回答同一個問題：「從選定的 server root 解析出來的套件，內容是不是我們剛裝的」。
# 比對對象是下載後暫存的 payload，不是安裝目標本身 —— 拿安裝目標去比對自己是恆真式，
# 就算複製從未發生也會通過。
find_node() {
  local candidate
  command -v node 2>/dev/null && return 0
  for candidate in "$HOME/.local/bin/node" /opt/homebrew/bin/node /usr/local/bin/node; do
    [ -x "$candidate" ] && { echo "$candidate"; return 0; }
  done
  return 1
}

# 找不到 node 時的等價解析：Node 解析裸模組名的規則是從起點目錄逐層往上，
# 取第一個命中的 node_modules/<套件>。非容器部署不保證裝得到 node，
# 因此驗證不能只有 node 這條路。
resolve_package_by_walking_up() {
  local dir
  dir="$(cd "$1" 2>/dev/null && pwd -P)" || return 1
  while :; do
    if [ -d "$dir/node_modules/i18n-iso-countries" ]; then
      echo "$dir/node_modules/i18n-iso-countries"
      return 0
    fi
    [ "$dir" = "/" ] && return 1
    dir="$(dirname "$dir")"
  done
}

# 用 node 解析時，是哪一個 node 不影響結果：解析規則只看檔案系統與起點目錄。
resolve_package_by_node() {
  local node_bin manifest
  node_bin="$(find_node)" || return 1
  manifest="$("$node_bin" -e \
    'console.log(require.resolve("i18n-iso-countries/package.json", { paths: [process.argv[1]] }))' \
    "$1" 2>/dev/null)" || return 1
  [ -n "$manifest" ] || return 1
  dirname "$manifest"
}

verify_installation() {
  local resolved server_root method name failed=0 src
  server_root="$(dirname "$(dirname "$SYSTEM_I18N_PATH")")"

  if resolved="$(resolve_package_by_node "$server_root")" && [ -n "$resolved" ]; then
    method="node 模組解析"
  elif resolved="$(resolve_package_by_walking_up "$server_root")"; then
    method="模組解析規則 (未使用 node)"
  else
    echo "錯誤：無法解析 i18n-iso-countries 的載入位置，無法驗證安裝結果。" >&2
    exit 1
  fi

  # 逐檔比對下載內容與解析結果。Immich 以 getName(code, 'en') 取國名，
  # 在地化實際是靠改寫 langs/en.json，所以驗證必須涵蓋整個 payload 而非單一語系檔。
  for src in "$STAGED_LANGS"/*.json; do
    name="$(basename "$src")"
    if ! cmp -s "$src" "$resolved/langs/$name"; then
      echo "錯誤：$name 與下載內容不一致 ($resolved/langs/$name)" >&2
      failed=1
    fi
  done

  if ! cmp -s "$STAGED_GEODATA/geodata-date.txt" "$SYSTEM_GEODATA_PATH/geodata-date.txt"; then
    echo "錯誤：geodata 內容與下載內容不一致 ($SYSTEM_GEODATA_PATH)" >&2
    failed=1
  fi

  if [ "$failed" -ne 0 ]; then
    echo "安裝結果驗證失敗，將復原為安裝前的狀態。" >&2
    echo "若安裝位置有誤，請以 IMMICH_SERVER_ROOT 指定正確的 Immich server 根目錄後重試。" >&2
    exit 1
  fi

  echo "驗證通過 ($method)：$resolved 的內容與下載資料一致。"
}
# --- 安裝結果驗證結束 ---

# --- 失敗復原 ---
# 先把失敗的目錄 rename 移開再把備份換回來，而不是直接 rm -rf。
# rename 只需要父目錄的寫入權限，即使目標本身因權限問題刪不掉也能完成復原；
# 移開後的殘骸再盡力清掉。
discard_dir() {
  [ -e "$1" ] || return 0
  chmod -R u+w "$1" 2>/dev/null || true
  rm -rf "$1" 2>/dev/null || echo "警告：無法清除 $1，請手動移除。" >&2
}

swap_in_backup() {
  local live="$1" backup="$2" stash="$1.rollback.$$.$RANDOM"

  if [ ! -e "$backup" ]; then
    echo "錯誤：復原用的備份不存在：$backup" >&2
    return 1
  fi
  if [ -e "$stash" ]; then
    echo "錯誤：復原用的暫存路徑已存在：$stash" >&2
    return 1
  fi

  # 移不開就放棄，絕不改以刪除：刪一半會連原狀都留不下來。
  if [ -e "$live" ]; then
    mv "$live" "$stash" || {
      echo "錯誤：無法移開 $live，放棄復原以免破壞現狀。" >&2
      return 1
    }
  fi

  if ! mv "$backup" "$live"; then
    echo "錯誤：無法將備份換回 $live，正在還原原狀。" >&2
    [ -e "$stash" ] && mv "$stash" "$live"
    return 1
  fi

  discard_dir "$stash"
}

restore_backups() {
  local rc=0
  case "$GEODATA_BACKUP_STATE" in
    existed)
      echo "復原 $SYSTEM_GEODATA_PATH..." >&2
      swap_in_backup "$SYSTEM_GEODATA_PATH" "$SYSTEM_GEODATA_PATH.bak" || rc=1
      ;;
    absent)
      echo "移除本次新建的 $SYSTEM_GEODATA_PATH..." >&2
      discard_dir "$SYSTEM_GEODATA_PATH"
      ;;
  esac

  if [ "$LANGS_BACKED_UP" = true ]; then
    echo "復原 $SYSTEM_I18N_PATH/langs..." >&2
    swap_in_backup "$SYSTEM_I18N_PATH/langs" "$SYSTEM_I18N_PATH/langs.bak" || rc=1
  fi
  return $rc
}
# --- 失敗復原結束 ---

# 主流程包在 main() 裡，最後一行才呼叫。
# entrypoint 常見的 `bash <(curl ...)` 是邊下載邊執行：下載中斷時 bash 會執行已收到的部分，
# 讀到 EOF 還會正常結束並回傳 0，等於做了一半卻回報成功。包成函式後，檔案沒收完就沒有呼叫，
# 也就不會執行任何步驟。
main() {
  # 解析參數
  while [[ "$#" -gt 0 ]]; do
      case $1 in
          --tag) RELEASE_TAG="$2"; shift; shift ;; # 讀取 --tag 後面的值
          --install) INSTALL_MODE=true; shift ;; # 識別 --install 參數
          --archive) ARCHIVE_FILE="$2"; shift; shift ;; # 使用本機既有的 release.tar.gz，不下載
          --print-paths) PRINT_PATHS_MODE=true; shift ;; # 只印出偵測到的安裝路徑後結束
          *) echo "未知的參數: $1"; exit 1 ;;
      esac
  done

  # 先行解析安裝路徑，讓錯誤在下載之前就浮現
  if [ "$PRINT_PATHS_MODE" = true ] || [ "$INSTALL_MODE" = true ]; then
    resolve_system_paths
  fi

  if [ "$PRINT_PATHS_MODE" = true ]; then
    echo "geodata: $SYSTEM_GEODATA_PATH"
    echo "i18n-iso-countries: $SYSTEM_I18N_PATH"
    exit 0
  fi

  # 構建下載連結和驗證 Tag (如果不是 latest)
  if [ -n "$ARCHIVE_FILE" ]; then
    if [ ! -f "$ARCHIVE_FILE" ]; then
      echo "錯誤：找不到指定的壓縮檔 '$ARCHIVE_FILE'。"
      exit 1
    fi
    DOWNLOAD_URL=""
  elif [ "$RELEASE_TAG" == "latest" ]; then
    DOWNLOAD_URL="https://github.com/RxChi1d/immich-geodata-zh-tw/releases/latest/download/release.tar.gz"
  else
    # 驗證 Tag 是否存在
    echo "正在驗證 Tag: $RELEASE_TAG ..."
    TAG_CHECK_URL="https://api.github.com/repos/RxChi1d/immich-geodata-zh-tw/releases/tags/${RELEASE_TAG}"
    HTTP_STATUS=$(curl -o /dev/null -s -w "%{http_code}" "$TAG_CHECK_URL")

    if [ "$HTTP_STATUS" -eq 404 ]; then
      echo "錯誤：找不到指定的 Release Tag '$RELEASE_TAG'。"
      echo "請確認 Tag 名稱是否正確，或使用 'latest' 來下載最新版本。"
      exit 1
    elif [ "$HTTP_STATUS" -ne 200 ]; then
      # 處理其他可能的錯誤，例如網路問題或 API rate limit
      echo "錯誤：驗證 Tag '$RELEASE_TAG' 時發生問題 (HTTP Status: $HTTP_STATUS)。"
      exit 1
    fi
    echo "Tag '$RELEASE_TAG' 驗證成功。"
    DOWNLOAD_URL="https://github.com/RxChi1d/immich-geodata-zh-tw/releases/download/${RELEASE_TAG}/release.tar.gz"
  fi

  # 根據安裝模式決定下載目錄
  if [ "$INSTALL_MODE" = true ]; then
    # 安裝模式：使用臨時目錄
    DOWNLOAD_DIR=$(mktemp -d -t immich_geodata_XXXXXX)
    echo "使用臨時目錄: $DOWNLOAD_DIR"

    # 唯一的 EXIT handler：先復原失敗的安裝，再清理臨時目錄。
    # 分成兩個 trap 會互相覆蓋，所以合併在這裡。
    cleanup() {
      local status=$?
      set +e
      if [ "$INSTALL_IN_PROGRESS" = true ] && [ "$status" -ne 0 ]; then
        echo "安裝未完成，正在復原為安裝前的狀態..." >&2
        if restore_backups; then
          echo "已復原為安裝前的狀態。" >&2
        else
          echo "警告：復原未完全成功，請檢查 $SYSTEM_GEODATA_PATH 與 $SYSTEM_I18N_PATH/langs。" >&2
        fi
      fi
      if [ -d "$DOWNLOAD_DIR" ]; then
        echo "清理臨時目錄: $DOWNLOAD_DIR"
        rm -rf "$DOWNLOAD_DIR"
      fi
      exit "$status"
    }
    trap cleanup EXIT
  else
    # 普通下載模式：使用指定目錄
    echo "使用指定目錄: $DOWNLOAD_DIR"
  
    # 確保下載目錄存在
    if [ ! -d "$DOWNLOAD_DIR" ]; then
      echo "創建下載目錄: $DOWNLOAD_DIR"
      mkdir -p "$DOWNLOAD_DIR"
    else
      GEODATA_DIR="$DOWNLOAD_DIR/geodata"
      I18N_ISO_COUNTRIES_DIR="$DOWNLOAD_DIR/i18n-iso-countries"
    
      if [ -d "$GEODATA_DIR" ]; then
        echo "清理舊版本 geodata..."
        rm -rf "$GEODATA_DIR"
      fi
    
      if [ -d "$I18N_ISO_COUNTRIES_DIR" ]; then
        echo "清理舊版本 i18n-iso-countries..."
        rm -rf "$I18N_ISO_COUNTRIES_DIR"
      fi
    fi
  fi

  # 取得壓縮檔 (-f 讓 HTTP 錯誤直接失敗，而非把錯誤頁面存成壓縮檔)
  if [ -n "$ARCHIVE_FILE" ]; then
    echo "使用本機壓縮檔: $ARCHIVE_FILE"
    cp "$ARCHIVE_FILE" "$DOWNLOAD_DIR/release.tar.gz"
  else
    echo "開始下載 release.tar.gz 從 $DOWNLOAD_URL ..."
    if ! curl -fL -o "$DOWNLOAD_DIR/release.tar.gz" "$DOWNLOAD_URL"; then
      echo "下載檔案失敗"
      exit 1
    fi
  fi

  # 解壓縮檔案
  echo "開始解壓縮 release.tar.gz..."
  if ! tar --no-same-permissions -xf "$DOWNLOAD_DIR/release.tar.gz" -C "$DOWNLOAD_DIR"; then
    echo "解壓縮檔案失敗"
    exit 1
  fi

  # 在安裝模式下，不需要特別刪除壓縮檔，因為整個臨時目錄會被清理
  # 在普通模式下，保留壓縮檔，讓用戶自行決定是否刪除

  # 如果指定了 --install，執行安裝步驟
  if [ "$INSTALL_MODE" = true ]; then
    echo "執行安裝步驟 (--install)..."
    echo "geodata 目標: $SYSTEM_GEODATA_PATH"
    echo "i18n 目標: $SYSTEM_I18N_PATH"

    STAGED_GEODATA="$DOWNLOAD_DIR/geodata"
    STAGED_LANGS="$DOWNLOAD_DIR/i18n-iso-countries/langs"

    # --- 前置檢查：在動任何系統檔案之前把來源與目標都確認完 ---
    # 檢查放在備份與覆寫之後，失敗時會留下只完成一半的安裝。
    if [ ! -d "$STAGED_GEODATA" ]; then
      echo "錯誤：下載內容缺少 geodata 資料夾，中止安裝。" >&2
      exit 1
    fi
    if [ ! -d "$STAGED_LANGS" ]; then
      echo "錯誤：下載內容缺少 i18n-iso-countries/langs 資料夾，中止安裝。" >&2
      exit 1
    fi
    # glob 沒有命中時會以字面值進入迴圈，所以先確認至少有一個檔案
    if [ "$(find "$STAGED_LANGS" -maxdepth 1 -type f -name '*.json' | wc -l)" -eq 0 ]; then
      echo "錯誤：下載內容的 langs 目錄沒有任何 json 檔，中止安裝。" >&2
      exit 1
    fi
    if [ ! -d "$SYSTEM_I18N_PATH/langs" ]; then
      echo "錯誤：$SYSTEM_I18N_PATH/langs 不存在，安裝位置可能有誤，中止安裝。" >&2
      exit 1
    fi

    # 確保 geodata 的父目錄存在
    mkdir -p "$(dirname "$SYSTEM_GEODATA_PATH")"

    # --- 備份 (供失敗時復原) ---
    echo "備份現有系統檔案..."
    if [ -d "$SYSTEM_GEODATA_PATH" ]; then
      rm -rf "$SYSTEM_GEODATA_PATH.bak"
      cp -a "$SYSTEM_GEODATA_PATH" "$SYSTEM_GEODATA_PATH.bak"
      GEODATA_BACKUP_STATE="existed"
    else
      GEODATA_BACKUP_STATE="absent"
    fi
    # 只備份 langs：套件目錄本身在 pnpm 版面下是 symlink，備份整個套件沒有意義，
    # 而我們也只會改寫 langs。
    rm -rf "$SYSTEM_I18N_PATH/langs.bak"
    cp -a "$SYSTEM_I18N_PATH/langs" "$SYSTEM_I18N_PATH/langs.bak"
    LANGS_BACKED_UP=true
    echo "備份完成。"

    INSTALL_IN_PROGRESS=true

    # 以 root 執行時統一擁有者；非 root 部署 (例如 macOS 原生 worker) 既無權限也不需要。
    # 使用數字 0:0 而非 root:root，因為 macOS 沒有名為 root 的群組。
    normalize_owner() {
      if [ "$(id -u)" = "0" ]; then
        chown -R 0:0 "$1"
      fi
    }

    # --- 更新系統檔案 ---
    echo "更新 geodata..."
    rm -rf "$SYSTEM_GEODATA_PATH"
    cp -a "$STAGED_GEODATA" "$SYSTEM_GEODATA_PATH"
    normalize_owner "$SYSTEM_GEODATA_PATH"

    # 逐檔以「複製到暫存檔再 rename」取代 overlay copy：
    #   1. rename 換掉的是目錄項目，不會跟隨目的地既有的 symlink 而寫穿到別的檔案
    #   2. 產生新的 inode，不會改動 pnpm store 中可能被硬連結共用的檔案
    #   3. 單檔替換是原子的，不會留下寫到一半的 json
    #   4. 保留上游有、payload 沒有的語系檔
    echo "更新 i18n-iso-countries langs..."
    for staged_lang in "$STAGED_LANGS"/*.json; do
      lang_name="$(basename "$staged_lang")"
      # 以 mktemp 產生暫存檔名。固定或可預測的暫存路徑一樣會被事先放置的 symlink
      # 攔截，等於換個檔名把剛修掉的寫穿問題再開一次。
      lang_tmp="$(mktemp "$SYSTEM_I18N_PATH/langs/.$lang_name.tmp.XXXXXX")"
      cp -a "$staged_lang" "$lang_tmp"
      mv -f "$lang_tmp" "$SYSTEM_I18N_PATH/langs/$lang_name"
    done
    normalize_owner "$SYSTEM_I18N_PATH/langs"
    echo "系統檔案更新完成。"

    verify_installation

    INSTALL_IN_PROGRESS=false
    echo "安裝步驟完成。"
    echo "[immich-geodata-zh-tw] 更新完成 (Tag: $RELEASE_TAG)"
    # 臨時目錄會由 trap 自動清理
  else
    echo "[immich-geodata-zh-tw] 下載完成 (Tag: $RELEASE_TAG)"
  fi

}

main "$@"
