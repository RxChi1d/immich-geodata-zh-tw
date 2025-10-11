"""專案級常數定義。

Schema 定義已移至 core.schemas。
國家特定常數已移至各 GeoDataHandler 子類別。
"""

import os
import sys

sys.path.append(os.path.dirname(os.path.dirname(__file__)))

# 中文語言優先順序
CHINESE_PRIORITY = ["zh-Hant", "zh-TW", "zh-HK", "zh", "zh-Hans", "zh-CN", "zh-SG"]


if __name__ == "__main__":
    from core.utils import logger

    logger.error("這個模組不應該被直接執行，請使用 main.py 作為專案的主要接口。")
