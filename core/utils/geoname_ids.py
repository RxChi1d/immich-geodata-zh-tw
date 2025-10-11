"""
GeoName ID 計算工具模組。

提供全域最大 geoname_id 掃描功能，用於為新增的地理資料分配唯一 ID。
"""

from pathlib import Path
import polars as pl
from .logging import logger


def calculate_global_max_geoname_id() -> int:
    """
    掃描所有相關檔案，計算全域最大 geoname_id。

    此函式會檢查專案中所有包含 geoname_id 的資料檔案，
    找出當前使用的最大 ID 值，以便為新資料分配不重複的 ID。

    檢查檔案:
        - geoname_data/cities500.txt
        - geoname_data/admin1CodesASCII.txt
        - geoname_data/admin2Codes.txt
        - output/cities500_optimized.txt (如果存在)
        - output/admin1CodesASCII_optimized.txt (如果存在)

    Returns:
        全域最大 geoname_id，若無資料則返回 0

    注意:
        此函式不會拋出例外，檔案不存在或讀取錯誤時會跳過並記錄警告。
    """
    # Reason: 延遲匯入避免循環依賴
    from core.schemas import CITIES_SCHEMA, ADMIN1_SCHEMA

    max_id = 0
    files_to_check = [
        ("geoname_data/cities500.txt", CITIES_SCHEMA),
        ("geoname_data/admin1CodesASCII.txt", ADMIN1_SCHEMA),
        ("geoname_data/admin2Codes.txt", ADMIN1_SCHEMA),
        ("output/cities500_optimized.txt", CITIES_SCHEMA),
        ("output/admin1CodesASCII_optimized.txt", ADMIN1_SCHEMA),
    ]

    logger.info("開始計算全域最大 geoname_id...")

    for file_path, schema in files_to_check:
        if not Path(file_path).exists():
            logger.debug(f"檔案不存在，跳過: {file_path}")
            continue

        try:
            # 判斷檔案類型並使用對應的 separator
            separator = "\t"

            df = pl.read_csv(
                file_path, separator=separator, has_header=False, schema=schema
            )

            if df.is_empty():
                logger.debug(f"檔案為空，跳過: {file_path}")
                continue

            # 轉換為整數再計算最大值
            file_max_id = df.select(pl.col("geoname_id").cast(pl.Int64).max()).item()

            if file_max_id is not None and file_max_id > max_id:
                max_id = file_max_id
                logger.debug(f"從 {file_path} 中找到最大 ID: {file_max_id}")

        except Exception as e:
            logger.warning(f"讀取 {file_path} 時發生錯誤，跳過: {e}")
            continue

    logger.info(f"全域最大 geoname_id: {max_id}")
    return max_id


__all__ = ["calculate_global_max_geoname_id"]
