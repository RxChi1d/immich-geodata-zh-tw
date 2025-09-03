# Immich 反向地理編碼 - 臺灣特化  

[繁體中文](README.md) | [English](README.en.md)
  
本專案為 Immich 提供反向地理編碼功能的臺灣特化優化，旨在提升地理資訊的準確性及使用體驗。主要功能包括：  
  
- **中文化處理**：將國內外地理名稱轉換為符合臺灣用語的繁體中文。  
- **行政區優化**：解決臺灣直轄市與省轄縣市僅顯示地區名稱的問題。  
- **提升臺灣資料準確性**：利用**中華民國國土測繪中心 (NLSC)** 的官方圖資處理臺灣地區的地理名稱與邊界資料，確保數據來源的權威性。  

> [!TIP]
> 版本相容性提示
> 
> - 自 Immich 1.139.4 起，容器內路徑有所調整。
> - 若你使用 1.139.3（含）以下且採「手動部署」，請依照本文件的「[手動部署](#手動部署)」章節調整 `volumes` 映射路徑。
> - 若使用本專案提供的整合式自動部署（update_data.sh），無需更動；腳本已更新以支援新舊版本。

### 使用前後對比  
![使用前後對比](./image/example.png) 

## 目錄

- [Immich 反向地理編碼 - 臺灣特化](#immich-反向地理編碼---臺灣特化)
    - [使用前後對比](#使用前後對比)
  - [目錄](#目錄)
  - [資料來源](#資料來源)
  - [使用方式](#使用方式)
    - [整合式部署（推薦，方便後續更新）](#整合式部署推薦方便後續更新)
    - [手動部署](#手動部署)
  - [指定特定版本](#指定特定版本)
  - [臺灣特化邏輯](#臺灣特化邏輯)
  - [更新地理資料](#更新地理資料)
    - [整合式部署](#整合式部署)
    - [手動部署](#手動部署-1)
  - [本地運行資料處理](#本地運行資料處理)
  - [致謝](#致謝)
  - [授權條款](#授權條款)
  
## 資料來源

本專案使用的地理數據主要來自以下來源：

1.  **GeoNames** ([geonames.org](https://www.geonames.org/)): 作為全球地理位置的基礎數據庫。
2.  **中華民國國土測繪中心 (NLSC)**:
    - 來源: [國土測繪中心開放資料平台](https://whgis-nlsc.moi.gov.tw/Opendata/Files.aspx)
    - 資料集: 村(里)界 (TWD97經緯度), 版本 1140620
    - 用途: 作為臺灣地區村里界線及行政區名稱的主要數據源，確保資料的準確性與權威性。
3.  **LocationIQ**: 用於處理非臺灣地區的反向地理編碼請求，校準行政區劃層級。
4.  **中華民國經濟部國際貿易署 & 中華民國外交部**: 作為部分國家/地區中文譯名的參考來源。

> **NOTE**:  
> 由於 Immich 的反向地理解析功能基於其載入的資料庫（本專案主要依賴 GeoNames 和 NLSC 資料），並採用最近距離原則匹配地名，部分結果可能無法完全精確，或與預期不同。  

## 使用方式

本專案支援以下兩種部署方式：  

1. 整合式部署（適用於 Immich 的 docker-compose 部署，可確保容器啟動時自動載入最新的臺灣特化資料）。

2. 手動部署（適用於自訂部署環境，可手動下載並配置特化資料）。

### 整合式部署（推薦，方便後續更新）
  
1. **修改 `docker-compose.yml` 配置**  
   在 `immich_server` 服務內新增 `entrypoint` 設定，使容器啟動時自動下載最新地理資料：  
   ```yaml  
   services:
     immich_server:
      container_name: immich_server

      # 其他配置省略

      entrypoint: [ "tini", "--", "/bin/bash", "-c", "bash <(curl -sSL https://raw.githubusercontent.com/RxChi1d/immich-geodata-zh-tw/refs/heads/main/update_data.sh) --install && exec /bin/bash start.sh" ]
   ```  
   > **NOTE**:  
   > - `entrypoint` 會在容器啟動時先執行本專案的 `update_data.sh` 腳本，自動下載並配置臺灣特化資料，隨後執行 Immich 伺服器的 `start.sh` 啟動服務。
   > - 整合式部署也支援指定特定版本下載，詳情請參考 [指定特定版本](#指定特定版本) 章節。

2. **重啟 Immich**  
   執行以下命令以重啟 Immich： 
   ```bash  
   # 如果使用 docker-compose 部署
   docker compose down && docker compose up
   ```  
   - 啟動後，檢查日誌中是否顯示 `10000 geodata records imported` 等類似訊息，確認 geodata 已成功更新。  
   - 若未更新，請修改 `geodata/geodata-date.txt` 為一個更新的時間戳，確保其晚於 Immich 上次加載的時間。 
  
3. **重新提取照片元數據**  
   登錄 Immich 管理後台，前往 **系統管理 > 任務**，點擊 **提取元數據 > 全部**，以觸發照片元數據的重新提取。完成後，所有照片的地理資訊將顯示為中文。  
   新上傳的照片無需額外操作，即可直接支援中文搜尋。  

### 手動部署

1. **修改 `docker-compose.yml` 配置**  
   在 `volumes` 內新增以下映射（請依據實際環境調整路徑）：  
   ```yaml
   volumes:
     - /mnt/user/appdata/immich/geodata:/build/geodata:ro
     - /mnt/user/appdata/immich/i18n-iso-countries/langs:/usr/src/app/server/node_modules/i18n-iso-countries/langs:ro
   ```
   > **NOTE**:  
   > 若使用 Immich < 1.139.4 版本，請將第二行改為：  
   > `/mnt/user/appdata/immich/i18n-iso-countries/langs:/usr/src/app/node_modules/i18n-iso-countries/langs:ro`
  
2. **下載臺灣特化資料**  
   提供以下兩種下載方式：  
       
   (1) **自動下載**  
      參考本專案中的 `update_data.sh` 腳本，修改 `DOWNLOAD_DIR` 為存放 geodata 和 i18n-iso-countries 的資料夾路徑，並執行腳本：  
      ```bash
      bash update_data.sh
      ```  
      > **NOTE**:  
      > - 手動部署也支援指定特定版本下載，詳情請參考 [指定特定版本](#指定特定版本) 章節。
      > - UnRAID 使用者可以通過 User Scripts 插件執行腳本。
     
   (2) **手動下載**  
      前往 [Release 頁面](https://github.com/RxChi1d/immich-geodata-zh-tw/releases) 查找所需的版本，下載對應的 `release.tar.gz` 或 `release.zip`，並將其解壓縮至指定位置。
  
3. **重啟 Immich 和重新提取照片元數據**  
   與[**整合式部署**](#整合式部署)的步驟 2、3 相同。

## 指定特定版本

在某些情況下（例如最新的 release 出現問題），你可能需要下載或回滾到特定的 release 版本。本專案的更新腳本支援透過 `--tag` 參數來指定要下載的 release tag。

**如何找到可用的 Tag？**
請前往本專案的 [Releases 頁面](https://github.com/RxChi1d/immich-geodata-zh-tw/releases) 查看所有可用的 release tag 名稱（例如  `v1.0.0`, `nightly` 等）。

**使用範例：**

1.  **整合式部署 (`docker-compose.yml` 中的 `entrypoint`)**
    在 `entrypoint` 的指令後面加上 `--tag <tag_name>`：
    ```yaml
    entrypoint: [ "tini", "--", "/bin/bash", "-c", "bash <(curl -sSL https://raw.githubusercontent.com/RxChi1d/immich-geodata-zh-tw/refs/heads/main/update_data.sh) --install --tag <tag_name> && exec /bin/bash start.sh" ] 
    ```
    將 `<tag_name>` 替換為你想要下載的具體 tag 名稱。如果省略 `--tag`，則預設下載最新的 release (`latest`)。

2.  **手動部署 (`update_data.sh`)**
    執行腳本時加上 `--tag <tag_name>`：
    ```bash
    bash update_data.sh --tag <tag_name>
    ```
    將 `<tag_name>` 替換為你想要下載的具體 tag 名稱。如果省略 `--tag`，則預設下載最新的 release (`latest`)。

> **NOTE**: 腳本會先驗證指定的 tag 是否存在於 GitHub Releases，如果 tag 無效則會提示錯誤並終止執行，因此請在執行前確保 tag 有效。
  
## 臺灣特化邏輯  
  
本專案針對臺灣地區的地理資訊處理，採用了更精確且符合在地需求的特化邏輯：  
  
1.  **以國土測繪中心 (NLSC) 資料為核心**:  
     *   臺灣的行政區邊界與名稱，主要基於 **國土測繪中心 (NLSC) 發布的村(里)界圖資**。這確保了地理資訊的**準確性**。  
     *   透過處理 NLSC 的村里資料，我們能將地理座標反向解析準確至村里，藉此提供更精確的鄉鎮市區及縣市層級。  
  
2.  **行政區劃層級定義**:  
     *   **一級行政區 (Admin1)**: 對應臺灣的 **22 個直轄市及省轄縣市** (例如：臺北市、基隆市、彰化縣)。  
     *   **二級行政區 (Admin2)**: 對應各縣市下的 **鄉、鎮、市、區** (例如：新北市的板橋區、彰化縣的彰化市)。  
     *   **三級行政區 (Admin3)**: 對應 NLSC 資料中的 **村、里**。  
     *   **四級行政區 (Admin4)**: 目前未使用。  
  
3.  **中文名稱處理**:  
     *   臺灣境內的地理名稱 (縣市、鄉鎮市區、村里) **直接採用 NLSC 圖資提供的官方名稱**。  
     *   非臺灣地區的地理名稱主要參考 **GeoNames** 資料庫，其中國家名稱的翻譯則採用**中華民國經濟部國際貿易署**及**中華民國外交部**提供的官方譯名，以確保符合臺灣用語習慣的繁體中文名稱。
  
透過上述邏輯，本專案旨在提供更貼近臺灣實際情況、更為精確的反向地理編碼結果。

## 更新地理資料

### 整合式部署
  
只需重新啟動 Immich 容器，即可自動更新地理資料。  

### 手動部署
  
1. 下載最新 release.zip，並解壓至指定位置。
   
2. 重新提取照片元數據（與[手動部署](#手動部署)相同）。
  
## 本地運行資料處理  
  
1. **安裝依賴**  
   執行以下指令安裝所需 Python 依賴：  

   ```bash
   pip install -r requirements.txt
   ```

   或手動安裝以下套件：

      | Package  | Version   |
      | -------- | --------- |
      | loguru   | 0.7.3     |
      | opencc   | 1.1.9     |
      | polars   | 1.27.1    |
      | regex    | 2024.11.6 |
      | requests | 2.32.3    |
      | tqdm     | 4.67.1    |

2. 至 [LocationIQ](https://locationiq.com/) 註冊帳號，並取得 API Key。  

3. **執行`main.py`**  
   ```bash  
   python main.py release --locationiq-api-key "YOUR_API_KEY" --country-code "JP" "KR" "TH"
   ```  
   > **NOTE:**  
   > - 可以通過 `python main.py --help` 或 `python main.py release --help` 查看更多選項。  
   > - `--country-code` 參數可指定需要處理的國家代碼，多個代碼之間使用空格分隔。(目前僅測試過 "JP" "KR" "TH")  
     
   > **WARNING:**  
   > - 由於 LocationIQ 的 API 有請求次數限制 (可登入後於後台查看)，因此請注意要處理的國家的地名數量，以免超出限制。  
   > - 本專案允許 LocationIQ 反向地理編碼查詢的進度恢復，若超過當日請求限制，可於更換 api 金鑰或次日繼續執行。  
   >   - 需加上 `--pass-cleanup`參數，以取消重設資料夾功能： `python main.py release --locationiq-api-key "YOUR_API_KEY" --country-code "TW" "JP" --pass-cleanup`。  
  
## 致謝  
  
本專案基於 [immich-geodata-cn](https://github.com/ZingLix/immich-geodata-cn) 修改，特別感謝原作者 [ZingLix](https://github.com/ZingLix) 的貢獻。  
  
## 授權條款  
  
本專案採用 GPL 授權。
