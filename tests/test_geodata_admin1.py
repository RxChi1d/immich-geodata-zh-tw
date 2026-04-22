"""Admin1Mixin 與 admin1 mapping cache 的單元測試。"""

from __future__ import annotations

import time
from pathlib import Path

import polars as pl
import pytest

from core.geodata import admin1 as admin1_mod
from core.geodata.base import GeoDataHandler


def _make_handler_cls(country_code: str = "XX", country_name: str = "Fakeland"):
    """建立測試專用的 handler 子類，避免動到真實 TW/JP/KR handler。"""

    class _TestHandler(GeoDataHandler):
        COUNTRY_NAME = country_name
        COUNTRY_CODE = country_code
        TIMEZONE = "UTC"

        def extract_from_shapefile(self, shapefile_path: str, output_csv: str) -> None:
            raise NotImplementedError

    _TestHandler.__qualname__ = f"_TestHandler_{country_code}"
    return _TestHandler


@pytest.fixture(autouse=True)
def _clear_mapping_cache():
    """每則測試開始時清空 mapping cache，確保測試隔離。"""
    admin1_mod._MAPPING_CACHE.clear()
    yield
    admin1_mod._MAPPING_CACHE.clear()


def _write_csv(path: Path, admin1_values: list[str]) -> None:
    df = pl.DataFrame({"admin_1": admin1_values})
    df.write_csv(path)


class TestGenerateAdmin1MappingFromCsv:
    """測試 generate_admin1_mapping_from_csv 的編號規則與快取行為。"""

    def test_pad_digits_at_9_items(self, tmp_path: Path):
        """9 個項目應使用 1 位數編號（XX.1 ~ XX.9）。"""
        csv = tmp_path / "admin9.csv"
        _write_csv(csv, [f"City{i:02d}" for i in range(9)])
        cls = _make_handler_cls("AA")

        mapping = cls.generate_admin1_mapping_from_csv(str(csv))

        assert len(mapping) == 9
        assert all(code.startswith("AA.") for code in mapping.values())
        # 全部為 1 位數
        for code in mapping.values():
            assert len(code.split(".")[-1]) == 1

    def test_pad_digits_at_10_items(self, tmp_path: Path):
        """10 個項目應使用 2 位數編號（XX.01 ~ XX.10）。"""
        csv = tmp_path / "admin10.csv"
        _write_csv(csv, [f"City{i:02d}" for i in range(10)])
        cls = _make_handler_cls("BB")

        mapping = cls.generate_admin1_mapping_from_csv(str(csv))

        assert len(mapping) == 10
        for code in mapping.values():
            assert len(code.split(".")[-1]) == 2

    def test_pad_digits_at_99_items(self, tmp_path: Path):
        """99 個項目應使用 2 位數編號。"""
        csv = tmp_path / "admin99.csv"
        _write_csv(csv, [f"City{i:03d}" for i in range(99)])
        cls = _make_handler_cls("CC")

        mapping = cls.generate_admin1_mapping_from_csv(str(csv))

        assert len(mapping) == 99
        for code in mapping.values():
            assert len(code.split(".")[-1]) == 2

    def test_pad_digits_at_100_items(self, tmp_path: Path):
        """100 個項目應升級到 3 位數編號。"""
        csv = tmp_path / "admin100.csv"
        _write_csv(csv, [f"City{i:03d}" for i in range(100)])
        cls = _make_handler_cls("DD")

        mapping = cls.generate_admin1_mapping_from_csv(str(csv))

        assert len(mapping) == 100
        for code in mapping.values():
            assert len(code.split(".")[-1]) == 3

    def test_alphabetical_ordering(self, tmp_path: Path):
        """編號應依 admin_1 名稱字母順序分配。"""
        csv = tmp_path / "ordered.csv"
        _write_csv(csv, ["Charlie", "Alpha", "Bravo"])
        cls = _make_handler_cls("EE")

        mapping = cls.generate_admin1_mapping_from_csv(str(csv))

        assert mapping["Alpha"] == "EE.1"
        assert mapping["Bravo"] == "EE.2"
        assert mapping["Charlie"] == "EE.3"

    def test_raises_chinese_file_not_found_error(self, tmp_path: Path):
        """檔案不存在時應拋出帶有中文訊息的 FileNotFoundError。"""
        cls = _make_handler_cls("FF")
        missing = tmp_path / "missing.csv"
        with pytest.raises(FileNotFoundError) as excinfo:
            cls.generate_admin1_mapping_from_csv(str(missing))
        assert "輸入檔案不存在" in str(excinfo.value)


class TestMappingCacheInvalidation:
    """測試 mapping cache 在 CSV 重寫時能自動失效（修復 lru_cache stale 問題）。"""

    def test_cache_hit_returns_same_object(self, tmp_path: Path):
        csv = tmp_path / "cache.csv"
        _write_csv(csv, ["Alpha", "Bravo"])
        cls = _make_handler_cls("GG")

        first = cls.generate_admin1_mapping_from_csv(str(csv))
        second = cls.generate_admin1_mapping_from_csv(str(csv))
        assert first is second  # 同一 cache 實例

    def test_cache_invalidates_on_rewrite(self, tmp_path: Path):
        """CSV 被重寫（mtime 改變）後，mapping 應重新產生。"""
        csv = tmp_path / "cache.csv"
        _write_csv(csv, ["Alpha", "Bravo"])
        cls = _make_handler_cls("HH")

        first = cls.generate_admin1_mapping_from_csv(str(csv))
        assert set(first.keys()) == {"Alpha", "Bravo"}

        # 確保 mtime 至少推進 1 奈秒
        time.sleep(0.01)
        _write_csv(csv, ["Alpha", "Bravo", "Charlie"])

        second = cls.generate_admin1_mapping_from_csv(str(csv))
        assert set(second.keys()) == {"Alpha", "Bravo", "Charlie"}
        assert first is not second

    def test_cache_isolated_across_handler_classes(self, tmp_path: Path):
        """不同 handler class 即使讀同一 CSV，cache key 互相獨立。"""
        csv = tmp_path / "shared.csv"
        _write_csv(csv, ["Alpha"])
        cls_a = _make_handler_cls("II")
        cls_b = _make_handler_cls("JJ")

        mapping_a = cls_a.generate_admin1_mapping_from_csv(str(csv))
        mapping_b = cls_b.generate_admin1_mapping_from_csv(str(csv))

        assert mapping_a["Alpha"] == "II.1"
        assert mapping_b["Alpha"] == "JJ.1"


class TestGenerateAdmin1Records:
    """測試 generate_admin1_records 的端到端 happy path 與錯誤處理。"""

    def test_happy_path(self, tmp_path: Path):
        csv = tmp_path / "geodata.csv"
        _write_csv(csv, ["Alpha", "Alpha", "Bravo"])  # 含重複
        cls = _make_handler_cls("KK")

        result = cls.generate_admin1_records(str(csv), base_geoname_id=10_000)

        assert result.height == 2
        names = set(result["name"].to_list())
        assert names == {"Alpha", "Bravo"}
        geoname_ids = sorted(result["geoname_id"].to_list())
        assert geoname_ids == ["10000", "10001"]

    def test_missing_admin_1_column_raises(self, tmp_path: Path):
        csv = tmp_path / "bad.csv"
        pl.DataFrame({"other": ["x"]}).write_csv(csv)
        cls = _make_handler_cls("LL")

        with pytest.raises(ValueError) as excinfo:
            cls.generate_admin1_records(str(csv), base_geoname_id=0)
        assert "admin_1" in str(excinfo.value)

    def test_missing_file_raises_chinese_error(self, tmp_path: Path):
        cls = _make_handler_cls("MM")
        with pytest.raises(FileNotFoundError) as excinfo:
            cls.generate_admin1_records(
                str(tmp_path / "missing.csv"), base_geoname_id=0
            )
        assert "輸入檔案不存在" in str(excinfo.value)


class TestGdfToPolarsBranchBehavior:
    """測試 GeoSpatialMixin._gdf_to_polars 兩個分支的差異（A-3 footgun 防護）。"""

    def test_fillna_true_converts_nan_to_empty_string(self):
        """fillna_object=True：NaN 應轉為空字串而非字面值 "nan"。"""
        import pandas as pd
        import geopandas as gpd
        from shapely.geometry import Point

        gdf = gpd.GeoDataFrame(
            {
                "name": pd.array(["A", None, "C"], dtype="object"),
                "geometry": [Point(0, 0), Point(1, 1), Point(2, 2)],
            },
            crs="EPSG:4326",
        )

        df = GeoDataHandler._gdf_to_polars(gdf, fillna_object=True)

        assert df["name"].to_list() == ["A", "", "C"]

    def test_fillna_false_preserves_nan_and_none_literals(self):
        """fillna_object=False：NaN/None 會變成字面值 "nan"/"None"（TW 既有行為）。"""
        import pandas as pd
        import geopandas as gpd
        from shapely.geometry import Point

        gdf = gpd.GeoDataFrame(
            {
                "name": pd.array(["A", None, "C"], dtype="object"),
                "geometry": [Point(0, 0), Point(1, 1), Point(2, 2)],
            },
            crs="EPSG:4326",
        )

        df = GeoDataHandler._gdf_to_polars(gdf, fillna_object=False)

        # pandas astype(str) 對 None 的字串表示為 "None"
        assert df["name"].to_list() == ["A", "None", "C"]
