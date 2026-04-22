"""地理資料處理器的註冊表與查找函式。"""

from __future__ import annotations

from typing import TYPE_CHECKING

if TYPE_CHECKING:
    from core.geodata.base import GeoDataHandler


_HANDLER_REGISTRY: dict[str, type["GeoDataHandler"]] = {}


def register_handler(country_code: str):
    """註冊處理器的裝飾器。

    Args:
        country_code: 國家代碼（ISO 3166-1 alpha-2）。
    """

    def decorator(
        handler_class: type["GeoDataHandler"],
    ) -> type["GeoDataHandler"]:
        _HANDLER_REGISTRY[country_code.upper()] = handler_class
        return handler_class

    return decorator


def get_handler(country_code: str) -> type["GeoDataHandler"]:
    """取得指定國家的處理器類別。

    Args:
        country_code: 國家代碼（ISO 3166-1 alpha-2）。

    Returns:
        處理器類別。

    Raises:
        ValueError: 當國家代碼不存在時。
    """
    country_code = country_code.upper()
    if country_code not in _HANDLER_REGISTRY:
        available = ", ".join(sorted(_HANDLER_REGISTRY.keys()))
        raise ValueError(
            f"未找到國家 '{country_code}' 的處理器。可用的國家: {available}"
        )
    return _HANDLER_REGISTRY[country_code]


def get_all_handlers() -> list[str]:
    """取得所有已註冊的 Handler 國家代碼列表。

    Returns:
        已註冊的國家代碼列表（按字母順序排序）。

    Example:
        >>> get_all_handlers()
        ['JP', 'KR', 'TW']
    """
    return sorted(_HANDLER_REGISTRY.keys())
