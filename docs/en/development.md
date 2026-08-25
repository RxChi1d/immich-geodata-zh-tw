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

> [!NOTE]
> The NLSC download page triggers the download through an ASP.NET postback, so `curl`
> cannot fetch the file directly; use a browser. The downloaded file is named
> `OFiles_<guid>.zip`, and the `VILLAGE_NLSC_<version>` directory only appears after
> unpacking it. The version is a Republic of China calendar date: `1150624` means
> 2026-06-24.

### Japan

Data source: [国土数値情報](https://nlftp.mlit.go.jp/ksj/gml/datalist/KsjTmplt-N03-2025.html)

```bash
# 1. Download and unpack the administrative area dataset (world geodetic system)
# 2. Run the extract command
cargo run --release -- extract --country JP \
  --shapefile geoname_data/N03-<version>_GML/N03-<version>.shp \
  --output meta_data/jp_geodata.csv
```

> [!NOTE]
> The download page requires clicking through several pages. The file is also available
> at a stable URL, where `<year>` and `<version>` belong to the same release (for
> example `2026` and `20260101`):
>
> ```
> https://nlftp.mlit.go.jp/ksj/gml/data/N03/N03-<year>/N03-<version>_GML.zip
> ```

### South Korea

Data source: [admdongkor](https://github.com/vuski/admdongkor)

```bash
# 1. Download the GeoJSON for that version (a single file, no unpacking needed)
VER=20260701   # set this to the version you want
curl -sSL --create-dirs -o "geoname_data/HangJeongDong_ver${VER}.geojson" \
  "https://raw.githubusercontent.com/vuski/admdongkor/master/ver${VER}/HangJeongDong_ver${VER}.geojson"

# 2. Run the extract command
cargo run --release -- extract --country KR \
  --shapefile "geoname_data/HangJeongDong_ver${VER}.geojson" \
  --output meta_data/kr_geodata.csv
```

> [!NOTE]
> The version number is the date the release takes effect (for example `20260701`). The
> available versions are the `ver*` directories at the root of the admdongkor repository;
> each holds one GeoJSON of the same name.

### Thailand

Data source: [Thailand COD-AB](https://data.humdata.org/dataset/cod-ab-tha)

```bash
# 1. Query the HDX API for the current shapefile download URL
URL=$(curl -sS "https://data.humdata.org/api/3/action/package_show?id=cod-ab-tha" \
  | jq -r '.result.resources[] | select(.name == "tha_admin_boundaries.shp.zip") | .url')

# 2. Download and unpack it (HDX issues a 302 to a presigned URL, so -L is required)
mkdir -p geoname_data
curl -sSL -o geoname_data/tha_admin_boundaries.shp.zip "$URL"
unzip -q -d geoname_data/tha_admin_boundaries geoname_data/tha_admin_boundaries.shp.zip

# 3. Use tha_admin3.shp to extract the Admin 3 / Tambon boundary data
cargo run --release -- extract --country TH \
  --shapefile geoname_data/tha_admin_boundaries/tha_admin3.shp \
  --output meta_data/th_geodata.csv
```

> [!NOTE]
> HDX download URLs embed a resource UUID that changes when the dataset is republished,
> so always resolve the current URL through the API above rather than reusing a fixed link
> from documentation or an existing script. The `last_modified` field in the same API
> response is the release date of that version, which tells you whether upstream has
> published a new one.

Thai extraction reads or creates `geoname_data/TH_wikidata_cache.json`, which holds Traditional Chinese translations for Admin1 and Admin2; Admin3 keeps the official COD-AB English names.

### Indonesia

Data source: [BIG (Badan Informasi Geospasial) geospatial services](https://geoservices.big.go.id/rbi/rest/services/BATASWILAYAH/BATAS_DESAKEL_AR/MapServer/0)

BIG does not offer the desa-level (village) data as a single file. Fetch it in batches through the ArcGIS REST `query` endpoint, then merge the batches.

```bash
L="https://geoservices.big.go.id/rbi/rest/services/BATASWILAYAH/BATAS_DESAKEL_AR/MapServer/0"

# 1. Check the feature count, the OBJECTID upper bound, and the data version
curl -sS -G "$L/query" --data-urlencode "where=1=1" \
  --data-urlencode "returnCountOnly=true" --data-urlencode "f=json"
curl -sS -G "$L/query" --data-urlencode "where=1=1" --data-urlencode "f=json" \
  --data-urlencode 'outStatistics=[{"statisticType":"max","onStatisticField":"OBJECTID","outStatisticFieldName":"m"}]'
curl -sS -G "$L/query" --data-urlencode "where=OBJECTID=1" \
  --data-urlencode "outFields=METADATA" --data-urlencode "returnGeometry=false" \
  --data-urlencode "f=json"

# 2. Download in OBJECTID ranges (adjust the upper bound to the value from step 1)
mkdir -p geoname_data/idn_oid
for ((lo=0; lo<93730; lo+=1000)); do
  hi=$((lo+1000))
  f="geoname_data/idn_oid/oid_$(printf '%06d' $lo).geojson"
  [ -s "$f" ] && head -c 40 "$f" | grep -q '{' && continue
  curl -sS --max-time 300 -G "$L/query" \
    --data-urlencode "where=OBJECTID>$lo AND OBJECTID<=$hi" \
    --data-urlencode "outFields=WADMPR,WADMKK,WADMKC,WADMKD" \
    --data-urlencode "geometryPrecision=6" \
    --data-urlencode "outSR=4326" \
    --data-urlencode "f=geojson" -o "$f"
done

# 3. Merge into a single GeoJSON and check the feature and province counts
python3 - <<'EOF'
import json, glob
feats = []
for f in sorted(glob.glob('geoname_data/idn_oid/*.geojson')):
    feats.extend(json.load(open(f, encoding='utf-8'))['features'])
print('features:', len(feats),
      '| provinces:', len({x['properties']['WADMPR'] for x in feats}))
json.dump({'type': 'FeatureCollection', 'features': feats},
          open('geoname_data/idn_desa_<version>.geojson', 'w', encoding='utf-8'),
          ensure_ascii=False)
EOF

# 4. Run the extract command
cargo run --release -- extract --country ID \
  --shapefile geoname_data/idn_desa_<version>.geojson \
  --output meta_data/id_geodata.csv
```

> [!IMPORTANT]
> When the service fails it returns HTTP 200 with an HTML error page instead of an HTTP
> error code. Verify that every downloaded batch starts with `{` (the loop above already
> does this), and confirm the merged feature count matches the total reported in step 1;
> otherwise data is dropped silently. For ranges that keep failing at 1000 records per
> batch, retry them 250 records at a time.
>
> OBJECTID values are not contiguous, so the upper bound (93730 in this case) being
> larger than the feature count (84503) is expected.

The data version is stored in the `METADATA` attribute of each feature (for example `TASWIL1000020260612DESAKEL_AR`, the 2026-06-12 release), so there is no need to look it up in the service directory.

Indonesian extraction reads or creates `geoname_data/ID_wikidata_cache.json`, which holds Traditional Chinese translations for Admin1 (provinces) and Admin2 (regencies and cities); Admin3 (districts) and Admin4 (villages) keep the official BIG Indonesian names.

> [!IMPORTANT]
> Wikidata translation failures are silent: when a lookup or verification fails, the
> handler quietly falls back to the source name instead of raising an error. After
> regenerating the data, compare the **full list** of untranslated names, not the count.
> For the failure patterns, how far the current safeguards reach, and how to clear the
> caches, see [Known Translation Failures on Wikidata](wikidata-translation.md).

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
