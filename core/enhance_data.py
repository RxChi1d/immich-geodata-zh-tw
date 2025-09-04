import os
import sys
import polars as pl
from pathlib import Path
from datetime import date

sys.path.append(os.path.join(os.path.dirname(__file__), ".."))

from core.utils import logger
from core.define import CITIES_SCHEMA, TAIWAN_ADMIN1


def convert_taiwan_geodata():
    """
    讀取 meta_data/taiwan_geodata.csv 並轉換成 CITIES_SCHEMA 格式的 DataFrame。
    會使用 TAIWAN_ADMIN1 將縣市名稱轉換為對應的 admin1_code。

    Returns:
        pl.DataFrame: 轉換後的台灣地理資料 DataFrame，符合 CITIES_SCHEMA。
    """
    logger.info("讀取並轉換台灣地理資料 (taiwan_geodata.csv)")

    input_file = Path("meta_data/taiwan_geodata.csv")

    if not input_file.exists():
        logger.error(f"輸入檔案不存在: {input_file}")
        sys.exit(1)

    df = pl.read_csv(input_file)

    # 清理 admin 欄位中的 "" 字串，替換為 None
    admin_cols = ["admin_1", "admin_2", "admin_3", "admin_4"]
    for col in admin_cols:
        if col in df.columns:
            df = df.with_columns(
                pl.when(pl.col(col) == '""')
                .then(None)
                .otherwise(pl.col(col))
                .alias(col)
            )

    # 生成唯一的 geoname_id
    base_id = 90000000
    df = df.with_columns(
        pl.Series("geoname_id", [base_id + i for i in range(df.height)]).cast(pl.Int64)
    )

    # 將 admin_1 (縣市名稱) 映射到 admin1_code
    df = df.with_columns(
        pl.col("admin_1")
        .map_elements(
            lambda name: TAIWAN_ADMIN1.get(name, None), return_dtype=pl.String
        )
        .alias("admin1_code_full")  # Store the full code temporarily "TW.XX"
    )

    # 檢查是否有無法映射的 admin_1
    null_admin1_codes = df.filter(pl.col("admin1_code_full").is_null())
    if null_admin1_codes.height > 0:
        missing_names = null_admin1_codes["admin_1"].unique().to_list()
        logger.warning(
            f"以下縣市名稱無法在 TAIWAN_ADMIN1 中找到對應代碼，admin1_code 將設為 None: {missing_names}"
        )

    # 提取 admin1_code 的數字/字母部分 "XX"
    df = df.with_columns(
        pl.when(pl.col("admin1_code_full").is_not_null())
        .then(
            pl.col("admin1_code_full").str.split(".").list.last()
        )  # Split by '.' and get last part
        .otherwise(None)  # Keep None if mapping failed
        .alias("admin1_code_mapped")  # Final code "XX"
    )

    # 獲取今天的日期字串
    today_date_str = date.today().strftime("%Y-%m-%d")

    # 建立新的 DataFrame
    new_df = pl.DataFrame(
        {
            "geoname_id": df["geoname_id"],
            "name": df["admin_2"],  # 鄉鎮市區名
            "asciiname": df["admin_2"],  # 同上
            "alternatenames": None,
            "latitude": df["latitude"],
            "longitude": df["longitude"],
            "feature_class": "A",
            "feature_code": "ADM2",  # 因為主要地名是鄉鎮市區
            "country_code": "TW",
            "cc2": None,
            "admin1_code": df["admin1_code_mapped"],  # 使用提取後的 "XX" 部分
            "admin2_code": None,  # 暫不提供鄉鎮市區代碼
            "admin3_code": None,  # 暫不提供村里代碼
            "admin4_code": None,  # 暫不提供更低級別代碼
            "population": 0,
            "elevation": None,
            "dem": None,
            "timezone": "Asia/Taipei",
            "modification_date": today_date_str,  # 使用今天的日期
        },
        schema=CITIES_SCHEMA,
    )

    # 將轉換後的台灣地理資料暫存到 output 資料夾
    output_path = os.path.join("output", "taiwan_geodata_converted.csv")
    new_df.write_csv(output_path)
    logger.info(f"已將轉換後的台灣地理資料暫存至: {output_path}")

    logger.info(f"台灣地理資料轉換完成，共 {new_df.height} 筆資料")
    return new_df


def replace_with_converted_taiwan_data(input_df: pl.DataFrame) -> pl.DataFrame:
    """
    使用轉換後的台灣地理資料取代輸入 DataFrame 中的現有台灣資料。

    Args:
        input_df (pl.DataFrame): 包含城市資料的 DataFrame (應符合 CITIES_SCHEMA)。

    Returns:
        pl.DataFrame: 已替換台灣資料的 DataFrame。
    """
    logger.info("開始使用轉換後的資料取代現有台灣資料")
    converted_tw_df = convert_taiwan_geodata()  # 取得新的台灣資料

    # 移除所有舊的台灣資料
    non_tw_df = input_df.filter(pl.col("country_code") != "TW")
    removed_count = input_df.height - non_tw_df.height
    if removed_count > 0:
        logger.info(f"移除了 {removed_count} 筆舊的台灣資料")
    else:
        logger.info("輸入資料中未找到需要移除的台灣資料")

    # 將新的台灣資料放在最前面，合併非台灣資料
    output_df = converted_tw_df.vstack(non_tw_df)
    logger.info(f"添加了 {converted_tw_df.height} 筆新的台灣資料")
    logger.info("台灣資料替換完成")
    return output_df


def update_cities500(cities_file, extra_files, output_file, min_population=100):
    """
    更新 cities500.txt 檔案，將新的城市資料合併進去，並進行資料清理和調整。
    其中，台灣地區的資料將被 meta_data/taiwan_geodata.csv 的內容完全取代。

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
        5. **調用 replace_with_converted_taiwan_data 函數，使用轉換後的資料替換台灣資料。**
        6. 將更新後的資料寫入指定的輸出檔案中。
    """

    logger.info("開始更新 cites500.txt")

    # 讀取 cites500.txt
    cities500_df = pl.read_csv(
        cities_file, separator="\t", has_header=False, schema=CITIES_SCHEMA
    )

    # 讀取 extra_data/country_code.txt
    extra_df = pl.DataFrame(schema=CITIES_SCHEMA)
    for file in extra_files:
        if Path(file).exists():
            extra_df = extra_df.vstack(
                pl.read_csv(
                    file, separator="\t", has_header=False, schema=CITIES_SCHEMA
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

    # --- 使用獨立函數替換台灣資料 ---
    cities500_df = replace_with_converted_taiwan_data(cities500_df)

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
