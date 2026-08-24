# macOS 加速器環境的注意事項

[epheterson/immich-apple-silicon](https://github.com/epheterson/immich-apple-silicon)（immich-accelerator）可將 Immich 的 microservices worker 與機器學習服務以原生行程跑在 Apple Silicon 上，資料庫與 API 仍留在 Docker。

安裝步驟本身與其他非容器環境相同，請依 [README 的非容器部署](../../README.md#非容器部署)操作。本文只說明這個環境特有的事項。

## 資料要裝在哪一台

反向地理編碼的資料匯入只在 microservices worker 啟動時執行，因此資料要裝在**執行 microservices worker 的那一台**。

| 加速器模式 | 安裝位置 |
| :--- | :--- |
| `--ml-only`（Mac 只跑機器學習） | Docker 主機，依照[整合式部署](../../README.md#整合式部署)進行 |
| 分離式部署（Mac 跑 worker 與機器學習） | **Mac**，依照 [README 的非容器部署](../../README.md#非容器部署)進行 |

> [!NOTE]
> Docker 端若仍在執行 microservices worker，兩端都會嘗試匯入地理資料，版本不一致時會互相覆蓋。分離式部署建議在 Docker 端設定 `IMMICH_WORKERS_INCLUDE=api`，設定後該端不會讀取地理資料，也可一併從其 `entrypoint` 移除更新指令，避免每次啟動重複下載。

## 路徑的判讀

執行 `--print-paths` 時，這個環境的輸出會像這樣：

```text
geodata: /build/geodata
i18n-iso-countries: /Users/you/.immich-accelerator/server/3.1.0/node_modules/i18n-iso-countries
```

- `geodata`：加速器會建立 `/build` 的 synthetic link，因此路徑與容器一致，不需額外設定。
- `i18n-iso-countries`：位於依 Immich 版本區分的 server 目錄下，請確認輸出的版本號與目前執行的 Immich 相符。

## 重啟服務

以 Homebrew services 管理時使用：

```bash
brew services restart immich-accelerator
```

> [!IMPORTANT]
> 請勿改用 `immich-accelerator stop && immich-accelerator start`。`stop` 會觸發 launchd 依 `KeepAlive` 立即重啟服務，隨後的 `start` 會因連接埠已被佔用而失敗。

未以 Homebrew services 管理時，才使用 `immich-accelerator stop && immich-accelerator start`。

## 更新資料

與容器不同，加速器的資料是持久的，不會在每次啟動時重新安裝。

| 動作 | 是否需要重新安裝 |
| :--- | :--- |
| `immich-accelerator stop` / `start`、重新開機 | 否 |
| `immich-accelerator update`（切換 Immich 版本） | **是** |
| 本專案發布新版資料 | **是** |

切換 Immich 版本時，加速器會重建 `build-data`（`geodata` 隨之移除），並改用新版本號的 server 目錄（語系檔隨之失效），因此兩份資料都會回到上游狀態。

更新步驟與安裝相同：重新執行安裝、重啟 worker、重新擷取照片中繼資料。上述步驟可以合併成一個腳本，需要更新時執行一次即可：

```bash
#!/bin/bash
# ~/.local/bin/immich-geodata-update
set -e
bash <(curl -sSL https://github.com/RxChi1d/immich-geodata-zh-tw/releases/latest/download/update_data.sh) --install
brew services restart immich-accelerator
```
