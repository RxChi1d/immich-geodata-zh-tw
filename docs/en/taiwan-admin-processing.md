# Taiwan Administrative Division Processing

> This document explains how the project processes geographic information for Taiwan. It expands on the Taiwan optimization section in the README.

## Data Sources

The Taiwan pipeline centers on official datasets published by the **National Land Surveying and Mapping Center (NLSC)** of the Republic of China (Taiwan):

- **Provider**: [NLSC Open Data Platform](https://whgis-nlsc.moi.gov.tw/Opendata/Files.aspx)
- **Dataset**: Village boundaries (TWD97 latitude and longitude)
- **Purpose**: Supplies authoritative village-level polygons and official names to guarantee accuracy

By processing the NLSC village dataset, the pipeline can reverse-geocode down to the village and ensure precise township- and county-level results.

## Administrative Hierarchy Mapping

The project adopts the GeoNames administrative schema and maps it to Taiwan as follows:

- **Admin1**: The 22 municipalities and counties (e.g., 臺北市, 新北市, 彰化縣, 南投縣)
- **Admin2**: Townships, districts, and county-administered cities (e.g., 新北市板橋區, 彰化縣彰化市, 南投縣埔里鎮)
- **Admin3**: Villages and neighborhoods (里) from the NLSC dataset (e.g., 臺北市大安區龍安里, 新北市板橋區文化里)
- **Admin4**: Currently unused

## Column Mapping to GeoNames

During extraction, the columns are mapped directly from the NLSC shapefile:

- **country**: "臺灣"
- **admin_1**: `COUNTYNAME` (municipality/county name)
- **admin_2**: `TOWNNAME` (township, district, or county-administered city)
- **admin_3**: `VILLNAME` (village/neighborhood)
- **admin_4**: Empty for now

## Display Rules

The pipeline trusts the NLSC source without additional transformations:

- **County or municipality (admin_1)**: Directly uses `COUNTYNAME`
- **Township/district (admin_2)**: Directly uses `TOWNNAME`
- **Village (admin_3)**: Directly uses `VILLNAME`

> [!NOTE]
> The NLSC dataset already contains complete administrative attributes. The implementation reuses these official values without injecting custom overrides or validation logic.

## Processing Details

### Centroid Calculation

- The source data is in TWD97 latitude and longitude (EPSG:4326)
- To compute accurate centroids, geometries are projected to TWD97 / TM2 zone 121 (EPSG:3826)
- Centroids are calculated in the projected system and converted back to WGS84 (EPSG:4326)
- Coordinates are rounded to eight decimal places (roughly 1.1 mm precision)

> [!NOTE]
> Working in a projected coordinate system avoids distortion that would otherwise occur when computing centroids directly in geographic coordinates.

### Data Cleaning

- Rows with missing coordinates are filtered out
- Object columns are normalized to strings to keep the CSV consistent

### Output Ordering

- The resulting CSV is sorted by `country → admin_1 → admin_2`
- Sorting makes version control diffs and manual reviews easier

## Processing Workflow

The end-to-end workflow looks like this:

```bash
# 1. Download the NLSC dataset
#    Visit https://whgis-nlsc.moi.gov.tw/Opendata/Files.aspx
#    and download the "Village boundaries (TWD97 latitude & longitude)" package.

# 2. Run the extraction script
uv run python main.py extract --country TW \
  --shapefile geoname_data/VILLAGE_NLSC_XXXXXX/VILLAGE_NLSC_XXXXXX.shp \
  --output meta_data/tw_geodata.csv
```

Refer to the [developer workflow](../../README.md#開發者本地資料處理) for additional context and integration steps.

## Reference Examples

The table below illustrates how different administrative types map into the GeoNames schema. Original names are preserved because translating them would lose important nuance.

| Administrative Type | COUNTYNAME | TOWNNAME | VILLNAME | admin_1 | admin_2 | admin_3 |
| --- | --- | --- | --- | --- | --- | --- |
| Municipality district | 臺北市 | 大安區 | 龍安里 | 臺北市 | 大安區 | 龍安里 |
| Municipality district | 新北市 | 板橋區 | 文化里 | 新北市 | 板橋區 | 文化里 |
| Provincial city district | 新竹市 | 東區 | 光復里 | 新竹市 | 東區 | 光復里 |
| County-administered city | 彰化縣 | 彰化市 | 中山里 | 彰化縣 | 彰化市 | 中山里 |
| County township | 南投縣 | 埔里鎮 | 南村里 | 南投縣 | 埔里鎮 | 南村里 |
| County rural township | 花蓮縣 | 壽豐鄉 | 壽豐村 | 花蓮縣 | 壽豐鄉 | 壽豐村 |

## References

- [NLSC Open Data Platform](https://whgis-nlsc.moi.gov.tw/Opendata/Files.aspx)
- [GeoNames Administrative Division Codes](https://www.geonames.org/export/codes.html)
- [TWD97 Coordinate System Overview](https://www.sunriver.com.tw/grid_tm2.htm)
