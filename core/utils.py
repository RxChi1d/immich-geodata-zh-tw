import os
import sys
import shutil
from loguru import logger
from pathlib import Path

from tqdm import tqdm
import polars as pl

sys.path.append(os.path.join(os.path.dirname(__file__), ".."))

from core.constants import CHINESE_PRIORITY


class TqdmLogSink:
    """使用 tqdm.write 來輸出 log，避免影響 tqdm 進度條"""

    def write(self, message):
        tqdm.write(message.strip())


# 確保 logger 只被設定一次
if not logger._core.handlers:
    logger.remove()  # 移除預設 handlers

    logger.add(
        TqdmLogSink(),  # 改用 tqdm.write() 避免影響 tqdm 進度條
        format="<green>{time:HH:mm:ss}</green> | "
        "<cyan>{name}</cyan>:<cyan>{function}</cyan>:<cyan>{line}</cyan> | "
        "<level>{level}</level> - <level>{message}</level>",
        level=os.environ.get("LOG_LEVEL", "INFO"),  # 從環境變數讀取等級
        colorize=True,
        backtrace=True,  # 美化 Traceback
        diagnose=True,  # 顯示變數資訊
    )

# 讓其他模組可以直接 `from utils import logger`
__all__ = ["logger"]


def rebuild_folder(folder="output"):
    if os.path.exists(folder):
        logger.info(f"正在刪除資料夾 {folder}")
        shutil.rmtree(folder, ignore_errors=True)
        os.makedirs(folder, exist_ok=True)
        logger.info(f"已重建資料夾 {folder}")


def ensure_folder_exists(file_path):
    folder = os.path.dirname(file_path)
    if folder:
        os.makedirs(folder, exist_ok=True)


def create_alternate_map(alternate_file, output_path):
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


def load_alternate_names(file_path):
    # logger.info(f"正在從 {file_path} 載入替代名稱對照表")

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

    # logger.info(f"已從 {file_path} 載入替代名稱對照表")

    return data


# ============================================================================
# 全域 geoname_id 管理工具
# ============================================================================


def calculate_global_max_geoname_id() -> int:
    """
    掃描所有相關檔案，計算全域最大 geoname_id。

    檢查檔案：
    - geoname_data/cities500.txt
    - geoname_data/admin1CodesASCII.txt
    - geoname_data/admin2Codes.txt
    - output/cities500_optimized.txt (如果存在)
    - output/admin1CodesASCII_optimized.txt (如果存在)

    Returns:
        全域最大 geoname_id，若無資料則返回 0

    Raises:
        無例外，檔案不存在或為空時返回 0
    """
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


if __name__ == "__main__":
    pass
