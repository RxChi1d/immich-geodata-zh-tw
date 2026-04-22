"""registry 模組單元測試。"""

from __future__ import annotations

import pytest

from core.geodata.base import GeoDataHandler
from core.geodata.registry import (
    _HANDLER_REGISTRY,
    get_all_handlers,
    get_handler,
    register_handler,
)


class _DummyHandler(GeoDataHandler):
    """測試用的 stub handler；不呼叫 __init__ 以避免實例化校驗。"""

    COUNTRY_NAME = "DummyLand"
    COUNTRY_CODE = "XX"
    TIMEZONE = "UTC"

    def extract_from_shapefile(self, shapefile_path: str, output_csv: str) -> None:
        raise NotImplementedError


@pytest.fixture(autouse=True)
def _snapshot_registry():
    """每則測試前後備份與還原 registry，避免污染其他測試。"""
    snapshot = dict(_HANDLER_REGISTRY)
    yield
    _HANDLER_REGISTRY.clear()
    _HANDLER_REGISTRY.update(snapshot)


class TestRegister:
    """測試 register_handler 行為。"""

    def test_register_and_lookup(self):
        register_handler("XX")(_DummyHandler)
        assert get_handler("XX") is _DummyHandler

    def test_register_is_case_insensitive(self):
        register_handler("xx")(_DummyHandler)
        assert get_handler("XX") is _DummyHandler
        assert get_handler("xx") is _DummyHandler

    def test_register_last_write_wins(self):
        """重複註冊同一國家代碼時，後註冊者覆寫前者（last-write-wins 語義）。"""
        register_handler("XX")(_DummyHandler)

        class _OtherHandler(_DummyHandler):
            pass

        register_handler("XX")(_OtherHandler)
        assert get_handler("XX") is _OtherHandler


class TestGetHandler:
    """測試 get_handler 的錯誤處理。"""

    def test_raises_with_helpful_message_on_unknown_code(self):
        """查詢未註冊代碼時應拋出 ValueError 並列出可用代碼。"""
        register_handler("XX")(_DummyHandler)
        with pytest.raises(ValueError) as excinfo:
            get_handler("ZZ")
        assert "ZZ" in str(excinfo.value)
        assert "XX" in str(excinfo.value)


class TestGetAllHandlers:
    """測試 get_all_handlers 回傳已註冊代碼列表。"""

    def test_returns_sorted_codes(self):
        _HANDLER_REGISTRY.clear()
        register_handler("KR")(_DummyHandler)
        register_handler("JP")(_DummyHandler)
        register_handler("TW")(_DummyHandler)
        assert get_all_handlers() == ["JP", "KR", "TW"]

    def test_returns_empty_when_no_handlers(self):
        _HANDLER_REGISTRY.clear()
        assert get_all_handlers() == []
