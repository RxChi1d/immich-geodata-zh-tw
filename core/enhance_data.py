import os
import sys
import polars as pl

sys.path.append(os.path.join(os.path.dirname(__file__), ".."))

from core.utils import logger
from core.define import CITIES_SCHEMA


def update_taiwan_admin1(cities500_df):
    """
    更新臺灣的行政區代碼。

    Args:
        cities500_df (pl.DataFrame): 包含城市資料的 DataFrame。

    Returns:
        pl.DataFrame: 更新後的城市資料 DataFrame。

    Description:
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
        logger.critical(
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


def update_cities500(cities_file, extra_file, output_file, min_population=100):
    """
    更新 cities500.txt 檔案，將新的城市資料合併進去，並進行資料清理和調整。

    Args:
        cities_file (str): 原始的 cities500.txt 檔案路徑。
        extra_file (str): 額外的城市資料檔案路徑。
        output_file (str): 更新後的 cities500.txt 檔案儲存路徑。
    Returns:
        None
    功能描述:
        1. 讀取 cities500.txt 和額外的城市資料檔案。
        2. 篩選出 geoname_id 不在 cities500.txt 中且人口數大於等於 MIN_POPULATION 的資料。
        3. 將篩選出的新資料合併到 cities500.txt 中。
        4. 刪除 country_code 是 TW 且 admin2_code 為空的資料。
        5. 調整臺灣的 Admin Code。
        6. 將更新後的資料寫入指定的輸出檔案中。
    """

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
        ~pl.col("geoname_id").is_in(cities500_df["geoname_id"])  # geoname_id 不能已存在
        & (pl.col("population") >= min_population)  # 人口數須 >= MIN_POPULATION
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

    # 檢查是否有重複座標的資料
    # 如果有則保留最大人口數的資料
    # 如果人口數一樣，則保留最小的 geoname_id
    filter_condition = cities500_df.group_by(["latitude", "longitude"]).agg(
        pl.max("population").alias("population_max"),
        pl.min("geoname_id").alias("geoname_id_min"),
    )

    cities500_df = (
        cities500_df.join(
            filter_condition,
            on=["latitude", "longitude"],
            how="inner",
        )
        .filter(
            (pl.col("population") == pl.col("population_max"))
            & (pl.col("geoname_id") == pl.col("geoname_id_min"))
        )
        .select(cities500_df.columns)
    )

    # 調整臺灣的 Admin Code
    cities500_df = update_taiwan_admin1(cities500_df)

    cities500_df.write_csv(output_file, separator="\t", include_header=False)

    logger.info(f"cites500.txt 更新完成，儲存至 {output_file}")


def test():
    min_population = 100

    # 文件路径
    data_base_folder = "./geoname_data"
    extra_data_folder = os.path.join(data_base_folder, "extra_data")
    output_folder = "./output"

    cities_file = os.path.join(data_base_folder, "cities500.txt")
    extra_file = os.path.join(extra_data_folder, "TW.txt")
    output_file = os.path.join(output_folder, "cities500_optimized.txt")

    update_cities500(cities_file, extra_file, output_file, min_population)


if __name__ == "__main__":
    logger.error("請使用 main.py 作為主要接口，而非直接執行 enhance_data.py")
