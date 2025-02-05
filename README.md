# Immich 反向地理編碼 - 臺灣特化  
  
本專案為 Immich 提供反向地理編碼功能的臺灣特化優化，旨在提升地理資訊的準確性及使用體驗。主要功能包括：  
  
- **中文化處理**：將國內外地理名稱轉換為符合臺灣用語的繁體中文。  
- **行政區優化**：解決臺灣直轄市與省轄縣市僅顯示地區名稱的問題。  

### 使用前後對比  
![使用前後對比](./image/example.png) 

## 目錄

- [Immich 反向地理編碼 - 臺灣特化](#immich-反向地理編碼---臺灣特化)
  - [目錄](#目錄)
  - [使用方式](#使用方式)
    - [整合式部署（推薦，方便後續更新）](#整合式部署推薦方便後續更新)
    - [手動部署](#手動部署)
  - [臺灣特化邏輯](#臺灣特化邏輯)
  - [更新地理資料](#更新地理資料)
    - [整合式部署](#整合式部署)
    - [手動部署](#手動部署-1)
  - [本地運行資料處理](#本地運行資料處理)
  - [致謝](#致謝)
  - [授權條款](#授權條款)
  
> **NOTE**:  
> 由於 Immich 的反向地理解析功能基於 GeoNames 資料庫，並採用最近距離原則匹配地名，部分結果可能無法完全精確，或與預期不同。  

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

      entrypoint: [ "tini", "--", "/bin/bash", "-c", "bash <(curl -sSL https://raw.githubusercontent.com/RxChi1d/immich-geodata-zh-tw/refs/heads/main/auto_update.sh) && exec /bin/bash start.sh" ]
   ```  
   > **NOTE**:  
   > - `entrypoint` 會在容器啟動時先執行本專案的 `auto_update.sh` 腳本，自動下載並配置臺灣特化資料，隨後執行 Immich 伺服器的 `start.sh` 啟動服務。

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
     - /mnt/user/appdata/immich/i18n-iso-countries/langs:/usr/src/app/node_modules/i18n-iso-countries/langs:ro
   ```
  
2. **下載臺灣特化資料**  
   提供以下兩種下載方式：  
       
   (1) **自動下載**  
      參考本專案中的 `update_data.sh` 腳本，修改 `TARGET_DIR` 為存放 geodata 和 i18n-iso-countries 的資料夾路徑，並執行腳本：  
      ```bash
      bash update_data.sh
      ```  
      > **NOTE**:  
      > UnRAID 使用者可以通過 User Scripts 插件執行腳本。
     
   (2) **手動下載**  
      前往 [Release 頁面](https://github.com/RxChi1d/immich-geodata-zh-tw/releases/latest) 下載 `release.tar.gz` 或 `release.zip`，並將其解壓縮至指定位置。
  
3. **重啟 Immich 和重新提取照片元數據**  
   與[**整合式部署**](#整合式部署)的步驟 2、3 相同。
  
## 臺灣特化邏輯  
  
1. **中文化**：調整地理名稱的翻譯優先級，優先使用符合臺灣用語的中文翻譯。  

2. **行政區調整**：因臺灣已將省級行政區虛級化，將 Immich 的行政區邏輯調整如下：  
   - 一級行政區：包含 22 個直轄市及省轄縣市（如臺北市、高雄市）。  

   - 二級行政區：包含各縣市的次級區域（如新北市的板橋區）。  
  
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
      | opencc   | 1.1.9     |
      | requests | 2.32.3    |
      | tqdm     | 4.67.1    |
      | polars   | 1.21.0    |
      | regex    | 2024.11.6 |
      | loguru   | 0.7.3     |
   
2. 至 [LocationIQ](https://locationiq.com/) 註冊帳號，並取得 API Key。  

3. **執行`main.py`**  
   ```bash  
   python main.py release --locationiq-api-key "YOUR_API_KEY" --country-code "TW" "JP"
   ```  
   > **NOTE:**  
   > - 可以通過 `python main.py --help` 或 `python main.py release --help` 查看更多選項。  
   > - `--country-code` 參數可指定需要處理的國家代碼，多個代碼之間使用空格分隔。(目前僅測試過 TW、JP)  
     
   > **WARNING:**  
   > - 由於 LocationIQ 的 API 有請求次數限制 (可登入後於後台查看)，因此請注意要處理的國家的地名數量，以免超出限制。  
   > - 本專案允許 LocationIQ 反向地理編碼查詢的進度恢復，若超過當日請求限制，可於更換 api 金鑰或次日繼續執行。  
   >   - 需加上 `--pass-cleanup`參數，以取消重設資料夾功能： `python main.py release --locationiq-api-key "YOUR_API_KEY" --country-code "TW" "JP" --pass-cleanup`。  
  
## 致謝  
  
本專案基於 [immich-geodata-cn](https://github.com/ZingLix/immich-geodata-cn) 修改，特別感謝原作者 [ZingLix](https://github.com/ZingLix) 的貢獻。  
  
## 授權條款  
  
本專案採用 GPL 授權。  
