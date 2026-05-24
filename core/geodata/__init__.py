"""地理資料處理模組（ETL 模式）。

模組結構：
    base.py / admin1.py / geospatial.py / registry.py
        共用框架：ABC、mixin 與 handler 註冊表
    handlers/
        各國的 handler 實作，每國一個套件（uniform package layout）

公開 API 以此 ``__init__`` 為主；呼叫端不應依賴各子模組路徑。
"""

from core.geodata.base import GeoDataHandler
from core.geodata.registry import get_all_handlers, get_handler, register_handler

# 匯入 handlers 子套件以觸發 @register_handler 副作用
from core.geodata.handlers import (
    JapanGeoDataHandler,
    SouthKoreaGeoDataHandler,
    TaiwanGeoDataHandler,
)

__all__ = [
    "GeoDataHandler",
    "register_handler",
    "get_handler",
    "get_all_handlers",
    "TaiwanGeoDataHandler",
    "JapanGeoDataHandler",
    "SouthKoreaGeoDataHandler",
]
