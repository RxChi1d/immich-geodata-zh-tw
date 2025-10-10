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

            # 選擇基本欄位
            df = df.select(
                [
                    pl.col("longitude"),
                    pl.col("latitude"),
                    pl.col("N03_001"),  # 都道府縣
                    pl.col("N03_003"),  # 郡名 or 政令市名（舊版資料格式）
                    pl.col("N03_004"),  # 市區町村名
                    pl.col("N03_005"),  # 政令市之區（新版資料格式）
                ]
            )

            # 標準化空值：將 null、空字串、"None"、"nan" 統一轉為 None
            df = df.with_columns(
                [
                    pl.when(
                        pl.col("N03_003").is_not_null()
                        & (pl.col("N03_003") != "")
                        & (pl.col("N03_003") != "None")
                        & (pl.col("N03_003") != "nan")
                    )
                    .then(pl.col("N03_003"))
                    .otherwise(None)
                    .alias("clean_n03_003"),
                    pl.when(
                        pl.col("N03_004").is_not_null()
                        & (pl.col("N03_004") != "")
                        & (pl.col("N03_004") != "None")
                        & (pl.col("N03_004") != "nan")
                    )
                    .then(pl.col("N03_004"))
                    .otherwise(None)
                    .alias("clean_n03_004"),
                    pl.when(
                        pl.col("N03_005").is_not_null()
                        & (pl.col("N03_005") != "")
                        & (pl.col("N03_005") != "None")
                        & (pl.col("N03_005") != "nan")
                    )
                    .then(pl.col("N03_005"))
                    .otherwise(None)
                    .alias("clean_n03_005"),
                ]
            )

            # 過濾無效資料：移除兩者都為空的記錄
            df = df.filter(
                pl.col("clean_n03_003").is_not_null()
                | pl.col("clean_n03_004").is_not_null()
                | pl.col("clean_n03_005").is_not_null()
            )

            # 識別行政區類型
            df = df.with_columns(
                [
                    # 郡轄町/村：N03_003 以「郡」結尾
                    pl.col("clean_n03_003")
                    .str.ends_with("郡")
                    .fill_null(False)
                    .alias("is_gun"),
                    # 政令指定都市：新版資料以 N03_005 標示區名
                    pl.col("clean_n03_005").is_not_null().alias("is_seirei_shi"),
                    # 普通市：N03_003 為空、N03_004 以「市」結尾且無區名欄位
                    (
                        pl.col("clean_n03_003").is_null()
                        & pl.col("clean_n03_004").str.ends_with("市").fill_null(False)
                        & pl.col("clean_n03_005").is_null()
                    ).alias("is_regular_shi"),
                ]
            )

            # R2' 規則：檢測真正的同名町/村衝突
            # Reason: 根據 PRP，預設應簡潔（僅顯示町/村名），
            #         只有在同一都道府縣內存在多個郡有「完全同名」的町/村時才補郡。
            #         「釧路市 vs 釧路町」不需補郡（尾碼已區分），
            #         「A郡 X村 vs B郡 X村」才需補郡（真正同名）。

            # 步驟 1：先按 (都道府縣, 郡名, 町/村名) 去重，避免同一町/村的多個幾何體被重複計算
            # Reason: 同一個行政區可能有多個多邊形（飛地、離島等），
            #         這些不應被視為「不同的町/村」
            unique_gun_towns = (
                df.filter(pl.col("is_gun"))
                .select(["N03_001", "clean_n03_003", "clean_n03_004"])
                .unique()
            )

            # 步驟 2：統計每個都道府縣內每個町/村名稱對應的郡數量
            gun_town_counts = unique_gun_towns.group_by(
                ["N03_001", "clean_n03_004"]
            ).agg(pl.count().alias("gun_count"))

            # 步驟 3：找出 gun_count > 1 的（表示有多個郡有同名町/村）
            duplicate_gun_towns = gun_town_counts.filter(pl.col("gun_count") > 1)

            # 步驟 4：與原 df join，標記需要補郡的記錄
            df = df.join(
                duplicate_gun_towns.select(
                    [
                        pl.col("N03_001"),
                        pl.col("clean_n03_004"),
                        pl.lit(True).alias("has_duplicate_name"),
                    ]
                ),
                how="left",
                on=["N03_001", "clean_n03_004"],
            )

            # 步驟 5：標記需要加郡名的郡轄町/村
            df = df.with_columns(
                (
                    pl.col("is_gun") & pl.col("has_duplicate_name").fill_null(False)
                ).alias("needs_gun_prefix")
            )

            # 生成 admin_2：根據 R1-R4 規則（R2 使用 R2' 預設簡潔模式）
            df = df.with_columns(
                pl.when(pl.col("is_regular_shi"))
                .then(pl.col("clean_n03_004"))  # R1: 普通市 → 直接顯示市名
                .when(pl.col("is_seirei_shi"))
                .then(
                    pl.col("clean_n03_004").fill_null("")
                    + pl.col("clean_n03_005").fill_null("")
                )  # R3: 政令市の区 → 政令市名＋區名
                .when(pl.col("needs_gun_prefix"))
                .then(
                    pl.col("clean_n03_003") + pl.lit(" ") + pl.col("clean_n03_004")
                )  # R2': 郡轄町/村（有真正同名衝突）→ 郡名＋町/村名
                .when(pl.col("is_gun"))
                .then(pl.col("clean_n03_004"))  # R2': 郡轄町/村（預設簡潔）→ 僅町/村名
                .otherwise(pl.col("clean_n03_003"))  # R4: 僅有郡名（罕見情況）
                .alias("admin_2")
            )

            # 選擇最終需要的欄位
            df = df.select(
                [
                    pl.col("longitude"),
                    pl.col("latitude"),
                    pl.col("N03_001").alias("admin_1"),  # 都道府縣
                    pl.col("admin_2"),  # 市區町村（根據 R1-R4 規則生成）
                    pl.lit("").alias("admin_3"),  # 空字串
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
