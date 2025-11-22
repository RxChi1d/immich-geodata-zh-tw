"""統一的地理資料處理模組（ETL 模式）"""

from .base import GeoDataHandler, register_handler, get_handler, get_all_handlers
from .taiwan import TaiwanGeoDataHandler
from .japan import JapanGeoDataHandler
from .south_korea import SouthKoreaGeoDataHandler

__all__ = [
    "GeoDataHandler",
    "register_handler",
    "get_handler",
    "get_all_handlers",
    "TaiwanGeoDataHandler",
    "JapanGeoDataHandler",
    "SouthKoreaGeoDataHandler",
]
