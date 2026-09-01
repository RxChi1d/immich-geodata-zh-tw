# update_data.sh Reference

`update_data.sh` downloads the geographic data published by this project and installs it where Immich reads it. Integrated deployment runs the script every time the container starts; other deployment methods run it manually.

Get the script from Releases:

```bash
curl -sSL https://github.com/RxChi1d/immich-geodata-zh-tw/releases/latest/download/update_data.sh -o update_data.sh
```

## Options

| Option | Description |
| :--- | :--- |
| `--install` | Install the data into Immich's system directories. Without it, the script only downloads and extracts into `DOWNLOAD_DIR`. |
| `--tag <tag>` | Download a specific release version. Defaults to the latest release. |
| `--archive <path>` | Install from a local `release.tar.gz` instead of downloading. |
| `--print-paths` | Print the detected install paths and exit, without downloading or installing. |

## Environment Variables

| Variable | Description |
| :--- | :--- |
| `IMMICH_SERVER_ROOT` | Immich server root directory (it must contain `node_modules/`). Once set, it is the only search scope: if it points at the wrong place, the script fails instead of installing somewhere else. |
| `IMMICH_BUILD_DATA` | Immich's own variable. `geodata` is installed into `geodata/` beneath it. Defaults to `/build`. |

For the detection rules and the reasoning behind them, see [Install Path Detection](install-path-detection.md).

## Download Location

Without `--install`, the data is extracted into the directory set by the script's `DOWNLOAD_DIR` variable, which defaults to `./temp` under the current working directory. Extraction produces two folders: `geodata/` and `i18n-iso-countries/`.

For manual deployment, to download straight into your mounted directories, edit this line near the top of `update_data.sh` (around line 25) and point it at the **common parent directory** of the two mount paths:

```bash
DOWNLOAD_DIR="/mnt/user/appdata/immich"
```

For the resulting directory layout, see step 2 of [Manual Deployment in the README](../../README.en.md#manual-deployment).

> [!NOTE]
> This variable can only be changed inside the script. Passing it as an environment variable has no effect.

## Pinning a Version

If the latest release causes problems, or you need to stay on a specific version, use `--tag` to pick the data version to install. Available tag names are listed on the [Releases page](https://github.com/RxChi1d/immich-geodata-zh-tw/releases), for example `v3.2.0` or `nightly`.

`nightly` is the automatically published data version. This project regenerates the data once a week and overwrites the `nightly` tag, using whatever upstream boundary data is available at that time. Its contents have not gone through the full validation of a formal release, and later automatic publishes overwrite the same tag. Use it when you want new boundary data early; for long-term stability, pin a `vX.Y.Z` tag.

The script itself always comes from the latest release; `--tag` only selects the data version. For integrated deployment:

```yaml
entrypoint: [ "tini", "--", "/bin/bash", "-c", "bash <(curl -sSL https://github.com/RxChi1d/immich-geodata-zh-tw/releases/latest/download/update_data.sh) --install --tag <tag_name> && exec start.sh" ]
```

When running the script manually, just add the option:

```bash
bash update_data.sh --install --tag <tag_name>
```

> [!IMPORTANT]
> Do not change the script URL to `releases/download/<tag_name>/update_data.sh`. Automatically published versions such as `nightly` do not include `update_data.sh`, so that URL returns 404 and integrated deployment fails to start.

The script verifies that the tag exists in GitHub Releases first, and stops with an error if it does not.

## Offline Installation

If you already have a `release.tar.gz` — an offline environment, or reinstalling the same data — install it directly with `--archive`:

```bash
bash update_data.sh --install --archive /path/to/release.tar.gz
```

## Install Behavior

- The script backs up the existing `geodata` and `i18n-iso-countries/langs` before installing. If anything fails while overwriting, it restores the pre-install state, so you never end up with half-installed data.
- Language files are replaced one by one. Language files that exist upstream but are not provided by this project are kept.
- After installing, the script verifies that the data really landed where Immich reads it, including `en.json`, which controls how country names are displayed. The script prints its messages in Traditional Chinese: `驗證通過` ("verification passed") means it succeeded.
- The install flow is idempotent. Running it again when the data is already current has no side effects.
