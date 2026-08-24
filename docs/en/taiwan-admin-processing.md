# Taiwan Administrative Division Processing

> This document explains how the project handles geographic information for Taiwan. It is the detailed version of [Supported Regions and Language Strategy](../../README.en.md#supported-regions-and-language-strategy) in the README.

## Data Sources

Taiwan processing is built on the official boundary data published by the **National Land Surveying and Mapping Center (NLSC)**:

- **Source**: [NLSC Open Data Platform](https://whgis-nlsc.moi.gov.tw/Opendata/Files.aspx)
- **Dataset**: Village boundaries (TWD97 latitude and longitude)
- **Purpose**: Serves as the primary source for Taiwan village boundaries and administrative division names, keeping the data accurate and authoritative

Processing the NLSC village data lets reverse geocoding resolve coordinates down to the village level, which in turn yields more precise township and county results.

## Administrative Level Definitions

> [!NOTE]
> `admin_3` and `admin_4` exist only in this project's intermediate CSV. They preserve the source administrative levels for traceability and debugging, and are never written to the cities500 file Immich consumes. The finest level Immich displays is `admin_2`. Representative point density depends on the granularity of the source features read during extract (villages for Taiwan), not on these two columns.

The project follows the GeoNames administrative level system, mapped to Taiwan as follows:

- **Admin 1**: The **22 special municipalities and provincial cities or counties**
  - Examples: 臺北市, 基隆市, 彰化縣, 南投縣

- **Admin 2**: The **townships, urban townships, county-administered cities, and districts** within each city or county
  - Examples: 新北市板橋區, 彰化縣彰化市, 南投縣埔里鎮

- **Admin 3**: The **villages (村, 里)** in the NLSC data
  - Examples: 臺北市大安區龍安里, 新北市板橋區文化里

- **Admin 4**: Currently unused

## GeoNames Column Mapping

Extraction from the NLSC village boundary data uses this column mapping:

- **country**: The intermediate CSV always writes 臺灣, for human inspection only. The column never reaches cities500; the country name Immich displays comes from the country code `TW`
- **admin_1**: `COUNTYNAME` (city or county name)
- **admin_2**: `TOWNNAME` (township, urban township, county-administered city, or district name)
- **admin_3**: `VILLNAME` (village name); writes the string `None` when the source lacks the field
- **admin_4**: Currently an empty string

## Display Rules

The project uses the administrative information from the official NLSC data as-is, with no extra modification or validation. During translation, Taiwan place names always keep their official names: no OpenCC conversion and no NAER official translation lookup. Only a one-off 裏 → 里 character fix and empty-value normalization remain, and neither takes effect with the current NLSC data.

### How the Data Is Handled

- **City and county level (admin_1)**: Reads `COUNTYNAME` directly
  - Examples: 臺北市, 新北市, 彰化縣

- **Township and district level (admin_2)**: Reads `TOWNNAME` directly
  - Examples: 板橋區, 彰化市, 埔里鎮

- **Village level (admin_3)**: Reads `VILLNAME` directly
  - Examples: 龍安里, 文化里
  - When the source `VILLNAME` is empty — mostly map sheets for outlying islands with no village assigned — the intermediate CSV writes the string `None`. The current data has 206 such rows, 134 of them in 連江縣. The column never reaches cities500, so it does not affect what Immich displays

> [!NOTE]
> The project relies on the completeness and accuracy of the official NLSC data. The NLSC village boundary data already carries the full administrative division information (city or county, township or district, village), so the code uses those official values directly and needs no correction or validation logic of its own.

## Processing Details

### Coordinate Calculation

- The raw data is TWD97 latitude and longitude (geographic coordinates). The implementation follows the CRS declared in the source Shapefile's `.prj` instead of assuming a fixed code; extraction aborts with an error when the source declares no CRS
- To keep centroid calculation accurate, geometries are first projected to TWD97 / TM2 zone 121 (EPSG:3826)
- Polygon centroids are computed in the projected coordinate system. Multipart features such as outlying islands and enclaves collapse to a single representative point using the area-weighted centroid of the merged parts, with no per-part splitting (per-part splitting is currently enabled for Indonesia only), so one village is always one row
- The centroid is converted back to WGS84 (EPSG:4326) before latitude and longitude are written out
- Coordinates are rounded to eight decimal places (roughly 1.1 mm precision), and trailing zeros are stripped on write, so a field may hold fewer than eight

> [!NOTE]
> Computing centroids in a projected coordinate system avoids the distance distortion of geographic coordinates and keeps centroid positions accurate.

### Data Cleaning and Output

- Records without geometry (Shapefile NullShape) are skipped; coordinates that cannot be parsed abort the run with an error rather than being dropped silently
- Only `COUNTYNAME`, `TOWNNAME`, and `VILLNAME` are read, and DBF values of every type are converted to strings on output
- Sorting and standardized columns come from the shared Rust output path

For the commands that reproduce this process locally, see [Local Data Processing](development.md#2-extract-raw-geographic-data).

## Reference Examples

The table below shows real processing examples for the different administrative types in Taiwan:

| Administrative Type | COUNTYNAME | TOWNNAME | VILLNAME | admin_1 shown | admin_2 shown | admin_3 shown |
|-----------|-----------|----------|----------|-------------|-------------|-------------|
| Special municipality district | 臺北市 | 大安區 | 龍安里 | 臺北市 | 大安區 | 龍安里 |
| Special municipality district | 新北市 | 板橋區 | 文化里 | 新北市 | 板橋區 | 文化里 |
| Provincial city district | 新竹市 | 東區 | 光復里 | 新竹市 | 東區 | 光復里 |
| County-administered city | 彰化縣 | 彰化市 | 中山里 | 彰化縣 | 彰化市 | 中山里 |
| Urban township (鎮) | 南投縣 | 埔里鎮 | 南村里 | 南投縣 | 埔里鎮 | 南村里 |
| Rural township (鄉) | 花蓮縣 | 壽豐鄉 | 壽豐村 | 花蓮縣 | 壽豐鄉 | 壽豐村 |

## References

- [NLSC Open Data Platform](https://whgis-nlsc.moi.gov.tw/Opendata/Files.aspx)
- [GeoNames Administrative Division Codes](https://www.geonames.org/export/codes.html)
- [TWD97 Coordinate System Overview](https://www.sunriver.com.tw/grid_tm2.htm)
