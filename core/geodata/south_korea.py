"""南韓地理資料處理器。"""

import polars as pl
import geopandas as gpd
import pyproj
import numpy as np
from pathlib import Path

from core.utils import logger
from core.utils.google_geocoding import GoogleGeocodingClient
from core.geodata.base import GeoDataHandler, register_handler


@register_handler("KR")
class SouthKoreaGeoDataHandler(GeoDataHandler):
    """南韓地理資料處理器。

    資料來源：https://github.com/vuski/admdongkor
    使用動態 UTM 區選擇方法（結合 Albers 投影）計算中心點。
    """

    COUNTRY_NAME = "南韓"
    COUNTRY_CODE = "KR"
    TIMEZONE = "Asia/Seoul"

    # 廣域市/道名稱對照表（韓文 → 台灣常用繁體中文名稱）
    # Reason: 使用台灣地圖常見的簡潔名稱，而非 Google Maps 的正式官方名稱
    ADMIN1_NAME_MAP = {
        # 特別市
        "서울특별시": "首爾",
        # 廣域市（6個）
        "부산광역시": "釜山",
        "대구광역시": "大邱",
        "인천광역시": "仁川",
        "광주광역시": "光州",
        "대전광역시": "大田",
        "울산광역시": "蔚山",
        # 特別自治市
        "세종특별자치시": "世宗",
        # 道（8個）
        "경기도": "京畿道",
        "강원특별자치도": "江原道",
        "충청북도": "忠清北道",
        "충청남도": "忠清南道",
        "전북특별자치도": "全羅北道",
        "전라남도": "全羅南道",
        "경상북도": "慶尚北道",
        "경상남도": "慶尚南道",
        "제주특별자치도": "濟州",
    }

    def _translate_to_chinese(
        self,
        latitude: float,
        longitude: float,
        korean_admin1: str,
        korean_admin2: str,
        korean_admin3: str,
        google_client: GoogleGeocodingClient | None,
    ) -> tuple[str, str, str]:
        """將韓文地名轉換為繁體中文。

        翻譯策略：
        - admin_1: 使用內建對照表（確保使用台灣常見的簡潔名稱）
        - admin_2/admin_3: 使用 Google Geocoding API（利用其翻譯能力）

        Args:
            latitude: 緯度
            longitude: 經度
            korean_admin1: 韓文 admin_1 (廣域市/道)
            korean_admin2: 韓文 admin_2 (市/區/郡)
            korean_admin3: 韓文 admin_3 (洞/邑/面)
            google_client: Google Geocoding API 客戶端（若為 None 則保留韓文原名）

        Returns:
            (中文 admin_1, 中文 admin_2, 中文 admin_3) 元組
        """
        # admin_1: 使用對照表翻譯（確保使用台灣慣用名稱）
        chinese_admin1 = self.ADMIN1_NAME_MAP.get(korean_admin1, korean_admin1)

        # admin_2/admin_3: 未提供 API 金鑰時，保留韓文原名
        if not google_client:
            return chinese_admin1, korean_admin2, korean_admin3

        try:
            # 使用反向地理編碼查詢 admin_2 和 admin_3 的繁體中文地名
            data = google_client.geocode(
                latlng=(latitude, longitude), language="zh-TW"
            )

            if not data:
                logger.warning(f"無法取得繁體中文地名: ({latitude}, {longitude})")
                return chinese_admin1, korean_admin2, korean_admin3

            # 提取 admin_2 和 admin_3 的繁體中文地名
            # Reason: 優先尋找縣市等級行政區（locality 或等同層級），若無再回退
            chinese_admin2 = google_client.extract_component(
                data,
                [
                    "administrative_area_level_2",
                    "locality",
                    "sublocality_level_1",
                ],
            )
            chinese_admin3 = google_client.extract_component(
                data,
                [
                    "sublocality_level_2",
                    "administrative_area_level_3",
                    "neighborhood",
                ],
            )

            # Reason: 使用韓文原名作為備援（若 API 未回傳對應組件）
            return (
                chinese_admin1,
                chinese_admin2 or korean_admin2,
                chinese_admin3 or korean_admin3,
            )

        except Exception as e:
            logger.error(f"Google API 查詢失敗: ({latitude}, {longitude}) - {e}")
            return chinese_admin1, korean_admin2, korean_admin3

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

    def extract_from_shapefile(
        self,
        shapefile_path: str,
        output_csv: str,
        *,
        google_api_key: str | None = None,
    ) -> None:
        """從南韓行政區 GeoJSON 提取地理資料並轉換為標準化 CSV。

        處理南韓行政區域資料，計算中心點座標並按照行政區層級映射。

        Args:
            shapefile_path: 輸入 GeoJSON 檔案的路徑
            output_csv: 輸出 CSV 檔案的路徑
            google_api_key: Google Cloud API 金鑰（選填，用於繁體中文翻譯）

        處理步驟：
            1. 讀取 GeoJSON 並使用動態 UTM 區選擇計算中心點
            2. 提取行政區欄位（sidonm, sggnm, adm_nm）
            3. 解析 admin_3（從 adm_nm 移除 sidonm 和 sggnm）
            4. 使用 Google Geocoding API 轉換為繁體中文（選填）
            5. 生成標準化 CSV

        Admin 欄位填充邏輯：
            - admin_1: 廣域市/道（sidonm → 繁體中文）
            - admin_2: 市/區/郡（sggnm → 繁體中文）
            - admin_3: 洞/邑/面（解析 adm_nm → 繁體中文）
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

            # === 步驟 3: 使用 Google Geocoding API 翻譯為繁體中文（選填）===
            # 初始化 Google API 客戶端（如果有提供 API 金鑰）
            google_client = None
            if google_api_key:
                logger.info("正在初始化 Google Geocoding API 客戶端...")
                google_client = GoogleGeocodingClient(google_api_key)
                logger.info(f"將使用 Google API 翻譯 {len(df)} 筆地名為繁體中文")
            else:
                logger.info("未提供 Google API 金鑰，將保留韓文原名")

            # 應用翻譯（逐列處理）
            logger.info("正在翻譯地名...")
            chinese_df = df.select(
                ["latitude", "longitude", "sidonm", "sggnm", "admin_3"]
            ).map_rows(
                lambda row: tuple(
                    self._translate_to_chinese(
                        latitude=row[0],
                        longitude=row[1],
                        korean_admin1=row[2],
                        korean_admin2=row[3],
                        korean_admin3=row[4],
                        google_client=google_client,
                    )
                )
            )

            df = df.with_columns(
                [
                    chinese_df.to_series(0).alias("chinese_admin_1"),
                    chinese_df.to_series(1).alias("chinese_admin_2"),
                    chinese_df.to_series(2).alias("chinese_admin_3"),
                ]
            )

            # 顯示 API 使用統計
            if google_client:
                logger.info(f"Google API 總查詢次數: {google_client.request_count}")

            # 重組為標準格式
            df = df.select(
                [
                    pl.col("longitude"),
                    pl.col("latitude"),
                    pl.col("chinese_admin_1").alias("admin_1"),  # 繁體中文廣域市/道
                    pl.col("chinese_admin_2").alias("admin_2"),  # 繁體中文市/區/郡
                    pl.col("chinese_admin_3").alias("admin_3"),  # 繁體中文洞/邑/面
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
