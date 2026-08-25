# Immich 反向地理編碼 - 臺灣特化

**讓 Immich 的相片地點，用臺灣人習慣的方式顯示。**

[![最新版本](https://img.shields.io/github/v/release/RxChi1d/immich-geodata-zh-tw?label=最新版本)](https://github.com/RxChi1d/immich-geodata-zh-tw/releases/latest)
[![下載次數](https://img.shields.io/github/downloads/RxChi1d/immich-geodata-zh-tw/release.tar.gz?label=下載次數)](https://github.com/RxChi1d/immich-geodata-zh-tw/releases)
[![支援版本](https://img.shields.io/badge/Immich-v2%20%7C%20v3-4250af)](https://immich.app/)
[![授權](https://img.shields.io/badge/授權-GPL--3.0-blue)](LICENSE)

[繁體中文](README.md) | [English](README.en.md)

[Immich](https://immich.app/) 會依照 GPS 座標標示相片的拍攝地點，但它預設的地理資料對臺灣使用者有幾個落差：地名多半是英文或當地拼音，中文搜尋派不上用場；臺灣的部分更是多數縣市名稱從缺，行政區的寫法也和日常慣用的不同；地名的點位稀疏，相片還常被標到鄰近、甚至隔壁的行政區。

本專案重新整理 Immich 使用的地理資料：臺灣、日本、南韓、泰國、印尼改用當地公開的行政區圖資重建地點，其餘地區則補上臺灣慣用的中文譯名。若使用 Docker Compose，安裝只需要在 `docker-compose.yml` 加一行設定。

![使用前後對比](./image/example.png)

## 目錄

- [特色](#特色)
- [支援地區與語言策略](#支援地區與語言策略)
- [安裝](#安裝)
- [更新資料](#更新資料)
- [常見問題](#常見問題)
- [資料來源](#資料來源)
- [延伸閱讀](#延伸閱讀)
- [問題回報與參與](#問題回報與參與)
- [致謝](#致謝)
- [授權條款](#授權條款)

## 特色

- **臺灣採用官方圖資**：以國土測繪中心（NLSC）的村里界圖資重建地名，縣市、鄉鎮市區到村里都依官方資料呈現，並重新計算每個地名的代表座標。
- **部分國家採用當地圖資**：日本、南韓、泰國、印尼已接入各國公開的行政區邊界資料，同樣重建地名與代表座標，減少相片被標到鄰近行政區。
- **其他地區補上臺灣譯名**：導入國家教育研究院《外國地名譯名》，讓收錄的地點顯示臺灣慣用的中文名；沒有官方譯名時使用 GeoNames 的中文資料，兩者都沒有時保留原文，因此冷門地點仍可能是英文。
- **更新只需重啟**：採用自動安裝方式（見[安裝](#安裝)）時，容器每次啟動都會自動取得最新資料，不必手動下載或搬移檔案。
- **安裝可以還原**：以安裝模式執行腳本時，會在覆寫前備份現有資料，中途失敗會還原成安裝前的狀態；手動下載檔案自行覆蓋則沒有這層保護。
- **支援多種部署環境**：Docker Compose、macOS 原生 worker、LXC 與裸機皆可安裝。

## 支援地區與語言策略

不同地區採用最適合臺灣使用者閱讀的語言策略：

| 地區 | 顯示語言 | 地名資料來源 | 詳細說明 |
| :--- | :--- | :--- | :--- |
| 🇹🇼 臺灣 | 繁體中文官方名稱 | 內政部國土測繪中心（NLSC） | [詳細說明](docs/zh-tw/taiwan-admin-processing.md) |
| 🇯🇵 日本 | 日文原名（漢字與假名） | 国土数値情報（日本國土交通省） | [詳細說明](docs/zh-tw/japan-admin-processing.md) |
| 🇰🇷 南韓 | 縣市為韓國官方漢字，一級行政區為繁體中文 | admdongkor（開源專案，整理南韓行政洞界資料） | [詳細說明](docs/zh-tw/south-korea-admin-processing.md) |
| 🇹🇭 泰國 | 繁體中文翻譯（官方英文、泰文備用） | COD-AB 泰國行政區邊界（聯合國 OCHA） | [詳細說明](docs/zh-tw/thailand-admin-processing.md) |
| 🇮🇩 印尼 | 繁體中文翻譯（官方印尼文備用） | 印尼地理空間資訊局（BIG） | [詳細說明](docs/zh-tw/indonesia-admin-processing.md) |
| 🌏 其他地區 | 繁體中文翻譯（無譯名時保留原文） | 國教院《外國地名譯名》、GeoNames | [詳細說明](docs/zh-tw/global-translation-processing.md) |

日本地區直接使用官方圖資的日文原名，不另做中文轉寫。日文漢字地名與中文寫法多半只差字形，例如「横浜市」與「橫濱市」。

南韓的縣市名稱採用韓國官方漢字（例如「淸州市」），理由相同——韓國行政區名本來就是漢字詞，漢字是原名而非翻譯。

## 安裝

本專案支援 Immich v2 與 v3。請先確認 Immich 的部署方式，再選擇對應的安裝方法：

| 部署方式 | 安裝方法 |
| :--- | :--- |
| Docker Compose（含 Portainer、UnRAID 等） | [整合式部署](#整合式部署) 或[手動部署](#手動部署) |
| macOS 原生 worker（immich-accelerator） | [非容器部署](#非容器部署) |
| LXC、裸機等自行安裝的環境 | [非容器部署](#非容器部署) |

### 整合式部署

容器每次啟動時自動下載並安裝最新資料，後續更新只需重啟容器，適合多數使用者。

1. **修改 `docker-compose.yml`**

   在 `immich_server` 服務內新增 `entrypoint` 設定：

   ```yaml
   services:
     immich-server:
       container_name: immich_server

       # 其他設定省略

       entrypoint: [ "tini", "--", "/bin/bash", "-c", "bash <(curl -sSL https://github.com/RxChi1d/immich-geodata-zh-tw/releases/latest/download/update_data.sh) --install && exec start.sh" ]
   ```

   請把 `entrypoint` 這一行加進**既有的** `immich-server` 服務底下，不要新增一個服務。

   容器啟動時會先執行本專案的 `update_data.sh` 下載並安裝臺灣特化資料，接著執行 Immich 的 `start.sh` 啟動服務。

   > [!IMPORTANT]
   > 指令結尾必須是 `exec start.sh`。寫成 `exec /bin/bash start.sh` 會使 Immich v1.142.0 以後的版本無法判斷自身路徑，導致容器不斷重啟。

2. **重啟 Immich**

   ```bash
   docker compose down && docker compose up -d
   ```

   啟動後檢查日誌是否出現 `10000 geodata records imported` 之類的訊息，確認資料已匯入。

3. **重新擷取照片中繼資料**

   登入 Immich 後前往 **系統管理 > 任務**，點擊 **提取中繼資料 > 全部**。完成後既有相片會套用新的地理資訊，之後新上傳的相片不需要再執行這個步驟。

### 手動部署

自行下載資料並掛載到容器，適合需要固定資料版本或無法在啟動時連線的環境。

1. **修改 `docker-compose.yml`**

   在 `volumes` 內新增以下映射，路徑請依實際環境調整：

   ```yaml
   volumes:
     - /mnt/user/appdata/immich/geodata:/build/geodata:ro
     - /mnt/user/appdata/immich/i18n-iso-countries/langs:/usr/src/app/server/node_modules/i18n-iso-countries/langs:ro
   ```

   > [!NOTE]
   > 舊版 Immich（1.136.0 以前）請將第二行改為 `/mnt/user/appdata/immich/i18n-iso-countries/langs:/usr/src/app/node_modules/i18n-iso-countries/langs:ro`。

2. **下載資料**

   先取得更新腳本：

   ```bash
   curl -sSL https://github.com/RxChi1d/immich-geodata-zh-tw/releases/latest/download/update_data.sh -o update_data.sh
   ```

   接著編輯腳本開頭的 `DOWNLOAD_DIR`（約第 25 行），填入兩個掛載路徑的**共同上層目錄**（以上方範例而言是 `/mnt/user/appdata/immich`），然後執行：

   ```bash
   bash update_data.sh
   ```

   完成後應該會得到這樣的結構：

   ```text
   /mnt/user/appdata/immich/geodata/
   /mnt/user/appdata/immich/i18n-iso-countries/langs/
   ```

   也可以直接從 [Releases 頁面](https://github.com/RxChi1d/immich-geodata-zh-tw/releases)下載 `release.tar.gz` 或 `release.zip`，解壓縮後把 `geodata` 與 `i18n-iso-countries` 兩個資料夾放到相同位置。

   > [!NOTE]
   > UnRAID 使用者可透過 User Scripts 外掛執行腳本。

3. **重啟 Immich 並重新擷取照片中繼資料**，步驟與[整合式部署](#整合式部署)的第 2、3 步相同。

指定資料版本、離線安裝等其他參數請參閱 [update_data.sh 使用說明](docs/zh-tw/update-script.md)。

### 非容器部署

Immich 沒有跑在 Docker 容器裡時（macOS 原生 worker、LXC、裸機）適用。

指令請在**執行 Immich microservices worker 的那台機器**上操作，因為地理資料只在該服務啟動時匯入。LXC 與裸機就是 Immich 本機；macOS 加速器若使用 `--ml-only`（Mac 只跑機器學習），worker 仍在 Docker 端，請改用[整合式部署](#整合式部署)。

1. **確認安裝位置**

   先讓腳本印出它打算安裝的位置，這個參數不會寫入任何檔案：

   ```bash
   bash <(curl -sSL https://github.com/RxChi1d/immich-geodata-zh-tw/releases/latest/download/update_data.sh) --print-paths
   ```

   輸出會像這樣：

   ```text
   geodata: /build/geodata
   i18n-iso-countries: /Users/you/.immich-accelerator/server/3.1.0/node_modules/i18n-iso-countries
   ```

   兩個路徑的來源不同，請分別確認：

   | 路徑 | 來源 | 不正確時 |
   | :--- | :--- | :--- |
   | `i18n-iso-countries` | 掃描系統得到，應位於 Immich 的安裝目錄下；路徑含版本號時（macOS 加速器）需與目前執行的 Immich 版本相同 | 以 `IMMICH_SERVER_ROOT` 指定 Immich server 根目錄（其下應有 `node_modules/`） |
   | `geodata` | 沿用 Immich 自己的 `IMMICH_BUILD_DATA` 設定，預設為 `/build` | Immich 有自訂這個變數時（LXC、裸機常見），這裡要一併帶上 |

   ```bash
   IMMICH_SERVER_ROOT=/path/to/immich IMMICH_BUILD_DATA=/var/lib/immich \
     bash <(curl -sSL https://github.com/RxChi1d/immich-geodata-zh-tw/releases/latest/download/update_data.sh) --print-paths
   ```

   > [!IMPORTANT]
   > 腳本不會驗證這兩個路徑是否為 Immich 實際讀取的位置。裝到錯誤的目錄一樣會顯示安裝成功，只是 Immich 讀不到，因此請務必在這一步確認清楚。

2. **安裝資料**

   沿用上一步確認過的**同一條指令**，把 `--print-paths` 換成 `--install`。上一步若帶了環境變數，這裡也要一併帶上：

   ```bash
   bash <(curl -sSL https://github.com/RxChi1d/immich-geodata-zh-tw/releases/latest/download/update_data.sh) --install
   ```

   看到 `驗證通過` 即為安裝成功；中途失敗會自動還原成安裝前的狀態。

   > [!NOTE]
   > LXC 與裸機的 Immich 目錄通常屬於 root，需要 `sudo`。由於 `sudo` 不會帶入環境變數，請先下載腳本再執行，並把變數寫在 `sudo` 之後：
   >
   > ```bash
   > curl -sSL https://github.com/RxChi1d/immich-geodata-zh-tw/releases/latest/download/update_data.sh -o update_data.sh
   > sudo IMMICH_SERVER_ROOT=/path/to/immich bash update_data.sh --install
   > ```

3. **重啟 Immich 並重新擷取照片中繼資料**

   重啟執行 microservices worker 的服務，讓 Immich 重新匯入地理資料：

   - macOS 加速器：`brew services restart immich-accelerator`。請勿改用 `stop` 再 `start`，原因見[加速器環境的注意事項](docs/zh-tw/deployment-macos-accelerator.md)。
   - LXC 與裸機：重啟 Immich 的 systemd 服務（服務名稱依安裝方式而定）。

   接著依[整合式部署](#整合式部署)的第 3 步重新擷取照片中繼資料。

LXC 與裸機沒有其他專屬步驟，完成本節即可。macOS 加速器的資料匯入位置、雙端同時匯入的衝突處理，以及需要重新安裝的時機，請參閱[加速器環境的注意事項](docs/zh-tw/deployment-macos-accelerator.md)。

## 更新資料

本專案會定期發布新的地理資料。更新方式依部署方式而定：

| 部署方式 | 更新方式 |
| :--- | :--- |
| [整合式部署](#整合式部署) | 重啟 Immich 容器即可，資料會自動更新 |
| [手動部署](#手動部署) | 重新執行 `bash update_data.sh`（`DOWNLOAD_DIR` 沿用安裝時的設定），再重啟容器 |
| [非容器部署](#非容器部署) | 重新執行同一條 `--install` 指令後重啟 Immich 服務。macOS 加速器另有需要重新安裝的時機，見[加速器環境的注意事項](docs/zh-tw/deployment-macos-accelerator.md#更新資料) |

資料更新後，Immich 會在下次啟動時匯入新資料，新上傳的相片直接套用。既有相片則要重新執行**提取中繼資料**才會更新。

日常的維護性更新（例如個別行政區的邊界微調）影響的相片有限，通常不需要為此重跑全部相片。以下三種情況才建議重新執行一次：

- 新增支援的國家或地區
- 地理資料的處理方式有大幅調整
- 上游官方圖資有大規模變動

這類變更會在該版本的 [release notes](https://github.com/RxChi1d/immich-geodata-zh-tw/releases) 中標註。

需要固定使用特定版本、離線安裝，或調整安裝路徑時，請參閱 [update_data.sh 使用說明](docs/zh-tw/update-script.md)。

## 常見問題

**執行提取中繼資料後，地名沒有變成中文。**

Immich 會比對 `geodata/geodata-date.txt` 與資料庫中的紀錄，兩者**內容不同**時才重新匯入資料，因此重啟本身不會造成重複匯入。請先看啟動日誌有沒有出現 `geodata records imported`：

- **有出現**：資料已匯入，請確認「提取中繼資料」選的是**全部**，並確認該相片本身含有 GPS 資訊。
- **沒有出現**：代表 Immich 認為資料沒變。手動部署與非容器部署可將 `geodata/geodata-date.txt` 改成與現值不同的內容（例如今天日期）後重啟；整合式部署每次啟動都會重新安裝資料，手動修改會被覆蓋，日期沒變即表示已匯入過同一份資料，請改從上一點檢查。

**行政區名稱已是中文，但國家名稱仍顯示英文。**

這是 Immich 1.136.0 以後的版本搭配本專案 v1.2.0 以前的資料造成的。安裝最新版本即可解決，相關討論見 [issue #8](https://github.com/RxChi1d/immich-geodata-zh-tw/issues/8)。

**容器不斷重啟，日誌顯示 `main.js not found`。**

`entrypoint` 的結尾寫成 `exec /bin/bash start.sh` 時會發生，請改為 `exec start.sh`。Immich v1.142.0 起的啟動腳本會依自身路徑推算安裝位置，多包一層 `/bin/bash` 會使推算結果錯誤。相關討論見 [issue #13](https://github.com/RxChi1d/immich-geodata-zh-tw/issues/13)。

**部分相片的地點與實際位置有落差。**

Immich 依照最近距離原則比對地名，靠近行政區邊界的座標可能被歸到鄰近行政區，小型島嶼或特殊地形也可能無法精確對應。這是 Immich 的解析方式所致，不是資料錯誤。

## 資料來源

本專案使用的所有第三方資料來源、授權條款與使用聲明，請參閱 [NOTICE.md](NOTICE.md)。各地區採用的資料來源見上方[支援地區與語言策略](#支援地區與語言策略)表格。

## 延伸閱讀

- [圖文安裝教學](https://inktrace.rxchi1d.me/posts/container-platform/immich-geodata-zh-tw/)：從零開始的完整步驟與截圖。
- [文件索引](docs/README.md)：各地區處理方式、腳本說明與開發文件。
- [安裝路徑偵測](docs/zh-tw/install-path-detection.md)（維護者向）：腳本決定安裝位置的規則與設計考量。

## 問題回報與參與

使用上遇到問題、發現地名有誤，或希望支援其他國家，歡迎到 [Issues](https://github.com/RxChi1d/immich-geodata-zh-tw/issues) 回報與討論。回報時附上 Immich 版本、部署方式與相關日誌，會更容易釐清問題。

想參與開發請先看 [CONTRIBUTING.md](CONTRIBUTING.md)，資料處理流程的操作說明則見[本地資料處理](docs/zh-tw/development.md)。

安全性問題請依 [SECURITY.md](SECURITY.md) 的方式私下回報，不要開公開 issue。

## 致謝

本專案基於 [immich-geodata-cn](https://github.com/ZingLix/immich-geodata-cn) 修改，感謝原作者 [ZingLix](https://github.com/ZingLix) 的貢獻。

## 授權條款

本專案的程式碼採用 [GNU General Public License v3.0](LICENSE)。

發布的地理資料依各原始來源的授權條款提供，詳見 [NOTICE.md](NOTICE.md)。
