# Immich Reverse Geocoding - Taiwan Localization

**Show photo locations in Immich the way people in Taiwan expect to read them.**

[![Latest release](https://img.shields.io/github/v/release/RxChi1d/immich-geodata-zh-tw?label=Latest%20release)](https://github.com/RxChi1d/immich-geodata-zh-tw/releases/latest)
[![Downloads](https://img.shields.io/github/downloads/RxChi1d/immich-geodata-zh-tw/release.tar.gz?label=Downloads)](https://github.com/RxChi1d/immich-geodata-zh-tw/releases)
[![Supported versions](https://img.shields.io/badge/Immich-v2%20%7C%20v3-4250af)](https://immich.app/)
[![License](https://img.shields.io/badge/License-GPL--3.0-blue)](LICENSE)

[繁體中文](README.md) | [English](README.en.md)

[Immich](https://immich.app/) tags each photo with a location derived from its GPS coordinates, but its default geographic data leaves several gaps for users in Taiwan: place names are mostly English or local romanization, so searching in Chinese gets you nowhere; most Taiwanese city and county names are missing altogether, and administrative divisions are spelled differently from everyday usage; place-name points are sparse, so photos frequently land in a neighboring — sometimes entirely adjacent — administrative division.

This project rebuilds the geographic data Immich uses. Taiwan, Japan, South Korea, Thailand, and Indonesia are rebuilt from each country's public administrative boundary data; everywhere else gets the Chinese place names used in Taiwan. On Docker Compose, installation takes one added line in `docker-compose.yml`.

![Before and after comparison](./image/example.png)

## Table of contents

- [Features](#features)
- [Supported regions and language strategy](#supported-regions-and-language-strategy)
- [Installation](#installation)
- [Updating the data](#updating-the-data)
- [FAQ](#faq)
- [Data sources](#data-sources)
- [Further reading](#further-reading)
- [Issues and contributing](#issues-and-contributing)
- [Acknowledgments](#acknowledgments)
- [License](#license)

## Features

- **Taiwan uses official boundary data**: Place names are rebuilt from the village boundary data of the National Land Surveying and Mapping Center (NLSC). Cities, counties, townships, districts, and villages all follow the official records, and every place name gets a recalculated representative point.
- **Several countries use local boundary data**: Japan, South Korea, Thailand, and Indonesia are wired to each country's public administrative boundary data, with the same rebuilt names and representative points, so fewer photos land in a neighboring administrative division.
- **Taiwan-style translations everywhere else**: The NAER *Translations of Foreign Place Names* supplies the Chinese names used in Taiwan for the places it covers. Where no official translation exists, GeoNames Chinese data is used; where neither exists, the original name stays, so obscure places may still appear in English.
- **Updating means restarting**: With the automatic installation method (see [Installation](#installation)), the container fetches the latest data on every start — no manual downloads, no moving files around.
- **Installation is reversible**: Run in install mode, the script backs up the existing data before overwriting it and restores the pre-install state if anything fails midway. Downloading and copying files by hand gives you no such protection.
- **Works across deployments**: Docker Compose, the macOS native worker, LXC, and bare metal are all supported.

## Supported regions and language strategy

Each region uses whichever language strategy reads best for users in Taiwan:

| Region | Display language | Boundary data source | Details |
| :--- | :--- | :--- | :--- |
| 🇹🇼 Taiwan | Official Traditional Chinese names | National Land Surveying and Mapping Center (NLSC), Ministry of the Interior | [Details](docs/en/taiwan-admin-processing.md) |
| 🇯🇵 Japan | Native Japanese (kanji and kana) | 国土数値情報 (Ministry of Land, Infrastructure, Transport and Tourism, Japan) | [Details](docs/en/japan-admin-processing.md) |
| 🇰🇷 South Korea | Official Korean hanja for cities and counties, Traditional Chinese for provinces | admdongkor (open-source project maintaining South Korean administrative dong boundaries) | [Details](docs/en/south-korea-admin-processing.md) |
| 🇹🇭 Thailand | Traditional Chinese translation (official English, then Thai, as fallback) | COD-AB Thailand administrative boundaries (UN OCHA) | [Details](docs/en/thailand-admin-processing.md) |
| 🇮🇩 Indonesia | Traditional Chinese translation (official Indonesian as fallback) | Geospatial Information Agency of Indonesia (BIG) | [Details](docs/en/indonesia-admin-processing.md) |
| 🌏 Other regions | Traditional Chinese translation (original name kept when none exists) | NAER *Translations of Foreign Place Names*, GeoNames | [Details](docs/en/global-translation-processing.md) |

Japan keeps the native Japanese names straight from the official boundary data, with no Chinese conversion. Japanese kanji place names usually differ from the Chinese spelling only in glyph shape — 「横浜市」 versus 「橫濱市」, for example.

South Korean city and county names use the official Korean hanja (「淸州市」, for example) for the same reason: Korean division names are Chinese-character words to begin with, so the characters are the original name rather than a translation.

## Installation

This project supports Immich v2 and v3. Check how your Immich is deployed, then pick the matching method:

| Deployment | Installation method |
| :--- | :--- |
| Docker Compose (including Portainer, UnRAID, and similar) | [Integrated deployment](#integrated-deployment) or [manual deployment](#manual-deployment) |
| macOS native worker (immich-accelerator) | [Non-container deployment](#non-container-deployment) |
| LXC, bare metal, and other self-managed installs | [Non-container deployment](#non-container-deployment) |

### Integrated deployment

The container downloads and installs the latest data on every start, so later updates only take a container restart. This suits most users.

1. **Edit `docker-compose.yml`**

   Add an `entrypoint` to the `immich_server` service:

   ```yaml
   services:
     immich-server:
       container_name: immich_server

       # Other settings omitted

       entrypoint: [ "tini", "--", "/bin/bash", "-c", "bash <(curl -sSL https://github.com/RxChi1d/immich-geodata-zh-tw/releases/latest/download/update_data.sh) --install && exec start.sh" ]
   ```

   Add the `entrypoint` line to your **existing** `immich-server` service. Do not create a second service.

   On start, the container first runs this project's `update_data.sh` to download and install the Taiwan localization data, then runs Immich's `start.sh` to bring up the service.

   > [!IMPORTANT]
   > The command must end with `exec start.sh`. Writing `exec /bin/bash start.sh` prevents Immich v1.142.0 and later from resolving its own path, which sends the container into a restart loop.

2. **Restart Immich**

   ```bash
   docker compose down && docker compose up -d
   ```

   After startup, check the logs for a message such as `10000 geodata records imported` to confirm the data was imported.

3. **Re-run metadata extraction**

   Sign in to Immich, go to **Administration > Jobs**, and click **Extract Metadata > All**. Existing photos then pick up the new geographic information. Photos uploaded afterwards need no further action.

### Manual deployment

Download the data yourself and mount it into the container. This suits environments that need a pinned data version or cannot reach the network at startup.

1. **Edit `docker-compose.yml`**

   Add the following mappings under `volumes`, adjusting the paths to your environment:

   ```yaml
   volumes:
     - /mnt/user/appdata/immich/geodata:/build/geodata:ro
     - /mnt/user/appdata/immich/i18n-iso-countries/langs:/usr/src/app/server/node_modules/i18n-iso-countries/langs:ro
   ```

   > [!NOTE]
   > On Immich older than 1.136.0, change the second line to `/mnt/user/appdata/immich/i18n-iso-countries/langs:/usr/src/app/node_modules/i18n-iso-countries/langs:ro`.

2. **Download the data**

   Get the update script:

   ```bash
   curl -sSL https://github.com/RxChi1d/immich-geodata-zh-tw/releases/latest/download/update_data.sh -o update_data.sh
   ```

   Then edit `DOWNLOAD_DIR` near the top of the script (around line 25) and set it to the **common parent directory** of both mount paths — `/mnt/user/appdata/immich` in the example above. Run:

   ```bash
   bash update_data.sh
   ```

   You should end up with this layout:

   ```text
   /mnt/user/appdata/immich/geodata/
   /mnt/user/appdata/immich/i18n-iso-countries/langs/
   ```

   You can also download `release.tar.gz` or `release.zip` straight from the [Releases page](https://github.com/RxChi1d/immich-geodata-zh-tw/releases), extract it, and place the `geodata` and `i18n-iso-countries` folders in the same locations.

   > [!NOTE]
   > UnRAID users can run the script through the User Scripts plugin.

3. **Restart Immich and re-run metadata extraction.** Steps 2 and 3 of [integrated deployment](#integrated-deployment) apply unchanged.

For pinning a data version, offline installation, and other options, see the [update_data.sh guide](docs/en/update-script.md).

### Non-container deployment

Use this when Immich does not run in a Docker container: the macOS native worker, LXC, or bare metal.

Run the commands on **the machine that runs the Immich microservices worker**, because the geographic data is imported only when that service starts. On LXC and bare metal, that is the Immich host itself. If the macOS accelerator runs with `--ml-only` (the Mac handles machine learning only), the worker still lives on the Docker side — use [integrated deployment](#integrated-deployment) instead.

1. **Confirm the install locations**

   Have the script print where it intends to install. This flag writes nothing:

   ```bash
   bash <(curl -sSL https://github.com/RxChi1d/immich-geodata-zh-tw/releases/latest/download/update_data.sh) --print-paths
   ```

   The output looks like this:

   ```text
   geodata: /build/geodata
   i18n-iso-countries: /Users/you/.immich-accelerator/server/3.1.0/node_modules/i18n-iso-countries
   ```

   The two paths come from different places, so check them separately:

   | Path | Where it comes from | If it is wrong |
   | :--- | :--- | :--- |
   | `i18n-iso-countries` | Found by scanning the system; it should sit under the Immich install directory. When the path contains a version number (macOS accelerator), that version must match the Immich you are running | Set `IMMICH_SERVER_ROOT` to the Immich server root (the directory containing `node_modules/`) |
   | `geodata` | Follows Immich's own `IMMICH_BUILD_DATA` setting, which defaults to `/build` | If Immich overrides that variable — common on LXC and bare metal — pass it here too |

   ```bash
   IMMICH_SERVER_ROOT=/path/to/immich IMMICH_BUILD_DATA=/var/lib/immich \
     bash <(curl -sSL https://github.com/RxChi1d/immich-geodata-zh-tw/releases/latest/download/update_data.sh) --print-paths
   ```

   > [!IMPORTANT]
   > The script does not verify that these two paths are the ones Immich actually reads. Installing into the wrong directory still reports success — Immich simply never sees the data. Confirm the paths carefully at this step.

2. **Install the data**

   Reuse the **exact command** you just confirmed, replacing `--print-paths` with `--install`. If you passed environment variables above, pass them here as well:

   ```bash
   bash <(curl -sSL https://github.com/RxChi1d/immich-geodata-zh-tw/releases/latest/download/update_data.sh) --install
   ```

   A `驗證通過` (verification passed) message means the install succeeded. If anything fails midway, the script restores the pre-install state automatically.

   > [!NOTE]
   > On LXC and bare metal the Immich directories usually belong to root, so you need `sudo`. Because `sudo` drops environment variables, download the script first and put the variables after `sudo`:
   >
   > ```bash
   > curl -sSL https://github.com/RxChi1d/immich-geodata-zh-tw/releases/latest/download/update_data.sh -o update_data.sh
   > sudo IMMICH_SERVER_ROOT=/path/to/immich bash update_data.sh --install
   > ```

3. **Restart Immich and re-run metadata extraction**

   Restart the service that runs the microservices worker so Immich re-imports the geographic data:

   - macOS accelerator: `brew services restart immich-accelerator`. Do not use `stop` followed by `start` — see [notes on the accelerator environment](docs/en/deployment-macos-accelerator.md) for why.
   - LXC and bare metal: restart the Immich systemd service (the service name depends on how it was installed).

   Then re-run metadata extraction as in step 3 of [integrated deployment](#integrated-deployment).

LXC and bare metal need nothing beyond this section. For the macOS accelerator — where the data is imported, how to handle both sides importing at once, and when a reinstall is required — see [notes on the accelerator environment](docs/en/deployment-macos-accelerator.md).

## Updating the data

New geographic data ships regularly. How you update depends on your deployment:

| Deployment | How to update |
| :--- | :--- |
| [Integrated deployment](#integrated-deployment) | Restart the Immich container; the data updates itself |
| [Manual deployment](#manual-deployment) | Run `bash update_data.sh` again (keeping the `DOWNLOAD_DIR` you set at install time), then restart the container |
| [Non-container deployment](#non-container-deployment) | Run the same `--install` command again, then restart the Immich service. The macOS accelerator needs a reinstall in a few extra situations — see [notes on the accelerator environment](docs/en/deployment-macos-accelerator.md#updating-the-data) |

Once the data is updated, Immich imports it on its next start, and newly uploaded photos use it right away. Existing photos only update after you re-run **Extract Metadata**.

Routine maintenance updates — a minor boundary adjustment to a single administrative division, say — affect few photos, so there is usually no reason to reprocess your whole library. Three situations do warrant a full re-run:

- A new country or region is supported
- The way the geographic data is processed changes substantially
- The upstream official boundary data changes on a large scale

Changes like these are flagged in that version's [release notes](https://github.com/RxChi1d/immich-geodata-zh-tw/releases).

To pin a specific version, install offline, or adjust the install paths, see the [update_data.sh guide](docs/en/update-script.md).

## FAQ

**I ran Extract Metadata, but the place names are still not in Chinese.**

Immich compares `geodata/geodata-date.txt` against the record in its database and re-imports only when the **contents differ**, so a restart alone never causes a redundant import. First check the startup log for `geodata records imported`:

- **It appears**: the data was imported. Make sure you chose **All** for Extract Metadata, and that the photo itself carries GPS information.
- **It does not appear**: Immich considers the data unchanged. On manual and non-container deployments, change `geodata/geodata-date.txt` to something different from its current value (today's date, for example) and restart. On integrated deployments the data is reinstalled on every start, so a manual edit gets overwritten; an unchanged date means the same data was already imported, so check the point above instead.

**Administrative division names are in Chinese, but country names are still in English.**

This comes from running Immich 1.136.0 or later against data from this project older than v1.2.0. Installing the latest version fixes it. See [issue #8](https://github.com/RxChi1d/immich-geodata-zh-tw/issues/8) for the discussion.

**The container restarts repeatedly and the log says `main.js not found`.**

This happens when the `entrypoint` ends with `exec /bin/bash start.sh`. Use `exec start.sh` instead. From Immich v1.142.0 on, the startup script derives the install location from its own path, and the extra `/bin/bash` layer throws that off. See [issue #13](https://github.com/RxChi1d/immich-geodata-zh-tw/issues/13) for the discussion.

**Some photos show a location that differs from where they were taken.**

Immich matches place names by nearest distance, so coordinates close to an administrative boundary can be attributed to the neighboring division, and small islands or unusual terrain may not map precisely. That is how Immich resolves locations, not a data error.

## Data sources

For every third-party data source used here, along with its license and attribution notice, see [NOTICE.md](NOTICE.md). The source used for each region is listed in [Supported regions and language strategy](#supported-regions-and-language-strategy) above.

## Further reading

- [Illustrated installation guide](https://inktrace.rxchi1d.me/posts/container-platform/immich-geodata-zh-tw/) (Chinese): the complete walkthrough from scratch, with screenshots.
- [Documentation index](docs/en/README.md): regional processing, script reference, and development docs.
- [Install path detection](docs/en/install-path-detection.md) (for maintainers): the rules the script uses to choose an install location, and the reasoning behind them.

## Issues and contributing

Run into a problem, spot a wrong place name, or want another country supported? Open a report or start a discussion in [Issues](https://github.com/RxChi1d/immich-geodata-zh-tw/issues). Including your Immich version, deployment method, and the relevant logs makes it much easier to pin down.

To contribute code, start with [the contributing guide](docs/en/contributing.md). For hands-on instructions on the data processing pipeline, see [local data processing](docs/en/development.md).

Report security issues privately as described in [the security policy](docs/en/security.md) — please do not open a public issue.

## Acknowledgments

This project is based on [immich-geodata-cn](https://github.com/ZingLix/immich-geodata-cn). Thanks to the original author, [ZingLix](https://github.com/ZingLix), for the work it builds on.

## License

The code in this project is licensed under the [GNU General Public License v3.0](LICENSE).

The published geographic data is provided under the licenses of its respective original sources; see [NOTICE.md](NOTICE.md) for details.
