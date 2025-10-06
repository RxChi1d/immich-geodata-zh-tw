"""Polars DataFrame Schema 定義。

所有 schema 集中在此模組，供各模組共用。
GeoDataHandler 透過類別變數引用，其他模組直接匯入。
"""

import polars as pl

ADMIN1_SCHEMA = pl.Schema(
    {
        "id": pl.String,
        "name": pl.String,
        "asciiname": pl.String,
        "geoname_id": pl.String,
    }
)

GEODATA_SCHEMA = pl.Schema(
    {
        "longitude": pl.String,
        "latitude": pl.String,
        "country": pl.String,
        "admin_1": pl.String,
        "admin_2": pl.String,
        "admin_3": pl.String,
        "admin_4": pl.String,
    }
)

CITIES_SCHEMA = pl.Schema(
    {
        "geoname_id": pl.String,
        "name": pl.String,
        "asciiname": pl.String,
        "alternatenames": pl.String,
        "latitude": pl.String,
        "longitude": pl.String,
        "feature_class": pl.String,
        "feature_code": pl.String,
        "country_code": pl.String,
        "cc2": pl.String,
        "admin1_code": pl.String,
        "admin2_code": pl.String,
        "admin3_code": pl.String,
        "admin4_code": pl.String,
        "population": pl.UInt32,
        "elevation": pl.String,
        "dem": pl.Int32,
        "timezone": pl.String,
        "modification_date": pl.Date,
    }
)
