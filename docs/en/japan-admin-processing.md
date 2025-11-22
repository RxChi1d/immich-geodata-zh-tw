# Japan Administrative Division Processing

> This document describes how the project handles geographic data for Japan. It expands on the Japan optimization section in the README.

## Data Sources

The pipeline relies on official datasets published via the **National Land Numerical Information download service (国土数値情報ダウンロードサービス)**:

- **Provider**: [National Land Numerical Information Download Service](https://nlftp.mlit.go.jp/ksj/)
- **Dataset**: Administrative boundary data (world geodetic system)
- **Purpose**: Supplies authoritative polygons and official place names so the output matches common usage in Taiwan (漢字 + かな)

Processing this dataset ensures Immich shows Japanese place names in their native form, which is widely understood by Taiwanese users.

## Administrative Hierarchy Mapping

The project again follows the GeoNames administrative schema and maps it to Japan as follows:

- **Admin1**: All 47 prefectures (e.g., 東京都, 北海道, 神奈川県)
- **Admin2**: Cities, wards, towns, and villages under each prefecture (e.g., 横浜市, 渋谷区, 鎌倉市)
- **Admin3**: Ward names for designated cities only
  - When `SEIREI_SHI_CITY_NAME_ONLY = True`, admin_2 shows only the city name (e.g., "横浜市") and admin_3 contains the ward name (e.g., "中区")
  - For all other administrative types, admin_3 remains empty
- **Admin4**: Not used (no corresponding data available)

## Column Mapping to GeoNames

When extracting the shapefile, the following columns are used:

- **country**: "日本"
- **admin_1**: `N03_001` (prefecture name)
- **admin_2**: Derived from `N03_003`, `N03_004`, and `N03_005` following the rules below
- **admin_3**: Ward name (`N03_005`) for designated cities when `SEIREI_SHI_CITY_NAME_ONLY = True`
- **admin_4**: Left empty

## Display Rules

Different administrative categories have distinct display rules to balance readability and ambiguity:

### 1. Standard Cities

- **Condition**: `N03_003` is empty, `N03_004` ends with 「市」, `N03_005` is empty
- **Display**: Show the city name only (e.g., 北海道 → 釧路市)

### 2. Towns, Villages, and Special Wards Directly Governed by the Prefecture

- **Condition**: `N03_003` is empty, `N03_004` does not end with 「市」, `N03_005` is empty
- **Display**: Show the town, village, or ward name (e.g., 東京都 → 小笠原村, 東京都 → 渋谷区)
- **Context**: Tokyo’s 23 special wards belong here; they are directly administered by the Tokyo Metropolitan Government

### 3. Designated Cities (政令指定都市)

- **Condition**: `N03_005` has a value (ward name)
- **Display**: admin_2 shows only the city name, admin_3 contains the ward name (e.g., 神奈川県, 横浜市, 中区 → admin_2="横浜市", admin_3="中区")
- **Rationale**:
  - Matches common behavior on services such as Google Maps and OpenStreetMap, which show prefecture + city
  - Avoids misclassification when district centroids lie close together
  - Preserves complete information: individual ward records, centroids, and ward names (admin_3) are all retained in the dataset
- **Configuration**: Set `SEIREI_SHI_CITY_NAME_ONLY = False` in `JapanGeoDataHandler` to revert to the "city + ward" display in admin_2. In this case, admin_3 will remain empty.

### 4. District-Governed Towns and Villages (郡轄町村)

- **Condition**: `N03_003` ends with 「郡」 and `N03_004` holds a town or village name
- **Display**:
  - If the same town/village name is unique within the prefecture, show the name only
  - If multiple districts share the same name within the prefecture, prefix the district (e.g., 北海道, 古宇郡, 泊村 → 古宇郡泊村)
- **Collision Detection**: The implementation checks for duplicates on "prefecture + town/village"; district prefixes are added only when needed to resolve ambiguity

### 5. District Records Without Subdivisions

- **Condition**: `N03_003` has a value while `N03_004` and `N03_005` are empty
- **Display**: Use the district name as-is
- **Usage**: A fallback for rare polygons that do not include lower administrative levels

## Processing Details

### Centroid Calculation

- Uses adaptive UTM zones combined with an Albers projection to compute centroids
- Coordinates are converted back to WGS84 and rounded to eight decimal places

### Data Cleaning and Output

- Trims empty strings and `None` values from `N03_003`, `N03_004`, and `N03_005` before applying the rules
- Uses base class shared method for sorting and standardized output

## Processing Workflow

```bash
# 1. Download the dataset
#    Visit https://nlftp.mlit.go.jp/ksj/gml/datalist/KsjTmplt-N03-2025.html
#    and download the "Administrative boundary data (world geodetic system)" package.

# 2. Run the extraction script
uv run python main.py extract --country JP \
  --shapefile geoname_data/N03-20250101_GML/N03-20250101.shp \
  --output meta_data/jp_geodata.csv
```

Refer to the [developer workflow](../../README.md#開發者本地資料處理) for end-to-end guidance.

## Reference Examples

| Category | N03_001 | N03_003 | N03_004 | N03_005 | admin_2 | admin_3 | Notes |
| --- | --- | --- | --- | --- | --- | --- | --- |
| Standard city | 北海道 |  | 釧路市 |  | 釧路市 |  | City not governed by a district |
| Direct-controlled village | 東京都 |  | 小笠原村 |  | 小笠原村 |  | Island directly managed by 東京都 |
| Special ward | 東京都 |  | 渋谷区 |  | 渋谷区 |  | Tokyo special ward |
| Designated city ward | 神奈川県 | 横浜市 | 横浜市 | 中区 | 横浜市 | 中区 | admin_2 shows city name, admin_3 preserves ward name |
| District town (unique) | 新潟県 | 岩船郡 | 関川村 |  | 関川村 |  | Name is unique, no district prefix |
| District village (duplicate) | 北海道 | 古宇郡 | 泊村 |  | 古宇郡泊村 |  | Prefix resolves duplicate with 国後郡泊村 |

## References

- [National Land Numerical Information Download Service](https://nlftp.mlit.go.jp/ksj/)
- [GeoNames Administrative Division Codes](https://www.geonames.org/export/codes.html)
