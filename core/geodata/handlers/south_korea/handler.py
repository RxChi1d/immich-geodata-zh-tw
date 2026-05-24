"""南韓地理資料處理器。"""

from collections.abc import Callable
from typing import ClassVar

import polars as pl
import geopandas as gpd

from core.utils import logger
from core.utils.wikidata_translator import (
    TranslationDatasetBuilder,
    WikidataTranslator,
)
from core.geodata.base import GeoDataHandler, register_handler
from core.geodata.handlers.south_korea.translations import (
    ADMIN1_NAME_MAP,
    CITY_DISTRICT_REGEX,
    EXCLUDED_KEYWORDS,
    SEJONG_ADMIN2_MAP,
)


@register_handler("KR")
class SouthKoreaGeoDataHandler(GeoDataHandler):
    """南韓地理資料處理器。

    資料來源：https://github.com/vuski/admdongkor
    使用動態 UTM 區選擇方法（結合 Albers 投影）計算中心點。
    """

    COUNTRY_NAME: ClassVar[str] = "南韓"
    COUNTRY_CODE: ClassVar[str] = "KR"
    TIMEZONE: ClassVar[str] = "Asia/Seoul"

    # 以南韓為中心的 Albers 等面積圓錐投影，供 UTM 區判定使用
    ALBERS_PROJ4: ClassVar[str] = (
        "+proj=aea +lat_1=33 +lat_2=43 +lat_0=37 +lon_0=127.5 "
        "+x_0=0 +y_0=0 +datum=WGS84 +units=m +no_defs"
    )

    # 對照表與過濾關鍵字由 translations.py 集中維護
    CITY_DISTRICT_REGEX: ClassVar[str] = CITY_DISTRICT_REGEX
    ADMIN1_NAME_MAP: ClassVar[dict[str, str]] = ADMIN1_NAME_MAP
    SEJONG_ADMIN2_MAP: ClassVar[dict[str, str]] = SEJONG_ADMIN2_MAP

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
                    .then(pl.lit(None, dtype=pl.String))
                    .otherwise(pl.col("admin_3"))
                    .alias("admin_3"),
                ]
            )

            logger.info(
                "世宗特別自治市行政層級正規化完成：읍/면/동 已上移到 admin_2 層級"
            )

        return df

    def _normalize_city_district_hierarchy(self, df: pl.DataFrame) -> pl.DataFrame:
        """將市＋區合併名稱拆分並調整 admin 層級（向量化）。"""

        if "sggnm" not in df.columns:
            return df

        if "admin_4" not in df.columns:
            df = df.with_columns(pl.lit(None, dtype=pl.String).alias("admin_4"))

        # 使用 Polars 原生 regex 提取 named capture groups；不符合的 row 兩欄皆為 null
        df = df.with_columns(
            pl.col("sggnm").str.extract_groups(self.CITY_DISTRICT_REGEX).alias("_parts")
        ).unnest("_parts")

        split_mask = pl.col("_district").is_not_null()
        split_count = df.filter(split_mask).height
        if split_count > 0:
            logger.info(f"偵測到 {split_count} 筆市＋區合併名稱，正在拆分階層...")

        # Reason: with_columns 並行評估所有 expr，讀到的都是原始欄位值，
        #         因此可安全地在同一個 with_columns 中同時更新 sggnm/admin_3/admin_4
        df = df.with_columns(
            [
                pl.when(split_mask)
                .then(pl.col("_city"))
                .otherwise(pl.col("sggnm"))
                .alias("sggnm"),
                pl.when(split_mask)
                .then(pl.col("_district"))
                .otherwise(pl.col("admin_3"))
                .alias("admin_3"),
                pl.when(split_mask)
                .then(pl.col("admin_3"))
                .otherwise(pl.col("admin_4"))
                .alias("admin_4"),
            ]
        )

        return df.drop(["_city", "_district"])

    @staticmethod
    def _derive_admin3_column(df: pl.DataFrame) -> pl.DataFrame:
        """從完整行政區名稱拆出 admin_3。

        Args:
            df: 包含 sidonm, sggnm, adm_nm 欄位的 DataFrame。

        Returns:
            新增或覆蓋 admin_3 欄位後的 DataFrame。
        """

        def remove_parent_names(row: dict[str, str]) -> str:
            """移除每列各自的廣域與市區名稱。"""
            adm_nm = row["adm_nm"]
            sidonm = row["sidonm"]
            sggnm = row["sggnm"]

            return adm_nm.replace(sidonm, "").replace(sggnm, "").strip()

        # Reason: Polars 1.33.0 尚不支援以欄位表達式作為 str.replace_all 的 pattern；
        #         這裡每列的 sidonm/sggnm 都不同，因此保留逐列字串處理避免 extract 失敗。
        return df.with_columns(
            pl.struct(["adm_nm", "sidonm", "sggnm"])
            .map_elements(remove_parent_names, return_dtype=pl.String)
            .alias("admin_3")
        )

    @staticmethod
    def _build_candidate_filter() -> Callable[[str, dict], bool]:
        """建立候選過濾器，排除議會機構等非行政區實體。

        Returns:
            過濾器函式，接收 (name, metadata) 並回傳 bool
        """

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

            # 轉換為 Polars DataFrame（共用 helper 處理 geometry/字串轉換）
            df = self._gdf_to_polars(gdf)

            # === 步驟 2: 提取並解析行政區欄位 ===
            df = df.select(
                [
                    pl.col("latitude"),
                    pl.col("longitude"),
                    pl.col("sidonm"),
                    pl.col("sggnm"),
                    pl.col("adm_nm"),
                ]
            )

            # 解析 admin_3：從 adm_nm 逐欄移除 sidonm 與 sggnm 後 trim。
            df = self._derive_admin3_column(df)

            # === 步驟 2.5: 正規化特殊行政區結構（世宗特別自治市）===
            df = self._normalize_special_admin_structures(df)

            # === 步驟 2.6: 拆分市＋區組合名稱並調整層級 ===
            df = self._normalize_city_district_hierarchy(df)

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
            dataset_builder = TranslationDatasetBuilder(
                country_code="KR",
                source_lang="ko",
                target_lang="zh-tw",
            )

            # 步驟 3.1: 批次翻譯 Admin_1（廣域市/道）
            admin1_dataset = dataset_builder.build_admin1(
                df,
                name_field="sidonm",
            )
            admin1_results = translator.batch_translate(
                admin1_dataset,
                batch_size=32,
                show_progress=True,
            )

            admin1_lookup: dict[str, dict[str, str | None]] = {}
            for item in admin1_dataset:
                result = admin1_results.get(item.id, {})
                translated = result.get("translated", item.original_name)
                if item.original_name in self.ADMIN1_NAME_MAP:
                    translated = self.ADMIN1_NAME_MAP[item.original_name]
                admin1_lookup[item.original_name] = {
                    "translated": translated,
                    "qid": result.get("qid"),
                }

            # 步驟 3.2: 批次翻譯 Admin_2（市/區/郡）
            sejong_parent = "세종특별자치시"
            sejong_df = df.filter(pl.col("sidonm") == sejong_parent)
            sejong_lookup: dict[tuple[str, str], str] = {}
            if sejong_df.height > 0:
                sejong_names = sejong_df["sggnm"].unique().to_list()
                logger.info(
                    f"世宗特別自治市 Admin_2 直接使用手動對照表（{len(sejong_names)} 筆）"
                )
                for korean_name in sejong_names:
                    translated = self.SEJONG_ADMIN2_MAP.get(korean_name)
                    if translated:
                        sejong_lookup[(sejong_parent, korean_name)] = translated
                        logger.debug(f"  {korean_name} → {translated} (手動對照)")
                    else:
                        logger.warning(f"  {korean_name} 不在手動對照表中，保持原樣")
                        sejong_lookup[(sejong_parent, korean_name)] = korean_name

            admin2_source_df = df.filter(pl.col("sidonm") != sejong_parent)
            admin2_dataset = dataset_builder.build_admin2(
                admin2_source_df,
                parent_field="sidonm",
                name_field="sggnm",
                deduplicate=True,
            )

            parent_qids_map: dict[str, str] = {}
            for item in admin2_dataset:
                parent_name = item.parent_chain[-1]
                parent_info = admin1_lookup.get(parent_name)
                parent_qid = parent_info.get("qid") if parent_info else None
                if parent_qid:
                    parent_qids_map[item.id] = parent_qid

            admin2_results = translator.batch_translate(
                admin2_dataset,
                batch_size=32,
                parent_qids=parent_qids_map,
                show_progress=True,
                candidate_filter=candidate_filter,
            )

            admin2_lookup = dict(sejong_lookup)
            for item in admin2_dataset:
                result = admin2_results.get(
                    item.id,
                    {
                        "translated": item.original_name,
                        "qid": None,
                        "source": "original",
                        "used_lang": "original",
                        "parent_verified": False,
                    },
                )
                admin2_lookup[(item.parent_chain[-1], item.original_name)] = result.get(
                    "translated", item.original_name
                )

            logger.info(
                f"Admin_2 翻譯完成，唯一組合: {len(admin2_lookup)} "
                f"(含手動 {len(sejong_lookup)})"
            )

            # 步驟 3.3: 應用翻譯結果到 DataFrame
            logger.info("正在應用翻譯結果...")

            # Admin_1：用 replace(mapping) 映射，找不到的保留原韓文
            admin1_map = {
                ko_name: data["translated"] for ko_name, data in admin1_lookup.items()
            }

            # Admin_2：將 (sidonm, sggnm) → 翻譯 的 lookup 轉為 DataFrame 後 left join
            admin2_df = pl.DataFrame(
                {
                    "sidonm": [k[0] for k in admin2_lookup],
                    "sggnm": [k[1] for k in admin2_lookup],
                    "chinese_admin_2": list(admin2_lookup.values()),
                },
                schema={
                    "sidonm": pl.String,
                    "sggnm": pl.String,
                    "chinese_admin_2": pl.String,
                },
            )

            df = df.with_columns(
                pl.col("sidonm").replace(admin1_map).alias("chinese_admin_1"),
                # Reason: Admin_3 保留韓文原文以降低 API 請求次數
                pl.col("admin_3").alias("chinese_admin_3"),
            ).join(admin2_df, on=["sidonm", "sggnm"], how="left")

            # 找不到翻譯時 fallback 回韓文 sggnm
            df = df.with_columns(pl.col("chinese_admin_2").fill_null(pl.col("sggnm")))

            # 針對光州移除 Wikidata 消歧義括號
            # Reason: 光州的東區/西區在 Wikidata 中帶有 "(光州)" 消歧義標記，
            #         但 admin_1 已經標明是「光州」，不需要重複標註
            gwangju_parent = "광주광역시"
            gwangju_df_before = df.filter(pl.col("sidonm") == gwangju_parent)
            disambig_count_before = gwangju_df_before.filter(
                pl.col("chinese_admin_2").str.contains(r"\([^)]+\)")
            ).height

            df = df.with_columns(
                pl.when(pl.col("sidonm") == gwangju_parent)
                .then(
                    pl.col("chinese_admin_2").str.replace_all(r"\s*\([^)]+\)\s*$", "")
                )
                .otherwise(pl.col("chinese_admin_2"))
                .alias("chinese_admin_2")
            )

            if disambig_count_before > 0:
                logger.info(
                    f"已移除光州 {disambig_count_before} 筆 Admin_2 的 Wikidata 消歧義括號"
                )

            logger.info(f"Admin_1 翻譯數量: {len(admin1_map)}")
            logger.info(f"Admin_2 翻譯數量: {len(admin2_lookup)}")
            logger.info("Admin_3 保留韓文原文（未翻譯）")

            # 重組為標準格式
            df = df.select(
                [
                    pl.col("latitude"),
                    pl.col("longitude"),
                    pl.lit(self.COUNTRY_NAME).alias("country"),
                    pl.col("chinese_admin_1").alias("admin_1"),  # 繁體中文廣域市/道
                    pl.col("chinese_admin_2").alias("admin_2"),  # 繁體中文市/區/郡
                    pl.col("chinese_admin_3").alias("admin_3"),  # 繁體中文洞/邑/面
                    pl.col("admin_4"),
                ]
            )

            # 標準化並儲存 CSV
            self._save_extract_csv(df, output_csv)

        except Exception as e:
            logger.error(f"處理 GeoJSON 時發生錯誤: {e}")
            raise
