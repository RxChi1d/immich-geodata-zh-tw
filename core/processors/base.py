"""地理資料處理器抽象基類"""

from abc import ABC, abstractmethod
import polars as pl


class GeoDataProcessor(ABC):
    """
    地理資料處理器抽象基類。

    定義所有地理資料處理器必須實作的介面，確保不同國家/地區的處理器
    遵循統一的設計模式。
    """

    @abstractmethod
    def convert_geodata(self) -> pl.DataFrame:
        """
        讀取並轉換地理資料為 CITIES_SCHEMA 格式的 DataFrame。

        此方法應該：
        1. 讀取對應國家/地區的地理資料 CSV 檔案
        2. 進行必要的資料清理和轉換
        3. 將資料轉換為符合 CITIES_SCHEMA 的格式
        4. 生成唯一的 geoname_id（每個國家使用不同的基礎值）

        Returns:
            pl.DataFrame: 轉換後的地理資料 DataFrame，符合 CITIES_SCHEMA。

        Raises:
            FileNotFoundError: 當輸入檔案不存在時
            ValueError: 當資料格式不符合預期時
        """
        pass

    @abstractmethod
    def replace_data(self, input_df: pl.DataFrame) -> pl.DataFrame:
        """
        使用轉換後的地理資料取代輸入 DataFrame 中的現有國家資料。

        此方法應該：
        1. 調用 convert_geodata() 取得新的地理資料
        2. 從輸入 DataFrame 中移除對應國家的舊資料
        3. 將新資料合併到 DataFrame 中

        Args:
            input_df: 包含城市資料的 DataFrame (應符合 CITIES_SCHEMA)。

        Returns:
            pl.DataFrame: 已替換國家資料的 DataFrame。
        """
        pass
