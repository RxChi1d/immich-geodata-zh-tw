#!/bin/bash

set -e

# 預設 release tag
RELEASE_TAG="latest"

# 解析參數
while [[ "$#" -gt 0 ]]; do
    case $1 in
        --tag) RELEASE_TAG="$2"; shift; shift ;; # 讀取 --tag 後面的值
        *) echo "未知的參數: $1"; exit 1 ;;
    esac
done

# 構建下載連結和驗證 Tag (如果不是 latest)
if [ "$RELEASE_TAG" == "latest" ]; then
  DOWNLOAD_URL="https://github.com/RxChi1d/immich-geodata-zh-tw/releases/latest/download/release.tar.gz"
else
  # 驗證 Tag 是否存在
  # echo "正在驗證 Tag: $RELEASE_TAG ..."
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
  # echo "Tag '$RELEASE_TAG' 驗證成功。"
  DOWNLOAD_URL="https://github.com/RxChi1d/immich-geodata-zh-tw/releases/download/${RELEASE_TAG}/release.tar.gz"
fi

# 定義目標資料夾
# geodata 和 i18n-iso-countries 會被放在這個資料夾下
TARGET_DIR="./temp"
# TARGET_DIR="/mnt/user/appdata/immich"

if [ ! -d "$TARGET_DIR" ]; then
  echo "目標資料夾 ($TARGET_DIR) 不存在，請確認路徑是否正確"
  exit 1
fi

# 移除舊版本
GEODATA_DIR="$TARGET_DIR/geodata"
I18N_ISO_COUNTRIES_DIR="$TARGET_DIR/i18n-iso-countries"

if [ -d "$GEODATA_DIR" ]; then
  echo "刪除舊版本 geodata..."
  rm -rf $GEODATA_DIR
fi

if [ -d "$I18N_ISO_COUNTRIES_DIR" ]; then
  echo "刪除舊版本 i18n-iso-countries..."
  rm -rf $I18N_ISO_COUNTRIES_DIR
fi

# 下載檔案
echo "開始下載 release.tar.gz 從 $DOWNLOAD_URL ..."
curl -L -o $TARGET_DIR/release.tar.gz "$DOWNLOAD_URL"

if [ $? -ne 0 ]; then
  echo "下載檔案失敗"
  exit 1
fi

# 解壓縮檔案
echo "開始解壓縮 release.tar.gz..."
tar -xvf $TARGET_DIR/release.tar.gz -C $TARGET_DIR

# 刪除檔案/資料夾
echo "刪除 release.tar.gz..."
rm -rf $TARGET_DIR/release.tar.gz

echo "更新完成"
