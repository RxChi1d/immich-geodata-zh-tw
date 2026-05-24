"""
檔案系統操作工具模組。

提供資料夾建立、重建等常用檔案系統操作功能。
"""

import os
import shutil
from .logging import logger


def rebuild_folder(folder: str = "output") -> None:
    """
    重建資料夾：確保執行後資料夾存在且為空。

    若資料夾存在則完整刪除再重新建立；若不存在則直接建立。

    Args:
        folder: 要重建的資料夾路徑，預設為 "output"
    """
    if os.path.exists(folder):
        logger.info(f"正在刪除資料夾 {folder}")
        shutil.rmtree(folder, ignore_errors=True)
        os.makedirs(folder, exist_ok=True)
        logger.info(f"已重建資料夾 {folder}")
    else:
        os.makedirs(folder, exist_ok=True)
        logger.info(f"已建立資料夾 {folder}")


def ensure_folder_exists(file_path: str) -> None:
    """
    確保檔案路徑的父資料夾存在。

    如果檔案路徑的父資料夾不存在，會自動建立所有必要的中間資料夾。
    此函式常用於在寫入檔案前確保目標資料夾已存在。

    Args:
        file_path: 檔案的完整路徑

    Example:
        >>> ensure_folder_exists("data/processed/output.csv")
        # 會建立 data/processed 資料夾（如果不存在）
    """
    folder = os.path.dirname(file_path)
    if folder:
        os.makedirs(folder, exist_ok=True)


__all__ = ["rebuild_folder", "ensure_folder_exists"]
