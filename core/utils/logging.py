"""
Logger 設定模組。

提供整合 tqdm 進度條的 Logger 配置，避免 log 輸出影響進度條顯示。
"""

import os
from loguru import logger
from tqdm import tqdm


_LOGGER_INITIALIZED = False


class TqdmLogSink:
    """使用 tqdm.write 來輸出 log，避免影響 tqdm 進度條。"""

    def write(self, message: str) -> None:
        """
        將 log 訊息寫入輸出。

        Args:
            message: 要輸出的 log 訊息
        """
        tqdm.write(message.strip())


def _initialize_logger() -> None:
    """
    初始化 logger 設定。

    此函式僅在模組首次載入時執行一次，確保 logger 不會被重複設定。
    Logger 會使用 TqdmLogSink 以相容 tqdm 進度條，並根據環境變數
    LOG_LEVEL 設定日誌等級（預設為 INFO）。
    """
    global _LOGGER_INITIALIZED
    if _LOGGER_INITIALIZED:
        return

    logger.remove()  # 移除 loguru 預設 handler，確保設定一致

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

    _LOGGER_INITIALIZED = True


# 模組載入時初始化 logger
_initialize_logger()


__all__ = ["logger"]
