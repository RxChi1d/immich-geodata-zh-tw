# Immich 反向地理編碼 - 臺灣特化

本專案為 Immich 提供反向地理編碼功能的臺灣特化優化，旨在提升地理資訊的準確性及使用體驗。主要功能包括：

- **中文化處理**：將國內外地理名稱轉換為符合臺灣用語的繁體中文。
- **行政區優化**：解決臺灣直轄市與省轄縣市僅顯示地區名稱的問題。

### 使用前後對比
![使用前後對比](./image/example.png)

## 使用方式

1. **下載檔案**  
   前往 [Release 頁面](https://github.com/RxChi1d/immich-geodata-zh-tw/releases/tag/release) 下載 `immich-geodata-zh-tw.zip`，並將其解壓縮。

2. **修改 `docker-compose.yaml` 配置**  
   在 `volumes` 中新增以下映射：
   ```yaml
   volumes:
     - ./geodata:/build/geodata
     - ./i18n-iso-countries/langs:/usr/src/app/node_modules/i18n-iso-countries/langs
   ```
   或根據不同部署方式，自行替換上述文件夾。

3. **重啟 Immich**  
   執行以下命令以重啟 Immich：
   ```bash
   # 如果使用 docker-compose 部署
   docker compose down && docker compose up
   ```
   或  
   ```bash
   # 如果使用 docker 部署
   docker restart immich_server
   ```
   - 啟動後，檢查日誌中是否顯示 `10000 geodata records imported` 等類似訊息，確認 geodata 已成功更新。
   - 若未更新，請修改 `geodata/geodata-date.txt` 為一個更新的時間戳，確保其晚於 Immich 上次加載的時間。

4. **重新提取照片元數據**  
   登錄 Immich 管理後台，前往 **系統管理 > 任務**，點擊 **提取元數據 > 全部**，以觸發照片元數據的重新提取。完成後，所有照片的地理資訊將顯示為中文，新上傳的照片則無需額外操作，並支援中文搜尋。

## 臺灣特化邏輯

1. **中文化**：調整地理名稱的翻譯優先級，優先使用符合臺灣用語的中文翻譯。
2. **行政區調整**：因臺灣已將省級行政區虛級化，將 Immich 的行政區邏輯調整如下：
   - 一級行政區：包含 22 個直轄市及省轄縣市（如臺北市、高雄市）。
   - 二級行政區：包含各縣市的次級區域（如新北市的板橋區）。

## TODO

- 優化其他國家城市名稱的中文翻譯。
- 優化資料處理的效率。

## 致謝

本專案基於 [immich-geodata-cn](https://github.com/ZingLix/immich-geodata-cn) 修改，特別感謝原作者 [ZingLix](https://github.com/ZingLix) 的貢獻。

## 授權條款

本專案採用 GPL 授權。

