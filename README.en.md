# Immich Reverse Geocoding - Taiwan Localization  

> [!IMPORTANT]
> - Upgrade notice: v3.1.0 upgrades place names worldwide with official NAER Taiwan-style translations and adds Indonesia geodata (Traditional Chinese localization); v3.0.0 added Thailand geodata. If you already deploy this project, please run "[Re-extract photo metadata](#integrated-deployment-recommended-convenient-for-future-updates)" after upgrading to apply the latest datasets.

> [!NOTE]
> Supports Immich v2 and v3.

[繁體中文](README.md) | [English](README.en.md)

This project delivers reverse geocoding enhancements tailored for users in Taiwan, providing natural and accurate location display that reflects local reading habits.

Currently supports: 🇹🇼 **Taiwan** | 🇯🇵 **Japan** | 🇰🇷 **South Korea** | 🇹🇭 **Thailand** | 🇮🇩 **Indonesia** | 🌏 **Traditional Chinese localization for other regions**

## Design Philosophy

We focus on the Taiwan user experience and apply the most suitable language strategy per region:

- **Taiwan**: Uses NLSC datasets to fix country and administrative naming issues
- **Japan**: Uses 国土数値情報 datasets and preserves native names (漢字 + かな)
- **South Korea**: Uses admdongkor project data and provides Traditional Chinese translations
- **Thailand**: Uses official COD-AB boundary data and provides Traditional Chinese translations for Admin1/Admin2, falling back to official English and Thai names
- **Indonesia**: Uses official village-level (desa) boundary data from BIG (Badan Informasi Geospasial) and provides Traditional Chinese translations for Admin1/Admin2, falling back to official Indonesian names when no Chinese label exists
- **Other regions**: Prioritizes the official Taiwan translations from the National Academy for Educational Research (NAER) *Translations of Foreign Place Names* to fill gaps and upgrade names, then GeoNames translations, falling back to English when no common translation exists

> [!TIP]
> Compatibility Notice
> 
> - Starting from Immich 1.136.0, container paths have changed.
> - If you're on 1.135.x or earlier and use Manual Deployment, adjust the `volumes` mapping as described in the [Manual Deployment](#manual-deployment) section.
> - If you use this project's integrated auto-deployment (update_data.sh), no changes are required; the script has been updated to support both old and new versions.

### Before and After Comparison
![Before and After Comparison](./image/example.png) 

## Table of Contents

- [Immich Reverse Geocoding - Taiwan Localization](#immich-reverse-geocoding---taiwan-localization)
  - [Design Philosophy](#design-philosophy)
    - [Before and After Comparison](#before-and-after-comparison)
  - [Table of Contents](#table-of-contents)
  - [Supported Regions and Language Strategy](#supported-regions-and-language-strategy)
  - [Data Sources](#data-sources)
  - [Usage](#usage)
    - [Integrated Deployment (Recommended, convenient for future updates)](#integrated-deployment-recommended-convenient-for-future-updates)
    - [Manual Deployment](#manual-deployment)
  - [Specify Specific Version](#specify-specific-version)
  - [Administrative Optimization Strategy](#administrative-optimization-strategy)
    - [🇹🇼 Taiwan](#-taiwan)
    - [🇯🇵 Japan](#-japan)
    - [🇰🇷 South Korea](#-south-korea)
    - [🇹🇭 Thailand](#-thailand)
    - [🇮🇩 Indonesia](#-indonesia)
    - [🌏 Other Regions](#-other-regions)
  - [Update Geographic Data](#update-geographic-data)
    - [Integrated Deployment](#integrated-deployment)
    - [Manual Deployment](#manual-deployment-1)
  - [Developer: Local Data Processing](#developer-local-data-processing)
    - [1. Install Dependencies](#1-install-dependencies)
    - [Official Prebuilt Binary](#official-prebuilt-binary)
    - [Build the Rust CLI Locally](#build-the-rust-cli-locally)
    - [2. Extract Raw Geographic Data (Optional)](#2-extract-raw-geographic-data-optional)
      - [Taiwan Data Extraction](#taiwan-data-extraction)
      - [Japan Data Extraction](#japan-data-extraction)
      - [South Korea Data Extraction](#south-korea-data-extraction)
      - [Thailand Data Extraction](#thailand-data-extraction)
      - [Indonesia Data Extraction](#indonesia-data-extraction)
    - [3. Complete Data Processing Workflow](#3-complete-data-processing-workflow)
      - [Register LocationIQ API](#register-locationiq-api)
      - [Execute Data Processing](#execute-data-processing)
    - [4. Rust Verification](#4-rust-verification)
  - [Acknowledgments](#acknowledgments)
  - [License](#license)

## Supported Regions and Language Strategy

The project applies region-specific language handling to reflect the expectations of users in Taiwan:

| Region | Language Strategy | Data Source | Notes |
| --- | --- | --- | --- |
| 🇹🇼 Taiwan | Official Traditional Chinese names | NLSC (National Land Surveying and Mapping Center) | Fixes incorrect country labels and missing municipality names |
| 🇯🇵 Japan | Native Japanese (漢字 + かな) | 国土数値情報ダウンロードサービス | Displays official Japanese names without translating them |
| 🇰🇷 South Korea | Traditional Chinese translations | admdongkor (Official administrative boundaries) | Provides Traditional Chinese translations |
| 🇹🇭 Thailand | Traditional Chinese translations (official English / Thai fallback) | COD-AB Thailand | Computes administrative center points from official boundaries |
| 🇮🇩 Indonesia | Traditional Chinese translations (official Indonesian fallback) | BIG (Badan Informasi Geospasial) | Computes administrative center points from village-level boundaries; covers all 38 provinces |
| 🌏 Others | Traditional Chinese translations | NAER official translations → GeoNames translations → GeoNames English | Prioritizes NAER official Taiwan-style translations; falls back to GeoNames when unavailable |

> **Why keep Japanese in Japanese?**
> Taiwanese users are familiar with Japanese kanji and kana in combination. Names such as 「横浜市」 or 「うるま市」 remain understandable without romanization or Chinese conversion.
  
## Data Sources

The geographic data used in this project mainly comes from the following sources:

1.  **GeoNames** ([geonames.org](https://www.geonames.org/))
    - **License**: Creative Commons Attribution 4.0 International (CC-BY 4.0)
    - **Purpose**: As the global geographic location database foundation
2.  **OpenStreetMap** (via LocationIQ)
    - **License**: Open Database License (ODbL) 1.0
    - **Purpose**: Via LocationIQ API for reverse geocoding of regions other than Taiwan, Japan, South Korea, Thailand, and Indonesia
    - **Attribution**: Data © OpenStreetMap contributors, ODbL 1.0
3.  **National Land Surveying and Mapping Center (NLSC)** of Taiwan
    - **Source**: [NLSC Open Data Platform](https://whgis-nlsc.moi.gov.tw/Opendata/Files.aspx)
    - **Dataset**: Village Boundaries (TWD97 Latitude/Longitude), Version 1140620
    - **License**: Government Open Data License, Version 1.0
    - **Purpose**: As the primary data source for Taiwan region village boundaries and administrative district names, ensuring data accuracy and authority
4.  **国土数値情報ダウンロードサイト** (Japan)
    - **Source**: [国土数値情報ダウンロードサービス](https://nlftp.mlit.go.jp/ksj/)
    - **Dataset**: Administrative Area Data (World Geodetic System)
    - **License**: Japanese Government Open Data
    - **Purpose**: As the primary data source for Japan administrative boundaries and names
5.  **admdongkor** (South Korea)
    - **Source**: [admdongkor](https://github.com/vuski/admdongkor)
    - **Dataset**: South Korean official administrative boundary data (GeoJSON format)
    - **License**: No additional restrictions (attribution requested; see [NOTICE.md](NOTICE.md))
    - **Purpose**: As the primary data source for South Korea administrative boundaries and names
6.  **Thailand COD-AB**
    - **Source**: [HDX Thailand Subnational Administrative Boundaries](https://data.humdata.org/dataset/cod-ab-tha)
    - **Dataset**: Thailand administrative level 0-3 boundaries (COD-AB)
    - **License**: Creative Commons Attribution for Intergovernmental Organisations (CC BY-IGO)
    - **Purpose**: As the primary data source for Thailand administrative boundaries and names
7.  **BIG (Badan Informasi Geospasial) — Indonesia Geospatial Information Agency**
    - **Source**: BIG official ArcGIS REST FeatureServer (desa village-level feature service)
    - **Dataset**: Indonesia administrative desa (village-level) boundaries, version TASWIL20230928, covering all 38 provinces
    - **License**: Indonesian official open geospatial data (this project uses it as derived input only and does not redistribute original vector polygons)
    - **Purpose**: As the primary data source for Indonesia administrative boundaries and names
8.  **NAER *Translations of Foreign Place Names***
    - **Source**: [Open Government Data Platform dataset 15211](https://data.gov.tw/dataset/15211)
    - **License**: Government Open Data License, Version 1.0
    - **Purpose**: Fills gaps and overwrites with official Taiwan translations for global non-handler regions
9.  **Other References**
    - **Ministry of Economic Affairs International Trade Administration & Ministry of Foreign Affairs of Taiwan**: As reference sources for Chinese translations of some countries/regions

> [!NOTE]
> For complete data source attributions and licensing information, please refer to [NOTICE.md](NOTICE.md).

> [!NOTE]
> Since Immich's reverse geocoding functionality is based on its loaded database (this project mainly relies on GeoNames and NLSC data) and uses nearest distance principle to match place names, some results may not be completely precise or may differ from expectations.  

## Usage

This project supports the following two deployment methods:  

1. Integrated deployment (suitable for Immich's docker-compose deployment, ensures automatic loading of latest Taiwan localization data when container starts).

2. Manual deployment (suitable for custom deployment environments, allows manual download and configuration of localization data).

### Integrated Deployment (Recommended, convenient for future updates)
  
1. **Modify `docker-compose.yml` configuration**  
   Add `entrypoint` setting to the `immich_server` service to automatically download the latest geographic data when the container starts:  
   ```yaml  
   services:
     immich_server:
      container_name: immich_server

      # Other configurations omitted

      entrypoint: [ "tini", "--", "/bin/bash", "-c", "bash <(curl -sSL https://github.com/RxChi1d/immich-geodata-zh-tw/releases/latest/download/update_data.sh) --install && exec start.sh" ]
   ```  
   > [!NOTE]
   > - The `entrypoint` will first execute this project's `update_data.sh` script when the container starts, automatically downloading and configuring Taiwan localization data, then execute Immich server's `start.sh` to start the service.
   > - Integrated deployment also supports specifying specific version downloads. For details, please refer to the [Specify Specific Version](#specify-specific-version) section.

2. **Restart Immich**  
   Execute the following command to restart Immich： 
   ```bash  
   # If using docker-compose deployment
   docker compose down && docker compose up
   ```  
   - After startup, check if logs show messages like `10000 geodata records imported` to confirm geodata has been successfully updated.  
   - If not updated, please modify `geodata/geodata-date.txt` to a newer timestamp, ensuring it's later than Immich's last load time. 
  
3. **Re-extract photo metadata**  
   Log into Immich admin backend, go to **Administration > Tasks**, click **Extract Metadata > All** to trigger re-extraction of photo metadata. After completion, all photos' geographic information will be displayed in Chinese.  
   Newly uploaded photos require no additional operations and can directly support Chinese search.  

### Manual Deployment

1. **Modify `docker-compose.yml` configuration**  
   Add the following mappings to `volumes` (please adjust paths according to actual environment):  
   ```yaml
   volumes:
     - /mnt/user/appdata/immich/geodata:/build/geodata:ro
     - /mnt/user/appdata/immich/i18n-iso-countries/langs:/usr/src/app/server/node_modules/i18n-iso-countries/langs:ro
   ```
  > [!NOTE]
  > For Immich < 1.136.0, please change the second line to:  
  > `/mnt/user/appdata/immich/i18n-iso-countries/langs:/usr/src/app/node_modules/i18n-iso-countries/langs:ro`
  
2. **Download Taiwan localization data**  
   Two download methods are provided:  
       
   (1) **Automatic download**  
      Refer to the `update_data.sh` script in this project, modify `DOWNLOAD_DIR` to the folder storing geodata and i18n-iso-countries, and execute the script:  
      ```bash
      bash update_data.sh
      ```  
      > [!NOTE]
      > - Manual deployment also supports specifying specific version downloads. For details, please refer to the [Specify Specific Version](#specify-specific-version) section.
      > - UnRAID users can execute the script through the User Scripts plugin.
     
   (2) **Manual download**  
      Go to the [Release page](https://github.com/RxChi1d/immich-geodata-zh-tw/releases) to find the required version, download the corresponding `release.tar.gz` or `release.zip`, and extract it to the specified location.
  
3. **Restart Immich and re-extract photo metadata**  
   Same as steps 2 and 3 in [**Integrated Deployment**](#integrated-deployment).

## Specify Specific Version

In some cases (e.g., when the latest release has issues), you may need to download or rollback to a specific release version. This project's update script supports specifying the release tag to download through the `--tag` parameter.

**How to find available Tags?**
Please go to this project's [Releases page](https://github.com/RxChi1d/immich-geodata-zh-tw/releases) to view all available release tag names (e.g., `v2.2.4`, `nightly`, etc.).

**Usage Examples:**

1.  **Integrated Deployment (`entrypoint` in `docker-compose.yml`)**
    Add `--tag <tag_name>` after the entrypoint command:
    ```yaml
    entrypoint: [ "tini", "--", "/bin/bash", "-c", "bash <(curl -sSL https://github.com/RxChi1d/immich-geodata-zh-tw/releases/download/<tag_name>/update_data.sh) --install --tag <tag_name> && exec start.sh" ] 
    ```
    Replace `<tag_name>` in both places with the specific tag name you want to download. If `--tag` is omitted, the latest release (`latest`) is downloaded by default.

2.  **Manual Deployment (`update_data.sh`)**
    Add `--tag <tag_name>` when executing the script:
    ```bash
    bash update_data.sh --tag <tag_name>
    ```
    Replace `<tag_name>` with the specific tag name you want to download. If `--tag` is omitted, the latest release (`latest`) is downloaded by default.

> [!NOTE]
> The script will first verify whether the specified tag exists in GitHub Releases. If the tag is invalid, it will prompt an error and terminate execution, so please ensure the tag is valid before execution.
  
## Administrative Optimization Strategy

### 🇹🇼 Taiwan

- **Official datasets as the foundation**: Uses NLSC village boundaries to guarantee authoritative data
- **Correct country and division names**: Fixes Immich defaults such as "China Taiwan Province" and missing municipalities
- **Administrative hierarchy optimization**: Optimized Admin1 (municipalities/counties) and cities500 (place names data)

> 📖 See [Taiwan Administrative Processing (English)](docs/en/taiwan-admin-processing.md)

### 🇯🇵 Japan

- **Preserve native names**: Keeps the original kanji + kana combinations (e.g., 「静岡県」 instead of "Shizuoka Prefecture")
- **Context-aware subdivision handling**: Handles standard cities, special wards, designated cities, and Tokyo's special wards
- **Intelligent district prefixes**: Adds district names only when multiple towns share the same name within a prefecture

> 📖 See [Japan Administrative Processing (zh-TW)](docs/zh-tw/japan-admin-processing.md) • [Japan Administrative Processing (English)](docs/en/japan-admin-processing.md)

### 🇰🇷 South Korea

- **Traditional Chinese translations**: Extracts official administrative boundaries from admdongkor project and auto-translates to Traditional Chinese
- **Administrative naming optimization**: Metropolitan cities unified with "市" suffix (Seoul City, Busan City, Daegu City, etc.)
- **Special administrative divisions**: Jeju Province distinguished from Jeju City, Sejong City uses industry-standard translations
- **Administrative hierarchy handling**: Auto-splits "City + District/County" structure, supports special administrative structures

> 📖 See [South Korea Administrative Processing (zh-TW)](docs/zh-tw/south-korea-admin-processing.md) • [South Korea Administrative Processing (English)](docs/en/south-korea-admin-processing.md)

### 🇹🇭 Thailand

- **Official boundary data**: Uses COD-AB `tha_admin3` sub-district / tambon boundary data
- **Traditional Chinese translations**: Admin1/Admin2 use the Wikidata translator, falling back to COD-AB official English and official Thai when no Chinese result exists
- **Nearest-distance optimization**: Computes geometric center points with a Thailand Albers projection to improve Immich's nearest-point reverse geocoding hit rate
- **Coordinate strategy validation**: Does not use the COD-AB built-in `center_lat` / `center_lon` by default, as they are closer to official representative points than to the optimal single point under the nearest-point model

> 📖 See [Thailand Administrative Processing (zh-TW)](docs/zh-tw/thailand-admin-processing.md) • [Thailand Administrative Processing (English)](docs/en/thailand-admin-processing.md)

### 🇮🇩 Indonesia

- **Official village-level data**: Uses BIG (Badan Informasi Geospasial) official ArcGIS REST desa (village-level) boundary data (version TASWIL20230928, 83,461 usable features across all 38 provinces)
- **Traditional Chinese translations**: Admin1 (provinces) and Admin2 (regencies/cities) use the Wikidata translator with P131 administrative hierarchy validation at each level; falls back to BIG official Indonesian names when no reliable Chinese label exists; Admin3 (districts) and Admin4 (villages) retain Indonesian original names
- **Three time zones**: Resolves Indonesia's three time zones (WIB `Asia/Jakarta`, WITA `Asia/Makassar`, WIT `Asia/Jayapura`) via a per-province lookup table covering all 38 provinces
- **Nearest-distance optimization**: Uses an Indonesia Albers equal-area projection (`+lat_1=1 +lat_2=-8 +lon_0=118`) to compute geometric center points; each MultiPolygon part produces an independent candidate row, improving nearest-neighbor hit rates across the archipelago (Admin2 hit rate: 96.99%)

> 📖 See [Indonesia Administrative Processing (zh-TW)](docs/zh-tw/indonesia-admin-processing.md) • [Indonesia Administrative Processing (English)](docs/en/indonesia-admin-processing.md)

### 🌏 Other Regions

- **Official NAER translations**: Integrates the National Academy for Educational Research (NAER) *Translations of Foreign Place Names* (64,000+ official Taiwan-style translations), matching by normalized names with coordinate verification to fill gaps and upgrade place names worldwide
- **Confidence-tier protection**: Only high-confidence matches (matching country code + qualified distance + unambiguous) may overwrite existing translations; matches with weakening signals (natural-feature markers, near-distance ambiguity, etc.) only fill gaps; conservative rejection when disambiguation fails, eliminating same-name mismatches
- **GeoNames fallback**: Uses GeoNames Chinese data (converted to Taiwan Traditional Chinese via OpenCC) when no official translation exists, falling back to English otherwise

> 📖 See [Global Translation Processing (zh-TW)](docs/zh-tw/global-translation-processing.md) • [Global Translation Processing (English)](docs/en/global-translation-processing.md)

## Update Geographic Data

### Integrated Deployment
  
Simply restart the Immich container to automatically update geographic data.  

### Manual Deployment
  
1. Download the latest release.zip and extract it to the specified location.

2. Re-extract photo metadata (same as [Manual Deployment](#manual-deployment)).

## Developer: Local Data Processing

### 1. Install Dependencies

Local production data processing now uses the Rust CLI by default. Install a Rust toolchain, and
make sure the system provides `unzip`, `pkg-config`, and a PROJ development library such as
Ubuntu's `libproj-dev`.

#### Official Prebuilt Binary

GitHub Releases currently provide a prebuilt Rust CLI binary for Linux x86_64 only. This binary
primarily targets GitHub Actions, Linux servers, and Immich container-like environments. For macOS
and Windows, build the CLI locally for now.

#### Build the Rust CLI Locally

To run data processing on non-Linux systems or customized Linux environments, install a Rust
toolchain and the PROJ development library, then run:

```bash
cargo build --release
```

After compilation, the binary is available at:

```bash
target/release/immich-geodata
```

You can also run the CLI directly through `cargo run`:

```bash
cargo run --release -- help
```

### 2. Extract Raw Geographic Data (Optional)

If you need to process new countries or update existing geographic data sources, you can use the `extract` command to extract data from Shapefiles or GeoJSON. This step is optional and only needed when updating data sources.

#### Taiwan Data Extraction

Data source: [National Land Surveying and Mapping Center (NLSC)](https://whgis-nlsc.moi.gov.tw/Opendata/Files.aspx)

```bash
# 1. Download "Village Boundaries (TWD97 Latitude/Longitude)" data and extract
# 2. Execute extraction command
cargo run --release -- extract --country TW \
  --shapefile geoname_data/VILLAGE_NLSC_1140825/VILLAGE_NLSC_1140825.shp \
  --output meta_data/tw_geodata.csv
```

#### Japan Data Extraction

Data source: [国土数値情報](https://nlftp.mlit.go.jp/ksj/gml/datalist/KsjTmplt-N03-2025.html)

```bash
# 1. Download "行政区域データ（世界測地系）" and extract
# 2. Execute extraction command
cargo run --release -- extract --country JP \
  --shapefile geoname_data/N03-20250101_GML/N03-20250101.shp \
  --output meta_data/jp_geodata.csv
```

#### South Korea Data Extraction

Data source: [admdongkor](https://github.com/vuski/admdongkor)

```bash
# 1. Download official administrative boundary data from admdongkor project and extract
# 2. Execute extraction command
cargo run --release -- extract --country KR \
  --shapefile geoname_data/HangJeongDong_verYYYYMMDD.geojson \
  --output meta_data/kr_geodata.csv
```

#### Thailand Data Extraction

Data source: [Thailand COD-AB](https://data.humdata.org/dataset/cod-ab-tha)

```bash
# 1. Download tha_admin_boundaries.shp.zip and extract it
# 2. Use tha_admin3.shp to extract Admin 3 / Tambon boundary data
cargo run --release -- extract --country TH \
  --shapefile geoname_data/tha_admin_boundaries/tha_admin3.shp \
  --output meta_data/th_geodata.csv
```

Thailand extraction reads or creates `geoname_data/TH_wikidata_cache.json` for Admin1/Admin2
Traditional Chinese translations; Admin3 currently keeps the COD-AB official English names.

After extraction is complete, the data will be automatically integrated when executing the Rust
`release` command.

#### Indonesia Data Extraction

Data source: BIG (Badan Informasi Geospasial) official ArcGIS REST FeatureServer

```bash
# 1. Download the desa village-level dataset from BIG official REST service
#    using paginated requests (geometryPrecision=6, version TASWIL20230928)
#    For download instructions and fixed parameters, see docs/research/indonesia-handler.md
# 2. Execute extraction command
cargo run --release -- extract --country ID \
  --shapefile <path_to_BIG_desa_geojson> \
  --output meta_data/id_geodata.csv
```

Indonesia extraction reads or creates `geoname_data/ID_wikidata_cache.json` for Admin1 (province)
and Admin2 (regency/city) Traditional Chinese translations; Admin3 (district) and Admin4 (village)
retain BIG official Indonesian names. For detailed download procedures and fixed parameters, see
[Indonesia handler research document](docs/research/indonesia-handler.md).

After extraction is complete, the data will be automatically integrated when executing the Rust
`release` command.

### 3. Complete Data Processing Workflow

After completing data extraction (or using existing data), you can execute the complete data processing workflow to generate releases.

#### Register LocationIQ API

Register an account at [LocationIQ](https://locationiq.com/) and obtain an API Key.

#### Execute Data Processing

```bash
cargo run --release -- release \
  --locationiq-api-key "YOUR_API_KEY" \
  --country-code "US"
```

> [!NOTE]
> - You can view more options through `cargo run -- help`.
> - The `--country-code` parameter can specify country codes to process, multiple codes separated by spaces.
> - Taiwan, Japan, South Korea, Thailand, and Indonesia (TW/JP/KR/TH/ID) are now generated by official-geodata handlers and must not be processed via LocationIQ; this flow is only for generating metadata for other countries.

> [!WARNING]
> - Since LocationIQ API has request limits (can be checked in the backend after login), please pay attention to the number of place names in the countries to be processed to avoid exceeding limits.
> - This project allows LocationIQ reverse geocoding query progress recovery. If daily request limits are exceeded, you can continue execution after changing API keys or the next day.
>   - Add `--pass-cleanup` to skip resetting the output folder: `cargo run --release -- release --locationiq-api-key "YOUR_API_KEY" --country-code "US" --pass-cleanup`.

### 4. Rust Verification

The Rust CLI provides a dry-run contract that validates the release orchestration without
calling external APIs or downloading data:

```bash
cargo run -- release \
  --dry-run \
  --locationiq-api-key "fixture" \
  --country-code "KR" "TH" \
  --batch-size 100 \
  --locationiq-qps 2
```

To verify the release archive and the directory layout required by `update_data.sh`, use
fixture mode to produce a local smoke artifact:

```bash
cargo run -- release \
  --fixture-mode \
  --pass-locationiq \
  --output-folder /tmp/rust-release-smoke
```

The release and nightly production workflows now run the Rust production path by default. The workflows keep a Rust fixture release smoke before the live build to validate archive layout and CLI contract without consuming LocationIQ quota.

## Acknowledgments  
  
This project is modified based on [immich-geodata-cn](https://github.com/ZingLix/immich-geodata-cn), special thanks to the original author [ZingLix](https://github.com/ZingLix) for their contribution.  
  
## License  
  
This project is licensed under GPL. 
