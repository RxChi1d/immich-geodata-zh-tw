import logging
import os
import csv
import json
import sys
import pandas as pd


logger = logging.getLogger("logger")
logger.setLevel(os.environ.get("LOG_LEVEL", "INFO"))  # 设置最低日志级别

console_handler = logging.StreamHandler()

# 设置日志格式
formatter = logging.Formatter("%(asctime)s - %(name)s - %(levelname)s - %(message)s")
console_handler.setFormatter(formatter)

# 添加处理器到 logger
logger.addHandler(console_handler)

GEODATA_HEADER = [
    "longitude",
    "latitude",
    "country",
    "admin_1",
    "admin_2",
    "admin_3",
    "admin_4",
]


def load_geo_data(file_path):
    result = {}
    if os.path.exists(file_path):
        # 读取 CSV 文件
        with open(file_path, mode="r", encoding="utf-8") as file:
            reader = csv.DictReader(file)

            # 遍历每行数据
            for row in reader:
                # 获取 (lon, lat) 作为键
                key = (str(row["longitude"]), str(row["latitude"]))

                # 将其他字段作为值存入字典
                value = {
                    "country": row["country"],
                    "admin_1": row["admin_1"],
                    "admin_2": row["admin_2"],
                    "admin_3": row["admin_3"],
                    "admin_4": row["admin_4"],
                }

                result[key] = value

    return result


def ensure_folder_exists(file_path):
    folder = os.path.dirname(file_path)
    if folder:
        os.makedirs(folder, exist_ok=True)


def create_alternate_map(alternate_file, output_folder):
    logger.info(f"Creating alternate name mapping from {alternate_file}")

    priority = ["zh-Hant", "zh-TW", "zh-HK", "zh", "zh-Hans", "zh-CN", "zh-SG"]

    # 使用pandas讀取檔案
    # 需要處理NaN值，因為lang可能為空
    # 如果lang欄位包含priority中的任何一個值，就保留
    data = pd.read_csv(
        alternate_file,
        sep="\t",
        header=None,
        usecols=[1, 2, 3],
        names=["geonameid", "lang", "name"],
        na_values=["\\N"],
    )
    data = data.dropna(subset=["lang"])
    data = data[data["lang"].isin(priority)]

    # 相同的geonameid，只保留lang欄位優先級最高的
    data = data.sort_values("lang", key=lambda x: x.map(lambda x: priority.index(x)))
    data = data.drop_duplicates(subset="geonameid", keep="first")

    # 轉換成字典，geonameid為key，name為value
    mapping = dict(zip(data["geonameid"], data["name"]))

    output_file = os.path.join(output_folder, "alternate_chinese_name.json")
    ensure_folder_exists(output_file)

    with open(output_file, mode="w", encoding="utf-8") as file:
        json.dump(mapping, file, ensure_ascii=False, indent=4)

    logger.info(f"Alternate name mapping saved to {output_file}")

    return mapping


def load_alternate_name(file_path):
    if not os.path.exists(file_path):
        logger.info(f"Alternate file {file_path} does not exist")

        alternate_file = "./geoname_data/alternateNamesV2.txt"

        if not os.path.exists(alternate_file):
            logger.error(f"The alternate file {alternate_file} does not exist")
            sys.exit(1)

        return create_alternate_map(alternate_file, os.path.dirname(file_path))
    else:
        with open(file_path, mode="r", encoding="utf-8") as file:
            data = json.load(file)

            logger.info(f"Alternate name mapping loaded from {file_path}")

            return data


if __name__ == "__main__":
    pass
