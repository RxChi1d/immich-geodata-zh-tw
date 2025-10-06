import os
import sys
import polars as pl
from pathlib import Path

sys.path.append(os.path.join(os.path.dirname(__file__), ".."))

from core.utils import logger
from core.schemas import CITIES_SCHEMA
from core.geodata import get_handler


def update_cities500(cities_file, extra_files, output_file, min_population=100):
    """
    更新 cities500.txt 檔案，將新的城市資料合併進去，並進行資料清理和調整。
    其中，台灣地區的資料將被 meta_data/tw_geodata.csv 的內容完全取代。

    Args:
        cities_file (str): 原始的 cities500.txt 檔案路徑。
        extra_files (list[str]): 額外的城市資料檔案路徑列表。
        output_file (str): 更新後的 cities500.txt 檔案儲存路徑。
        min_population (int): 加入額外資料的最低人口數要求。
    Returns:
        None
    功能描述:
        1. 讀取 cities500.txt 和額外的城市資料檔案。
        2. 篩選出 geoname_id 不在 cities500.txt 中且人口數大於等於 min_population 的資料。
        3. 將篩選出的新資料合併到 cities500.txt 中。
        4. 檢查是否有重複座標的資料，若有則保留人口數最大者 (若人口數相同則保留 geoname_id 最小者)。
        5. 調用 GeoDataHandler 的 replace_in_dataset，使用轉換後的資料替換對應國家資料。
        6. 將更新後的資料寫入指定的輸出檔案中。
    """

    logger.info("開始更新 cites500.txt")

    # 讀取 cites500.txt
    cities500_df = pl.read_csv(
        cities_file,
        separator="\t",
        has_header=False,
        schema=CITIES_SCHEMA,
    )

    # 讀取 extra_data/country_code.txt
    extra_df = pl.DataFrame(schema=CITIES_SCHEMA)
    for file in extra_files:
        if Path(file).exists():
            extra_df = extra_df.vstack(
                pl.read_csv(
                    file,
                    separator="\t",
                    has_header=False,
                    schema=CITIES_SCHEMA,
                )
            )
            logger.info(f"讀取額外資料檔案: {file}")
        else:
            logger.warning(f"額外資料檔案不存在，跳過: {file}")

    # 篩選條件：
    #   - `geoname_id` 不在 `cities500_df` 的 `geoname_id` 中
    #   - `population` 大於等於 `min_population`
    filtered_extra_df = extra_df.filter(
        ~pl.col("geoname_id").is_in(cities500_df["geoname_id"])  # geoname_id 不能已存在
        & (pl.col("population") >= min_population)  # 人口數須 >= MIN_POPULATION
    )

    # 合併新資料到 `cities500_df`
    cities500_df = cities500_df.vstack(filtered_extra_df)
    logger.info(f"成功新增 {filtered_extra_df.height} 行數據到 cities500.txt")

    # 檢查是否有重複座標的資料
    logger.info("開始處理重複座標的資料")
    initial_rows = cities500_df.height
    duplicates_check = cities500_df.group_by(["latitude", "longitude"]).agg(
        pl.max("population").alias("population_max"),
        pl.min("geoname_id").alias("geoname_id_min"),
    )

    cities500_df = (
        cities500_df.join(
            duplicates_check,
            on=["latitude", "longitude"],
            how="inner",
        )
        .filter(
            (pl.col("population") == pl.col("population_max"))
            & (pl.col("geoname_id") == pl.col("geoname_id_min"))
        )
        .select(cities500_df.columns)
    )
    final_rows = cities500_df.height
    if initial_rows > final_rows:
        logger.info(f"處理重複座標完成，移除了 {initial_rows - final_rows} 筆資料")
    else:
        logger.info("未發現需要處理的重複座標資料")

    # --- 使用處理器替換台灣資料 ---
    taiwan_handler_class = get_handler("TW")
    taiwan_handler = taiwan_handler_class()
    cities500_df = taiwan_handler.replace_in_dataset(cities500_df, "TW")

    cities500_df.write_csv(output_file, separator="\t", include_header=False)

    logger.info(
        f"cites500.txt 更新完成 ({cities500_df.height} 筆資料)，儲存至 {output_file}"
    )


def test():
    min_population = 100

    # 文件路径
    data_base_folder = "./geoname_data"
    extra_data_folder = os.path.join(data_base_folder, "extra_data")
    output_folder = "./output"
    country_code = ["TW", "JP"]

    cities_file = os.path.join(data_base_folder, "cities500.txt")
    extra_files = [
        os.path.join(extra_data_folder, f"{code}.txt") for code in country_code
    ]
    output_file = os.path.join(output_folder, "cities500_optimized.txt")

    update_cities500(cities_file, extra_files, output_file, min_population)


if __name__ == "__main__":
    logger.error("請使用 main.py 作為主要接口，而非直接執行 enhance_data.py")
