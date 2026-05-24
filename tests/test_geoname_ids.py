"""GeoName ID 工具測試。"""

from core.utils.geoname_ids import calculate_global_max_geoname_id


def test_calculate_global_max_geoname_id_skips_malformed_ids(tmp_path, monkeypatch):
    """遇到非數字 geoname_id 時應跳過該檔案並繼續掃描。"""
    monkeypatch.chdir(tmp_path)

    geoname_data = tmp_path / "geoname_data"
    output = tmp_path / "output"
    geoname_data.mkdir()
    output.mkdir()

    (geoname_data / "admin1CodesASCII.txt").write_text(
        "TW.01\tTaipei\tTaipei\t123\n",
        encoding="utf-8",
    )
    (output / "admin1CodesASCII_optimized.txt").write_text(
        "id\tname\tasciiname\tgeoname_id\n",
        encoding="utf-8",
    )

    assert calculate_global_max_geoname_id() == 123
