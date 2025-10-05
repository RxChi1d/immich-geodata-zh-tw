"""統一的地理資料處理模組（ETL 模式）"""

from .base import GeoDataHandler, register_handler, get_handler
from .taiwan import TaiwanGeoDataHandler
from .japan import JapanGeoDataHandler

__all__ = [
    "GeoDataHandler",
    "register_handler",
    "get_handler",
    "TaiwanGeoDataHandler",
    "JapanGeoDataHandler",
]
