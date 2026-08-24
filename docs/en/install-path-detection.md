# Install Path Detection

> This document records the rules `update_data.sh --install` uses to choose an
> install location, how the result is verified, and how the design evolved. It is
> written for maintenance and troubleshooting. For the actual steps, see the
> [installation section of the README](../../README.en.md#installation).

## Install targets

The script installs two sets of files:

- **geodata**: the place-name data used for reverse geocoding.
- **`langs/` of i18n-iso-countries**: country name translations. Immich resolves
  country names with `getName(countryCode, 'en')`, so the localization rewrites
  `langs/en.json`.

## Detection rules

The script checks candidate roots in order and takes the first one that actually
contains an installed `i18n-iso-countries`:

1. The `IMMICH_SERVER_ROOT` environment variable, which accepts both the app root
   and its `server/` subdirectory.
2. `/usr/src/app/server` and `/usr/src/app`, for the official container.
3. `server_dir` from `~/.immich-accelerator/config.json`, for immich-accelerator.

Additional rules:

- When `IMMICH_SERVER_ROOT` is set, it is the only search scope. If the package
  is not found there, the script exits non-zero instead of falling back to the
  other candidates.
- Candidates are de-duplicated by canonical path, so several sources pointing at
  one directory are not counted as multiple results. De-duplication applies only
  to the comparison; the install uses the original path, because in pnpm layouts
  the canonical path lies inside `.pnpm` and using it as the install location
  would change how later resolution behaves.
- If no candidate matches, the script scans the candidate roots with
  `find -maxdepth 5 -type d -name node_modules -prune` to locate where the
  package actually lives.
- If several distinct targets match, the script emits a warning and uses the
  first. The integrated deployment entrypoint is
  `update_data.sh --install && exec start.sh`, where a non-zero status prevents
  Immich from starting.
- If nothing matches at all, the script exits non-zero and does not create the
  directory. Creating it would write the language files to a location Immich
  never reads, without producing an error.

## geodata path

geodata is installed under `IMMICH_BUILD_DATA` as `geodata/`, which defaults to
`/build/geodata`. Immich defines that variable itself:

```js
const buildFolder = dto.IMMICH_BUILD_DATA || '/build';
geodata: join(buildFolder, 'geodata'),
```

The script reuses the same variable, so a second setting cannot drift from
Immich's. The macOS accelerator creates a synthetic link for `/build`, so the
path is the same in container and native environments.

## Verifying the result

After installing, the script runs these checks:

1. Resolve the package location. When `node` is available, Node's own module
   resolution provides the answer; otherwise the same rule is applied directly,
   walking up from the starting directory and taking the first
   `node_modules/<package>` found.
2. Compare each staged file against the corresponding file in the resolved
   package.

The comparison uses the staged download rather than the install target. In a pnpm
layout the install path and the resolved path are two names for one file, so
comparing them always matches and says nothing about whether the copy happened.

The check covers all of `langs/`, including `en.json`, which determines the
displayed country names.

## Design history

Earlier versions read the version from Immich's `package.json` and inferred the
location of `i18n-iso-countries` from a version boundary:

```
< 1.136.0  ->  /usr/src/app/node_modules/i18n-iso-countries
>= 1.136.0 ->  /usr/src/app/server/node_modules/i18n-iso-countries
```

Reasons for moving to structure detection:

- The boundary was first recorded as 1.139.4 and later corrected to 1.136.0; the
  version comparison itself was also corrected once.
- A version number cannot describe a directory change that has not happened yet,
  so every upstream move required a script change.
- The macOS native worker runs Immich 3.1.0 with a flat layout, which version
  inference maps to the nested path.
- Reading the version requires running `node`, and non-container deployments do
  not guarantee a usable `node`.

## Verified environments

| Environment | Layout | Package manager | Install location |
| :--- | :--- | :--- | :--- |
| immich-server v1.135.3 | flat | npm | `/usr/src/app/node_modules/…` |
| immich-server v1.136.0 | nested | npm | `/usr/src/app/server/node_modules/…` |
| immich-server v3.1.0 | nested | pnpm | `/usr/src/app/server/node_modules/…` |
| macOS native worker 3.1.0 | flat | pnpm | `~/.immich-accelerator/server/3.1.0/node_modules/…` |

The v3.1.0 image was also used with the application moved to `/opt/immich` to
verify three cases: exiting non-zero without `IMMICH_SERVER_ROOT`, installing
successfully once it is set, and installing as a non-root user.

## Regression tests

- `tests/update_data_paths.sh`: detection rules.
- `tests/update_data_install.sh`: install flow, driven by `--archive` with a
  locally built payload.

Neither requires network access.
