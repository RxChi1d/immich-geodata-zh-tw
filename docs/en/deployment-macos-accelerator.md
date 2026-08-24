# Notes for the macOS Accelerator Environment

[epheterson/immich-apple-silicon](https://github.com/epheterson/immich-apple-silicon) (immich-accelerator) runs Immich's microservices worker and machine learning service as native processes on Apple Silicon, while the database and API stay in Docker.

The installation steps are the same as in any other non-container environment, so follow [Non-container Deployment](../../README.en.md#non-container-deployment) in the README. This document covers only what is specific to this environment.

## Which Machine Gets the Data

Immich imports the reverse geocoding data only when the microservices worker starts, so install the data on **the machine that runs the microservices worker**.

| Accelerator mode | Install location |
| :--- | :--- |
| `--ml-only` (Mac runs machine learning only) | The Docker host, following [Integrated Deployment](../../README.en.md#integrated-deployment) |
| Split deployment (Mac runs the worker and machine learning) | **The Mac**, following [Non-container Deployment](../../README.en.md#non-container-deployment) in the README |

> [!NOTE]
> If the Docker side still runs a microservices worker, both sides try to import the geographic data and overwrite each other whenever their versions differ. For a split deployment, set `IMMICH_WORKERS_INCLUDE=api` on the Docker side. That side then never reads the geographic data, so you can also drop the update command from its `entrypoint` to avoid a redundant download on every start.

## Reading the Path Output

In this environment, `--print-paths` produces output like this:

```text
geodata: /build/geodata
i18n-iso-countries: /Users/you/.immich-accelerator/server/3.1.0/node_modules/i18n-iso-countries
```

- `geodata`: The accelerator creates a synthetic link for `/build`, so the path matches the container and needs no extra configuration.
- `i18n-iso-countries`: This lives under a server directory named after the Immich version. Check that the version in the output matches the Immich version you are running.

## Restarting the Service

When Homebrew services manages the accelerator, use:

```bash
brew services restart immich-accelerator
```

> [!IMPORTANT]
> Do not use `immich-accelerator stop && immich-accelerator start` instead. `stop` makes launchd restart the service immediately because of `KeepAlive`, and the following `start` then fails because the port is already in use.

Use `immich-accelerator stop && immich-accelerator start` only when Homebrew services does not manage the accelerator.

## Updating the Data

Unlike the container, the accelerator keeps the data persistently and does not reinstall it on every start.

| Action | Reinstall required |
| :--- | :--- |
| `immich-accelerator stop` / `start`, or a reboot | No |
| `immich-accelerator update` (switching the Immich version) | **Yes** |
| A new data release from this project | **Yes** |

When you switch the Immich version, the accelerator rebuilds `build-data` (which removes `geodata`) and switches to a server directory named after the new version (which invalidates the locale files), so both sets of data revert to their upstream state.

The update steps match the installation steps: run the installer again, restart the worker, and run Extract Metadata on your photos again. You can combine these steps into a single script and run it whenever an update is needed:

```bash
#!/bin/bash
# ~/.local/bin/immich-geodata-update
set -e
bash <(curl -sSL https://github.com/RxChi1d/immich-geodata-zh-tw/releases/latest/download/update_data.sh) --install
brew services restart immich-accelerator
```
