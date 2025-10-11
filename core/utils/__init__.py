"""
Core 工具模組套件。

提供 Logger、檔案系統操作、替代名稱處理與 GeoName ID 計算等工具函式。
此套件統一匯出常用工具，支援 `from core.utils import logger` 等便捷匯入方式。

模組結構:
    - logging: Logger 設定與 TqdmLogSink
    - filesystem: 檔案與資料夾操作工具
    - alternate_names: 地理名稱替代名稱處理
    - geoname_ids: GeoName ID 計算工具
"""

from .logging import logger
from .filesystem import rebuild_folder, ensure_folder_exists
from .alternate_names import create_alternate_map, load_alternate_names
from .geoname_ids import calculate_global_max_geoname_id

__all__ = [
    # Logger
    "logger",
    # 檔案系統工具
    "rebuild_folder",
    "ensure_folder_exists",
    # 替代名稱工具
    "create_alternate_map",
    "load_alternate_names",
    # GeoName ID 工具
    "calculate_global_max_geoname_id",
]
