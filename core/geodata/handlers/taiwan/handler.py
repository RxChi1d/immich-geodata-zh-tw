"""臺灣地理資料處理器。"""

from typing import ClassVar

import polars as pl
import geopandas as gpd

from core.utils import logger
from core.geodata.base import GeoDataHandler, register_handler


@register_handler("TW")
class TaiwanGeoDataHandler(GeoDataHandler):
    """臺灣地理資料處理器。

    資料來源：中華民國國土測繪中心 (NLSC) 村(里)界資料。
    """

    COUNTRY_NAME: ClassVar[str] = "臺灣"
    COUNTRY_CODE: ClassVar[str] = "TW"
    TIMEZONE: ClassVar[str] = "Asia/Taipei"

    # 臺灣直轄市與省轄市列表（供 locationiq 流程引用）
    MUNICIPALITIES: ClassVar[list[str]] = [
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

    def extract_from_shapefile(
        self,
        shapefile_path: str,
        output_csv: str,
    ) -> None:
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

            # 轉換為 Polars DataFrame（共用 helper 處理 geometry/字串轉換）
            # Reason: TW 沿用既有行為，NaN 透過 astype(str) 保留為 "None"/"nan" 字面值
            df = self._gdf_to_polars(gdf, fillna_object=False)

            # 選擇需要的欄位並重新命名
            df = df.select(
                [
                    pl.col("latitude"),
                    pl.col("longitude"),
                    pl.lit(self.COUNTRY_NAME).alias("country"),
                    pl.col("COUNTYNAME").alias("admin_1"),  # 縣市
                    pl.col("TOWNNAME").alias("admin_2"),  # 鄉鎮市區
                    pl.col("VILLNAME").alias("admin_3"),  # 村里
                    pl.lit(None, dtype=pl.String).alias("admin_4"),  # 鄰
                ]
            )

            # 標準化並儲存 CSV
            self._save_extract_csv(df, output_csv)

        except Exception as e:
            logger.error(f"處理 Shapefile 時發生錯誤: {e}")
            raise
