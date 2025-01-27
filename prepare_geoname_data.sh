#!/bin/bash

set -e

# 定義目標資料夾和URL
TARGET_DIR="geoname_data"
ZIP_FILE="$TARGET_DIR/cities500.zip"
TXT_FILE="$TARGET_DIR/cities500.txt"
ADMIN1_FILE="$TARGET_DIR/admin1CodesASCII.txt"
ADMIN2_FILE="$TARGET_DIR/admin2Codes.txt"
GEOJESON_FILE="$TARGET_DIR/ne_10m_admin_0_countries.geojson"
EXTRA_DATA_DIR="$TARGET_DIR/extra_data"
DOWNLOAD_URL="https://download.geonames.org/export/dump/cities500.zip"

if [[ "$1" == "--update" ]]; then
    if [[ -d "$TARGET_DIR" ]]; then
        echo "偵測到 --update 參數，刪除資料夾 $TARGET_DIR ..."
        rm -rf "$TARGET_DIR"
        echo "資料夾 $TARGET_DIR 已刪除。"
    else
        echo "資料夾 $TARGET_DIR 不存在，無需刪除。"
    fi
    # exit 0
fi

# 建立目標資料夾
mkdir -p "$TARGET_DIR"

# 下載檔案
echo "正在下載 $DOWNLOAD_URL 到 $ZIP_FILE..."
wget -O "$ZIP_FILE" "$DOWNLOAD_URL"

# 檢查下載是否成功
if [[ $? -ne 0 ]]; then
    echo "下載失敗，請檢查網路連線或URL是否正確。"
    exit 1
fi

mkdir -p "$EXTRA_DATA_DIR"

LIST=("TW")
for item in "${LIST[@]}"; do
    echo "下載額外資料 $item..."
    wget -O "$EXTRA_DATA_DIR/$item.zip" "https://download.geonames.org/export/dump/$item.zip"
    if [[ $? -ne 0 ]]; then
        echo "下載 $item.zip 失敗！退出。"
        exit 1
    fi
    unzip -o "$EXTRA_DATA_DIR/$item.zip" -d "$EXTRA_DATA_DIR"
    
    # 移動解壓後的檔案
    if [[ -f "$EXTRA_DATA_DIR/$item.txt" ]]; then
        echo "解壓 $item.zip 成功"
    else
        echo "未找到 $item.txt，請檢查解壓結果。"
        exit 1
    fi
    
    # 刪除中間檔案
    rm -f "$EXTRA_DATA_DIR/$item.zip"
done

rm -f "$EXTRA_DATA_DIR/readme.txt"

echo "正在下載 $ADMIN1_FILE..."
wget -O "$ADMIN1_FILE" "https://download.geonames.org/export/dump/admin1CodesASCII.txt"

# 檢查下載是否成功
if [[ $? -ne 0 ]]; then
    echo "下載失敗，請檢查網路連線或URL是否正確。"
    exit 1
fi

# 下載檔案
echo "正在下載 $ADMIN2_FILE..."
wget -O "$ADMIN2_FILE" "https://download.geonames.org/export/dump/admin2Codes.txt"

# 檢查下載是否成功
if [[ $? -ne 0 ]]; then
    echo "下載失敗，請檢查網路連線或URL是否正確。"
    exit 1
fi

# 下載檔案
echo "正在下載 $GEOJESON_FILE..."
wget -O "$GEOJESON_FILE" "https://raw.githubusercontent.com/nvkelso/natural-earth-vector/master/geojson/ne_10m_admin_0_countries.geojson"

# 檢查下載是否成功
if [[ $? -ne 0 ]]; then
    echo "下載失敗，請檢查網路連線或URL是否正確。"
    exit 1
fi

echo "下載成功，解壓縮中..."

# 解壓縮檔案到目標目錄
unzip -o "$ZIP_FILE" -d "$TARGET_DIR"

# 移動解壓後的檔案
if [[ -f "$TARGET_DIR/cities500.txt" ]]; then
    echo "解壓成功，將 cities500.txt 移動到目標目錄。"
else
    echo "未找到 cities500.txt，請檢查解壓結果。"
    exit 1
fi

# 刪除中間檔案
echo "清理中間檔案..."
rm -f "$ZIP_FILE"

echo "操作完成！cities500.txt 已放置於 $TARGET_DIR 目錄下。"

# 設定下載的zip檔案名稱和目標txt檔案名稱
ZIP_FILE="$TARGET_DIR/alternateNamesV2.zip"
TXT_FILE="$TARGET_DIR/alternateNamesV2.txt"
DOWNLOAD_URL="https://download.geonames.org/export/dump/alternateNamesV2.zip" # 替換為實際的下載連結

# 檢查是否已存在目標txt檔案
if [[ -f "$TXT_FILE" ]]; then
    echo "目標檔案 $TXT_FILE 已存在，不需要下載。"
    exit 0
fi

# 如果zip檔案存在，則直接解壓
if [[ -f "$ZIP_FILE" ]]; then
    echo "$ZIP_FILE 已存在"
else
    # 下載zip檔案
    echo "正在下載 $ZIP_FILE ..."
    wget -O "$ZIP_FILE" "$DOWNLOAD_URL"
    if [[ $? -ne 0 ]]; then
        echo "下載失敗，退出。"
        exit 1
    fi
fi

# 解壓zip檔案
echo "正在解壓 $ZIP_FILE ..."
unzip -o "$ZIP_FILE" -d "$TARGET_DIR"
if [[ $? -eq 0 ]]; then
    echo "解壓完成，刪除 $ZIP_FILE ..."
    rm -f "$ZIP_FILE"
else
    echo "解壓失敗。"
    exit 1
fi
