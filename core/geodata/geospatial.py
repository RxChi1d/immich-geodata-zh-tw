"""地理空間處理相關的共用 helper。

此模組以 ``GeoSpatialMixin`` 提供 Shapefile / GeoDataFrame 前處理工具，
以及階層式多樣化取樣。``GeoDataHandler`` 繼承此 mixin 即可使用。
"""

from __future__ import annotations

from typing import ClassVar

import geopandas as gpd
import numpy as np
import polars as pl
import pyproj

from core.utils import logger


class GeoSpatialMixin:
    """地理空間相關的靜態/實例工具方法。"""

    # 子類可選擇性覆寫的 Albers 等面積圓錐投影 proj4 字串
    ALBERS_PROJ4: ClassVar[str | None] = None

    @staticmethod
    def _gdf_to_polars(
        gdf: gpd.GeoDataFrame, *, fillna_object: bool = True
    ) -> pl.DataFrame:
        """移除幾何欄位、統一字串 dtype 並轉為 Polars DataFrame。

        Args:
            gdf: 已計算完經緯度的 GeoDataFrame。
            fillna_object: 是否將 object 欄位的 NaN 先填成空字串再轉型。
                True（預設）：NaN → ""，適用於後續會以 "" / "nan" 等 sentinel 做空值正規化者。
                False：保留原行為，`astype(str)` 會將 NaN 轉為字串 "nan" / "None"。

        Returns:
            已移除 geometry 並轉為 Polars 的 DataFrame。
        """
        gdf = gdf.drop(columns=["geometry"])
        for col in gdf.columns:
            if gdf[col].dtype != "object":
                continue
            series = gdf[col].fillna("") if fillna_object else gdf[col]
            gdf[col] = series.astype(str)
        return pl.from_pandas(gdf)

    def _calculate_centroids_utm(self, gdf: gpd.GeoDataFrame) -> gpd.GeoDataFrame:
        """使用動態 UTM 區選擇計算中心點（向量化）。

        結合 Albers 投影與動態 UTM 區選擇，提供高精確度的中心點計算。
        子類必須定義 `ALBERS_PROJ4` 類別屬性才能使用此方法。

        Args:
            gdf: 原始 GeoDataFrame。

        Returns:
            已附加 `longitude` 與 `latitude` 欄位的 GeoDataFrame。

        Raises:
            NotImplementedError: 當子類未定義 `ALBERS_PROJ4` 時。
        """
        if self.ALBERS_PROJ4 is None:
            raise NotImplementedError(
                f"{self.__class__.__name__} 必須定義 ALBERS_PROJ4 類別變數才能使用 "
                f"_calculate_centroids_utm"
            )

        if gdf.crs.to_epsg() != 4326:
            logger.info("正在轉換到 WGS84...")
            gdf = gdf.to_crs(epsg=4326)

        # Reason: 邊界框平均值對不規則形狀不準確，尤其在 UTM 區邊界附近
        logger.info("正在計算準確的幾何中心點（使用 Albers 投影）...")
        albers_crs = pyproj.CRS.from_proj4(self.ALBERS_PROJ4)

        gdf_albers = gdf.to_crs(albers_crs)
        centroids_albers = gdf_albers.geometry.centroid

        centroids_wgs84_temp = centroids_albers.to_crs(epsg=4326)
        center_lons = centroids_wgs84_temp.x

        logger.info("正在根據中心點經度決定 UTM 區...")
        utm_zones = ((center_lons + 180) / 6).astype(int) + 1
        utm_epsgs = 32600 + utm_zones

        gdf["_utm_zone"] = utm_zones
        gdf["_utm_epsg"] = utm_epsgs

        logger.info(f"識別到 {utm_epsgs.nunique()} 個不同的 UTM 區")

        longitudes = np.full(len(gdf), np.nan)
        latitudes = np.full(len(gdf), np.nan)

        # Reason: 每個 UTM 區需要不同投影，區內可一次處理所有幾何體（向量化）
        logger.info("正在按 UTM 區批次計算中心點...")
        for utm_epsg, group_idx in gdf.groupby("_utm_epsg").groups.items():
            group_gdf = gdf.iloc[group_idx]
            group_utm = group_gdf.to_crs(epsg=utm_epsg)
            centroids_utm = group_utm.geometry.centroid
            centroids_wgs84 = centroids_utm.to_crs(epsg=4326)

            longitudes[group_idx] = centroids_wgs84.x.values
            latitudes[group_idx] = centroids_wgs84.y.values

        gdf["longitude"] = longitudes
        gdf["latitude"] = latitudes

        return gdf.drop(columns=["_utm_zone", "_utm_epsg"])

    @staticmethod
    def get_diverse_sample(
        df: pl.DataFrame,
        n: int = 5,
    ) -> pl.DataFrame:
        """取得多樣化的資料樣本（階層式去重）。

        使用階層式去重策略，優先確保不同的省/道/市（admin_1），
        資料不足時才使用更細的層級（admin_2, admin_3, admin_4）。

        階層式邏輯：
        1. 先用 admin_1 去重，如果結果 >= n，回傳前 n 筆
        2. 如果不足，用 admin_1 + admin_2 去重，如果結果 >= n，回傳前 n 筆
        3. 依此類推到 admin_3, admin_4
        4. 如果所有層級都不足 n 筆，回傳所有去重後的結果

        Args:
            df: 來源 DataFrame。
            n: 要取樣的資料筆數（預設 5）。

        Returns:
            包含最多 n 筆多樣化資料的 DataFrame。

        Examples:
            >>> # 5 個不同的 admin_1，n=5 → 回傳 5 筆（每個 admin_1 一筆）
            >>> df = pl.DataFrame({
            ...     "admin_1": ["台北市", "新北市", "台中市", "台南市", "高雄市"],
            ...     "admin_2": ["中正區", "板橋區", "西屯區", "東區", "前金區"],
            ... })
            >>> result = GeoDataHandler.get_diverse_sample(df, n=5)
            >>> len(result)
            5

            >>> # 3 個 admin_1，n=5 → 先用 admin_1 得 3 筆，不足，
            >>> # 改用 admin_1+admin_2 得 5 筆
            >>> df = pl.DataFrame({
            ...     "admin_1": ["台北市", "台北市", "新北市", "新北市", "台中市"],
            ...     "admin_2": ["中正區", "大安區", "板橋區", "新莊區", "西屯區"],
            ... })
            >>> result = GeoDataHandler.get_diverse_sample(df, n=5)
            >>> len(result)
            5
        """
        diversity_columns = ["admin_1", "admin_2", "admin_3", "admin_4"]
        available_columns = [col for col in diversity_columns if col in df.columns]

        if not available_columns:
            return df.head(n)

        # 階層式去重：從最粗粒度（admin_1）開始，逐步擴展到更細的層級
        for level in range(1, len(available_columns) + 1):
            subset = available_columns[:level]
            result = df.unique(subset=subset, keep="first")

            if len(result) >= n:
                return result.head(n)

        return result
