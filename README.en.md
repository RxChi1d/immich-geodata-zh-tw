# Immich Reverse Geocoding - Taiwan Localization  

[繁體中文](README.md) | [English](README.en.md)

This project delivers reverse geocoding enhancements tailored for users in Taiwan, providing natural and accurate location display that reflects local reading habits.

Currently supports: 🇹🇼 **Taiwan** | 🇯🇵 **Japan** | 🌏 **Traditional Chinese localization for other regions**

## Design Philosophy

We focus on the Taiwan user experience and apply the most suitable language strategy per region:

- **Taiwan**: Uses NLSC datasets to fix country and administrative naming issues
- **Japan**: Uses 国土数値情報 datasets and preserves native names (漢字 + かな)
- **Other regions**: Provides Traditional Chinese translations, falling back to English when no common translation exists

> [!WARNING]
> If integrated deployment continues to use `exec /bin/bash start.sh` as the `entrypoint`, Immich 1.142.0+ will exit on startup with `Error: /usr/src/dist/main.js not found`, leading to a reboot loop.
> Switch to `exec start.sh` instead (the Integrated Deployment section provides updated examples and guidance).

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
  - [Update Geographic Data](#update-geographic-data)
    - [Integrated Deployment](#integrated-deployment)
    - [Manual Deployment](#manual-deployment-1)
  - [Developer: Local Data Processing](#developer-local-data-processing)
    - [1. Install Dependencies](#1-install-dependencies)
    - [2. Extract Raw Geographic Data (Optional)](#2-extract-raw-geographic-data-optional)
      - [Taiwan Data Extraction](#taiwan-data-extraction)
      - [Japan Data Extraction](#japan-data-extraction)
    - [3. Complete Data Processing Workflow](#3-complete-data-processing-workflow)
      - [Register LocationIQ API](#register-locationiq-api)
      - [Execute Data Processing](#execute-data-processing)
  - [Acknowledgments](#acknowledgments)
  - [License](#license)

## Supported Regions and Language Strategy

The project applies region-specific language handling to reflect the expectations of users in Taiwan:

| Region | Language Strategy | Data Source | Notes |
| --- | --- | --- | --- |
| 🇹🇼 Taiwan | Official Traditional Chinese names | NLSC (National Land Surveying and Mapping Center) | Fixes incorrect country labels and missing municipality names |
| 🇯🇵 Japan | Native Japanese (漢字 + かな) | 国土数値情報ダウンロードサービス | Displays official Japanese names without translating them |
| 🌏 Others | Traditional Chinese translations | Custom glossary → GeoNames translations → GeoNames English | Prioritizes Taiwan-style translations; falls back when unavailable |

> **Why keep Japanese in Japanese?**
> Taiwanese users are familiar with Japanese kanji and kana in combination. Names such as 「横浜市」 or 「うるま市」 remain understandable without romanization or Chinese conversion.
  
## Data Sources

The geographic data used in this project mainly comes from the following sources:

1.  **GeoNames** ([geonames.org](https://www.geonames.org/)): As the global geographic location database foundation.
2.  **National Land Surveying and Mapping Center (NLSC)** of Taiwan:
    - Source: [NLSC Open Data Platform](https://whgis-nlsc.moi.gov.tw/Opendata/Files.aspx)
    - Dataset: Village Boundaries (TWD97 Latitude/Longitude), Version 1140620
    - Purpose: As the primary data source for Taiwan region village boundaries and administrative district names, ensuring data accuracy and authority.
3.  **LocationIQ**: Used for processing reverse geocoding requests for non-Taiwan regions, calibrating administrative division levels.
4.  **Ministry of Economic Affairs International Trade Administration & Ministry of Foreign Affairs of Taiwan**: As reference sources for Chinese translations of some countries/regions.

> **NOTE**:  
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

      entrypoint: [ "tini", "--", "/bin/bash", "-c", "bash <(curl -sSL https://raw.githubusercontent.com/RxChi1d/immich-geodata-zh-tw/refs/heads/main/update_data.sh) --install && exec start.sh" ]
   ```  
   > **NOTE**:  
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
     > **NOTE**:  
  > For Immich < 1.136.0, please change the second line to:  
  > `/mnt/user/appdata/immich/i18n-iso-countries/langs:/usr/src/app/node_modules/i18n-iso-countries/langs:ro`
  
2. **Download Taiwan localization data**  
   Two download methods are provided:  
       
   (1) **Automatic download**  
      Refer to the `update_data.sh` script in this project, modify `DOWNLOAD_DIR` to the folder path storing geodata and i18n-iso-countries, and execute the script:  
      ```bash
      bash update_data.sh
      ```  
      > **NOTE**:  
      > - Manual deployment also supports specifying specific version downloads. For details, please refer to the [Specify Specific Version](#specify-specific-version) section.
      > - UnRAID users can execute the script through the User Scripts plugin.
     
   (2) **Manual download**  
      Go to the [Release page](https://github.com/RxChi1d/immich-geodata-zh-tw/releases) to find the required version, download the corresponding `release.tar.gz` or `release.zip`, and extract it to the specified location.
  
3. **Restart Immich and re-extract photo metadata**  
   Same as steps 2 and 3 in [**Integrated Deployment**](#integrated-deployment).

## Specify Specific Version

In some cases (e.g., when the latest release has issues), you may need to download or rollback to a specific release version. This project's update script supports specifying the release tag to download through the `--tag` parameter.

**How to find available Tags?**
Please go to this project's [Releases page](https://github.com/RxChi1d/immich-geodata-zh-tw/releases) to view all available release tag names (e.g., `v1.0.0`, `nightly`, etc.).

**Usage Examples:**

1.  **Integrated Deployment (`entrypoint` in `docker-compose.yml`)**
    Add `--tag <tag_name>` after the entrypoint command:
    ```yaml
    entrypoint: [ "tini", "--", "/bin/bash", "-c", "bash <(curl -sSL https://raw.githubusercontent.com/RxChi1d/immich-geodata-zh-tw/refs/heads/main/update_data.sh) --install --tag <tag_name> && exec start.sh" ] 
    ```
    Replace `<tag_name>` with the specific tag name you want to download. If `--tag` is omitted, the latest release (`latest`) is downloaded by default.

2.  **Manual Deployment (`update_data.sh`)**
    Add `--tag <tag_name>` when executing the script:
    ```bash
    bash update_data.sh --tag <tag_name>
    ```
    Replace `<tag_name>` with the specific tag name you want to download. If `--tag` is omitted, the latest release (`latest`) is downloaded by default.

> **NOTE**: The script will first verify whether the specified tag exists in GitHub Releases. If the tag is invalid, it will prompt an error and terminate execution, so please ensure the tag is valid before execution.
  
## Administrative Optimization Strategy

### 🇹🇼 Taiwan

- **Official datasets as the foundation**: Uses NLSC village boundaries to guarantee authoritative data
- **Correct country and division names**: Fixes Immich defaults such as "China Taiwan Province" and missing municipalities
- **Administrative hierarchy refinement**: Admin1 = municipalities/counties, Admin2 = districts/townships

> 📖 See [Taiwan Administrative Processing (English)](docs/en/taiwan-admin-processing.md)

### 🇯🇵 Japan

- **Preserve native names**: Keeps the original kanji + kana combinations (e.g., 「静岡県」 instead of "Shizuoka Prefecture")
- **Context-aware subdivision handling**: Handles standard cities, special wards, designated cities, and Tokyo’s special wards
- **Intelligent district prefixes**: Adds district names only when multiple towns share the same name within a prefecture

> 📖 See [Japan Administrative Processing (zh-TW)](docs/zh-tw/japan-admin-processing.md) • [Japan Administrative Processing (English)](docs/en/japan-admin-processing.md)

## Update Geographic Data

### Integrated Deployment
  
Simply restart the Immich container to automatically update geographic data.  

### Manual Deployment
  
1. Download the latest release.zip and extract it to the specified location.

2. Re-extract photo metadata (same as [Manual Deployment](#manual-deployment)).

## Developer: Local Data Processing

### 1. Install Dependencies

First install uv (if not already installed):

Please refer to the [uv official installation guide](https://docs.astral.sh/uv/getting-started/installation/) to install uv for your operating system.

Then install project dependencies:

```bash
uv sync
```

### 2. Extract Raw Geographic Data (Optional)

If you need to process new countries or update existing geographic data sources, you can use the `extract` command to extract data from Shapefiles. This step is optional and only needed when updating data sources.

#### Taiwan Data Extraction

Data source: [National Land Surveying and Mapping Center (NLSC)](https://whgis-nlsc.moi.gov.tw/Opendata/Files.aspx)

```bash
# 1. Download "Village Boundaries (TWD97 Latitude/Longitude)" data and extract
# 2. Execute extraction command
uv run python main.py extract --country TW \
  --shapefile geoname_data/VILLAGE_NLSC_1140825/VILLAGE_NLSC_1140825.shp \
  --output meta_data/tw_geodata.csv
```

#### Japan Data Extraction

Data source: [国土数値情報](https://nlftp.mlit.go.jp/ksj/gml/datalist/KsjTmplt-N03-2025.html)

```bash
# 1. Download "行政区域データ（世界測地系）" and extract
# 2. Execute extraction command
uv run python main.py extract --country JP \
  --shapefile geoname_data/N03-20250101_GML/N03-20250101.shp \
  --output meta_data/jp_geodata.csv
```

After extraction is complete, the data will be automatically integrated when executing `main.py release`.

### 3. Complete Data Processing Workflow

After completing data extraction (or using existing data), you can execute the complete data processing workflow to generate releases.

#### Register LocationIQ API

Register an account at [LocationIQ](https://locationiq.com/) and obtain an API Key.

#### Execute Data Processing

```bash
uv run python main.py release --locationiq-api-key "YOUR_API_KEY" --country-code "KR" "TH"
```

> **NOTE:**
> - You can view more options through `uv run python main.py --help` or `uv run python main.py release --help`.
> - The `--country-code` parameter can specify country codes to process, multiple codes separated by spaces. (Currently only tested with "KR" "TH")

> **WARNING:**
> - Since LocationIQ API has request limits (can be checked in the backend after login), please pay attention to the number of place names in the countries to be processed to avoid exceeding limits.
> - This project allows LocationIQ reverse geocoding query progress recovery. If daily request limits are exceeded, you can continue execution after changing API keys or the next day.
>   - Need to add `--pass-cleanup` parameter to cancel folder reset function: `uv run python main.py release --locationiq-api-key "YOUR_API_KEY" --country-code "KR" "TH" --pass-cleanup`.

## Acknowledgments  
  
This project is modified based on [immich-geodata-cn](https://github.com/ZingLix/immich-geodata-cn), special thanks to the original author [ZingLix](https://github.com/ZingLix) for their contribution.  
  
## License  
  
This project is licensed under GPL. 
