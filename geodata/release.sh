#!/bin/bash

set -e

# 設定時間格式和檔案名稱
CURRENT_DATE=$(date -u +"%Y-%m-%dT%H:%M:%S+00:00")
TODAY_DATE=$(date -u +"%Y-%m-%d")

# 下載 geoname_data 資料
echo "執行 prepare_geoname_data.sh..."
bash prepare_geoname_data.sh
if [[ $? -ne 0 ]]; then
    echo "執行 prepare_geoname_data.sh 失敗！退出。"
    exit 1
fi

# 修改 admin1CodesASCII.txt 檔案以符合台灣慣用格式
echo "執行 python modify_admin1.py..."
python modify_admin1.py
if [[ $? -ne 0 ]]; then
    echo "執行 python modify_admin1.py 失敗！退出。"
    exit 1
fi

# 根據 extra_data/TW.txt 優化 cities500.txt
echo "執行 python enhance_data.py..."
python enhance_data.py
if [[ $? -ne 0 ]]; then
    echo "執行 python enhance_data.py 失敗！退出。"
    exit 1
fi

# 利用 locationiq API 取得 metadata
LIST=("TW", "JP")
for item in "${LIST[@]}"; do
    echo "執行 python generate_geodata_locationiq.py --country_code $item..."
    python generate_geodata_locationiq.py --country_code "$item"
    if [[ $? -ne 0 ]]; then
        echo "執行 python generate_geodata_locationiq.py $item 失敗！退出。"
        exit 1
    fi
done

# 翻譯 cities500_en.txt, admin1CodesASCII_en.txt, admin2Codes_en.txt
echo "執行 python translate.py..."
python translate.py
if [[ $? -ne 0 ]]; then
    echo "執行 python translate.py 失敗！退出。"
    exit 1
fi

# 複製 geojson 檔案到 output 資料夾
echo "複製 geoname_data/ne_10m_admin_0_countries.geojson 到 output 資料夾..."
mkdir -p output
cp geoname_data/ne_10m_admin_0_countries.geojson output/
if [[ $? -ne 0 ]]; then
    echo "複製 geojson 檔案失敗！退出。"
    exit 1
fi

# 建立 geodata-date.txt 檔案
echo "建立 geodata-date.txt 檔案..."
echo "$CURRENT_DATE" > output/geodata-date.txt

# 打包 output 資料夾
ZIP_FILE="geodata.zip"
echo "打包 output 資料夾為 $ZIP_FILE..."
mv output geodata
zip -r "$ZIP_FILE" geodata/
mv geodata output
if [[ $? -ne 0 ]]; then
    echo "打包檔案失敗！退出。"
    exit 1
fi

echo "腳本執行完成！打包檔案：$ZIP_FILE"