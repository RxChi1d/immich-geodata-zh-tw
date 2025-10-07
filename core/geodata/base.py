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
        ADMIN1_MAPPING: 行政區代碼映射（dict 或 None）
    """

    # Schema 引用（從 core.schemas 匯入，供子類繼承）
    ADMIN1_SCHEMA = ADMIN1_SCHEMA
    GEODATA_SCHEMA = GEODATA_SCHEMA
    CITIES_SCHEMA = CITIES_SCHEMA

    # 子類必須覆寫的類別變數
    COUNTRY_NAME: str = ""
    COUNTRY_CODE: str = ""
    TIMEZONE: str = ""
    ADMIN1_MAPPING: dict[str, str] | None = None

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

    def replace_in_dataset(
        self,
        input_df: pl.DataFrame,
        country_code: str,
        csv_path: str | None = None,
    ) -> pl.DataFrame:
        """將轉換後的資料替換到主資料集中。

        Args:
            input_df: 主資料集 DataFrame。
            country_code: 國家代碼。
            csv_path: CSV 檔案路徑（預設 meta_data/{country}_geodata.csv）。

        Returns:
            已替換資料的 DataFrame。
        """
        # 預設 CSV 路徑
        if csv_path is None:
            csv_path = f"meta_data/{country_code.lower()}_geodata.csv"

        logger.info(f"開始使用 {country_code} 地理資料替換現有資料")

        # 移除舊資料
        non_country_df = input_df.filter(pl.col("country_code") != country_code)
        removed_count = input_df.height - non_country_df.height
        if removed_count > 0:
            logger.info(f"移除了 {removed_count} 筆舊的 {country_code} 資料")
        else:
            logger.info(f"輸入資料中未找到需要移除的 {country_code} 資料")

        # 計算 geoname_id 起始值（避免與現有資料衝突）
        if not non_country_df.is_empty():
            base_geoname_id = (
                non_country_df.select(pl.col("geoname_id").max()).item() + 1
            )
            logger.info(f"計算得到的 geoname_id 起始值: {base_geoname_id}")
        else:
            base_geoname_id = 0
            logger.info("資料集為空，geoname_id 從 0 開始")

        # 轉換資料（傳入計算得到的起始 ID）
        converted_df = self.convert_to_cities_schema(csv_path, base_geoname_id)

        # 合併新資料（新資料放在前面）
        output_df = converted_df.vstack(non_country_df)
        logger.info(f"添加了 {converted_df.height} 筆新的 {country_code} 資料")
        logger.info(f"{country_code} 資料替換完成")

        return output_df


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
