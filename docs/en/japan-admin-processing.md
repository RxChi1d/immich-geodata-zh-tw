# Japan Administrative Division Processing

> This document explains how the project processes geographic data for Japan. It is the detailed version of [Supported Regions and Language Strategy](../../README.en.md#supported-regions-and-language-strategy) in the README.

## Data Sources

Processing for Japan is built on the official boundary data published by the **国土数値情報ダウンロードサイト** (National Land Numerical Information download site):

- **Source**: [国土数値情報ダウンロードサービス](https://nlftp.mlit.go.jp/ksj/)
- **Dataset**: 行政区域データ（世界測地系）N03-2025 (`N03-20250101.shp`)
- **Fields used**: `N03_001`, `N03_003`, `N03_004`, `N03_005` (`N03_002`, the subprefecture name, is not used)
- **Purpose**: Serves as the primary source of administrative boundaries and names for Japan, keeping the data accurate and authoritative

Processing the 国土数値情報 administrative data keeps Japanese place names in Immich in their original Japanese form (kanji + kana), which matches the reading habits of Taiwanese users.

## Administrative Level Definitions

> [!NOTE]
> `admin_3` and `admin_4` exist only in this project's intermediate CSV. They preserve the source administrative levels for traceability and debugging, and are not written to the cities500 file that Immich consumes. The finest level Immich displays is `admin_2`. Representative point density depends on the granularity of the source features used during extract (municipalities for Japan) and is unrelated to these two fields.

The project follows the GeoNames administrative level system, mapped to Japan as follows:

- **Admin 1**: The **47 prefectures**
  - Examples: 東京都, 北海道, 神奈川県

- **Admin 2**: The **cities, wards, towns, and villages** under each prefecture
  - Examples: 横浜市, 渋谷区, 鎌倉市
  - The displayed value depends on the administrative type (see the rules below)

- **Admin 3**: Used only for **ward names of designated cities**
  - For a designated city, `admin_2` holds the city name only (such as 「横浜市」) and `admin_3` holds the ward name (such as 「中区」)
  - `admin_3` is empty for every other administrative type

- **Admin 4**: Always empty (no corresponding data)

## GeoNames Column Mapping

Extraction from the 国土数値情報 dataset uses the following column mapping:

- **Country**: 「日本」
- **admin_1**: `N03_001` (prefecture name)
- **admin_2**: Depends on the administrative type (see the rules below)
- **admin_3**: `N03_005` (ward name), for designated cities only
- **admin_4**: Left empty

## Display Rules

Each type of Japanese administrative division gets the display form that suits it best:

### 1. Ordinary Cities

- **Applies when**: The city is not governed by a district (`N03_003` and `N03_005` are both empty, and `N03_004` ends with 「市」)
- **Display**: City name only
- **Example**:
  - 北海道 → 釧路市

### 2. Prefecture-Governed Towns, Villages, and Special Wards

- **Applies when**: The town, village, or special ward reports directly to the prefecture (`N03_003` and `N03_005` are both empty, and `N03_004` has a value that does not end with 「市」)
- **Display**: Town, village, or ward name only
- **Examples**:
  - 東京都 → 小笠原村 (remote island)
  - 東京都 → 渋谷区 (special ward)

> [!NOTE]
> The 23 special wards of Tokyo (千代田区, 港区, and so on) report directly to the Tokyo Metropolitan Government and are not governed by any other administrative division.

### 3. Designated Cities

- **Applies when**: The record is a ward of a designated city (`N03_005` has a value; a non-empty `N03_005` alone triggers this rule, regardless of the other fields)
- **Display**: `admin_2` holds the city name only, `admin_3` holds the ward name
- **Rationale**:
  - **More consistent display**: Matches mainstream map services (Google Maps, OpenStreetMap), which also return only the prefecture plus the designated city name
  - **Fewer misclassifications**: Centroids of adjacent wards in a designated city sit close together, so lookups easily land on the wrong ward
  - **No information loss**: Each ward keeps its own record, centroid, and ward name (`admin_3`) in the dataset
- **Example**:
  - 神奈川県, 横浜市, 中区 → `admin_2` = 「横浜市」, `admin_3` = 「中区」

> [!NOTE]
> The Rust production handler currently hard-codes the "city in `admin_2`, ward in `admin_3`" strategy. To restore a combined "city + ward" value in `admin_2` (such as 「横浜市中区」), adjust the Japan handler in `src/pipeline/extract/handlers.rs` and update the parity fixtures and real-data validation along with it.

### 4. District-Governed Towns and Villages

- **Applies when**: The town or village is governed by a district (`N03_003` ends with 「郡」)
- **Display**: Depends on whether the name collides
  - **No collision**: Town or village name only (concise form)
  - **Collision**: District name followed by the town or village name
- **Examples**:
  - 新潟県, 岩船郡, 関川村 → 「関川村」 (concise form)
  - 北海道, 古宇郡, 泊村 → 「古宇郡泊村」 (avoids confusion with 国後郡泊村)

> [!NOTE]
> The project automatically detects whether several districts within the same prefecture share a town or village name. The district prefix is added only when such a collision exists.

**How collisions are judged**:
- 「釧路市」 and 「釧路町」 do **not** count as a collision (one is a city, the other a town)
- 「古宇郡泊村」 and 「国後郡泊村」 **do** count as a collision (both are 泊村)

### 5. Upper-Level Division Name Only (Fallback Rule)

- **Applies when**: The record provides only an upper-level division name other than a district, with no municipality (`N03_004` and `N03_005` are both empty, and `N03_003` has a value that does not end with 「郡」)
- **Display**: The upper-level division name
- **Note**: This is a fallback for incomplete data. The current N03 dataset contains no such records (zero rows in `meta_data/jp_geodata.csv`)

> [!NOTE]
> If the upper-level name ends with 「郡」 and the municipality is empty, the record falls into the district-governed rule (rule 4) first. `admin_2` is then empty, so the translation stage drops the record and it never reaches Immich.

## Processing Details

### Coordinate Calculation

- Compute an approximate centroid in the Japan Albers equal-area projection (`+proj=aea +lat_1=30 +lat_2=45 +lat_0=37.5 +lon_0=138`) to pick the applicable UTM zone
- Compute the polygon centroid in that UTM zone, then convert back to WGS84 for the output latitude and longitude
- A MultiPolygon yields a single merged centroid; no per-part splitting
- Coordinates are rounded to 8 decimal places, and trailing zeros are stripped in the CSV, so a value may show fewer than 8 digits

### Data Cleaning and Output

- Empty strings, `None`, and `nan` in `N03_003`, `N03_004`, and `N03_005` are all treated as empty, keeping the rule conditions consistent
- Features with all three fields empty are dropped and produce no output row
- Sorting and standardized columns come from the shared Rust output path

For the commands that reproduce this workflow locally, see [Local Data Processing](development.md#2-extract-raw-geographic-data).

## Reference Examples

The table below shows real processing results for each administrative type:

| Administrative type | N03_001 | N03_003 | N03_004 | N03_005 | admin_2 | admin_3 | Notes |
|---------------------|---------|---------|---------|---------|---------|---------|-------|
| Ordinary city | 北海道 | (empty) | 釧路市 | (empty) | 釧路市 | (empty) | City not governed by a district, so the city name is used directly |
| Prefecture-governed village | 東京都 | (empty) | 小笠原村 | (empty) | 小笠原村 | (empty) | Remote island administered directly by the metropolis |
| Special ward | 東京都 | (empty) | 渋谷区 | (empty) | 渋谷区 | (empty) | One of Tokyo's 23 special wards |
| Designated city ward | 神奈川県 | (empty) | 横浜市 | 中区 | 横浜市 | 中区 | `N03_003` plays no part in the decision; `admin_2` holds the city name, `admin_3` keeps the ward name |
| District-governed village (no collision) | 新潟県 | 岩船郡 | 関川村 | (empty) | 関川村 | (empty) | Concise form |
| District-governed village (collision) | 北海道 | 古宇郡 | 泊村 | (empty) | 古宇郡泊村 | (empty) | District prefix added automatically when districts share a name |

## References

- [国土数値情報ダウンロードサービス](https://nlftp.mlit.go.jp/ksj/)
- [GeoNames Administrative Division Codes](https://www.geonames.org/export/codes.html)
