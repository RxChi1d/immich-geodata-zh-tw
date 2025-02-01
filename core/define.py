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

MUNICIPALITIES = [
    "臺北市",
    "新北市",
    "桃園市",
    "臺中市",
    "臺南市",
    "高雄市",
    "基隆市",
    "新竹市",
    "嘉義市",
]

CHINESE_PRIORITY = ["zh-Hant", "zh-TW", "zh-HK", "zh", "zh-Hans", "zh-CN", "zh-SG"]

TAIWAN_ADMIN1 = {
    # 直轄市 (Special Municipalities)
    "臺北市": "TW.01",
    "新北市": "TW.02",
    "桃園市": "TW.03",
    "臺中市": "TW.04",
    "臺南市": "TW.05",
    "高雄市": "TW.06",
    # 省轄市 (Provincial Cities)
    "基隆市": "TW.07",
    "新竹市": "TW.08",
    "嘉義市": "TW.09",
    # 縣 (Counties)
    "宜蘭縣": "TW.10",
    "新竹縣": "TW.11",
    "苗栗縣": "TW.12",
    "彰化縣": "TW.13",
    "南投縣": "TW.14",
    "雲林縣": "TW.15",
    "嘉義縣": "TW.16",
    "屏東縣": "TW.17",
    "臺東縣": "TW.18",
    "花蓮縣": "TW.19",
    "澎湖縣": "TW.20",
    "金門縣": "TW.21",
    "連江縣": "TW.22",
}
