"""國家特定 handler 彙整。

匯入此子套件會觸發所有 handler 模組，並透過 ``@register_handler`` 裝飾器
將它們登錄到 registry。
"""

from core.geodata.handlers.taiwan import TaiwanGeoDataHandler
from core.geodata.handlers.japan import JapanGeoDataHandler
from core.geodata.handlers.south_korea import SouthKoreaGeoDataHandler

__all__ = [
    "TaiwanGeoDataHandler",
    "JapanGeoDataHandler",
    "SouthKoreaGeoDataHandler",
]
