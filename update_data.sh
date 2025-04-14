#!/bin/bash

# 這個腳本用於下載和安裝最新的 geodata 和 i18n-iso-countries 資料夾
# 下載的檔案會被解壓縮到指定的目錄 (DOWNLOAD_DIR)
# 如果指定了 --install 參數，則會將檔案安裝到系統目錄 (僅限於 Docker 環境)

set -e

# 用戶可修改的配置
DOWNLOAD_DIR="./temp" # 普通模式下的下載目錄

# 預設值
RELEASE_TAG="latest"
INSTALL_MODE=false

# 解析參數
while [[ "$#" -gt 0 ]]; do
    case $1 in
        --tag) RELEASE_TAG="$2"; shift; shift ;; # 讀取 --tag 後面的值
        --install) INSTALL_MODE=true; shift ;; # 識別 --install 參數
        *) echo "未知的參數: $1"; exit 1 ;;
    esac
done

# 構建下載連結和驗證 Tag (如果不是 latest)
if [ "$RELEASE_TAG" == "latest" ]; then
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
  
  # 註冊清理函數，在腳本結束時自動刪除臨時目錄
  cleanup() {
    if [ -d "$DOWNLOAD_DIR" ]; then
      echo "清理臨時目錄: $DOWNLOAD_DIR"
      rm -rf "$DOWNLOAD_DIR"
    fi
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

# 下載檔案
echo "開始下載 release.tar.gz 從 $DOWNLOAD_URL ..."
curl -L -o "$DOWNLOAD_DIR/release.tar.gz" "$DOWNLOAD_URL"

if [ $? -ne 0 ]; then
  echo "下載檔案失敗"
  exit 1
fi

# 解壓縮檔案
echo "開始解壓縮 release.tar.gz..."
tar -xvf "$DOWNLOAD_DIR/release.tar.gz" -C "$DOWNLOAD_DIR" --no-same-permissions || echo "解壓縮過程中出現權限警告，但將繼續執行..."


# 在安裝模式下，不需要特別刪除壓縮檔，因為整個臨時目錄會被清理
# 在普通模式下，保留壓縮檔，讓用戶自行決定是否刪除

# 如果指定了 --install，執行安裝步驟
if [ "$INSTALL_MODE" = true ]; then
  echo "執行安裝步驟 (--install)..."

  # 定義系統目標路徑
  SYSTEM_GEODATA_PATH="/build/geodata"
  SYSTEM_I18N_PATH="/usr/src/app/node_modules/i18n-iso-countries"
  SYSTEM_I18N_MODULE_PATH="/usr/src/app/node_modules" # i18n 的父目錄

  # 確保目標父目錄存在
  echo "確保目標系統目錄存在..."
  mkdir -p /build
  mkdir -p "$SYSTEM_I18N_MODULE_PATH"

  # --- 先備份系統路徑 ---
  echo "備份現有系統檔案..."
  if [ -d "$SYSTEM_GEODATA_PATH" ]; then
    echo "備份 $SYSTEM_GEODATA_PATH 到 $SYSTEM_GEODATA_PATH.bak..."
    rm -rf "$SYSTEM_GEODATA_PATH.bak" # 先移除舊備份
    cp -a "$SYSTEM_GEODATA_PATH" "$SYSTEM_GEODATA_PATH.bak"
  fi
  if [ -d "$SYSTEM_I18N_PATH" ]; then
    echo "備份 $SYSTEM_I18N_PATH 到 $SYSTEM_I18N_PATH.bak..."
    rm -rf "$SYSTEM_I18N_PATH.bak" # 先移除舊備份
    cp -a "$SYSTEM_I18N_PATH" "$SYSTEM_I18N_PATH.bak"
  fi
  echo "備份完成。"
  # --- 備份結束 ---

  # --- 更新系統檔案 ---
  echo "更新系統檔案..."
  # 檢查來源是否存在
  if [ -d "$DOWNLOAD_DIR/geodata" ]; then
    echo "更新 geodata..."
    rm -rf "$SYSTEM_GEODATA_PATH"
    cp -a "$DOWNLOAD_DIR/geodata" "$SYSTEM_GEODATA_PATH"
    # 確保複製後的檔案擁有者為 root
    echo "設定 geodata 擁有者為 root..."
    chown -R root:root "$SYSTEM_GEODATA_PATH"
  else
    echo "錯誤：geodata 資料夾不存在，無法完成更新。"
  fi

  if [ -d "$DOWNLOAD_DIR/i18n-iso-countries" ]; then
    echo "更新 i18n-iso-countries..."
    # 確保目標模組目錄存在 (它應該由基礎映像檔安裝好)
    mkdir -p "$SYSTEM_I18N_PATH"
    # 複製下載目錄的 *內容* 到目標目錄，覆蓋現有檔案
    # 注意來源路徑結尾的 /. 表示複製內容而非目錄本身
    cp -a "$DOWNLOAD_DIR/i18n-iso-countries/." "$SYSTEM_I18N_PATH/"
    # 確保複製後的檔案擁有者為 root
    echo "設定 i18n-iso-countries 擁有者為 root..."
    chown -R root:root "$SYSTEM_I18N_PATH"
  else
    echo "錯誤：i18n-iso-countries 資料夾不存在，無法完成更新。"
  fi
  echo "系統檔案更新完成。"
  # --- 更新結束 ---

  echo "安裝步驟完成。"
  echo "更新完成 (Tag: $RELEASE_TAG)"
  # 臨時目錄會由 trap 自動清理
else
  echo "下載完成 (Tag: $RELEASE_TAG)"
fi
