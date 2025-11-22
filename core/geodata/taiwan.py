"""臺灣地理資料處理器。"""

import polars as pl
import geopandas as gpd
from pathlib import Path

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

    MUNICIPALITIES = _MUNICIPALITIES

    def extract_from_shapefile(
        self,
        shapefile_path: str,
        output_csv: str,
        *,
        google_api_key: str | None = None,
    ) -> None:
        try:
            _ = google_api_key

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
                    pl.col("latitude"),
                    pl.col("longitude"),
                    pl.lit("臺灣").alias("country"),  # 國家
                    pl.col("COUNTYNAME").alias("admin_1"),  # 縣市
                    pl.col("TOWNNAME").alias("admin_2"),  # 鄉鎮市區
                    pl.col("VILLNAME").alias("admin_3"),  # 村里
                    pl.lit(None, dtype=pl.String).alias("admin_4"),  # 鄰
                ]
            )

            # 按照 country, admin_1, admin_2 進行排序
            df = df.sort(["country", "admin_1", "admin_2"])

            # 移除無效的資料點
            df = df.filter(
                pl.col("longitude").is_not_null() & pl.col("latitude").is_not_null()
            )

            # 固定經緯度小數位數以確保輸出穩定性
            df = self.standardize_coordinate_precision(df)

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
