# Immich Reverse Geocoding - Taiwan Localization  

[繁體中文](README.md) | [English](README.en.md)

This project provides Taiwan-localized optimization for Immich's reverse geocoding functionality, aiming to improve geographical information accuracy and user experience. Main features include:  

- **Chinese Localization**: Converting geographical names from home and abroad to Traditional Chinese conforming to Taiwan terminology.  
- **Administrative Division Optimization**: Solving the issue where Taiwan's municipalities and provincial cities/counties only display region names.  
- **Enhanced Taiwan Data Accuracy**: Utilizing official map data from **National Land Surveying and Mapping Center (NLSC)** of Taiwan to process geographical names and boundary data for Taiwan region, ensuring authoritative data sources.  

> [!TIP]
> Compatibility Notice
> 
> - Starting from Immich 1.139.4, container paths have changed.
> - If you're on 1.139.3 or earlier and use Manual Deployment, adjust the `volumes` mapping as described in the [Manual Deployment](#manual-deployment) section.
> - If you use this project's integrated auto-deployment (update_data.sh), no changes are required; the script has been updated to support both old and new versions.

### Before and After Comparison  
![Before and After Comparison](./image/example.png) 

## Table of Contents

- [Immich Reverse Geocoding - Taiwan Localization](#immich-reverse-geocoding---taiwan-localization)
    - [Before and After Comparison](#before-and-after-comparison)
  - [Table of Contents](#table-of-contents)
  - [Data Sources](#data-sources)
  - [Usage](#usage)
    - [Integrated Deployment (Recommended, convenient for future updates)](#integrated-deployment-recommended-convenient-for-future-updates)
    - [Manual Deployment](#manual-deployment)
  - [Specify Specific Version](#specify-specific-version)
  - [Taiwan Localization Logic](#taiwan-localization-logic)
  - [Update Geographic Data](#update-geographic-data)
    - [Integrated Deployment](#integrated-deployment)
    - [Manual Deployment](#manual-deployment-1)
  - [Local Data Processing](#local-data-processing)
  - [Acknowledgments](#acknowledgments)
  - [License](#license)
  
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

      entrypoint: [ "tini", "--", "/bin/bash", "-c", "bash <(curl -sSL https://raw.githubusercontent.com/RxChi1d/immich-geodata-zh-tw/refs/heads/main/update_data.sh) --install && exec /bin/bash start.sh" ]
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
   > For Immich < 1.139.4, please change the second line to:  
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
    entrypoint: [ "tini", "--", "/bin/bash", "-c", "bash <(curl -sSL https://raw.githubusercontent.com/RxChi1d/immich-geodata-zh-tw/refs/heads/main/update_data.sh) --install --tag <tag_name> && exec /bin/bash start.sh" ] 
    ```
    Replace `<tag_name>` with the specific tag name you want to download. If `--tag` is omitted, the latest release (`latest`) is downloaded by default.

2.  **Manual Deployment (`update_data.sh`)**
    Add `--tag <tag_name>` when executing the script:
    ```bash
    bash update_data.sh --tag <tag_name>
    ```
    Replace `<tag_name>` with the specific tag name you want to download. If `--tag` is omitted, the latest release (`latest`) is downloaded by default.

> **NOTE**: The script will first verify whether the specified tag exists in GitHub Releases. If the tag is invalid, it will prompt an error and terminate execution, so please ensure the tag is valid before execution.
  
## Taiwan Localization Logic  
  
This project adopts more precise and locally-tailored localization logic for Taiwan region geographic information processing:  
  
1.  **NLSC data as the core**:  
     *   Taiwan's administrative boundaries and names are mainly based on **village boundary map data published by the National Land Surveying and Mapping Center (NLSC)**. This ensures **accuracy** of geographic information.  
     *   By processing NLSC village data, we can reverse-resolve geographic coordinates accurately to the village level, thus providing more precise township and county/city levels.  
  
2.  **Administrative division level definitions**:  
     *   **Level 1 Administrative Division (Admin1)**: Corresponds to Taiwan's **22 municipalities and provincial cities/counties** (e.g., Taipei City, Keelung City, Changhua County).  
     *   **Level 2 Administrative Division (Admin2)**: Corresponds to **townships, towns, cities, districts** under each county/city (e.g., Banqiao District in New Taipei City, Changhua City in Changhua County).  
     *   **Level 3 Administrative Division (Admin3)**: Corresponds to **villages and wards** in NLSC data.  
     *   **Level 4 Administrative Division (Admin4)**: Currently not used.  
  
3.  **Chinese name processing**:  
     *   Geographic names within Taiwan (counties/cities, townships/towns/cities/districts, villages/wards) **directly adopt official names provided by NLSC map data**.  
     *   Geographic names outside Taiwan mainly refer to the **GeoNames** database, where country name translations adopt official translations provided by **Ministry of Economic Affairs International Trade Administration** and **Ministry of Foreign Affairs of Taiwan**, ensuring Traditional Chinese names that conform to Taiwan terminology habits.
  
Through the above logic, this project aims to provide reverse geocoding results that are more aligned with Taiwan's actual situation and more accurate.

## Update Geographic Data

### Integrated Deployment
  
Simply restart the Immich container to automatically update geographic data.  

### Manual Deployment
  
1. Download the latest release.zip and extract it to the specified location.
   
2. Re-extract photo metadata (same as [Manual Deployment](#manual-deployment)).
  
## Local Data Processing  
  
1. **Install Dependencies**  
   First install uv (if not already installed):
   
   Please refer to the [uv official installation guide](https://docs.astral.sh/uv/getting-started/installation/) to install uv for your operating system.
   
   Then install project dependencies:
   
   ```bash
   uv sync
   ```

2. Register an account at [LocationIQ](https://locationiq.com/) and obtain an API Key.  

3. **Execute `main.py`**  
   ```bash  
   uv run python main.py release --locationiq-api-key "YOUR_API_KEY" --country-code "JP" "KR" "TH"
   ```  
   > **NOTE:**  
   > - You can view more options through `uv run python main.py --help` or `uv run python main.py release --help`.  
   > - The `--country-code` parameter can specify country codes to process, multiple codes separated by spaces. (Currently only tested with JP, KR, TH)  
     
   > **WARNING:**  
   > - Since LocationIQ API has request limits (can be checked in the backend after login), please pay attention to the number of place names in the countries to be processed to avoid exceeding limits.  
   > - This project allows LocationIQ reverse geocoding query progress recovery. If daily request limits are exceeded, you can continue execution after changing API keys or the next day.  
   >   - Need to add `--pass-cleanup` parameter to cancel folder reset function: `uv run python main.py release --locationiq-api-key "YOUR_API_KEY" --country-code "TW" "JP" --pass-cleanup`.  
  
## Acknowledgments  
  
This project is modified based on [immich-geodata-cn](https://github.com/ZingLix/immich-geodata-cn), special thanks to the original author [ZingLix](https://github.com/ZingLix) for their contribution.  
  
## License  
  
This project is licensed under GPL. 
