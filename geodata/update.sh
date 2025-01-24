#!/bin/bash

# 启用调试模式
set -e

cd "$(dirname "$0")"

# 获取当前脚本文件名
script_name=$(basename "$0")

echo "开始获取 ZingLix/immich-geodata-cn 仓库最新的 geodata.zip 文件..."

# 获取最新 release 的 API URL
LATEST_RELEASE_API="https://api.github.com/repos/ZingLix/immich-geodata-cn/releases/latest"

echo "调用 GitHub API 获取最新 release 信息..."
response=$(curl -s $LATEST_RELEASE_API)
if [[ $? -ne 0 ]]; then
  echo "错误：无法获取最新 release 信息，请检查网络连接。"
  exit 1
fi

# 从返回的 JSON 中提取 geodata.zip 的下载 URL
zip_url=$(echo "$response" | grep -oP '"browser_download_url":\s*"\K(.*geodata\.zip)(?=")')
if [[ -z $zip_url ]]; then
  echo "错误：未找到 geodata.zip 的下载链接，请检查仓库的最新 release。"
  exit 1
fi

echo "最新的 geodata.zip 文件下载地址为：$zip_url"

# 下载 geodata.zip
echo "开始下载 geodata.zip 文件..."
curl -L -o geodata.zip "$zip_url"
if [[ $? -ne 0 ]]; then
  echo "错误：geodata.zip 下载失败，请检查网络连接。"
  exit 1
fi

echo "geodata.zip 下载成功！"

# 解压 geodata.zip 文件
echo "开始解压 geodata.zip 文件..."
unzip geodata.zip -d temp_geodata
if [[ $? -ne 0 ]]; then
  echo "错误：解压 geodata.zip 失败，请检查文件完整性。"
  exit 1
fi

echo "解压成功，文件已存放在临时目录 temp_geodata/ 中。"

# 将解压内容移动到当前目录，排除当前脚本文件
echo "将解压内容移动到当前目录，排除脚本文件 ($script_name)..."
for file in temp_geodata/geodata/*; do
  if [[ $(basename "$file") == "$script_name" ]]; then
    echo "跳过脚本文件：$file"
    continue
  fi
  mv "$file" ./
done

# 删除临时文件和文件夹
echo "清理中间文件..."
rm -rf geodata.zip temp_geodata
if [[ $? -ne 0 ]]; then
  echo "警告：中间文件删除失败，请手动检查并删除。"
else
  echo "中间文件已成功删除。"
fi

echo "操作完成，所有 geodata 文件已放置到当前目录！请重启 Immich！"

# 替换成你的重启 Immich 的命令，即可更新完数据后自动重启 Immich
# docker restart immich_server
