"""南韓地理資料處理器的單元測試。"""

from __future__ import annotations

import polars as pl

from core.geodata.handlers.south_korea import SouthKoreaGeoDataHandler


class TestSouthKoreaAdmin3Extraction:
    """測試南韓 admin_3 拆解規則。"""

    def test_removes_row_specific_sidonm_and_sggnm_prefixes(self):
        """每列應移除該列自己的 sidonm 與 sggnm，保留洞/邑/面名稱。"""
        df = pl.DataFrame(
            {
                "sidonm": ["서울특별시", "경기도", "광주광역시"],
                "sggnm": ["중구", "고양시 일산동구", "북구"],
                "adm_nm": [
                    "서울특별시 중구 명동",
                    "경기도 고양시 일산동구 장항2동",
                    "광주광역시 북구 두암3동",
                ],
            }
        )

        result = SouthKoreaGeoDataHandler._derive_admin3_column(df)

        assert result["admin_3"].to_list() == ["명동", "장항2동", "두암3동"]
