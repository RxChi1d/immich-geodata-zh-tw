"""地理資料處理器抽象基類（ETL 模式）。"""

from abc import ABC, abstractmethod
import polars as pl
from core.utils import logger
from core.schemas import ADMIN1_SCHEMA, GEODATA_SCHEMA, CITIES_SCHEMA


class GeoDataHandler(ABC):
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

    # Schema 引用（從 core.schemas 匯入，供子類繼承）
    ADMIN1_SCHEMA = ADMIN1_SCHEMA
    GEODATA_SCHEMA = GEODATA_SCHEMA
    CITIES_SCHEMA = CITIES_SCHEMA

    # 子類必須覆寫的類別變數
    COUNTRY_NAME: str = ""
    COUNTRY_CODE: str = ""
    TIMEZONE: str = ""

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
    def extract_from_shapefile(self, shapefile_path: str, output_csv: str) -> None:
        """從 Shapefile 提取資料並儲存為標準化 CSV。

        Args:
            shapefile_path: Shapefile 檔案路徑。
            output_csv: 輸出 CSV 檔案路徑。
        """
        pass

    @abstractmethod
    def convert_to_cities_schema(
        self, csv_path: str, base_geoname_id: int
    ) -> pl.DataFrame:
        """讀取 CSV 並轉換為 CITIES_SCHEMA 格式。

        Args:
            csv_path: 輸入 CSV 檔案路徑。
            base_geoname_id: geoname_id 起始值。
                當整合到現有資料集時，應傳入資料集中的最大 ID + 1 以避免衝突。

        Returns:
            符合 CITIES_SCHEMA 的 DataFrame。
        """
        pass

    @classmethod
    def prepare_admin1_source(cls, df: pl.DataFrame) -> pl.DataFrame:
        """前處理 admin1 來源資料的鉤子方法。

        預設行為為直接回傳輸入 DataFrame。
        子類可覆寫此方法以進行資料前處理，例如：
        - 正規化行政區名稱（去除空白、替換舊稱）
        - 額外排序或過濾特定記錄
        - 合併或分割欄位

        Args:
            df: 從 CSV 讀取的原始 DataFrame。

        Returns:
            前處理後的 DataFrame。

        Example:
            >>> class CustomHandler(GeoDataHandler):
            ...     @classmethod
            ...     def prepare_admin1_source(cls, df: pl.DataFrame) -> pl.DataFrame:
            ...         # 正規化名稱，移除前後空白
            ...         return df.with_columns(
            ...             pl.col("admin_1").str.strip_chars().alias("admin_1")
            ...         )
        """
        return df

    @classmethod
    def generate_admin1_records(
        cls, csv_path: str, base_geoname_id: int
    ) -> pl.DataFrame:
        """從地理資料 CSV 產生 admin1 記錄（預設實作）。

        此方法提供通用的 admin1 記錄產生流程：
        1. 讀取 CSV 檔案
        2. 呼叫 prepare_admin1_source 進行前處理
        3. 提取唯一的 admin_1 值並排序
        4. 透過 get_admin1_mapping 取得或生成 mapping
        5. 分配 geoname_id 並建立符合 ADMIN1_SCHEMA 的 DataFrame

        子類可選擇：
        - 覆寫 prepare_admin1_source 進行資料前處理
        - 完全覆寫此方法以實作特殊邏輯

        Args:
            csv_path: extract_from_shapefile 產生的 CSV 路徑。
            base_geoname_id: geoname_id 起始值。

        Returns:
            符合 ADMIN1_SCHEMA 的 DataFrame。

        Raises:
            FileNotFoundError: 當 CSV 檔案不存在時。
            ValueError: 當 CSV 缺少 admin_1 欄位或無有效資料時。

        說明:
            此方法用於產生「新的」admin1 記錄，這些記錄會取代 admin1CodesASCII.txt 中
            對應國家的資料。例如臺灣需要將縣市層級提升為 admin1。
        """
        from pathlib import Path

        logger.info(f"正在為 {cls.COUNTRY_NAME} 生成 admin1 記錄...")

        # 驗證檔案存在
        input_file = Path(csv_path)
        if not input_file.exists():
            error_msg = (
                f"輸入檔案不存在: {input_file}\n"
                f"建議：請先執行 extract 階段以生成 CSV 檔案"
            )
            logger.error(error_msg)
            raise FileNotFoundError(error_msg)

        # 讀取 CSV
        df = pl.read_csv(csv_path)

        # 呼叫前處理鉤子
        df = cls.prepare_admin1_source(df)

        # 驗證必要欄位
        if "admin_1" not in df.columns:
            error_msg = (
                f"CSV 檔案缺少 'admin_1' 欄位\n"
                f"檔案路徑: {csv_path}\n"
                f"可用欄位: {df.columns}\n"
                f"建議：請檢查 extract_from_shapefile 的實作"
            )
            logger.error(error_msg)
            raise ValueError(error_msg)

        # 提取唯一的 admin_1 並排序
        unique_admin1 = sorted(df["admin_1"].unique().to_list())

        if not unique_admin1:
            error_msg = "CSV 檔案中沒有有效的 admin_1 資料"
            logger.error(error_msg)
            raise ValueError(error_msg)

        # 獲取 admin1_mapping
        admin1_mapping = cls.get_admin1_mapping(csv_path)

        # 建立 admin1 記錄
        admin1_records = []
        for idx, admin1_name in enumerate(unique_admin1):
            admin1_code = admin1_mapping.get(admin1_name)
            if admin1_code is None:
                logger.warning(f"無法找到 {admin1_name} 的 admin1_code，跳過")
                continue

            admin1_records.append(
                {
                    "id": admin1_code,  # 例如 "TW.01"
                    "name": admin1_name,
                    "asciiname": admin1_name,
                    "geoname_id": str(base_geoname_id + idx),
                }
            )

        # 建立 DataFrame
        admin1_df = pl.DataFrame(admin1_records, schema=cls.ADMIN1_SCHEMA)

        logger.info(f"產生了 {admin1_df.height} 筆 {cls.COUNTRY_NAME} admin1 記錄")
        logger.info(
            f"Admin1 geoname_id 範圍: {base_geoname_id} - "
            f"{base_geoname_id + admin1_df.height - 1}"
        )

        return admin1_df

    @classmethod
    def get_admin1_mapping(cls, csv_path: str | None = None) -> dict[str, str]:
        """獲取或生成 ADMIN1_MAPPING（支援緩存）。

        Args:
            csv_path: CSV 檔案路徑。若為 None，自動使用標準路徑：
                      meta_data/{country_code}_geodata.csv

        Returns:
            admin1 名稱到代碼的映射字典。

        Example:
            >>> TaiwanHandler.get_admin1_mapping()  # 自動使用 meta_data/tw_geodata.csv
            >>> JapanHandler.get_admin1_mapping("custom/path.csv")  # 使用自訂路徑
        """
        cache_attr = "_admin1_mapping_cache"

        # 檢查緩存
        if not hasattr(cls, cache_attr) or getattr(cls, cache_attr) is None:
            # 自動推導路徑（與 main.py 的 extract 輸出路徑一致）
            if csv_path is None:
                csv_path = f"meta_data/{cls.COUNTRY_CODE.lower()}_geodata.csv"

            # 直接調用類別方法生成 mapping
            mapping = cls.generate_admin1_mapping_from_csv(csv_path)

            # 緩存到類別屬性
            setattr(cls, cache_attr, mapping)
            logger.info(f"已緩存 {cls.COUNTRY_NAME} 的 admin1_mapping")

        return getattr(cls, cache_attr)

    @classmethod
    def generate_admin1_mapping_from_csv(cls, csv_path: str) -> dict[str, str]:
        """從 CSV 自動生成 ADMIN1_MAPPING。

        根據 admin_1 欄位的唯一值，按字母順序排序後編號。
        編號格式：{COUNTRY_CODE}.{編號}（位數根據數量自動調整）

        Args:
            csv_path: extract_from_shapefile 產生的 CSV 檔案路徑。

        Returns:
            admin_1 名稱到代碼的映射字典。

        Example:
            >>> mapping = TaiwanGeoDataHandler.generate_admin1_mapping_from_csv(
            ...     "meta_data/tw_geodata.csv"
            ... )
            >>> # 如果有 22 個 admin_1，生成 TW.01 到 TW.22
        """
        logger.info(f"正在從 {csv_path} 生成 {cls.COUNTRY_NAME} 的 admin_1 mapping...")

        df = pl.read_csv(csv_path)

        # 提取唯一的 admin_1 值並排序
        admin1_list = sorted(df["admin_1"].unique().to_list())

        # 計算需要的位數
        total_count = len(admin1_list)
        num_digits = len(str(total_count))

        # 生成 mapping
        mapping = {}
        for idx, admin1_name in enumerate(admin1_list, start=1):
            code = f"{cls.COUNTRY_CODE}.{str(idx).zfill(num_digits)}"
            mapping[admin1_name] = code

        logger.info(f"生成了 {total_count} 個 admin_1 代碼（{num_digits} 位數）")
        if total_count <= 10:
            # 如果數量較少，顯示所有 mapping
            logger.info(f"Admin1 mapping: {mapping}")
        else:
            # 數量較多時，顯示前 3 個範例
            sample_items = list(mapping.items())[:3]
            logger.info(f"Admin1 mapping 範例: {dict(sample_items)} ...")

        return mapping

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

        # 轉換資料
        converted_df = self.convert_to_cities_schema(csv_path, base_geoname_id)

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


_HANDLER_REGISTRY: dict[str, type[GeoDataHandler]] = {}


def register_handler(country_code: str):
    """註冊處理器的裝飾器。

    Args:
        country_code: 國家代碼（ISO 3166-1 alpha-2）。
    """

    def decorator(handler_class: type[GeoDataHandler]) -> type[GeoDataHandler]:
        _HANDLER_REGISTRY[country_code.upper()] = handler_class
        return handler_class

    return decorator


def get_handler(country_code: str) -> type[GeoDataHandler]:
    """取得指定國家的處理器類別。

    Args:
        country_code: 國家代碼（ISO 3166-1 alpha-2）。

    Returns:
        處理器類別。

    Raises:
        ValueError: 當國家代碼不存在時。
    """
    country_code = country_code.upper()
    if country_code not in _HANDLER_REGISTRY:
        available = ", ".join(sorted(_HANDLER_REGISTRY.keys()))
        raise ValueError(
            f"未找到國家 '{country_code}' 的處理器。可用的國家: {available}"
        )
    return _HANDLER_REGISTRY[country_code]


def get_all_handlers() -> list[str]:
    """取得所有已註冊的 Handler 國家代碼列表。

    Returns:
        已註冊的國家代碼列表（按字母順序排序）。

    Example:
        >>> get_all_handlers()
        ['JP', 'TW']
    """
    return sorted(_HANDLER_REGISTRY.keys())
