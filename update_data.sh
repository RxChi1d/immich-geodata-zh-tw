#!/bin/bash

set -e

# 定義目標資料夾
# geodata 和 i18n-iso-countries 會被放在這個資料夾下
TARGET_DIR="/mnt/user/appdata/immich"

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
echo "開始下載 release.tar.gz..."
curl -L -o $TARGET_DIR/release.tar.gz "https://github.com/RxChi1d/immich-geodata-zh-tw/releases/latest/download/release.tar.gz"

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
