"""台灣地理資料處理器"""

import os
import sys
import polars as pl
from pathlib import Path
from datetime import date

sys.path.append(os.path.join(os.path.dirname(__file__), "../.."))

from core.utils import logger
from core.define import CITIES_SCHEMA, TAIWAN_ADMIN1
from core.processors.base import GeoDataProcessor


class TaiwanProcessor(GeoDataProcessor):
    """
    處理台灣地理資料的處理器類別。

    負責將 meta_data/taiwan_geodata.csv 轉換為 CITIES_SCHEMA 格式，
    並替換 cities500 資料中的台灣資料。
    """

    def convert_geodata(self) -> pl.DataFrame:
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
            pl.Series("geoname_id", [base_id + i for i in range(df.height)]).cast(
                pl.Int64
            )
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

    def replace_data(self, input_df: pl.DataFrame) -> pl.DataFrame:
        """
        使用轉換後的台灣地理資料取代輸入 DataFrame 中的現有台灣資料。

        Args:
            input_df: 包含城市資料的 DataFrame (應符合 CITIES_SCHEMA)。

        Returns:
            pl.DataFrame: 已替換台灣資料的 DataFrame。
        """
        logger.info("開始使用轉換後的資料取代現有台灣資料")
        converted_tw_df = self.convert_geodata()  # 取得新的台灣資料

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
