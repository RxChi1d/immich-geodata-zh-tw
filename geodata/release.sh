#!/bin/bash

set -e

# 设置时间格式和文件名
CURRENT_DATE=$(date -u +"%Y-%m-%dT%H:%M:%S+00:00")
TODAY_DATE=$(date -u +"%Y-%m-%d")

# 运行 prepare_geoname_data.sh
echo "运行 prepare_geoname_data.sh..."
bash prepare_geoname_data.sh
if [[ $? -ne 0 ]]; then
    echo "运行 prepare_geoname_data.sh 失败！退出。"
    exit 1
fi

# 运行 python enhance_data.py
echo "运行 python enhance_data.py..."
python enhance_data.py
if [[ $? -ne 0 ]]; then
    echo "运行 python enhance_data.py 失败！退出。"
    exit 1
fi

# 运行 python generate_geodata_amap.py
echo "运行 python generate_geodata_amap.py..."
python generate_geodata_amap.py
if [[ $? -ne 0 ]]; then
    echo "运行 python generate_geodata_amap.py 失败！退出。"
    exit 1
fi

# 准备列表并运行 generate_geodata_locationiq.py
LIST=("JP")
for item in "${LIST[@]}"; do
    echo "运行 python generate_geodata_locationiq.py $item..."
    python generate_geodata_locationiq.py "$item"
    if [[ $? -ne 0 ]]; then
        echo "运行 python generate_geodata_locationiq.py $item 失败！退出。"
        exit 1
    fi
done

# 运行 python translate.py
echo "运行 python translate.py..."
python translate.py
if [[ $? -ne 0 ]]; then
    echo "运行 python translate.py 失败！退出。"
    exit 1
fi

# 复制 geojson 文件到 output 文件夹
echo "复制 geoname_data/ne_10m_admin_0_countries.geojson 到 output 文件夹..."
mkdir -p output
cp geoname_data/ne_10m_admin_0_countries.geojson output/
if [[ $? -ne 0 ]]; then
    echo "复制 geojson 文件失败！退出。"
    exit 1
fi

# 创建 geodata-date.txt 文件
echo "创建 geodata-date.txt 文件..."
echo "$CURRENT_DATE" > output/geodata-date.txt

# 打包 output 文件夹
ZIP_FILE="geodata.zip"
echo "打包 output 文件夹为 $ZIP_FILE..."
mv output geodata
zip -r "$ZIP_FILE" geodata/
mv geodata output
if [[ $? -ne 0 ]]; then
    echo "打包文件失败！退出。"
    exit 1
fi

echo "脚本执行完成！打包文件：$ZIP_FILE"