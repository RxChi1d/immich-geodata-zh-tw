"""日本地理資料處理器。"""

import polars as pl
import geopandas as gpd

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

    # 以日本為中心的 Albers 等面積圓錐投影，供 UTM 區判定使用
    ALBERS_PROJ4 = (
        "+proj=aea +lat_1=30 +lat_2=45 +lat_0=37.5 +lon_0=138 "
        "+x_0=0 +y_0=0 +datum=WGS84 +units=m +no_defs"
    )

    # 彈性設定：政令市 admin_2 顯示策略
    # True: 僅顯示市名（例：横浜市）- 提升顯示一致性，降低 Immich 誤判率
    # False: 顯示市名＋區名（例：横浜市中区）- 提供更細緻的行政區資訊
    SEIREI_SHI_CITY_NAME_ONLY = True

    def extract_from_shapefile(
        self,
        shapefile_path: str,
        output_csv: str,
    ) -> None:
        """從日本行政區 Shapefile 提取地理資料並轉換為標準化 CSV。

        處理日本國土數值情報的行政區域資料，計算中心點座標並按照
        R1-R5 規則生成 admin_2 欄位。詳細處理邏輯請參考 japan-admin-guidelines.md。

        Args:
            shapefile_path: 輸入 Shapefile 的路徑
            output_csv: 輸出 CSV 檔案的路徑

        處理步驟：
            1. 讀取 Shapefile 並使用動態 UTM 區選擇計算中心點
            2. 標準化空值處理（統一 null、空字串、"None"、"nan"）
            3. 識別五種行政區類型（R1-R5）
            4. 檢測郡轄町/村的同名衝突（R4 規則）
            5. 生成 admin_2 和 admin_3 欄位並輸出標準化 CSV

        Admin 欄位填充邏輯：
            - admin_1: 都道府縣（N03_001）
            - admin_2: 市區町村名（依 R1-R5 規則生成）
            - admin_3: 僅政令市在 SEIREI_SHI_CITY_NAME_ONLY=True 時填入區名（N03_005）
            - admin_4: 保持空白

        Raises:
            Exception: Shapefile 讀取失敗或資料處理錯誤時拋出
        """
        try:
            logger.info(f"正在讀取 Shapefile: {shapefile_path}")

            # === 步驟 1: 讀取 Shapefile 並計算中心點 ===
            gdf = gpd.read_file(shapefile_path)
            logger.info(
                f"成功讀取 Shapefile，資料集大小: {gdf.shape[0]} 行 x {gdf.shape[1]} 列"
            )
            logger.info(f"原始座標系統: {gdf.crs}")

            # 使用動態 UTM 區選擇方法（結合 Albers 投影）計算中心點
            # Reason: 日本橫跨多個 UTM 區（53N, 54N, 55N），
            #         需要根據每個幾何體的實際位置動態選擇 UTM 區以確保精確度
            logger.info("使用方法：動態 UTM 區選擇（結合 Albers 投影進行 UTM 區判定）")
            gdf = self._calculate_centroids_utm(gdf)

            # 轉換為 Polars DataFrame（共用 helper 處理 geometry/字串轉換）
            df = self._gdf_to_polars(gdf)

            # === 步驟 2: 選擇並標準化欄位 ===
            df = df.select(
                [
                    pl.col("latitude"),
                    pl.col("longitude"),
                    pl.col("N03_001"),  # 都道府縣
                    pl.col("N03_003"),  # 郡名 or 政令市名（舊版資料格式）
                    pl.col("N03_004"),  # 市區町村名
                    pl.col("N03_005"),  # 政令市之區（新版資料格式）
                ]
            )

            # 標準化空值：將 null、空字串、"None"、"nan" 統一轉為 None
            # Reason: 原始資料可能包含多種形式的空值表示，
            #         統一處理後才能正確執行後續的 is_null() 判斷
            null_sentinels = ["", "None", "nan"]
            df = df.with_columns(
                [
                    pl.when(
                        pl.col(col).is_not_null() & ~pl.col(col).is_in(null_sentinels)
                    )
                    .then(pl.col(col))
                    .otherwise(None)
                    .alias(f"clean_{col.lower()}")
                    for col in ["N03_003", "N03_004", "N03_005"]
                ]
            )

            # 過濾無效資料：至少需要有一個行政區名稱欄位
            df = df.filter(
                pl.col("clean_n03_003").is_not_null()
                | pl.col("clean_n03_004").is_not_null()
                | pl.col("clean_n03_005").is_not_null()
            )

            # === 步驟 3: 識別五種行政區類型（R1-R5）===
            # 根據 N03_003、N03_004、N03_005 的組合判斷行政區類型
            df = df.with_columns(
                [
                    # R1: 普通市
                    # 範例：北海道釧路市
                    (
                        pl.col("clean_n03_003").is_null()
                        & pl.col("clean_n03_004").str.ends_with("市").fill_null(False)
                        & pl.col("clean_n03_005").is_null()
                    ).alias("is_regular_shi"),
                    # R2: 直轄町/村/特別區
                    # 範例：東京都小笠原村、東京都千代田區
                    (
                        pl.col("clean_n03_003").is_null()
                        & pl.col("clean_n03_004").is_not_null()
                        & pl.col("clean_n03_004")
                        .str.ends_with("市")
                        .fill_null(False)
                        .not_()
                        & pl.col("clean_n03_005").is_null()
                    ).alias("is_direct_town"),
                    # R3: 政令指定都市的區
                    # 範例：神奈川県横浜市中区
                    # 註：admin_2 僅輸出市名以提升顯示一致性與降低 Immich 誤判率，
                    #     但保留各區的獨立記錄與中心點以維持定位精度
                    pl.col("clean_n03_005").is_not_null().alias("is_seirei_shi"),
                    # R4: 郡轄町/村
                    # 範例：新潟県岩船郡関川村
                    pl.col("clean_n03_003")
                    .str.ends_with("郡")
                    .fill_null(False)
                    .alias("is_gun"),
                ]
            )

            # === 步驟 4: R4 規則特別處理 - 檢測郡轄町/村的同名衝突 ===
            # 預設策略：簡潔優先（僅顯示町/村名）
            # 衝突判定：同一都道府縣內，多個郡擁有完全同名的町/村時才補郡名
            # 範例說明：
            #   - 不衝突：「釧路市 vs 釧路町」（尾碼不同）-> 不補郡
            #   - 真衝突：「古宇郡泊村 vs 国後郡泊村」（完全同名）-> 補郡

            # Reason: 同一行政區可能有多個多邊形（飛地、離島），需先去重避免重複計算
            unique_gun_towns = (
                df.filter(pl.col("is_gun"))
                .select(["N03_001", "clean_n03_003", "clean_n03_004"])
                .unique()
            )

            gun_town_counts = unique_gun_towns.group_by(
                ["N03_001", "clean_n03_004"]
            ).agg(pl.count().alias("gun_count"))

            duplicate_gun_towns = gun_town_counts.filter(pl.col("gun_count") > 1)

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

            df = df.with_columns(
                (
                    pl.col("is_gun") & pl.col("has_duplicate_name").fill_null(False)
                ).alias("needs_gun_prefix")
            )

            # === 步驟 5: 生成 admin_2 欄位（依 R1-R5 規則）===
            # 使用 when-then-otherwise 鏈式判斷，優先級由上至下
            seirei_shi_output = (
                pl.col("clean_n03_004")
                if self.SEIREI_SHI_CITY_NAME_ONLY
                else (
                    pl.col("clean_n03_004").fill_null("")
                    + pl.col("clean_n03_005").fill_null("")
                )
            )

            df = df.with_columns(
                pl.when(pl.col("is_regular_shi"))
                .then(pl.col("clean_n03_004"))  # R1: 普通市
                .when(pl.col("is_direct_town"))
                .then(pl.col("clean_n03_004"))  # R2: 直轄町/村/特別區
                .when(pl.col("is_seirei_shi"))
                .then(seirei_shi_output)  # R3: 政令市
                .when(pl.col("needs_gun_prefix"))
                .then(
                    pl.col("clean_n03_003") + pl.col("clean_n03_004")
                )  # R4: 郡轄町/村（同名衝突）
                .when(pl.col("is_gun"))
                .then(pl.col("clean_n03_004"))  # R4: 郡轄町/村（預設）
                .otherwise(pl.col("clean_n03_003"))  # R5: 僅郡名（罕見）
                .alias("admin_2")
            )

            # === 步驟 6: 輸出標準化 CSV ===
            df = df.select(
                [
                    pl.col("latitude"),
                    pl.col("longitude"),
                    pl.lit(self.COUNTRY_NAME).alias("country"),
                    pl.col("N03_001").alias("admin_1"),  # 都道府縣
                    pl.col("admin_2"),
                    # admin_3：僅政令市在 SEIREI_SHI_CITY_NAME_ONLY=True 時填入區名
                    pl.when(
                        pl.col("is_seirei_shi") & pl.lit(self.SEIREI_SHI_CITY_NAME_ONLY)
                    )
                    .then(pl.col("clean_n03_005"))
                    .otherwise(pl.lit(None, dtype=pl.String))
                    .alias("admin_3"),
                    pl.lit(None, dtype=pl.String).alias("admin_4"),
                ]
            )

            # 標準化並儲存 CSV
            self._save_extract_csv(df, output_csv)

        except Exception as e:
            logger.error(f"處理 Shapefile 時發生錯誤: {e}")
            raise
