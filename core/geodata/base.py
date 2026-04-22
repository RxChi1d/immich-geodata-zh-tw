"""地理資料處理器抽象基類（ETL 模式）。"""

from abc import ABC, abstractmethod
from datetime import date
from pathlib import Path

import polars as pl

from core.utils import logger, fill_admin_columns
from core.schemas import ADMIN1_SCHEMA, GEODATA_SCHEMA, CITIES_SCHEMA
from core.geodata.admin1 import Admin1Mixin
from core.geodata.geospatial import GeoSpatialMixin
from core.geodata.registry import register_handler, get_handler, get_all_handlers


class GeoDataHandler(GeoSpatialMixin, Admin1Mixin, ABC):
    """地理資料處理器（ETL 模式）。

    提供三階段處理流程：
        Extract: Shapefile → 標準化 CSV
        Transform: CSV → CITIES_SCHEMA DataFrame
        Load: 整合到主資料集

    子類必須定義的類別變數：
        COUNTRY_NAME: 國家名稱
        COUNTRY_CODE: ISO 3166-1 alpha-2 代碼
        TIMEZONE: IANA 時區名稱
    """

    # 基底共用設定，可由子類視需求覆寫
    COORD_DECIMAL_PLACES: int = 8

    # 子類必須覆寫的類別變數
    COUNTRY_NAME: str = ""
    COUNTRY_CODE: str = ""
    TIMEZONE: str = ""

    # Schema 引用（從 core.schemas 匯入，供子類繼承）
    ADMIN1_SCHEMA = ADMIN1_SCHEMA
    GEODATA_SCHEMA = GEODATA_SCHEMA
    CITIES_SCHEMA = CITIES_SCHEMA

    def __init__(self):
        if not self.COUNTRY_NAME:
            raise NotImplementedError(
                f"{self.__class__.__name__} 必須定義 COUNTRY_NAME 類別變數"
            )
        if not self.COUNTRY_CODE:
            raise NotImplementedError(
                f"{self.__class__.__name__} 必須定義 COUNTRY_CODE 類別變數"
            )
        if not self.TIMEZONE:
            raise NotImplementedError(
                f"{self.__class__.__name__} 必須定義 TIMEZONE 類別變數"
            )

        logger.info(
            f"初始化 {self.__class__.__name__} "
            f"(國家: {self.COUNTRY_NAME}, 代碼: {self.COUNTRY_CODE})"
        )

    @abstractmethod
    def extract_from_shapefile(
        self,
        shapefile_path: str,
        output_csv: str,
    ) -> None:
        """從 Shapefile 提取資料並儲存為標準化 CSV。

        Args:
            shapefile_path: Shapefile 檔案路徑。
            output_csv: 輸出 CSV 檔案路徑。
        """
        pass

    @classmethod
    def convert_to_cities_schema(
        cls, csv_path: str, base_geoname_id: int
    ) -> pl.DataFrame:
        """讀取 CSV 並轉換為 CITIES_SCHEMA 格式（共用實作）。

        此方法提供通用的城市資料轉換流程：
        1. 驗證輸入路徑並讀取 CSV
        2. 呼叫 prepare_cities_source 進行前處理
        3. 檢查必要欄位是否存在
        4. 分配 geoname_id 並映射 admin1_code
        5. 呼叫 build_cities_dataframe 建立輸出
        6. 寫入暫存檔案並回傳結果

        子類通常不需覆寫此方法，而是透過以下鉤子自訂行為：
        - prepare_cities_source: 前處理來源資料
        - build_cities_dataframe: 自訂 DataFrame 組裝邏輯

        Args:
            csv_path: 輸入 CSV 檔案路徑。
            base_geoname_id: geoname_id 起始值。
                當整合到現有資料集時，應傳入資料集中的最大 ID + 1 以避免衝突。

        Returns:
            符合 CITIES_SCHEMA 的 DataFrame。

        Raises:
            FileNotFoundError: 當 CSV 檔案不存在時。
            ValueError: 當 CSV 缺少必要欄位時。
        """
        logger.info(f"正在轉換 {cls.COUNTRY_NAME} 地理資料...")

        # Reason: 不做 exists() 預檢（TOCTOU），讓 read_csv 直接報錯後轉為中文訊息
        try:
            df = fill_admin_columns(pl.read_csv(csv_path))
        except FileNotFoundError as exc:
            error_msg = (
                f"輸入檔案不存在: {csv_path}\n"
                f"建議：請先執行 extract 階段以生成 CSV 檔案"
            )
            logger.error(error_msg)
            raise FileNotFoundError(error_msg) from exc
        logger.info(f"成功讀取 CSV，共 {df.height} 筆資料")

        # 呼叫前處理鉤子
        df = cls.prepare_cities_source(df)

        # 驗證必要欄位
        required_cols = ["admin_1", "admin_2", "latitude", "longitude"]
        missing_cols = [col for col in required_cols if col not in df.columns]
        if missing_cols:
            error_msg = (
                f"CSV 檔案缺少必要欄位: {missing_cols}\n"
                f"檔案路徑: {csv_path}\n"
                f"可用欄位: {df.columns}\n"
                f"建議：請檢查 extract_from_shapefile 的實作"
            )
            logger.error(error_msg)
            raise ValueError(error_msg)

        # 獲取 admin1_mapping
        admin1_mapping = cls.get_admin1_mapping(csv_path)

        # 生成唯一的 geoname_id（向量化）
        df = df.with_columns(
            (pl.int_range(pl.len(), dtype=pl.Int64) + base_geoname_id).alias(
                "geoname_id"
            )
        )

        # 將 admin_1 映射到 admin1_code（"XX.YY"）
        df = df.with_columns(
            pl.col("admin_1")
            .replace_strict(admin1_mapping, default=None, return_dtype=pl.String)
            .alias("admin1_code_full")
        )

        # 檢查是否有無法映射的 admin_1
        null_admin1_codes = df.filter(pl.col("admin1_code_full").is_null())
        if null_admin1_codes.height > 0:
            missing_names = null_admin1_codes["admin_1"].unique().to_list()
            logger.warning(
                f"以下 admin_1 無法映射到 admin1_code（將設為 None）: {missing_names}"
            )

        # 提取 admin1_code 的數字/字母部分（"XX.YY" -> "YY"；null 自然傳遞）
        df = df.with_columns(
            pl.col("admin1_code_full")
            .str.split(".")
            .list.last()
            .alias("admin1_code_mapped")
        )

        # 呼叫 build_cities_dataframe 建立輸出
        result = cls.build_cities_dataframe(df)

        # 寫入暫存檔案
        output_path = (
            Path("output") / f"{cls.COUNTRY_CODE.lower()}_geodata_converted.csv"
        )
        output_path.parent.mkdir(parents=True, exist_ok=True)
        result.write_csv(output_path)
        logger.info(f"已將轉換後的資料暫存至: {output_path}")

        logger.info(f"{cls.COUNTRY_NAME} 地理資料轉換完成，共 {result.height} 筆資料")
        logger.info(
            f"Geoname ID 範圍: {base_geoname_id} - "
            f"{base_geoname_id + result.height - 1}"
        )

        return result

    @classmethod
    def standardize_coordinate_precision(
        cls,
        df: pl.DataFrame,
        latitude_column: str = "latitude",
        longitude_column: str = "longitude",
    ) -> pl.DataFrame:
        """統一經緯度小數位數。

        Args:
            df: 需要處理的資料框。
            latitude_column: 緯度欄位名稱。
            longitude_column: 經度欄位名稱。

        Returns:
            已套用標準小數位數的資料框。

        Raises:
            ValueError: 當指定欄位不存在時。
        """
        required_cols = [latitude_column, longitude_column]
        missing_cols = [col for col in required_cols if col not in df.columns]
        if missing_cols:
            error_msg = (
                f"缺少必要欄位: {missing_cols}\n"
                "建議：請確認 extract_from_shapefile 的輸出欄位名稱"
            )
            logger.error(error_msg)
            raise ValueError(error_msg)

        return df.with_columns(
            pl.col(latitude_column)
            .round(cls.COORD_DECIMAL_PLACES)
            .alias(latitude_column),
            pl.col(longitude_column)
            .round(cls.COORD_DECIMAL_PLACES)
            .alias(longitude_column),
        )

    def _save_extract_csv(
        self,
        df: pl.DataFrame,
        output_csv: str,
        sort_columns: list[str] | None = None,
    ) -> None:
        """標準化並儲存 extract 階段產生的 CSV 檔案。

        執行標準收尾步驟：
        1. 全欄位排序
        2. 移除無效座標
        3. 標準化座標精度
        4. 建立輸出目錄
        5. 寫入 CSV
        6. 記錄日誌
        7. 顯示前五筆資料

        Args:
            df: 待儲存的 DataFrame。
            output_csv: 輸出 CSV 檔案路徑。
            sort_columns: 排序欄位列表。預設為完整欄位順序。

        Raises:
            Exception: 儲存過程中發生的任何錯誤。
        """
        # 預設排序欄位
        if sort_columns is None:
            sort_columns = [
                "country",
                "admin_1",
                "admin_2",
                "admin_3",
                "admin_4",
                "latitude",
                "longitude",
            ]

        # 全欄位排序可在資料更新時最小化 git diff，便於版本追蹤
        df = df.sort(sort_columns)

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

        # 顯示多樣化的資料樣本供檢查
        sample_df = self.get_diverse_sample(df, n=5)
        logger.info("資料預覽（多樣化取樣）：")
        logger.info(sample_df)

    @classmethod
    def prepare_cities_source(cls, df: pl.DataFrame) -> pl.DataFrame:
        """前處理城市來源資料的鉤子方法。

        預設行為為標準化空值並排序。
        子類可覆寫此方法以進行資料前處理，例如：
        - 正規化地名（去除空白、統一格式）
        - 過濾或合併特定記錄
        - 額外的欄位處理

        Args:
            df: 從 CSV 讀取的原始 DataFrame。

        Returns:
            前處理後的 DataFrame。

        Example:
            >>> class CustomHandler(GeoDataHandler):
            ...     @classmethod
            ...     def prepare_cities_source(cls, df: pl.DataFrame) -> pl.DataFrame:
            ...         # 清理 admin 欄位中的 "" 字串
            ...         for col in ["admin_1", "admin_2", "admin_3", "admin_4"]:
            ...             if col in df.columns:
            ...                 df = df.with_columns(
            ...                     pl.when(pl.col(col) == '""')
            ...                     .then(None)
            ...                     .otherwise(pl.col(col))
            ...                     .alias(col)
            ...                 )
            ...         return df.sort(["admin_1", "admin_2"])
        """
        # 標準化空值：將空字串、'""'、字面值 "nan"/"None" 轉為 None
        # Reason: pandas astype(str) 對 NaN/None 會產生 "nan"/"None" 字面值；
        #         extract 階段如未統一填補，這些字串會滲漏進 cities500 輸出
        null_sentinels = ["", '""', "nan", "None"]
        admin_cols = ["admin_1", "admin_2", "admin_3", "admin_4"]
        present_cols = [col for col in admin_cols if col in df.columns]
        if present_cols:
            df = df.with_columns(
                [
                    pl.when(pl.col(col).is_in(null_sentinels))
                    .then(None)
                    .otherwise(pl.col(col))
                    .alias(col)
                    for col in present_cols
                ]
            )

        # 排序以確保輸出穩定性
        sort_cols = [col for col in ["admin_1", "admin_2"] if col in df.columns]
        if sort_cols:
            df = df.sort(sort_cols)

        return df

    @classmethod
    def build_cities_dataframe(cls, df: pl.DataFrame) -> pl.DataFrame:
        """建立符合 CITIES_SCHEMA 的 DataFrame（可覆寫）。

        預設實作根據常見欄位需求建立輸出 DataFrame。
        子類可覆寫此方法以自訂欄位取值或處理邏輯。

        Args:
            df: 已經過前處理並分配 geoname_id、admin1_code 的 DataFrame。
                必須包含欄位：geoname_id, admin1_code_mapped, latitude, longitude, admin_2

        Returns:
            符合 CITIES_SCHEMA 的 DataFrame。

        Example:
            >>> class CustomHandler(GeoDataHandler):
            ...     @classmethod
            ...     def build_cities_dataframe(cls, df: pl.DataFrame) -> pl.DataFrame:
            ...         # 自訂欄位取值
            ...         from datetime import date
            ...         return pl.DataFrame({
            ...             "geoname_id": df["geoname_id"],
            ...             "name": df["custom_name_column"],  # 使用自訂欄位
            ...             "asciiname": df["custom_name_column"],
            ...             ...
            ...         }, schema=cls.CITIES_SCHEMA)
        """
        # 獲取今天的日期字串
        today_date_str = date.today().strftime("%Y-%m-%d")

        # 建立符合 CITIES_SCHEMA 的 DataFrame
        return pl.DataFrame(
            {
                "geoname_id": df["geoname_id"],
                "name": df["admin_2"],  # 預設使用 admin_2 作為地名
                "asciiname": df["admin_2"],
                "alternatenames": None,
                "latitude": df["latitude"],
                "longitude": df["longitude"],
                "feature_class": "A",
                "feature_code": "ADM2",  # 預設為 admin2 層級
                "country_code": cls.COUNTRY_CODE,
                "cc2": None,
                "admin1_code": df["admin1_code_mapped"],  # 使用 "XX" 部分
                "admin2_code": None,
                "admin3_code": None,
                "admin4_code": None,
                "population": 0,
                "elevation": None,
                "dem": None,
                "timezone": cls.TIMEZONE,
                "modification_date": today_date_str,
            },
            schema=cls.CITIES_SCHEMA,
        )

    def replace_in_dataset(
        self,
        input_df: pl.DataFrame,
        base_geoname_id: int,
        csv_path: str | None = None,
    ) -> tuple[pl.DataFrame, int]:
        """將轉換後的資料替換到主資料集中。

        Args:
            input_df: 主資料集 DataFrame。
            base_geoname_id: geoname_id 起始值（由呼叫者管理以避免衝突）。
            csv_path: CSV 檔案路徑（預設 meta_data/{country}_geodata.csv）。

        Returns:
            (已替換資料的 DataFrame, 使用的最大 geoname_id)
        """
        # 預設 CSV 路徑（使用實例的 COUNTRY_CODE）
        if csv_path is None:
            csv_path = f"meta_data/{self.COUNTRY_CODE.lower()}_geodata.csv"

        logger.info(f"開始使用 {self.COUNTRY_CODE} 地理資料替換現有資料")
        logger.info(f"使用 geoname_id 起始值: {base_geoname_id}")

        # 移除舊資料
        non_country_df = input_df.filter(pl.col("country_code") != self.COUNTRY_CODE)
        removed_count = input_df.height - non_country_df.height
        if removed_count > 0:
            logger.info(f"移除了 {removed_count} 筆舊的 {self.COUNTRY_CODE} 資料")
        else:
            logger.info(f"輸入資料中未找到需要移除的 {self.COUNTRY_CODE} 資料")

        # 轉換資料（使用類別方法）
        converted_df = self.__class__.convert_to_cities_schema(
            csv_path, base_geoname_id
        )

        # 計算使用的最大 ID（需要轉換為整數）
        max_id_used = converted_df.select(
            pl.col("geoname_id").cast(pl.Int64).max()
        ).item()
        logger.info(
            f"{self.COUNTRY_CODE} 使用的 ID 範圍: {base_geoname_id} - {max_id_used}"
        )

        # 合併新資料（新資料放在前面）
        output_df = converted_df.vstack(non_country_df)
        logger.info(f"添加了 {converted_df.height} 筆新的 {self.COUNTRY_CODE} 資料")
        logger.info(f"{self.COUNTRY_CODE} 資料替換完成")

        return output_df, max_id_used


# 註冊表 API 維持從 base 模組可匯入，減少呼叫端遷移成本
__all__ = [
    "GeoDataHandler",
    "register_handler",
    "get_handler",
    "get_all_handlers",
]
