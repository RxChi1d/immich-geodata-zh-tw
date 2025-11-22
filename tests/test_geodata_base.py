"""GeoDataHandler 基類的單元測試。"""

import polars as pl
from core.geodata.base import GeoDataHandler


class TestGetDiverseSample:
    """測試 get_diverse_sample 方法。"""

    def test_normal_case(self):
        """測試正常情況：多樣化資料取樣。"""
        df = pl.DataFrame(
            {
                "admin_1": ["台北市", "台北市", "新北市", "台中市", "高雄市"],
                "admin_2": ["中正區", "大安區", "板橋區", "西屯區", "前金區"],
                "admin_3": ["", "", "", "", ""],
                "admin_4": ["", "", "", "", ""],
                "latitude": [25.0, 25.1, 25.2, 24.0, 22.0],
                "longitude": [121.5, 121.6, 121.7, 120.5, 120.3],
            }
        )

        result = GeoDataHandler.get_diverse_sample(df, n=5)

        # 所有資料的 admin 組合都不同，應該回傳全部 5 筆
        assert len(result) == 5

    def test_duplicate_combinations(self):
        """測試組合重複：10 筆資料但只有 3 種組合。"""
        df = pl.DataFrame(
            {
                "admin_1": [
                    "台北市",
                    "台北市",
                    "台北市",
                    "新北市",
                    "新北市",
                    "台中市",
                    "台中市",
                    "台中市",
                    "台中市",
                    "台中市",
                ],
                "admin_2": [
                    "中正區",
                    "中正區",
                    "中正區",
                    "板橋區",
                    "板橋區",
                    "西屯區",
                    "西屯區",
                    "西屯區",
                    "西屯區",
                    "西屯區",
                ],
                "admin_3": [""] * 10,
                "admin_4": [""] * 10,
            }
        )

        result = GeoDataHandler.get_diverse_sample(df, n=5)

        # 只有 3 種不同的 admin 組合
        assert len(result) == 3
        # 確認是不同的組合（不檢查順序）
        assert set(result["admin_1"].to_list()) == {"台北市", "新北市", "台中市"}
        assert set(result["admin_2"].to_list()) == {"中正區", "板橋區", "西屯區"}

    def test_insufficient_data(self):
        """測試資料不足：只有 3 筆資料，要求 5 筆。"""
        df = pl.DataFrame(
            {
                "admin_1": ["台北市", "新北市", "台中市"],
                "admin_2": ["中正區", "板橋區", "西屯區"],
                "admin_3": ["", "", ""],
                "admin_4": ["", "", ""],
            }
        )

        result = GeoDataHandler.get_diverse_sample(df, n=5)

        # 資料不足，只回傳 3 筆
        assert len(result) == 3

    def test_missing_columns(self):
        """測試缺少欄位：DataFrame 只有 admin_1, admin_2。"""
        df = pl.DataFrame(
            {
                "admin_1": ["台北市", "台北市", "新北市"],
                "admin_2": ["中正區", "中正區", "板橋區"],
                "latitude": [25.0, 25.1, 25.2],
                "longitude": [121.5, 121.6, 121.7],
            }
        )

        result = GeoDataHandler.get_diverse_sample(df, n=5)

        # 即使缺少 admin_3, admin_4，仍然能正常運作
        # 應該有 2 種不同的組合（台北市/中正區 和 新北市/板橋區）
        assert len(result) == 2

    def test_empty_dataframe(self):
        """測試空 DataFrame。"""
        df = pl.DataFrame(
            {
                "admin_1": [],
                "admin_2": [],
                "admin_3": [],
                "admin_4": [],
            }
        )

        result = GeoDataHandler.get_diverse_sample(df, n=5)

        # 空 DataFrame 應該回傳空結果
        assert len(result) == 0

    def test_no_admin_columns(self):
        """測試完全沒有 admin 欄位。"""
        df = pl.DataFrame(
            {
                "latitude": [25.0, 25.1, 25.2, 24.0, 22.0],
                "longitude": [121.5, 121.6, 121.7, 120.5, 120.3],
            }
        )

        result = GeoDataHandler.get_diverse_sample(df, n=3)

        # 沒有 admin 欄位時，應該回傳前 3 筆
        assert len(result) == 3

    def test_hierarchical_logic(self):
        """測試階層式邏輯：admin_1 不足時使用 admin_2。"""
        df = pl.DataFrame(
            {
                "admin_1": ["台北市", "台北市", "新北市", "新北市", "台中市"],
                "admin_2": ["中正區", "大安區", "板橋區", "新莊區", "西屯區"],
                "admin_3": ["", "", "", "", ""],
                "admin_4": ["", "", "", "", ""],
            }
        )

        result = GeoDataHandler.get_diverse_sample(df, n=5)

        # 只有 3 個 admin_1，不足 5 筆
        # 應該使用 admin_1 + admin_2，得到 5 筆
        assert len(result) == 5
        # 確認包含不同的 admin_1
        assert len(set(result["admin_1"].to_list())) == 3

    def test_all_same_admin1(self):
        """測試所有資料同一個 admin_1 的情況。"""
        df = pl.DataFrame(
            {
                "admin_1": ["台北市"] * 5,
                "admin_2": ["中正區", "大安區", "信義區", "松山區", "大同區"],
                "admin_3": ["", "", "", "", ""],
                "admin_4": ["", "", "", "", ""],
            }
        )

        result = GeoDataHandler.get_diverse_sample(df, n=5)

        # 所有 admin_1 都是台北市（只有 1 種）
        # 應該使用 admin_1 + admin_2，得到 5 筆（5 個不同的區）
        assert len(result) == 5
        assert set(result["admin_1"].to_list()) == {"台北市"}
        assert len(set(result["admin_2"].to_list())) == 5

    def test_with_null_values(self):
        """測試包含 null 值的情況。"""
        df = pl.DataFrame(
            {
                "admin_1": ["台北市", "台北市", None, "新北市"],
                "admin_2": ["中正區", None, "板橋區", "板橋區"],
                "admin_3": [None, None, None, None],
                "admin_4": [None, None, None, None],
            }
        )

        result = GeoDataHandler.get_diverse_sample(df, n=5)

        # Polars unique 會將 null 視為不同的值
        # 應該能正常處理 null 值
        assert len(result) <= 5
        assert len(result) > 0
