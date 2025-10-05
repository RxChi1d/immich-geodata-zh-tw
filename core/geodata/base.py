"""地理資料統一處理器抽象基類（ETL 模式）"""

from abc import ABC, abstractmethod
import polars as pl
from core.utils import logger


class GeoDataHandler(ABC):
    """
    地理資料統一處理器（ETL 模式）。

    每個國家應繼承此基類並實作三階段處理方法：
    - Extract: Shapefile → 標準化 CSV
    - Transform: CSV → CITIES_SCHEMA DataFrame
    - Load: 整合到主資料集
    """

    # ==================== Extract 階段 ====================

    @abstractmethod
    def extract_from_shapefile(self, shapefile_path: str, output_csv: str) -> None:
        """
        從 Shapefile 提取資料並儲存為標準化 CSV。

        Args:
            shapefile_path: Shapefile 檔案路徑
            output_csv: 輸出 CSV 檔案路徑

        處理流程：
            1. 讀取 Shapefile
            2. 計算中心點座標（使用適當的投影）
            3. 轉換為標準化格式（longitude, latitude, admin_1-4, country）
            4. 儲存為 CSV

        Raises:
            FileNotFoundError: 當 Shapefile 不存在時
            Exception: 當處理過程發生錯誤時
        """
        pass

    # ==================== Transform 階段 ====================

    @abstractmethod
    def convert_to_cities_schema(self, csv_path: str) -> pl.DataFrame:
        """
        讀取 CSV 並轉換為 CITIES_SCHEMA 格式。

        Args:
            csv_path: 輸入 CSV 檔案路徑

        Returns:
            符合 CITIES_SCHEMA 的 DataFrame

        處理流程：
            1. 讀取標準化 CSV
            2. 生成唯一 geoname_id
            3. 映射行政區代碼
            4. 轉換為 CITIES_SCHEMA 格式

        Raises:
            FileNotFoundError: 當 CSV 檔案不存在時
            ValueError: 當資料格式不符合預期時
        """
        pass

    # ==================== Load 階段 ====================

    def replace_in_dataset(
        self,
        input_df: pl.DataFrame,
        country_code: str,
        csv_path: str | None = None,
    ) -> pl.DataFrame:
        """
        將轉換後的資料替換到主資料集中（通用實作）。

        子類別通常不需要覆寫此方法。

        Args:
            input_df: 主資料集 DataFrame
            country_code: 國家代碼（例如 "TW", "JP"）
            csv_path: CSV 檔案路徑（預設 meta_data/{country}_geodata.csv）

        Returns:
            已替換資料的 DataFrame
        """
        # 預設 CSV 路徑
        if csv_path is None:
            csv_path = f"meta_data/{country_code.lower()}_geodata.csv"

        # 轉換資料
        logger.info(f"開始使用 {country_code} 地理資料替換現有資料")
        converted_df = self.convert_to_cities_schema(csv_path)

        # 移除舊資料
        non_country_df = input_df.filter(pl.col("country_code") != country_code)
        removed_count = input_df.height - non_country_df.height
        if removed_count > 0:
            logger.info(f"移除了 {removed_count} 筆舊的 {country_code} 資料")
        else:
            logger.info(f"輸入資料中未找到需要移除的 {country_code} 資料")

        # 合併新資料（新資料放在前面）
        output_df = converted_df.vstack(non_country_df)
        logger.info(f"添加了 {converted_df.height} 筆新的 {country_code} 資料")
        logger.info(f"{country_code} 資料替換完成")

        return output_df


# ==================== Registry ====================

_HANDLER_REGISTRY: dict[str, type[GeoDataHandler]] = {}


def register_handler(country_code: str):
    """
    註冊處理器的裝飾器。

    Args:
        country_code: 國家代碼（ISO 3166-1 alpha-2），例如 "TW", "JP"

    Example:
        @register_handler("TW")
        class TaiwanGeoDataHandler(GeoDataHandler):
            ...
    """

    def decorator(handler_class: type[GeoDataHandler]) -> type[GeoDataHandler]:
        _HANDLER_REGISTRY[country_code.upper()] = handler_class
        return handler_class

    return decorator


def get_handler(country_code: str) -> type[GeoDataHandler]:
    """
    取得指定國家的處理器類別。

    Args:
        country_code: 國家代碼（ISO 3166-1 alpha-2）

    Returns:
        處理器類別

    Raises:
        ValueError: 當國家代碼不存在時

    Example:
        handler_class = get_handler("TW")
        handler = handler_class()
        handler.extract_from_shapefile("path/to/file.shp", "output.csv")
    """
    country_code = country_code.upper()
    if country_code not in _HANDLER_REGISTRY:
        available = ", ".join(sorted(_HANDLER_REGISTRY.keys()))
        raise ValueError(
            f"未找到國家 '{country_code}' 的處理器。可用的國家: {available}"
        )
    return _HANDLER_REGISTRY[country_code]
