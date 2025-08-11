# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## About This Project

This project provides Taiwan-localized optimization for Immich's reverse geocoding functionality, improving geographical information accuracy and user experience through Chinese localization, administrative division optimization, and enhanced Taiwan data accuracy using official NLSC (National Land Surveying and Mapping Center) data.

## Version Types

- **Stable Releases** (v1.x.x): Manually released versions with comprehensive testing
- **Nightly Releases**: Automated builds with the latest geodata updates, tagged as `nightly`
- **Pre-releases**: Historical development snapshots (release-YYYY-MM-DD format)

For installation instructions and usage, see the [README](README.md).

## Release Links

- [Latest Release](https://github.com/RxChi1d/immich-geodata-zh-tw/releases/latest)
- [All Releases](https://github.com/RxChi1d/immich-geodata-zh-tw/releases)
- [Compare Versions](https://github.com/RxChi1d/immich-geodata-zh-tw/compare)

---

## [Unreleased]

## [1.1.4] - 2025-08-11

### Added
- **AI Collaboration Documentation**: Comprehensive CLAUDE.md file with project guidelines, coding standards, and AI collaboration instructions for improved development workflow
- **Enhanced Development Guidelines**: Complete coding conventions, commit standards, and language usage rules for better code quality

### Changed
- **Project Dependencies**: Updated core dependencies (polars 1.32.2, regex 2025.7.34, requests 2.32.4) for improved performance and security
- **Development Environment**: Enhanced development dependencies (geopandas 1.1.1, ruff 0.12.8, scipy 1.16.1) for better code quality and analysis tools
- **Project Maintenance**: Improved .gitignore configuration to exclude temporary files and development artifacts

## [1.1.3] - 2025-07-19

### Changed
- **Enhanced Data Tracking**: Improved metadata CSV file tracking capabilities for better data management and monitoring

### Fixed
- Resolved issues with CSV file handling in automated data processing workflows

## [1.1.2] - 2025-06-10

### Added
- **English Documentation**: Complete English README for international users and contributors
- **Bilingual Support**: Full documentation available in both Traditional Chinese and English

### Changed
- **Documentation Structure**: Improved organization and clarity of installation and usage instructions
- **User Experience**: Enhanced accessibility for non-Chinese speaking users

## [1.1.1] - 2025-05-30

### Fixed
- **Release Automation**: Resolved date ordering issues in nightly release system
- **CI/CD Pipeline**: Improved reliability of automated release recreation process

## [1.1.0] - 2025-04-12

### Added
- **NLSC Integration**: Official Taiwan geodata processing using National Land Surveying and Mapping Center (NLSC) Shapefile data
- **Enhanced Taiwan Accuracy**: Authoritative boundary and administrative data for Taiwan region

### Changed
- **Documentation Updates**: Synchronized dependency versions and improved project documentation
- **Geodata Quality**: Significantly improved accuracy for Taiwan geographical information

## [1.0.0] - 2025-04-09

### Added
- **Core Taiwan Localization**: Complete reverse geocoding optimization for Taiwan region
- **Chinese Translation**: Traditional Chinese names for domestic and international locations
- **Administrative Optimization**: Fixed Taiwan municipalities and counties display issues
- **Automated Updates**: Streamlined release system with automated data refresh
- **Docker Integration**: Containerized deployment with integrated and manual options

### Changed
- **Release System**: Refactored and streamlined release automation processes
- **Script Enhancements**: Improved update scripts with tag validation and error handling

## Pre-Release Versions

### [release-2025-04-05] - 2025-04-05

### Added
- **Thailand Support**: Geodata processing for Thailand (TH) region
- **International Expansion**: Extended localization capabilities beyond Taiwan

### [release-2025-02-06] - 2025-02-06

### Changed
- **Translation Improvements**: Enhanced translation processing and accuracy

### [release-2025-02-05] - 2025-02-05  

### Added
- **Korean Metadata**: Support for Korean region geodata processing

### Fixed
- **Translation Processing**: Resolved translation script issues and improved reliability

## Nightly Releases

The `nightly` tag provides continuously updated builds with the latest geodata. These automated releases include:

- **Automated Data Updates**: Fresh reverse geocoding data pulled regularly
- **Latest Improvements**: Most recent bug fixes and enhancements
- **Development Features**: Early access to new functionality before stable releases

**Note**: Nightly releases are recommended for users who want the most up-to-date geodata but may include experimental features. For production use, stable releases (v1.x.x) are recommended.

## Historical Development

### Early Development (2025-01-01 to 2025-03-31)

- **Project Initialization**: Initial commit and project structure setup
- **Core Development**: Implementation of Taiwan localization algorithms
- **CI/CD Setup**: Automated release and data update workflows
- **Documentation**: Initial README and usage instructions
- **Testing**: Quality assurance and feature validation

---

For more information about specific changes, see the [commit history](https://github.com/RxChi1d/immich-geodata-zh-tw/commits/main) or [releases page](https://github.com/RxChi1d/immich-geodata-zh-tw/releases).

[Unreleased]: https://github.com/RxChi1d/immich-geodata-zh-tw/compare/v1.1.4...HEAD
[1.1.4]: https://github.com/RxChi1d/immich-geodata-zh-tw/compare/v1.1.3...v1.1.4
[1.1.3]: https://github.com/RxChi1d/immich-geodata-zh-tw/compare/v1.1.2...v1.1.3
[1.1.2]: https://github.com/RxChi1d/immich-geodata-zh-tw/compare/v1.1.1...v1.1.2
[1.1.1]: https://github.com/RxChi1d/immich-geodata-zh-tw/compare/v1.1.0...v1.1.1
[1.1.0]: https://github.com/RxChi1d/immich-geodata-zh-tw/compare/v1.0.0...v1.1.0
[1.0.0]: https://github.com/RxChi1d/immich-geodata-zh-tw/compare/cb70535...v1.0.0
[release-2025-04-05]: https://github.com/RxChi1d/immich-geodata-zh-tw/releases/tag/release-2025-04-05
[release-2025-02-06]: https://github.com/RxChi1d/immich-geodata-zh-tw/releases/tag/release-2025-02-06
[release-2025-02-05]: https://github.com/RxChi1d/immich-geodata-zh-tw/releases/tag/release-2025-02-05