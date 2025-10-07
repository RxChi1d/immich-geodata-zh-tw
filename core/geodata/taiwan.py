"""臺灣地理資料處理器。"""

import os
import sys
import polars as pl
import geopandas as gpd
from pathlib import Path
from datetime import date

sys.path.append(os.path.join(os.path.dirname(__file__), "../.."))

from core.utils import logger
from core.geodata.base import GeoDataHandler, register_handler


# 臺灣直轄市列表
_MUNICIPALITIES = [
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


@register_handler("TW")
class TaiwanGeoDataHandler(GeoDataHandler):
    """臺灣地理資料處理器。

    資料來源：中華民國國土測繪中心 (NLSC) 村(里)界資料。
    """

    COUNTRY_NAME = "臺灣"
    COUNTRY_CODE = "TW"
    TIMEZONE = "Asia/Taipei"

    # 臺灣行政區代碼映射（一級行政區）
    ADMIN1_MAPPING = {
        # 直轄市
        "臺北市": "TW.01",
        "新北市": "TW.02",
        "桃園市": "TW.03",
        "臺中市": "TW.04",
        "臺南市": "TW.05",
        "高雄市": "TW.06",
        # 省轄市
        "基隆市": "TW.07",
        "新竹市": "TW.08",
        "嘉義市": "TW.09",
        # 縣
        "宜蘭縣": "TW.10",
        "新竹縣": "TW.11",
        "苗栗縣": "TW.12",
        "彰化縣": "TW.13",
        "南投縣": "TW.14",
        "雲林縣": "TW.15",
        "嘉義縣": "TW.16",
        "屏東縣": "TW.17",
        "臺東縣": "TW.18",
        "花蓮縣": "TW.19",
        "澎湖縣": "TW.20",
        "金門縣": "TW.21",
        "連江縣": "TW.22",
    }

    MUNICIPALITIES = _MUNICIPALITIES

    def extract_from_shapefile(self, shapefile_path: str, output_csv: str) -> None:
        try:
            logger.info(f"正在讀取 Shapefile: {shapefile_path}")

            # 使用 geopandas 讀取 Shapefile
            gdf = gpd.read_file(shapefile_path)
            logger.info(
                f"成功讀取 Shapefile，資料集大小: {gdf.shape[0]} 行 x {gdf.shape[1]} 列"
            )

            # 檢查原始座標系統
            logger.info(f"原始座標系統: {gdf.crs}")

            # 先轉換到投影座標系統計算中心點
            logger.info("正在轉換到投影座標系統 (TWD97 / TM2 zone 121)...")
            gdf = gdf.to_crs(epsg=3826)

            # 在投影座標系統下計算中心點
            logger.info("正在計算中心點...")
            centroids = gdf.geometry.centroid

            # 將中心點轉換回 WGS84
            logger.info("正在將中心點轉換回 WGS84...")
            centroids = centroids.to_crs(epsg=4326)
            gdf["longitude"] = centroids.x
            gdf["latitude"] = centroids.y

            # 移除 geometry 欄位
            gdf = gdf.drop(columns=["geometry"])

            # 將所有 object 類型的欄位轉換為字串
            for col in gdf.columns:
                if gdf[col].dtype == "object":
                    gdf[col] = gdf[col].astype(str)

            # 轉換為 Polars DataFrame
            df = pl.from_pandas(gdf)

            # 選擇需要的欄位並重新命名
            df = df.select(
                [
                    pl.col("longitude"),
                    pl.col("latitude"),
                    pl.col("COUNTYNAME").alias("admin_1"),  # 縣市
                    pl.col("TOWNNAME").alias("admin_2"),  # 鄉鎮市區
                    pl.col("VILLNAME").alias("admin_3"),  # 村里
                    pl.lit("").alias("admin_4"),  # 鄰 - 設為空字串
                    pl.lit("臺灣").alias("country"),  # 國家
                ]
            )

            # 按照 country, admin_1, admin_2 進行排序
            df = df.sort(["country", "admin_1", "admin_2"])

            # 移除無效的資料點
            df = df.filter(
                pl.col("longitude").is_not_null() & pl.col("latitude").is_not_null()
            )

            # 儲存 CSV
            output_path = Path(output_csv)
            output_path.parent.mkdir(parents=True, exist_ok=True)

            logger.info(f"正在儲存 CSV 檔案: {output_path}")
            df.write_csv(output_path)
            logger.info(f"成功儲存 CSV 檔案，共 {len(df)} 筆資料")

            # 顯示前五筆資料供檢查
            logger.info(df.head(5))

        except Exception as e:
            logger.error(f"處理 Shapefile 時發生錯誤: {e}")
            raise

    def convert_to_cities_schema(
        self, csv_path: str, base_geoname_id: int
    ) -> pl.DataFrame:
        logger.info(f"讀取並轉換臺灣地理資料 ({Path(csv_path).name})")

        input_file = Path(csv_path)

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
        df = df.with_columns(
            pl.Series(
                "geoname_id",
                [base_geoname_id + i for i in range(df.height)],
            ).cast(pl.Int64)
        )

        # 將 admin_1 (縣市名稱) 映射到 admin1_code
        df = df.with_columns(
            pl.col("admin_1")
            .map_elements(
                lambda name: self.ADMIN1_MAPPING.get(name, None),
                return_dtype=pl.String,
            )
            .alias("admin1_code_full")  # 暫存完整代碼 "TW.XX"
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
            .then(pl.col("admin1_code_full").str.split(".").list.last())
            .otherwise(None)
            .alias("admin1_code_mapped")
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
                "country_code": self.COUNTRY_CODE,
                "cc2": None,
                "admin1_code": df["admin1_code_mapped"],  # 使用提取後的 "XX" 部分
                "admin2_code": None,  # 暫不提供鄉鎮市區代碼
                "admin3_code": None,  # 暫不提供村里代碼
                "admin4_code": None,  # 暫不提供更低級別代碼
                "population": 0,
                "elevation": None,
                "dem": None,
                "timezone": self.TIMEZONE,
                "modification_date": today_date_str,  # 使用今天的日期
            },
            schema=self.CITIES_SCHEMA,
        )

        # 將轉換後的臺灣地理資料暫存到 output 資料夾
        output_path = Path("output") / "tw_geodata_converted.csv"
        output_path.parent.mkdir(parents=True, exist_ok=True)
        new_df.write_csv(output_path)
        logger.info(f"已將轉換後的臺灣地理資料暫存至: {output_path}")

        logger.info(f"臺灣地理資料轉換完成，共 {new_df.height} 筆資料")
        return new_df
