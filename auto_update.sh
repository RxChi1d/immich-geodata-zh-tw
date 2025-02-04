#!/bin/bash

set -e

# 定義目標資料夾
# geodata 和 i18n-iso-countries 會被放在這個資料夾下
DOWNLOAD_DIR="/root/immich-geodata-zh-tw"

if [ ! -d "$DOWNLOAD_DIR" ]; then
    mkdir -p $DOWNLOAD_DIR
fi

# 移除舊版本
DOWNLOAD_GEODATA_DIR="$DOWNLOAD_DIR/geodata"
DOWNLOAD_I18N_ISO_COUNTRIES_DIR="$DOWNLOAD_DIR/i18n-iso-countries"

if [ -d "$DOWNLOAD_GEODATA_DIR" ]; then
    echo "刪除舊版本 geodata..."
    rm -rf $DOWNLOAD_GEODATA_DIR
fi

if [ -d "$DOWNLOAD_I18N_ISO_COUNTRIES_DIR" ]; then
    echo "刪除舊版本 i18n-iso-countries..."
    rm -rf $DOWNLOAD_I18N_ISO_COUNTRIES_DIR
fi

# 下載檔案
echo "開始下載 release.tar.gz..."
curl -L -o $DOWNLOAD_DIR/release.tar.gz "https://github.com/RxChi1d/immich-geodata-zh-tw/releases/latest/download/release.tar.gz"

if [ $? -ne 0 ]; then
    echo "下載檔案失敗"
    exit 1
fi

# 解壓縮檔案
echo "開始解壓縮 release.tar.gz..."
tar -xvf $DOWNLOAD_DIR/release.tar.gz -C $DOWNLOAD_DIR

# 複製檔案前先備份舊檔案
echo "備份 geodata..."
cp -rf /build/geodata /build/geodata.bak

echo "備份 i18n-iso-countries..."
cp -rf /usr/src/app/node_modules/i18n-iso-countries /usr/src/app/node_modules/i18n-iso-countries.bak

# 複製檔案
echo "複製 geodata..."
cp -rf $DOWNLOAD_DIR/geodata /build

echo "複製 i18n-iso-countries..."
cp -rf $DOWNLOAD_DIR/i18n-iso-countries /usr/src/app/node_modules

# 刪除檔案/資料夾
echo "刪除 release.zip..."
rm -rf $DOWNLOAD_DIR/release.tar.gz

echo "更新完成"
