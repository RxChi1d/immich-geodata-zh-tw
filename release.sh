#!/bin/bash

set -e

OUTPUT_DIR="output"
ZIP_FILENAME="immich-geodata-zh-tw"

############################################
# 設定時間格式和檔案名稱
############################################

CURRENT_DATE=$(date -u +"%Y-%m-%dT%H:%M:%S+00:00")
TODAY_DATE=$(date -u +"%Y-%m-%d")

############################################
# 初始化資料夾
############################################

# rm -rf $OUTPUT_DIR
# mkdir -p $OUTPUT_DIR

############################################
# 下載 geoname_data 資料
############################################

# echo "執行 prepare_geoname_data.sh..."
# bash prepare_geoname_data.sh
# if [[ $? -ne 0 ]]; then
#     echo "執行 prepare_geoname_data.sh 失敗！退出。"
#     exit 1
# fi

############################################
# 優化 geoname_data 資料
############################################

# # 修改 admin1CodesASCII.txt 檔案以符合台灣慣用格式
# echo "執行 python geodata/modify_admin1.py..."
# python geodata/modify_admin1.py
# if [[ $? -ne 0 ]]; then
#     echo "執行 python geodata/modify_admin1.py 失敗！退出。"
#     exit 1
# fi

# # 根據 extra_data/TW.txt 優化 cities500.txt
# echo "執行 python geodata/enhance_data.py..."
# python geodata/enhance_data.py
# if [[ $? -ne 0 ]]; then
#     echo "執行 python geodata/enhance_data.py 失敗！退出。"
#     exit 1
# fi

############################################
# 翻譯 geoname_data 資料
############################################

# # 利用 locationiq API 取得 metadata
# LIST=("TW")
# for item in "${LIST[@]}"; do
#     echo "執行 python geodata/generate_geodata_locationiq.py --country_code $item..."
#     python geodata/generate_geodata_locationiq.py --country_code "$item"
#     if [[ $? -ne 0 ]]; then
#         echo "執行 python geodata/generate_geodata_locationiq.py --country_code  $item 失敗！退出。"
#         exit 1
#     fi
# done

# # 翻譯 cities500_en.txt, admin1CodesASCII_en.txt, admin2Codes_en.txt
# echo "執行 python geodata/translate.py..."
# python geodata/translate.py
# if [[ $? -ne 0 ]]; then
#     echo "執行 python geodata/translate.py 失敗！退出。"
#     exit 1
# fi

############################################
# 產生 release 檔案
############################################

RELEASE_DIR="$OUTPUT_DIR/release"
GEODATA_DIR="$RELEASE_DIR/geodata"
ZIP_FILE="$ZIP_FILENAME.zip"

# release 資料夾初始化
rm -rf $RELEASE_DIR
rm -rf $OUTPUT_DIR/$ZIP_FILENAME
rm -rf $OUTPUT_DIR/$ZIP_FILE
mkdir -p $RELEASE_DIR
mkdir -p $GEODATA_DIR

echo "複製 geoname_data/ne_10m_admin_0_countries.geojson 到 geodata 資料夾..."
cp geoname_data/ne_10m_admin_0_countries.geojson $GEODATA_DIR
if [[ $? -ne 0 ]]; then
    echo "複製 geojson 檔案失敗！退出。"
    exit 1
fi

echo "複製 output/admin1CodesASCII_optimized_translated.txt 到 geodata 資料夾..."
cp output/admin1CodesASCII_optimized_translated.txt $GEODATA_DIR/admin1CodesASCII.txt
if [[ $? -ne 0 ]]; then
    echo "複製 admin1CodesASCII.txt 檔案失敗！退出。"
    exit 1
fi

echo "複製 output/admin2Codes_translated.txt 到 geodata 資料夾..."
cp output/admin2Codes_translated.txt $GEODATA_DIR/admin2Codes.txt
if [[ $? -ne 0 ]]; then
    echo "複製 admin2Codes.txt 檔案失敗！退出。"
    exit 1
fi

echo "複製 output/cities500_translated.txt 到 geodata 資料夾..."
cp output/cities500_translated.txt $GEODATA_DIR/cities500.txt
if [[ $? -ne 0 ]]; then
    echo "複製 cities500.txt 檔案失敗！退出。"
    exit 1
fi

# 建立 geodata-date.txt 檔案
echo "建立 geodata-date.txt 檔案..."
echo "$CURRENT_DATE" > $GEODATA_DIR/geodata-date.txt

# 複製 i18n-iso-countries 到 release 資料夾
echo "複製 i18n-iso-countries 到 release 資料夾..."
cp -r i18n-iso-countries $RELEASE_DIR

# 打包 release 資料夾
cd $OUTPUT_DIR
echo "打包 release 資料夾為 $ZIP_FILE..."
mv release $ZIP_FILENAME
zip -r "$ZIP_FILE" $ZIP_FILENAME
mv $ZIP_FILENAME release
cd ..
if [[ $? -ne 0 ]]; then
    echo "打包檔案失敗！退出。"
    exit 1
fi

echo "腳本執行完成！打包檔案：$ZIP_FILE"