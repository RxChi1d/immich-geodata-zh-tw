"""Admin1 記錄與代碼映射的生成邏輯。

此模組以 ``Admin1Mixin`` 的形式提供 admin1 相關處理，讓 ``GeoDataHandler``
可以在不膨脹主要檔案的情況下繼承這些 classmethod。

子類必須已定義以下類別屬性：
    * ``COUNTRY_NAME``
    * ``COUNTRY_CODE``
    * ``ADMIN1_SCHEMA``
"""

from __future__ import annotations

from pathlib import Path
from typing import ClassVar

import polars as pl

from core.utils import logger

# 以 (handler cls, 絕對路徑, 檔案 mtime_ns) 作為 cache key，
# 避免 CSV 在同一程序內被重寫後仍回傳舊資料（原 lru_cache 版本的 stale bug）
_MAPPING_CACHE: dict[tuple[type, str, int], dict[str, str]] = {}


class Admin1Mixin:
    """提供 admin1 映射與記錄生成相關的 classmethod。"""

    # 由 GeoDataHandler 提供；此處僅作型別註記
    COUNTRY_NAME: ClassVar[str]
    COUNTRY_CODE: ClassVar[str]
    ADMIN1_SCHEMA: ClassVar[dict[str, pl.DataType] | pl.Schema]

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
        logger.info(f"正在為 {cls.COUNTRY_NAME} 生成 admin1 記錄...")

        # Reason: 不做 exists() 預檢（TOCTOU），直接讀檔並將 FileNotFoundError 包成中文訊息
        try:
            df = pl.read_csv(csv_path)
        except FileNotFoundError as exc:
            error_msg = (
                f"輸入檔案不存在: {csv_path}\n"
                f"建議：請先執行 extract 階段以生成 CSV 檔案"
            )
            logger.error(error_msg)
            raise FileNotFoundError(error_msg) from exc

        df = cls.prepare_admin1_source(df)

        if "admin_1" not in df.columns:
            error_msg = (
                f"CSV 檔案缺少 'admin_1' 欄位\n"
                f"檔案路徑: {csv_path}\n"
                f"可用欄位: {df.columns}\n"
                f"建議：請檢查 extract_from_shapefile 的實作"
            )
            logger.error(error_msg)
            raise ValueError(error_msg)

        unique_admin1 = sorted(df["admin_1"].unique().to_list())

        if not unique_admin1:
            error_msg = "CSV 檔案中沒有有效的 admin_1 資料"
            logger.error(error_msg)
            raise ValueError(error_msg)

        admin1_mapping = cls.get_admin1_mapping(csv_path)

        admin1_records: list[dict[str, str]] = []
        for admin1_name in unique_admin1:
            admin1_code = admin1_mapping.get(admin1_name)
            if admin1_code is None:
                logger.warning(f"無法找到 {admin1_name} 的 admin1_code，跳過")
                continue

            # Reason: 使用 len(admin1_records) 作為當前索引，確保 geoname_id 連續無間隔
            admin1_records.append(
                {
                    "id": admin1_code,
                    "name": admin1_name,
                    "asciiname": admin1_name,
                    "geoname_id": str(base_geoname_id + len(admin1_records)),
                }
            )

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
        if csv_path is None:
            csv_path = f"meta_data/{cls.COUNTRY_CODE.lower()}_geodata.csv"
        return cls.generate_admin1_mapping_from_csv(csv_path)

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
        # Reason: 以 (cls, 絕對路徑, mtime_ns) 作為 cache key；CSV 被重寫時 mtime 變動，自動失效。
        #         直接讀檔並於失敗時轉為中文 FileNotFoundError，避免 TOCTOU pre-check。
        path_obj = Path(csv_path)
        try:
            mtime_ns = path_obj.stat().st_mtime_ns
        except FileNotFoundError as exc:
            error_msg = (
                f"輸入檔案不存在: {csv_path}\n"
                f"建議：請先執行 extract 階段以生成 CSV 檔案"
            )
            logger.error(error_msg)
            raise FileNotFoundError(error_msg) from exc

        cache_key = (cls, str(path_obj.resolve()), mtime_ns)
        cached = _MAPPING_CACHE.get(cache_key)
        if cached is not None:
            return cached

        logger.info(f"正在從 {csv_path} 生成 {cls.COUNTRY_NAME} 的 admin_1 mapping...")

        df = pl.read_csv(csv_path)
        admin1_list = sorted(df["admin_1"].unique().to_list())

        total_count = len(admin1_list)
        num_digits = len(str(total_count))

        mapping = {}
        for idx, admin1_name in enumerate(admin1_list, start=1):
            code = f"{cls.COUNTRY_CODE}.{str(idx).zfill(num_digits)}"
            mapping[admin1_name] = code

        logger.info(f"生成了 {total_count} 個 admin_1 代碼（{num_digits} 位數）")
        if total_count <= 10:
            logger.info(f"Admin1 mapping: {mapping}")
        else:
            sample_items = list(mapping.items())[:3]
            logger.info(f"Admin1 mapping 範例: {dict(sample_items)} ...")

        _MAPPING_CACHE[cache_key] = mapping
        return mapping
