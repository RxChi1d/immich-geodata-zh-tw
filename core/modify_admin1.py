import os
import polars as pl

from utils import logger, load_alternate_names
from define import ADMIN1_SCHEMA, TAIWAN_ADMIN1


def create_new_taiwan_admin1(admin2_path, output_path):
    """
    建立新的臺灣一級行政區對應表

    Args:
        admin2_path (str): admin2Codes.txt 的檔案路徑
        output_path (str): 輸出新的臺灣一級行政區對應表的檔案路徑

    Description:
        1. 讀取 admin1CodesASCII.txt 和 admin2Codes.txt
        2. 讀取 alternate_chinese_name.json (中文地名對照表)
        3. 從 admin2Codes.txt 中獲取臺灣的行政區的 geoname_id 和編號 (e.g. TW.03.TPQ)
        4. 根據 geoname_id 在 alternate_chinese_name.json 中找到對應的中文名稱
        5. 根據找到的中文名稱，在 TAIWAN_ADMIN1 找到新的 id
        6. 將新的臺灣 admin1 對應表存檔到 output_path (header = ["name", "new_id", "old_id", "geoname_id"])

    Returns:
        None
    """
    logger.info("開始建立新的臺灣一級行政區對應表")

    output_folder = os.path.dirname(output_path)

    admin2_df = pl.read_csv(
        admin2_path,
        separator="\t",
        has_header=False,
        schema=ADMIN1_SCHEMA,
    )

    alternate_names_df = load_alternate_names(
        os.path.join(output_folder, "alternate_chinese_name.csv")
    )  # pl.DataFrame: geoname_id, name

    # 從 admin2Codes.txt 中獲取臺灣的行政區的 geoname_id 和編號 (e.g. TW.03.TPQ)
    tw_admin1_df = admin2_df.filter(pl.col("id").str.starts_with("TW."))

    # 1️. 先 Join `alternate_names_df` 來獲取中文名稱
    merged_df = tw_admin1_df.join(
        alternate_names_df, left_on="geoname_id", right_on="geoname_id", how="left"
    )

    # 2️. 根據 `name` 轉換為 `new_id`
    merged_df = merged_df.with_columns(
        pl.col("name_right")
        .map_elements(lambda x: TAIWAN_ADMIN1.get(x, None), return_dtype=pl.String)
        .alias("new_id")
    )

    # 3️. 選擇需要的欄位，並重新命名
    new_admin1_df = merged_df.select(
        [
            "name",
            pl.col("name").alias("name_en"),
            "new_id",
            pl.col("id").alias("old_id"),
            pl.col("geoname_id").alias("geoname_id"),
        ]
    ).sort("new_id")

    # 儲存新的臺灣 admin1 對應表
    new_admin1_df.write_csv(output_path)

    logger.info(f"新的臺灣一級行政區對應表建立完成，儲存至 {output_path}")


def update_taiwan_admin1(admin1_path, tw_admin1_map_path, output_path):
    """
    更新臺灣的行政區劃資料到 admin1CodesASCII.txt

    Args:
        admin1_path (str): admin1CodesASCII.txt 的檔案路徑
        tw_admin1_map_path (str): tw_admin1_map.csv 的檔案路徑
        output_path (str): 更新後的 admin1CodesASCII.txt 儲存路徑

    Returns:
        None

    Description:
        1. 讀取 admin1CodesASCII.txt 和 tw_admin1_map.csv
        2. 將臺灣的行政區劃資料更新到 admin1CodesASCII.txt
            2.1. 移除 admin1CodesASCII.txt 中 id 開頭為 "TW." 的行政區劃資料
            2.2. 依照 new_admin1_map ，將新的行政區劃資料插入到 admin1CodesASCII.txt
            2.4. 將新的行政區劃資料存檔到 output_path
    """
    logger.info("開始更新 admin1CodesASCII.txt 中的臺灣資料")

    # id, name, asciiname, geoname_id
    admin1_df = pl.read_csv(
        admin1_path,
        separator="\t",
        has_header=False,
        schema=ADMIN1_SCHEMA,
    )

    tw_admin_map = pl.read_csv(
        tw_admin1_map_path,
        schema_overrides={
            "name": pl.String,
            "name_en": pl.String,
            "new_id": pl.String,
            "old_id": pl.String,
            "geoname_id": pl.String,
        },
    )

    # 準備要插入的新資料，確保欄位一致
    new_rows = tw_admin_map.select(
        [
            pl.col("new_id").alias("id"),
            pl.col("name_en").alias("name"),
            pl.col("name_en").alias("asciiname"),
            pl.col("geoname_id").alias("geoname_id"),
        ]
    )

    # 移除 admin1CodesASCII.txt 中 id 開頭為 "TW." 的行政區劃資料
    logger.info("移除 admin1CodesASCII.txt 中原始的臺灣一級行政區資料")
    admin1_df = admin1_df.filter(~pl.col("id").str.starts_with("TW."))

    # 使用 vstack 合併，臺灣的資料放在最前面
    logger.info("插入新的臺灣一級行政區資料")
    admin1_df = new_rows.vstack(admin1_df)

    # 儲存新的 admin1CodesASCII.txt
    admin1_df.write_csv(output_path, separator="\t", include_header=False)

    logger.info(f"admin1CodesASCII.txt 中的臺灣資料更新完成，儲存至 {output_path}")


if __name__ == "__main__":
    data_folder = "geoname_data"
    output_folder = "output"

    admin1_path = os.path.join(data_folder, "admin1CodesASCII.txt")
    admin2_path = os.path.join(data_folder, "admin2Codes.txt")
    new_admin1_path = os.path.join(output_folder, "admin1CodesASCII_optimized.txt")
    tw_admin1_map_path = os.path.join(output_folder, "tw_admin1_map.csv")

    create_new_taiwan_admin1(admin2_path, tw_admin1_map_path)
    update_taiwan_admin1(admin1_path, tw_admin1_map_path, new_admin1_path)
