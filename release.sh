#!/bin/bash

set -e

OUTPUT_DIR="output"

############################################
# 設定時間格式和檔案名稱
############################################

CURRENT_DATE=$(date -u +"%Y-%m-%dT%H:%M:%S+00:00")
TODAY_DATE=$(date -u +"%Y-%m-%d")

############################################
# 下載 geoname_data 資料
############################################

echo "執行 geodata/prepare_geoname_data.py..."
python geodata/prepare_geoname_data.py
if [[ $? -ne 0 ]]; then
    echo "執行 geodata/prepare_geoname_data.py 失敗！退出。"
    exit 1
fi

############################################
# 初始化資料夾
############################################

rm -rf $OUTPUT_DIR
mkdir -p $OUTPUT_DIR

############################################
# 優化 geoname_data 資料
############################################

# 修改 admin1CodesASCII.txt 檔案以符合臺灣慣用格式
echo "執行 python geodata/modify_admin1.py..."
python geodata/modify_admin1.py
if [[ $? -ne 0 ]]; then
    echo "執行 python geodata/modify_admin1.py 失敗！退出。"
    exit 1
fi

# 根據 extra_data/TW.txt 優化 cities500.txt
echo "執行 python geodata/enhance_data.py..."
python geodata/enhance_data.py
if [[ $? -ne 0 ]]; then
    echo "執行 python geodata/enhance_data.py 失敗！退出。"
    exit 1
fi

############################################
# 翻譯 geoname_data 資料
############################################

# 利用 locationiq API 取得 metadata
LIST=("TW")
for item in "${LIST[@]}"; do
    echo "執行 python geodata/generate_geodata_locationiq.py --country_code $item..."
    python geodata/generate_geodata_locationiq.py --country_code "$item"
    if [[ $? -ne 0 ]]; then
        echo "執行 python geodata/generate_geodata_locationiq.py --country_code  $item 失敗！退出。"
        exit 1
    fi
done

# 翻譯 cities500_en.txt, admin1CodesASCII_en.txt, admin2Codes_en.txt
echo "執行 python geodata/translate.py..."
python geodata/translate.py
if [[ $? -ne 0 ]]; then
    echo "執行 python geodata/translate.py 失敗！退出。"
    exit 1
fi

############################################
# 產生 release 檔案
############################################

echo "執行 python geodata/generate_release.py..."
python geodata/generate_release.py
if [[ $? -ne 0 ]]; then
    echo "執行 python geodata/generate_release.py 失敗！退出。"
    exit 1
fi