import logging
import os
import csv
import json
import sys
import pandas as pd
from tqdm import tqdm


class TqdmLoggingHandler(logging.StreamHandler):
    def emit(self, record):
        try:
            msg = self.format(record)
            tqdm.write(msg)
            self.flush()
        except Exception:
            self.handleError(record)


logger = logging.getLogger("logger")
logger.setLevel(os.environ.get("LOG_LEVEL", "INFO"))  # 設置最低日誌級別

console_handler = TqdmLoggingHandler()

# 設置日誌格式
formatter = logging.Formatter("%(asctime)s - %(name)s - %(levelname)s - %(message)s")
console_handler.setFormatter(formatter)

# 添加處理器到 logger
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

CITIES_HEADER = [
    "geonameid",
    "name",
    "asciiname",
    "alternatenames",
    "latitude",
    "longitude",
    "feature_class",
    "feature_code",
    "country_code",
    "cc2",
    "admin1_code",
    "admin2_code",
    "admin3_code",
    "admin4_code",
    "population",
    "elevation",
    "dem",
    "timezone",
    "modification_date",
]

MUNICIPALITIES = [
    "臺北市",
    "新北市",
    "桃園市",
    "臺中市",
    "臺南市",
    "高雄市",
    "基隆市",
    "新竹市",
    "嘉義市",
]


def load_meta_data(file_path):
    """
    從指定的 CSV 檔案中載入地理數據，將 (longitude, latitude) 作為鍵，
    其他欄位組成的字典作為值，返回一個字典。

    :param file_path: CSV 檔案路徑
    :return: 包含地理數據的字典
    """
    result = {}

    # 確認檔案是否存在
    if os.path.exists(file_path):
        with open(file_path, mode="r", encoding="utf-8") as file:
            reader = csv.DictReader(file)

            # 逐行讀取資料並構建字典
            for row in reader:
                # 使用 (longitude, latitude) 作為鍵
                key = (row["longitude"].strip(), row["latitude"].strip())

                # 組合其他相關欄位作為值
                result[key] = {
                    "country": row["country"].strip(),
                    "admin_1": row["admin_1"].strip(),
                    "admin_2": row["admin_2"].strip(),
                    "admin_3": row["admin_3"].strip(),
                    "admin_4": row["admin_4"].strip(),
                }

    return result


def ensure_folder_exists(file_path):
    folder = os.path.dirname(file_path)
    if folder:
        os.makedirs(folder, exist_ok=True)


def create_alternate_map(alternate_file, output_folder):
    logger.info(f"正在從 {alternate_file} 建立替代名稱對照表")

    priority = ["zh-Hant", "zh-TW", "zh-HK", "zh", "zh-Hans", "zh-CN", "zh-SG"]

    # 使用pandas讀取檔案
    # 需要處理NaN值，因為lang可能為空
    # 如果lang欄位包含priority中的任何一個值，就保留
    data = pd.read_csv(
        alternate_file,
        sep="\t",
        header=None,
        usecols=[1, 2, 3, 4],
        names=["geonameid", "lang", "name", "is_preferred_name"],
        na_values=["\\N"],
    )
    data = data.dropna(subset=["lang"])  # 丟棄lang為空的項目
    data = data[data["lang"].isin(priority)]  # 僅保留中文名稱

    # 創建priority，作為優先級判斷
    # 如果is_preferred_name為1，則優先級為0
    # 如果is_preferred_name為0，則優先級為priority中的index+1
    data["is_preferred_name"] = data["is_preferred_name"].fillna(0)
    data["priority"] = data["lang"].map(lambda x: priority.index(x) + 1)
    data["priority"] = data.apply(
        lambda row: (
            0 if row["is_preferred_name"] == 1 else priority.index(row["lang"]) + 1
        ),
        axis=1,
    )

    # 相同的geonameid，僅保留優先級最高的（數字越小越高，0為最高）
    data = data.sort_values("priority")
    data = data.drop_duplicates(subset="geonameid", keep="first")

    # 轉換成字典，geonameid為key，name為value
    mapping = dict(zip(data["geonameid"], data["name"]))

    # 更新地名
    update_name = {
        "桃園縣": "桃園市",
    }
    for key, value in mapping.items():
        for k, v in update_name.items():
            if k in value:
                mapping[key] = value.replace(k, v)

    output_file = os.path.join(output_folder, "alternate_chinese_name.json")
    ensure_folder_exists(output_file)

    with open(output_file, mode="w", encoding="utf-8") as file:
        json.dump(mapping, file, ensure_ascii=False, indent=4)

    logger.info(f"替代名稱對照表已儲存至 {output_file}")


def load_alternate_names(file_path):
    if not os.path.exists(file_path):
        logger.info(f"替代名稱檔案 {file_path} 不存在")

        alternate_file = "./geoname_data/alternateNamesV2.txt"

        if not os.path.exists(alternate_file):
            logger.error(f"替代名稱檔案 {alternate_file} 不存在")
            sys.exit(1)

        create_alternate_map(alternate_file, os.path.dirname(file_path))
        
        return load_alternate_names(file_path)
    else:
        with open(file_path, mode="r", encoding="utf-8") as file:
            data = json.load(file)

            logger.info(f"已從 {file_path} 載入替代名稱對照表")

            return data


if __name__ == "__main__":
    pass
