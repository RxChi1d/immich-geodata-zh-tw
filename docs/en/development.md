# Local Data Processing (Developers)

This document explains how to reproduce the project's data processing flow locally: extracting administrative division data from official boundary data, building a release, and validating the flow without calling external APIs. Regular users who only want to install the data do not need any of these steps.

## 1. Install Dependencies

Local data processing uses the Rust CLI. Install the Rust toolchain first, and make sure `pkg-config` and the PROJ development library are available on your system (on Ubuntu, `libproj-dev`).

### Official Prebuilt Binary

GitHub Releases currently ship a prebuilt binary for Linux x86_64 only, mainly for GitHub Actions, Linux servers, and Immich container environments. On macOS and Windows, build locally instead.

### Local Build

```bash
cargo build --release
```

After the build, the binary lives at `target/release/immich-geodata`. You can also run it directly with `cargo run`:

```bash
cargo run --release -- help
```

## 2. Extract Raw Geographic Data

The `extract` command reads a Shapefile or GeoJSON and produces a normalized CSV. This step is optional; run it only when you need to update a data source or add a new country.

In the commands below, replace `<version>` with the version in the filename you actually downloaded. The source versions used by the current published data are recorded in [NOTICE.md](../../NOTICE.md).

### Taiwan

Data source: [National Land Surveying and Mapping Center (NLSC)](https://whgis-nlsc.moi.gov.tw/Opendata/Files.aspx)

```bash
# 1. Download and unpack the village boundary dataset (TWD97 latitude/longitude)
# 2. Run the extract command
cargo run --release -- extract --country TW \
  --shapefile geoname_data/VILLAGE_NLSC_<version>/VILLAGE_NLSC_<version>.shp \
  --output meta_data/tw_geodata.csv
```

### Japan

Data source: [国土数値情報](https://nlftp.mlit.go.jp/ksj/gml/datalist/KsjTmplt-N03-2025.html)

```bash
# 1. Download and unpack the administrative area dataset (world geodetic system)
# 2. Run the extract command
cargo run --release -- extract --country JP \
  --shapefile geoname_data/N03-<version>_GML/N03-<version>.shp \
  --output meta_data/jp_geodata.csv
```

### South Korea

Data source: [admdongkor](https://github.com/vuski/admdongkor)

```bash
# 1. Download and unpack the official administrative boundary data from the admdongkor project
# 2. Run the extract command
cargo run --release -- extract --country KR \
  --shapefile geoname_data/HangJeongDong_ver<version>.geojson \
  --output meta_data/kr_geodata.csv
```

### Thailand

Data source: [Thailand COD-AB](https://data.humdata.org/dataset/cod-ab-tha)

```bash
# 1. Download and unpack tha_admin_boundaries.shp.zip
# 2. Use tha_admin3.shp to extract the Admin 3 / Tambon boundary data
cargo run --release -- extract --country TH \
  --shapefile geoname_data/tha_admin_boundaries/tha_admin3.shp \
  --output meta_data/th_geodata.csv
```

Thai extraction reads or creates `geoname_data/TH_wikidata_cache.json`, which holds Traditional Chinese translations for Admin1 and Admin2; Admin3 keeps the official COD-AB English names.

### Indonesia

Data source: the official ArcGIS REST FeatureServer of BIG (Badan Informasi Geospasial)

```bash
# 1. Download the desa-level boundary data page by page from the official BIG REST service
#    (geometryPrecision=6, version TASWIL20230928).
#    For the download procedure and fixed parameters, see docs/research/indonesia-handler.md
# 2. Run the extract command
cargo run --release -- extract --country ID \
  --shapefile <path_to_BIG_desa_geojson> \
  --output meta_data/id_geodata.csv
```

Indonesian extraction reads or creates `geoname_data/ID_wikidata_cache.json`, which holds Traditional Chinese translations for Admin1 (provinces) and Admin2 (regencies and cities); Admin3 (districts) and Admin4 (villages) keep the official BIG Indonesian names. For the full download procedure and fixed parameters, see the [Indonesia handler research document (Chinese)](../research/indonesia-handler.md).

Once extraction finishes, `release` integrates the resulting data automatically.

## 3. Full Data Processing Flow

### Register a LocationIQ API Key

Sign up at [LocationIQ](https://locationiq.com/) and obtain an API key.

### Run the Data Processing

```bash
cargo run --release -- release \
  --locationiq-api-key "YOUR_API_KEY" \
  --country-code "US"
```

> [!NOTE]
> - `cargo run -- help` lists only the basic usage; for the full set of options, see `parse_production_options` in `src/cli.rs`.
> - `--country-code` accepts multiple country codes separated by spaces.
> - Taiwan, Japan, South Korea, Thailand, and Indonesia (TW/JP/KR/TH/ID) are produced by official boundary data handlers and must not be processed through LocationIQ; this flow only generates metadata for other countries.

> [!WARNING]
> The LocationIQ API enforces a request quota (check it in the dashboard after logging in), so watch the number of place names in the countries you plan to process.
>
> Lookup progress is recorded in `meta_data/<country_code>.csv`. When you hit the daily limit, switch to another API key or rerun the same command the next day; coordinates already looked up are skipped automatically. Add `--pass-cleanup` to keep the existing intermediate files in `output/` and skip re-downloading and re-preprocessing them:
>
> ```bash
> cargo run --release -- release --locationiq-api-key "YOUR_API_KEY" --country-code "US" --pass-cleanup
> ```
>
> The API key can also be supplied through the `LOCATIONIQ_API_KEY` environment variable.

## 4. Validation

The Rust CLI provides a dry-run contract that validates release orchestration without calling external APIs or downloading data from the network:

```bash
cargo run -- release \
  --dry-run \
  --locationiq-api-key "fixture" \
  --country-code "KR" "TH" \
  --batch-size 100 \
  --locationiq-qps 2
```

To validate the release archive and the directory layout that `update_data.sh` expects, use fixture mode to produce a local smoke artifact:

```bash
cargo run -- release \
  --fixture-mode \
  --pass-locationiq \
  --output-folder /tmp/rust-release-smoke
```

Both the release and the nightly production workflow run the Rust production path, keeping the fixture release smoke as a preflight check. Automated tests for real GeoNames / Natural Earth downloads and the LocationIQ quota path still rely on fixtures, stubs, or an explicit dry-run gate.

## Code Checks

```bash
cargo fmt --check
cargo clippy -- -D warnings
cargo test
```
