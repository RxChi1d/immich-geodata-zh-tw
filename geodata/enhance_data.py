import os
import polars as pl
from utils import logger, CITIES_SCHEMA

import sys


def update_taiwan_admin1(cities500_df):
    """
    以下操作針對 country code 是 TW 的資料進行：
    1. 讀取 tw_admin1_map.csv
    2. 將 admin1_code 以 new_id 取代
    3. admin2_code 以 admin3_code 取代
    4. admin3_code 以 admin4_code 取代
    5. admin4_code 清空
    """
    logger.info("開始調整臺灣的 Admin Code")

    # 讀取 tw_admin1_map.csv
    admin1_map = pl.read_csv(os.path.join("output", "tw_admin1_map.csv"), separator=",")

    # 提取 old_id 的最後部分，使其與 cities500_df 的 admin2_code 匹配
    admin1_map = admin1_map.with_columns(
        pl.col("old_id").str.split(".").list.last().alias("admin2_code_match")
    )

    # 檢查是否唯一，防止 Join 時無法匹配
    unique_count = admin1_map["admin2_code_match"].n_unique()
    total_count = admin1_map.height

    # 如果唯一值數量 < 總數，表示有重複值
    if unique_count < total_count:
        logger.error(
            f"`admin2_code_match` 有重複值！唯一值數量: {unique_count}, 總數: {total_count}"
        )
        sys.exit(1)

    # 只更新 country code 是 TW 的資料
    taiwan_df = cities500_df.filter(pl.col("country_code") == "TW")

    # 透過 `join()` 來對應 ` `
    taiwan_df = taiwan_df.join(
        admin1_map, left_on="admin2_code", right_on="admin2_code_match", how="left"
    )

    # 更新所有欄位
    taiwan_df = taiwan_df.with_columns(
        pl.col("new_id")
        .str.split(".")
        .list.last()
        .alias("admin1_code"),  # 提取 new_id 的最後部分，作為新的 `admin1_code`
        pl.col("admin3_code").alias("admin2_code"),  # admin2_code 變成 admin3_code
        pl.col("admin4_code").alias("admin3_code"),  # admin3_code 變成 admin4_code
        pl.lit("").alias("admin4_code"),  # admin4_code 變成空字串
    )

    # 將更新後的臺灣資料，與原始 `cities500_df` 合併回去
    # NOTE: 臺灣的資料放在最前面
    cities500_df = taiwan_df.select(cities500_df.columns).vstack(  # 把舊數據放在後面
        cities500_df.filter(pl.col("country_code") != "TW")  # 移除舊的 TW 資料
    )

    logger.info("臺灣的 Admin Code 調整完成")

    return cities500_df


def update_cities500(cities_file, extra_file, output_file):
    logger.info("開始更新 cites500.txt")

    # 讀取 cites500.txt
    cities500_df = pl.read_csv(
        cities_file, separator="\t", has_header=False, schema=CITIES_SCHEMA
    )

    # 讀取 extra_data/TW.txt
    extra_df = pl.read_csv(
        extra_file, separator="\t", has_header=False, schema=CITIES_SCHEMA
    )

    # 篩選條件：
    #   - `geoname_id` 不在 `cities500_df` 的 `geoname_id` 中
    #   - `population` 大於等於 `MIN_POPULATION`
    filtered_extra_df = extra_df.filter(
        ~pl.col("geoname_id").is_in(cities500_df["geoname_id"])  # geonameid 不能已存在
        & (pl.col("population") >= MIN_POPULATION)  # 人口數須 >= MIN_POPULATION
    )

    # 合併新資料到 `cities500_df`
    cities500_df = cities500_df.vstack(filtered_extra_df)

    # 計算新增的數據量
    logger.info(f"成功新增 {filtered_extra_df.height} 行數據到 cities500.txt")

    # 將country是TW，且"admin2_code"為空的資料刪除
    filter_condition = (cities500_df["country_code"] == "TW") & (
        (cities500_df["admin2_code"].is_null()) | (cities500_df["admin2_code"] == "")
    )
    cities500_df = cities500_df.filter(~filter_condition)

    logger.info(
        f"移除臺灣資料中 {filter_condition.sum()} 筆 admin2_code 為空的資料，剩餘 {cities500_df.height} 筆資料"
    )

    # 調整臺灣的 Admin Code
    cities500_df = update_taiwan_admin1(cities500_df)

    cities500_df.write_csv(
        output_file,
        separator="\t",
        include_header=False
    )

    logger.info(f"cites500.txt 更新完成，儲存至 {output_file}")


if __name__ == "__main__":
    MIN_POPULATION = 100

    # 文件路径
    data_base_folder = "./geoname_data"
    extra_data_folder = os.path.join(data_base_folder, "extra_data")
    output_folder = "./output"

    cities_file = os.path.join(data_base_folder, "cities500.txt")
    extra_file = os.path.join(extra_data_folder, "TW.txt")
    output_file = os.path.join(output_folder, "cities500_optimized.txt")

    update_cities500(cities_file, extra_file, output_file)
