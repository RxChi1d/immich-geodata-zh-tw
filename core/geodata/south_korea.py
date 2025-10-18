"""南韓地理資料處理器。"""

import polars as pl
import geopandas as gpd
import pyproj
import numpy as np
from pathlib import Path

from core.utils import logger
from core.geodata.base import GeoDataHandler, register_handler


# TODO: 註冊處理器
# @register_handler("KR")
class SouthKoreaGeoDataHandler(GeoDataHandler):
    """南韓地理資料處理器。

    資料來源：https://github.com/vuski/admdongkor
    使用動態 UTM 區選擇方法（結合 Albers 投影）計算中心點。
    """

    COUNTRY_NAME = "南韓"
    COUNTRY_CODE = "KR"
    TIMEZONE = "Asia/Seoul"

    # TODO: 未來實作韓文→繁體中文地名對照表

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
        #         特別是在 UTM 區邊界附近（南韓橫跨 126°E）
        logger.info("正在計算準確的幾何中心點（使用 Albers 投影）...")

        # 定義以南韓為中心的 Albers 等面積圓錐投影
        korea_albers = pyproj.CRS.from_proj4(
            "+proj=aea +lat_1=33 +lat_2=43 +lat_0=37 +lon_0=127.5 "
            "+x_0=0 +y_0=0 +datum=WGS84 +units=m +no_defs"
        )

        # 投影到 Albers 並計算中心點
        gdf_albers = gdf.to_crs(korea_albers)
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
        """從南韓行政區 GeoJSON 提取地理資料並轉換為標準化 CSV。

        處理南韓行政區域資料，計算中心點座標並按照行政區層級映射。

        Args:
            shapefile_path: 輸入 GeoJSON 檔案的路徑
            output_csv: 輸出 CSV 檔案的路徑

        處理步驟：
            1. 讀取 GeoJSON 並使用動態 UTM 區選擇計算中心點
            2. 提取行政區欄位（sidonm, sggnm, adm_nm）
            3. 解析 admin_3（從 adm_nm 移除 sidonm 和 sggnm）
            4. 生成標準化 CSV

        Admin 欄位填充邏輯：
            - admin_1: 廣域市/道（sidonm）
            - admin_2: 市/區/郡（sggnm）
            - admin_3: 洞/邑/面（解析 adm_nm）
            - admin_4: 保持空白

        Raises:
            Exception: GeoJSON 讀取失敗或資料處理錯誤時拋出
        """
        try:
            logger.info(f"正在讀取 GeoJSON: {shapefile_path}")

            # === 步驟 1: 讀取 GeoJSON 並計算中心點 ===
            gdf = gpd.read_file(shapefile_path)
            logger.info(
                f"成功讀取 GeoJSON，資料集大小: {gdf.shape[0]} 行 x {gdf.shape[1]} 列"
            )
            logger.info(f"原始座標系統: {gdf.crs}")

            # 使用動態 UTM 區選擇方法（結合 Albers 投影）計算中心點
            # Reason: 南韓橫跨多個 UTM 區（51N, 52N），
            #         需要根據每個幾何體的實際位置動態選擇 UTM 區以確保精確度
            logger.info("使用方法：動態 UTM 區選擇（結合 Albers 投影進行 UTM 區判定）")
            gdf = self._calculate_centroids_utm(gdf)

            # 移除不需要的幾何欄位
            gdf = gdf.drop(columns=["geometry"])

            # 統一資料型態：將 object 類型轉為字串並填充 NaN
            for col in gdf.columns:
                if gdf[col].dtype == "object":
                    gdf[col] = gdf[col].fillna("").astype(str)

            # 轉換為 Polars DataFrame 以進行高效的資料處理
            df = pl.from_pandas(gdf)

            # === 步驟 2: 提取並解析行政區欄位 ===
            # 先建立所需的基本欄位
            df = df.select(
                [
                    pl.col("longitude"),
                    pl.col("latitude"),
                    pl.col("sidonm"),
                    pl.col("sggnm"),
                    pl.col("adm_nm"),
                ]
            )

            # 解析 admin_3：從 adm_nm 移除 sidonm 和 sggnm
            # Reason: Polars 不支援動態模式的 str.replace，需使用 apply 或分步處理
            def extract_admin3(row):
                """從完整地名中提取 admin_3（洞/邑/面）。"""
                adm_nm = row["adm_nm"]
                sidonm = row["sidonm"]
                sggnm = row["sggnm"]

                # 移除 sidonm 和 sggnm
                result = adm_nm.replace(sidonm, "").replace(sggnm, "").strip()
                return result

            # 使用 map_rows 進行逐列處理
            df = df.with_columns(
                [
                    pl.struct(["adm_nm", "sidonm", "sggnm"])
                    .map_elements(
                        lambda row: row["adm_nm"]
                        .replace(row["sidonm"], "")
                        .replace(row["sggnm"], "")
                        .strip(),
                        return_dtype=pl.String,
                    )
                    .alias("admin_3")
                ]
            )

            # 重組為標準格式
            df = df.select(
                [
                    pl.col("longitude"),
                    pl.col("latitude"),
                    pl.col("sidonm").alias("admin_1"),  # 廣域市/道
                    pl.col("sggnm").alias("admin_2"),  # 市/區/郡
                    pl.col("admin_3"),  # 洞/邑/面
                    pl.lit("").alias("admin_4"),  # 空字串（保留欄位）
                    pl.lit("南韓").alias("country"),  # 國家名稱
                ]
            )

            # 排序：便於版本控制差異比對
            df = df.sort(["country", "admin_1", "admin_2"])

            # 過濾：移除無效座標
            df = df.filter(
                pl.col("longitude").is_not_null() & pl.col("latitude").is_not_null()
            )

            # 標準化座標精度（預設 8 位小數）
            df = self.standardize_coordinate_precision(df)

            # 建立輸出目錄並寫入 CSV
            output_path = Path(output_csv)
            output_path.parent.mkdir(parents=True, exist_ok=True)

            logger.info(f"正在儲存 CSV 檔案: {output_path}")
            df.write_csv(output_path)
            logger.info(f"成功儲存 CSV 檔案，共 {len(df)} 筆資料")

            # 顯示前五筆供檢查
            logger.info(df.head(5))

        except Exception as e:
            logger.error(f"處理 GeoJSON 時發生錯誤: {e}")
            raise
