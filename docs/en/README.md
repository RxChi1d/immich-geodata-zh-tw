# Documentation Index

[繁體中文](../README.md) | English

The documentation is grouped by purpose. For installation steps and common questions, start with the [project README](../../README.en.md).

## Installation and Update Reference

| Document | Contents |
| :--- | :--- |
| [Notes for the macOS Accelerator Setup](deployment-macos-accelerator.md) | Install locations, path interpretation, restart procedures, and update timing specific to immich-accelerator (LXC and bare metal follow the non-container deployment in the README) |
| [update_data.sh Usage](update-script.md) | Script options, environment variables, pinning a version, offline installation |
| [Install Path Detection](install-path-detection.md) | How the script chooses an install location and how to verify the result (for maintainers) |

## Regional Processing

| Region | Document |
| :--- | :--- |
| 🇹🇼 Taiwan | [Taiwan Administrative Division Processing](taiwan-admin-processing.md) |
| 🇯🇵 Japan | [Japan Administrative Division Processing](japan-admin-processing.md) |
| 🇰🇷 South Korea | [South Korea Administrative Division Processing](south-korea-admin-processing.md) |
| 🇹🇭 Thailand | [Thailand Administrative Division Processing](thailand-admin-processing.md) |
| 🇮🇩 Indonesia | [Indonesia Administrative Division Processing](indonesia-admin-processing.md) |
| 🌏 Other regions | [Global Translation Processing](global-translation-processing.md) |

## Development

| Document | Contents |
| :--- | :--- |
| [Contributing Guide](contributing.md) | Development workflow, commit conventions, testing requirements |
| [Local Data Processing](development.md) | Reproduce the data pipeline locally: extract boundary data, build a release, verify |

## Research and History

These documents record decisions as they were made and explain why things were done a certain way. The implementation may have changed since, and they are not kept in sync with the code. Treat the regional documents above as authoritative.

| Document | Contents |
| :--- | :--- |
| [Chinese Translation Source Evaluation (Chinese)](../research/chinese-translation-sources.md) | Comparison of translation sources for global place names and the rationale for the choice |
| [Thailand handler Design (Chinese)](../research/thailand-handler.md) | Source selection and design evaluation for Thailand support |
| [Indonesia handler Design (Chinese)](../research/indonesia-handler.md) | Source selection, administrative levels, and translation strategy for Indonesia support |
| [Indonesia Projection and Coordinate Experiment (Chinese)](../research/idn-handler-projection-coordinate-experiment.md) | Experimental data on projection methods and representative point strategies |
| [Python to Rust Migration (Chinese)](../history/python-to-rust-migration.md) | Record of the data pipeline migration |

## Languages

The documentation is maintained in both Traditional Chinese and English. The Traditional Chinese index is at [docs/README.md](../README.md). Research and history documents are available in Traditional Chinese only. Where the two versions differ, the Traditional Chinese version is authoritative.
