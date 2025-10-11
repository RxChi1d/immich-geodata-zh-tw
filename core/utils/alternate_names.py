"""
替代名稱處理模組。

提供地理名稱的替代名稱（alternate names）載入與對照表建立功能，
主要用於處理中文地名的優先順序選擇。
"""

import os
import sys
import polars as pl
from .logging import logger
from .filesystem import ensure_folder_exists


def create_alternate_map(alternate_file: str, output_path: str) -> None:
    """
    從 GeoNames 替代名稱檔案建立中文名稱對照表。

    讀取 GeoNames 的 alternateNamesV2.txt 檔案，篩選出中文名稱並根據
    優先順序規則（is_preferred_name 與語言順序）選擇最適當的中文名稱，
    輸出為簡化的 CSV 對照表。

    Args:
        alternate_file: alternateNamesV2.txt 檔案路徑
        output_path: 輸出的 CSV 檔案路徑

    處理流程:
        1. 讀取原始 TSV 檔案並篩選中文名稱
        2. 根據 is_preferred_name 與 CHINESE_PRIORITY 計算優先級
        3. 每個 geoname_id 僅保留優先級最高的中文名稱
        4. 更新地名（例如將「桃園縣」更新為「桃園市」）
        5. 輸出為兩欄 CSV：geoname_id, name
    """
    # Reason: 延遲匯入避免循環依賴
    from core.constants import CHINESE_PRIORITY

    logger.info(f"正在從 {alternate_file} 建立替代名稱對照表")

    ensure_folder_exists(output_path)

    data = pl.read_csv(
        alternate_file,
        separator="\t",  # 設定 Tab 為分隔符號
        has_header=False,  # 表示檔案沒有標題列
        columns=[1, 2, 3, 4],  # 只讀取第 1, 2, 3, 4 欄
        new_columns=["geoname_id", "lang", "name", "is_preferred_name"],  # 重新命名欄位
        null_values="\\N",  # 把 "\N" 視為空值 (null)
        dtypes={
            "geoname_id": pl.String,
            "lang": pl.String,
            "name": pl.String,
            "is_preferred_name": pl.UInt8,
        },  # 指定所有欄位為 String
    )

    data = data.filter(data["lang"].is_in(CHINESE_PRIORITY))  # 僅保留中文名稱

    # 創建 `priority` 欄位，作為優先級判斷
    # - 如果 `is_preferred_name` 為 1，則優先級為 0
    # - 如果 `is_preferred_name` 為 0，則優先級為 CHINESE_PRIORITY 中 key 的 index + 1
    data = data.with_columns(
        pl.when(pl.col("is_preferred_name") == 1)
        .then(pl.lit(0))  # is_preferred_name == "1"，則優先級為 0
        .otherwise(
            pl.col("lang")
            .fill_null("")
            .map_elements(
                lambda x: (
                    CHINESE_PRIORITY.index(x) + 1
                    if x in CHINESE_PRIORITY
                    else len(CHINESE_PRIORITY) + 1
                ),
                return_dtype=pl.UInt8,  # 明確指定回傳型別
            )
        )
        .alias("priority")
    )

    # 相同的geoname_id，僅保留優先級最高的（數字越小越高，0為最高）
    data = (
        data.sort("priority")  # 按 `priority` 排序（越小越優先）
        .group_by("geoname_id")
        .first()  # 只保留 `geoname_id` 相同的第一筆資料（優先級最高的）
        .select(["geoname_id", "name"])  # 只保留指定的兩個欄位
    )

    # 更新地名
    data = data.with_columns(
        pl.col("name").str.replace("桃園縣", "桃園市").alias("name")
    )

    # 儲存為 alternate_chinese_name.csv
    data.write_csv(output_path)

    logger.info(f"替代名稱對照表已儲存至 {output_path}")


def load_alternate_names(file_path: str) -> pl.DataFrame:
    """
    載入替代名稱對照表。

    如果對照表檔案不存在，會自動從 alternateNamesV2.txt 建立。

    Args:
        file_path: 對照表 CSV 檔案路徑

    Returns:
        包含 geoname_id 與 name 兩欄的 Polars DataFrame

    Raises:
        SystemExit: 當 alternateNamesV2.txt 也不存在時終止程式
    """
    if not os.path.exists(file_path):
        logger.info(f"替代名稱檔案 {file_path} 不存在")

        alternate_file = "./geoname_data/alternateNamesV2.txt"

        if not os.path.exists(alternate_file):
            logger.critical(f"替代名稱檔案 {alternate_file} 不存在")
            sys.exit(1)

        create_alternate_map(alternate_file, file_path)

    data = pl.read_csv(
        file_path,
        has_header=True,
        schema=pl.Schema(
            {
                "geoname_id": pl.String,
                "name": pl.String,
            }
        ),
    )

    return data


__all__ = ["create_alternate_map", "load_alternate_names"]
