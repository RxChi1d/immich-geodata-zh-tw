# Security Policy

[繁體中文](../../SECURITY.md) | English

## Supported Versions

Security fixes are provided for the latest release only.

## Reporting a Vulnerability

**Do not report security issues through public issues.**

Report them through GitHub [Private vulnerability reporting](https://github.com/RxChi1d/immich-geodata-zh-tw/security/advisories/new), and do not disclose them publicly until a fix is released.

Include the following in your report:

- The vulnerability type and its impact
- Reproduction steps or a proof of concept
- The affected versions and deployment method

We confirm receipt and reply with the status. Once a fix ships, we publish the corresponding advisory.

## Scope

In scope:

- File writes and permission handling in the `update_data.sh` install script
- Integrity of release artifacts
- Defects in the data pipeline triggered by external input

Out of scope. Open a regular [issue](https://github.com/RxChi1d/immich-geodata-zh-tw/issues) instead:

- Errors in place names or administrative division data
- Installation failures, undetected install paths, data not taking effect
- Issues in Immich itself (report those to the [Immich project](https://github.com/immich-app/immich))
