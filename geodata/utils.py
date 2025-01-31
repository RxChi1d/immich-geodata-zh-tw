import logging
import os
import csv
import sys
import pandas as pd
from tqdm import tqdm
import polars as pl


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

ADMIN1_SCHEMA = pl.Schema(
    {
        "ID": pl.String(),
        "Name": pl.String(),
        "Name_ASCII": pl.String(),
        "Geoname_ID": pl.String(),
    }
)

GEODATA_SCHEMA = pl.Schema(
    {
        "longitude": pl.Float32,
        "latitude": pl.Float32,
        "country": pl.String,
        "admin_1": pl.String,
        "admin_2": pl.String,
        "admin_3": pl.String,
        "admin_4": pl.String,
    }
)

CITIES_SCHEMA = pl.Schema(
    {
        "geoname_id": pl.String,
        "name": pl.String,
        "asciiname": pl.String,
        "alternatenames": pl.String,
        "latitude": pl.Float32,
        "longitude": pl.Float32,
        "feature_class": pl.String,
        "feature_code": pl.String,
        "country_code": pl.String,
        "cc2": pl.String,
        "admin1_code": pl.String,
        "admin2_code": pl.String,
        "admin3_code": pl.String,
        "admin4_code": pl.String,
        "population": pl.UInt32,
        "elevation": pl.String,
        "dem": pl.Int32,
        "timezone": pl.String,
        "modification_date": pl.Date,
    }
)

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

CHINESE_PRIORITY = ["zh-Hant", "zh-TW", "zh-HK", "zh", "zh-Hans", "zh-CN", "zh-SG"]


def ensure_folder_exists(file_path):
    folder = os.path.dirname(file_path)
    if folder:
        os.makedirs(folder, exist_ok=True)


def create_alternate_map(alternate_file, output_path):
    logger.info(f"正在從 {alternate_file} 建立替代名稱對照表")

    output_folder = os.path.dirname(output_path)
    ensure_folder_exists(output_path)

    data = pl.read_csv(
        alternate_file,
        separator="\t",  # 設定 Tab 為分隔符號
        has_header=False,  # 表示檔案沒有標題列
        columns=[1, 2, 3, 4],  # 只讀取第 1, 2, 3, 4 欄
        new_columns=["geoname_id", "lang", "name", "is_preferred_name"],  # 重新命名欄位
        null_values="\\N",  # 把 "\N" 視為空值 (null)
        dtypes={
            "geoname_id": pl.String,
            "lang": pl.String,
            "name": pl.String,
            "is_preferred_name": pl.UInt8,
        },  # 指定所有欄位為 String
    )

    data = data.filter(data["lang"].is_in(CHINESE_PRIORITY))  # 僅保留中文名稱

    # 創建 `priority` 欄位，作為優先級判斷
    # - 如果 `is_preferred_name` 為 1，則優先級為 0
    # - 如果 `is_preferred_name` 為 0，則優先級為 CHINESE_PRIORITY 中 key 的 index + 1
    data = data.with_columns(
        pl.when(pl.col("is_preferred_name") == 1)
        .then(pl.lit(0))  # is_preferred_name == "1"，則優先級為 0
        .otherwise(
            pl.col("lang")
            .fill_null("")
            .map_elements(
                lambda x: (
                    CHINESE_PRIORITY.index(x) + 1
                    if x in CHINESE_PRIORITY
                    else len(CHINESE_PRIORITY) + 1
                ),
                return_dtype=pl.UInt8,  # 明確指定回傳型別
            )
        )
        .alias("priority")
    )

    # 相同的geoname_id，僅保留優先級最高的（數字越小越高，0為最高）
    data = (
        data.sort("priority")  # 按 `priority` 排序（越小越優先）
        .group_by("geoname_id")
        .first()  # 只保留 `geoname_id` 相同的第一筆資料（優先級最高的）
        .select(["geoname_id", "name"])  # 只保留指定的兩個欄位
    )

    # 更新地名
    data = data.with_columns(
        pl.col("name").str.replace("桃園縣", "桃園市").alias("name")
    )

    # 儲存為 alternate_chinese_name.csv
    data.write_csv(output_path)

    logger.info(f"替代名稱對照表已儲存至 {output_path}")


def load_alternate_names(file_path):
    logger.info(f"正在從 {file_path} 載入替代名稱對照表")

    if not os.path.exists(file_path):
        logger.info(f"替代名稱檔案 {file_path} 不存在")

        alternate_file = "./geoname_data/alternateNamesV2.txt"

        if not os.path.exists(alternate_file):
            logger.error(f"替代名稱檔案 {alternate_file} 不存在")
            sys.exit(1)

        create_alternate_map(alternate_file, file_path)

        return load_alternate_names(file_path)
    else:
        data = pl.read_csv(
            file_path,
            has_header=True,
            schema=pl.Schema(
                {
                    "geoname_id": pl.String,
                    "name": pl.String,
                }
            ),
        )

        logger.info(f"已從 {file_path} 載入替代名稱對照表")

        return data


if __name__ == "__main__":
    pass
