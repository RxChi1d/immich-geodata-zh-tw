"""日本地理資料處理器。"""

import os
import sys
import polars as pl
import geopandas as gpd
import pyproj
import numpy as np
from pathlib import Path

sys.path.append(os.path.join(os.path.dirname(__file__), "../.."))

from core.utils import logger
from core.geodata.base import GeoDataHandler, register_handler


# TODO: 日本 handler 開發中，完成後取消註解以啟用
@register_handler("JP")
class JapanGeoDataHandler(GeoDataHandler):
    """日本地理資料處理器。

    資料來源：国土数値情報ダウンロードサイト 行政区域データ。
    使用動態 UTM 區選擇方法（結合 Albers 投影）計算中心點。
    """

    COUNTRY_NAME = "日本"
    COUNTRY_CODE = "JP"
    TIMEZONE = "Asia/Tokyo"

    def _get_utm_epsg_from_lon(self, longitude: float) -> int:
        """根據經度計算 UTM 區的 EPSG 代碼。"""
        zone = int((longitude + 180) / 6) + 1
        return 32600 + zone

    def _calculate_centroids_utm(self, gdf: gpd.GeoDataFrame) -> gpd.GeoDataFrame:
        """使用動態 UTM 區選擇計算中心點（向量化）。

        結合 Albers 投影和動態 UTM 區選擇，提供高精確度的中心點計算。
        """
        # 確保使用 WGS84 座標系統
        if gdf.crs.to_epsg() != 4326:
            logger.info("正在轉換到 WGS84...")
            gdf = gdf.to_crs(epsg=4326)

        # 使用 Albers 投影計算準確的中心點經度
        # Reason: 邊界框平均值對於不規則形狀可能不準確，
        #         特別是在 UTM 區邊界附近（例如日本的 138°E）
        logger.info("正在計算準確的幾何中心點（使用 Albers 投影）...")

        # 定義以日本為中心的 Albers 等面積圓錐投影
        japan_albers = pyproj.CRS.from_proj4(
            "+proj=aea +lat_1=30 +lat_2=45 +lat_0=37.5 +lon_0=138 "
            "+x_0=0 +y_0=0 +datum=WGS84 +units=m +no_defs"
        )

        # 投影到 Albers 並計算中心點
        gdf_albers = gdf.to_crs(japan_albers)
        centroids_albers = gdf_albers.geometry.centroid

        # 將中心點轉回 WGS84 以取得準確的經度
        centroids_wgs84_temp = centroids_albers.to_crs(epsg=4326)
        center_lons = centroids_wgs84_temp.x

        # 根據準確的中心點經度計算 UTM 區（向量化）
        logger.info("正在根據中心點經度決定 UTM 區...")
        utm_zones = ((center_lons + 180) / 6).astype(int) + 1
        utm_epsgs = 32600 + utm_zones

        # 將 UTM 區資訊加入 GeoDataFrame
        gdf["_utm_zone"] = utm_zones
        gdf["_utm_epsg"] = utm_epsgs

        logger.info(f"識別到 {utm_epsgs.nunique()} 個不同的 UTM 區")

        # 建立陣列儲存結果（初始化為 NaN）
        longitudes = np.full(len(gdf), np.nan)
        latitudes = np.full(len(gdf), np.nan)

        # 按 UTM 區批次處理（依 UTM EPSG 分組）
        # Reason: 每個 UTM 區需要不同的投影，
        #         但在每個區內我們一次處理所有幾何體（向量化）
        logger.info("正在按 UTM 區批次計算中心點...")
        for utm_epsg, group_idx in gdf.groupby("_utm_epsg").groups.items():
            # 取得此 UTM 區的幾何體
            group_gdf = gdf.iloc[group_idx]

            # 轉換到 UTM 投影（批次操作，非迴圈）
            group_utm = group_gdf.to_crs(epsg=utm_epsg)

            # 在 UTM 中計算中心點（向量化）
            centroids_utm = group_utm.geometry.centroid

            # 轉回 WGS84（批次操作）
            centroids_wgs84 = centroids_utm.to_crs(epsg=4326)

            # 使用向量化的 .x 和 .y 屬性提取座標
            # Reason: 直接使用 NumPy 陣列運算，無 Python 迴圈
            longitudes[group_idx] = centroids_wgs84.x.values
            latitudes[group_idx] = centroids_wgs84.y.values

        # 將座標加入 GeoDataFrame（向量化賦值）
        gdf["longitude"] = longitudes
        gdf["latitude"] = latitudes

        # 清理暫存欄位
        gdf = gdf.drop(columns=["_utm_zone", "_utm_epsg"])

        return gdf

    # TODO: Extract - 從 Shapefile 提取地理資料並轉換為標準化 CSV
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

            # 使用動態 UTM 區選擇方法（結合 Albers 投影）
            logger.info("使用方法：動態 UTM 區選擇（結合 Albers 投影進行 UTM 區判定）")
            gdf = self._calculate_centroids_utm(gdf)

            # 移除 geometry 欄位
            gdf = gdf.drop(columns=["geometry"])

            # 將所有 object 類型的欄位轉換為字串，並將 None/NaN 轉為空字串
            for col in gdf.columns:
                if gdf[col].dtype == "object":
                    gdf[col] = gdf[col].fillna("").astype(str)

            # 轉換為 Polars DataFrame
            df = pl.from_pandas(gdf)

            # 選擇需要的欄位並進行欄位對應
            # admin_2: 合併郡名（N03_003）和市區町村名（N03_004）
            # 如果有郡名，則合併為「郡名+市區町村名」；否則僅使用市區町村名
            df = df.select(
                [
                    pl.col("longitude"),
                    pl.col("latitude"),
                    pl.col("N03_001").alias("admin_1"),  # 都道府縣
                    (
                        pl.when(
                            pl.col("N03_003").is_not_null()
                            & (pl.col("N03_003") != "")
                            & (pl.col("N03_003") != "None")
                            & (pl.col("N03_003") != "nan")
                        )
                        .then(pl.col("N03_003") + pl.col("N03_004"))
                        .otherwise(pl.col("N03_004"))
                    ).alias("admin_2"),  # 郡+市區町村 或 市區町村
                    (
                        pl.when(
                            pl.col("N03_005").is_not_null()
                            & (pl.col("N03_005") != "")
                            & (pl.col("N03_005") != "None")
                            & (pl.col("N03_005") != "nan")
                        )
                        .then(pl.col("N03_005"))
                        .otherwise(pl.lit(""))
                    ).alias("admin_3"),  # 政令指定都市的行政區
                    pl.lit("").alias("admin_4"),  # 空字串
                    pl.lit("日本").alias("country"),  # 國家
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
