# 安裝路徑偵測

> 本文件記錄 `update_data.sh --install` 決定安裝位置的規則、驗證方式與設計
> 沿革，供維護與問題排查參考。實際操作步驟請見 README 的部署章節。

## 安裝目標

腳本安裝兩份資料：

- **geodata**：反向地理編碼使用的地名資料。
- **i18n-iso-countries 的 `langs/`**：國名翻譯。Immich 以
  `getName(countryCode, 'en')` 取得國名，因此在地化改寫的是 `langs/en.json`。

## 偵測規則

腳本依序檢查候選根目錄，採用第一個確實裝有 `i18n-iso-countries` 的位置：

1. 環境變數 `IMMICH_SERVER_ROOT`，同時接受 app 根目錄與其下的 `server/`。
2. `/usr/src/app/server` 與 `/usr/src/app`，對應官方容器。
3. `~/.immich-accelerator/config.json` 中的 `server_dir`，對應 immich-accelerator。

附加規則：

- 設定 `IMMICH_SERVER_ROOT` 後，該值為唯一搜尋範圍。該範圍內找不到套件時，
  腳本以非零狀態結束，不改用其他候選。
- 候選以 canonical path 去重，避免不同來源指向同一目錄時被計為多個結果。
  去重僅用於比對，安裝仍使用原始路徑：pnpm 版面的 canonical path 位於
  `.pnpm` 目錄下，作為安裝位置會改變後續的解析語意。
- 候選皆未命中時，以 `find -maxdepth 5 -type d -name node_modules -prune`
  掃描候選根目錄，取得套件的實際位置。
- 命中多個不同目標時，腳本輸出警告並採用第一個。整合式部署的 entrypoint 為
  `update_data.sh --install && exec start.sh`，非零狀態會使 Immich 無法啟動。
- 完全未命中時，腳本以非零狀態結束，不建立目錄。若建立目錄，語系檔會寫入
  Immich 不會讀取的位置，且不會產生錯誤訊息。

## geodata 路徑

geodata 安裝至 `IMMICH_BUILD_DATA` 之下的 `geodata/`，預設為 `/build/geodata`。
該變數由 Immich 自身定義：

```js
const buildFolder = dto.IMMICH_BUILD_DATA || '/build';
geodata: join(buildFolder, 'geodata'),
```

腳本沿用同一個變數，避免另立設定後與 Immich 的設定不同步。macOS 加速器會為
`/build` 建立 synthetic link，因此容器與原生環境的路徑一致。

## 安裝結果驗證

安裝完成後，腳本執行下列檢查：

1. 解析套件位置。環境中有 `node` 時，交由 `node` 的模組解析取得結果；沒有
   `node` 時套用相同規則，自起點目錄逐層向上，取第一個命中的
   `node_modules/<套件>`。
2. 逐檔比對下載後暫存的內容與解析結果的內容。

比對對象是暫存內容而非安裝目標。pnpm 版面下，安裝路徑與解析結果是同一個檔案
的兩個名稱，兩者相互比對的結果恆為相同，無法反映複製是否實際發生。

檢查涵蓋整個 `langs/`，因此包含決定國名顯示的 `en.json`。

## 設計沿革

早期版本讀取 Immich 的 `package.json` 取得版本號，再依版本分界推斷
`i18n-iso-countries` 的位置：

```
< 1.136.0  ->  /usr/src/app/node_modules/i18n-iso-countries
>= 1.136.0 ->  /usr/src/app/server/node_modules/i18n-iso-countries
```

改為偵測目錄結構的原因：

- 分界點最初記為 1.139.4，之後修正為 1.136.0；版本比較的實作亦修正過一次。
- 版本號無法描述尚未發生的目錄調整，上游每次搬遷都需要修改腳本。
- macOS 原生 worker 的 Immich 3.1.0 使用扁平版面，依版本推斷會選擇巢狀路徑。
- 讀取版本需要執行 `node`，而非容器部署不保證環境中有可用的 `node`。

## 已驗證環境

| 環境 | 版面 | 套件管理 | 安裝位置 |
| :--- | :--- | :--- | :--- |
| immich-server v1.135.3 | 扁平 | npm | `/usr/src/app/node_modules/…` |
| immich-server v1.136.0 | 巢狀 | npm | `/usr/src/app/server/node_modules/…` |
| immich-server v3.1.0 | 巢狀 | pnpm | `/usr/src/app/server/node_modules/…` |
| macOS 原生 worker 3.1.0 | 扁平 | pnpm | `~/.immich-accelerator/server/3.1.0/node_modules/…` |

另以 v3.1.0 映像將應用移至 `/opt/immich`，驗證三種情況：未設定
`IMMICH_SERVER_ROOT` 時以非零狀態結束、設定後安裝成功、以及非 root 使用者的
安裝行為。

## 回歸測試

- `tests/update_data_paths.sh`：偵測規則。
- `tests/update_data_install.sh`：安裝流程，以 `--archive` 餵入自製 payload。

兩者皆不需要網路。
