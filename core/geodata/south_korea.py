"""南韓地理資料處理器。"""

import polars as pl
import geopandas as gpd
import pyproj
import numpy as np
from collections.abc import Callable
from pathlib import Path

from core.utils import logger
from core.utils.wikidata_translator import WikidataTranslator
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

    def _normalize_special_admin_structures(self, df: pl.DataFrame) -> pl.DataFrame:
        """正規化特殊行政區結構（如世宗特別自治市）。

        世宗特別自治市是南韓唯一的單層制特別自治市，沒有傳統的市/郡/區層級。
        行政層級直接從廣域市到讀/面/洞。

        為了確保 cities500 資料的 name 欄位有值（預設使用 admin_2），
        需要將 admin_3（읍/면/동）上移到 admin_2，以便翻譯和顯示。

        Args:
            df: 包含 sidonm, sggnm, admin_3 欄位的 DataFrame

        Returns:
            正規化後的 DataFrame
        """
        # 檢測條件：sidonm == "세종특별자치시" 且 sggnm 不是真實的行政區名稱
        # Reason: 世宗的真實行政區應該是읍/면/동，如果不是就表示是機構名稱（議會、市廳等）
        sejong_mask = (pl.col("sidonm") == "세종특별자치시") & (
            ~pl.col("sggnm").str.ends_with("읍")
            & ~pl.col("sggnm").str.ends_with("면")
            & ~pl.col("sggnm").str.ends_with("동")
        )

        sejong_count = df.filter(sejong_mask).height
        if sejong_count > 0:
            logger.info(
                f"偵測到 {sejong_count} 筆世宗特別自治市記錄，正在正規化行政層級..."
            )

            df = df.with_columns(
                [
                    # 將 admin_3 上移到 sggnm
                    pl.when(sejong_mask)
                    .then(pl.col("admin_3"))
                    .otherwise(pl.col("sggnm"))
                    .alias("sggnm"),
                    # 將原 admin_3 清空（世宗沒有更下層級）
                    pl.when(sejong_mask)
                    .then(pl.lit(""))
                    .otherwise(pl.col("admin_3"))
                    .alias("admin_3"),
                ]
            )

            logger.info(
                "世宗特別自治市行政層級正規化完成：읍/면/동 已上移到 admin_2 層級"
            )

        return df

    @staticmethod
    def _build_candidate_filter() -> Callable[[str, dict], bool]:
        """建立候選過濾器，排除議會機構等非行政區實體。

        Returns:
            過濾器函式，接收 (name, metadata) 並回傳 bool
        """
        # 需要排除的關鍵字（多語言）
        # Reason: 防止將議會機構、政府機關誤判為行政區
        EXCLUDED_KEYWORDS = [
            "의회",  # 韓文：議會
            "議會",  # 中文：議會
            "council",  # 英文：議會
            "assembly",  # 英文：議會
            "委員會",  # 中文：委員會
            "legislature",  # 英文：立法機構
            "청",  # 韓文：廳
            "廳",  # 中文：廳
            "government",  # 英文：政府
            "교육청",  # 韓文：教育廳
            "시청",  # 韓文：市廳
        ]

        def filter_func(name: str, metadata: dict) -> bool:
            """過濾候選項：排除包含議會相關關鍵字的候選。

            Args:
                name: 地名（未使用，保留以符合介面）
                metadata: 包含 qid 和 labels 的字典

            Returns:
                True 保留此候選，False 排除此候選
            """
            labels = metadata.get("labels", {})

            # 檢查所有語言的標籤
            for lang_code, label in labels.items():
                label_lower = label.lower()
                for keyword in EXCLUDED_KEYWORDS:
                    if keyword.lower() in label_lower:
                        logger.debug(
                            f"過濾掉候選 {metadata.get('qid')}: "
                            f"標籤 [{lang_code}] '{label}' 包含關鍵字 '{keyword}'"
                        )
                        return False  # 排除此候選

            return True  # 保留此候選

        return filter_func

    def extract_from_shapefile(
        self,
        shapefile_path: str,
        output_csv: str,
    ) -> None:
        """從南韓行政區 GeoJSON 提取地理資料並轉換為標準化 CSV。

        處理南韓行政區域資料，計算中心點座標並按照行政區層級映射。

        Args:
            shapefile_path: 輸入 GeoJSON 檔案的路徑
            output_csv: 輸出 CSV 檔案的路徑

        處理步驟：
            1. 讀取 GeoJSON 並使用動態 UTM 區選擇計算中心點
            2. 提取行政區欄位（sidonm, sggnm, adm_nm）
            3. 解析 admin_3（從 adm_nm 移除 sidonm 和 sggnm）
            4. 使用 Wikidata 翻譯為繁體中文（Admin_1 和 Admin_2）
            5. 生成標準化 CSV

        Admin 欄位填充邏輯：
            - admin_1: 廣域市/道（sidonm → 繁體中文，優先使用內建對照表）
            - admin_2: 市/區/郡（sggnm → 繁體中文，使用 Wikidata）
            - admin_3: 洞/邑/面（保留韓文原文）
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

            # === 步驟 2.5: 正規化特殊行政區結構（世宗特別自治市）===
            df = self._normalize_special_admin_structures(df)

            # === 步驟 3: 使用 Wikidata 翻譯為繁體中文 ===
            logger.info("正在初始化 Wikidata 翻譯工具...")
            translator = WikidataTranslator(
                source_lang="ko",
                target_lang="zh-tw",
                fallback_langs=["zh-hant", "zh", "en", "ko"],
                cache_path="geoname_data/KR_wikidata_cache.json",
                use_opencc=True,
            )

            # 建立候選過濾器（用於排除議會機構等非行政區實體）
            candidate_filter = self._build_candidate_filter()

            # 步驟 3.1: 批次翻譯 Admin_1（廣域市/道）
            logger.info("正在批次翻譯 Admin_1（廣域市/道）...")
            unique_admin1 = df["sidonm"].unique().to_list()
            admin1_qids = {}  # 儲存 Admin_1 的 QID 用於 P131 驗證

            # 批次翻譯所有 Admin_1（主要為批次取得 QID）
            admin1_translations = translator.batch_translate(
                unique_admin1, show_progress=True
            )

            # 套用內建對照表覆蓋翻譯結果
            for ko_name, result in admin1_translations.items():
                # Reason: 使用內建對照表優先，確保使用台灣慣用簡稱
                if ko_name in self.ADMIN1_NAME_MAP:
                    admin1_qids[ko_name] = {
                        "translated": self.ADMIN1_NAME_MAP[ko_name],
                        "qid": result.get("qid"),
                    }
                else:
                    admin1_qids[ko_name] = {
                        "translated": result.get("translated", ko_name),
                        "qid": result.get("qid"),
                    }

            # 步驟 3.2: 按 Admin_1 分組批次翻譯 Admin_2（市/區/郡）
            logger.info("正在按 Admin_1 分組批次翻譯 Admin_2（市/區/郡）...")
            admin2_translations = {}

            # Reason: 按 Admin_1 分組翻譯，確保同名 Admin_2 使用正確的 parent QID
            for admin1_ko_name, admin1_data in admin1_qids.items():
                admin1_qid = admin1_data.get("qid")
                if not admin1_qid:
                    continue

                # 取得此 Admin_1 下的所有 Admin_2
                admin2_list = (
                    df.filter(pl.col("sidonm") == admin1_ko_name)["sggnm"]
                    .unique()
                    .to_list()
                )

                logger.info(
                    f"正在翻譯 {admin1_data['translated']} 下的 {len(admin2_list)} 個 Admin_2..."
                )

                # 為這組 Admin_2 建立統一的 parent_qids
                parent_qids = {name: admin1_qid for name in admin2_list}

                # 批次翻譯這組 Admin_2
                group_translations = translator.batch_translate(
                    admin2_list,
                    parent_qids=parent_qids,
                    show_progress=False,  # 避免進度條混亂
                    candidate_filter=candidate_filter,  # 過濾議會機構等非行政區實體
                )

                # 合併到總翻譯結果
                admin2_translations.update(group_translations)

            logger.info(f"Admin_2 翻譯完成，共 {len(admin2_translations)} 個唯一名稱")

            # 步驟 3.3: 建立對照字典並應用到 DataFrame
            logger.info("正在應用翻譯結果...")

            # 建立 Admin_1 對照字典
            admin1_map = {
                ko_name: data["translated"] for ko_name, data in admin1_qids.items()
            }

            # 建立 Admin_2 對照字典
            admin2_map = {
                ko_name: data.get("translated", ko_name)
                for ko_name, data in admin2_translations.items()
            }

            # 應用翻譯到 DataFrame
            df = df.with_columns(
                [
                    pl.col("sidonm")
                    .map_elements(
                        lambda x: admin1_map.get(x, x), return_dtype=pl.String
                    )
                    .alias("chinese_admin_1"),
                    pl.col("sggnm")
                    .map_elements(
                        lambda x: admin2_map.get(x, x), return_dtype=pl.String
                    )
                    .alias("chinese_admin_2"),
                    # Reason: Admin_3 保留韓文原文以降低 API 請求次數
                    pl.col("admin_3").alias("chinese_admin_3"),
                ]
            )

            # 顯示翻譯統計
            logger.info(f"Admin_1 翻譯數量: {len(admin1_qids)}")
            logger.info(f"Admin_2 翻譯數量: {len(admin2_translations)}")
            logger.info("Admin_3 保留韓文原文（未翻譯）")

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
