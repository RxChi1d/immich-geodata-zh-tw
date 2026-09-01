# update_data.sh 使用說明

`update_data.sh` 負責下載本專案發布的地理資料，並安裝到 Immich 讀取的位置。整合式部署會在容器每次啟動時執行它；其他部署方式則由使用者手動執行。

腳本可從 Releases 取得：

```bash
curl -sSL https://github.com/RxChi1d/immich-geodata-zh-tw/releases/latest/download/update_data.sh -o update_data.sh
```

## 參數

| 參數 | 說明 |
| :--- | :--- |
| `--install` | 將資料安裝到 Immich 的系統目錄。省略時只下載並解壓縮到 `DOWNLOAD_DIR`。 |
| `--tag <tag>` | 指定要下載的 release 版本，省略時為最新版本。 |
| `--archive <path>` | 使用本機既有的 `release.tar.gz`，不連線下載。 |
| `--print-paths` | 只印出偵測到的安裝路徑後結束，不下載也不安裝。 |

## 環境變數

| 變數 | 說明 |
| :--- | :--- |
| `IMMICH_SERVER_ROOT` | Immich server 根目錄（其下應有 `node_modules/`）。一旦設定即為唯一搜尋範圍，指定錯誤時腳本會直接失敗，不會改裝到其他位置。 |
| `IMMICH_BUILD_DATA` | Immich 自身的變數，`geodata` 會安裝到其下的 `geodata/`，預設為 `/build`。 |

安裝路徑的偵測規則與設計原因請參閱[安裝路徑偵測](install-path-detection.md)。

## 下載位置

未加 `--install` 時，資料會解壓縮到腳本內的 `DOWNLOAD_DIR` 變數指定的目錄，預設為執行目錄下的 `./temp`，解壓後會產生 `geodata/` 與 `i18n-iso-countries/` 兩個資料夾。

手動部署若要直接下載到掛載目錄，請編輯 `update_data.sh` 開頭的這一行（約第 25 行），填入兩個掛載路徑的**共同上層目錄**：

```bash
DOWNLOAD_DIR="/mnt/user/appdata/immich"
```

解壓後的目錄結構見 [README 手動部署](../../README.md#手動部署)的第 2 步。

> [!NOTE]
> 這個變數只能修改腳本內容，以環境變數帶入無效。

## 指定版本

最新版本出現問題，或需要固定使用特定版本時，可用 `--tag` 指定要安裝的資料版本。可用的 tag 名稱請見 [Releases 頁面](https://github.com/RxChi1d/immich-geodata-zh-tw/releases)，例如 `v3.2.0` 或 `nightly`。

其中 `nightly` 是自動發布的資料版本。本專案每週重新產生一次資料並覆蓋 `nightly` 這個 tag，內容取自當次執行時的上游圖資。它的內容未經正式版本的完整驗證，且同一個 tag 會被後續的自動發布覆蓋，適合想搶先取得新圖資的情況；長期穩定使用請指定 `vX.Y.Z`。

腳本本身一律從最新版本取得，只用 `--tag` 指定資料版本。整合式部署的寫法：

```yaml
entrypoint: [ "tini", "--", "/bin/bash", "-c", "bash <(curl -sSL https://github.com/RxChi1d/immich-geodata-zh-tw/releases/latest/download/update_data.sh) --install --tag <tag_name> && exec start.sh" ]
```

手動執行時加上參數即可：

```bash
bash update_data.sh --install --tag <tag_name>
```

> [!IMPORTANT]
> 不要把腳本網址改成 `releases/download/<tag_name>/update_data.sh`。`nightly` 等自動發布的版本不包含 `update_data.sh`，該網址會回傳 404，整合式部署會因此無法啟動。

腳本會先驗證 tag 是否存在於 GitHub Releases，tag 無效時會顯示錯誤並終止。

## 離線安裝

已經有 `release.tar.gz`（例如離線環境，或想重複安裝同一份資料）時，可用 `--archive` 直接安裝：

```bash
bash update_data.sh --install --archive /path/to/release.tar.gz
```

## 安裝行為

- 安裝前會備份現有的 `geodata` 與 `i18n-iso-countries/langs`，覆寫過程中出錯時會還原成安裝前的狀態，不會留下只裝了一半的資料。
- 語系檔逐檔替換，上游有而本專案沒有提供的語系檔會保留。
- 安裝後會驗證資料確實寫入 Immich 會讀取的位置，包含決定國家名稱顯示的 `en.json`；看到 `驗證通過` 即為成功。
- 安裝流程是冪等的，資料已是最新時重複執行不會有副作用。
