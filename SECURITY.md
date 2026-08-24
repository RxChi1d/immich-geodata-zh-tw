# 安全性政策

繁體中文 | [English](docs/en/security.md)

## 支援的版本

安全性修復僅提供給最新的 release 版本。

## 回報漏洞

**請勿透過公開的 issue 回報安全性問題。**

請透過 GitHub 的 [Private vulnerability reporting](https://github.com/RxChi1d/immich-geodata-zh-tw/security/advisories/new) 回報，並在修復版本發布前勿公開揭露。

回報內容請包含：

- 漏洞類型與影響範圍
- 重現步驟或概念驗證
- 受影響的版本與部署方式

收到後會確認並回覆處理進度，修復發布後會公開對應的 advisory。

## 範圍

適用：

- 安裝腳本 `update_data.sh` 的檔案寫入與權限處理
- release 產物的完整性
- 資料處理流程中由外部輸入觸發的缺陷

不適用，請開一般 [issue](https://github.com/RxChi1d/immich-geodata-zh-tw/issues)：

- 地名或行政區資料錯誤
- 安裝失敗、路徑偵測不到、資料沒有生效
- Immich 本身的問題（請回報至 [Immich 專案](https://github.com/immich-app/immich)）
