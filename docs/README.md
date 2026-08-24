# 文件索引

本專案的說明文件依用途分成以下幾類。安裝步驟與常見問題請先看[專案 README](../README.md)。

## 安裝與更新參考

| 文件 | 內容 |
| :--- | :--- |
| [macOS 加速器環境的注意事項](zh-tw/deployment-macos-accelerator.md) | immich-accelerator 環境特有的安裝位置、路徑判讀、重啟方式與更新時機（LXC 與裸機依 README 的非容器部署即可） |
| [update_data.sh 使用說明](zh-tw/update-script.md) | 腳本參數、環境變數、指定版本、離線安裝 |
| [安裝路徑偵測](zh-tw/install-path-detection.md) | 腳本如何決定安裝位置，以及安裝結果的驗證方式（維護者向） |

## 各地區的處理方式

| 地區 | 文件 |
| :--- | :--- |
| 🇹🇼 臺灣 | [臺灣行政區處理](zh-tw/taiwan-admin-processing.md) |
| 🇯🇵 日本 | [日本行政區處理](zh-tw/japan-admin-processing.md) |
| 🇰🇷 南韓 | [南韓行政區處理](zh-tw/south-korea-admin-processing.md) |
| 🇹🇭 泰國 | [泰國行政區處理](zh-tw/thailand-admin-processing.md) |
| 🇮🇩 印尼 | [印尼行政區處理](zh-tw/indonesia-admin-processing.md) |
| 🌏 其他地區 | [全球翻譯處理](zh-tw/global-translation-processing.md) |

## 開發

| 文件 | 內容 |
| :--- | :--- |
| [貢獻指南](../CONTRIBUTING.md) | 開發流程、提交規範、測試要求 |
| [本地資料處理](zh-tw/development.md) | 在本機重現資料處理流程：提取圖資、產生 release、驗證 |

## 研究與歷史紀錄

以下文件是決策當下的紀錄，用於說明「為什麼這樣做」。實作後續可能調整，內容不隨程式碼同步更新，請以上方的地區處理文件為準。

| 文件 | 內容 |
| :--- | :--- |
| [中文譯名來源評估](research/chinese-translation-sources.md) | 全球地名譯名的來源比較與選擇依據 |
| [泰國 handler 設計](research/thailand-handler.md) | 泰國支援的來源選擇與設計評估 |
| [印尼 handler 設計](research/indonesia-handler.md) | 印尼支援的來源選擇、行政層級與翻譯策略 |
| [印尼投影與座標實驗](research/idn-handler-projection-coordinate-experiment.md) | 投影法與代表座標策略的實驗數據 |
| [Python 至 Rust 遷移](history/python-to-rust-migration.md) | 資料處理工具鏈的遷移過程紀錄 |

## 語言版本

文件同步維護繁體中文與英文兩個版本，本索引的英文版為 [docs/en/README.md](en/README.md)。研究與歷史紀錄僅有繁體中文版。兩種語言內容不一致時，以繁體中文版為準。
