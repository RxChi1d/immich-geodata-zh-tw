# NAER《外國地名譯名》vendored 資料

- **來源**：國家教育研究院《外國地名譯名》，政府資料開放平臺
  dataset 15211（https://data.gov.tw/dataset/15211）
- **原始檔下載日期**：2026-06-06
- **授權**：政府資料開放授權條款－第 1 版（OGDL 1.0），與 CC BY 4.0
  相容；attribution 見 `NOTICE.md`
- **欄位**：`name_norm`（正規化英文名，匹配 key）、`name_zh`（清理後
  中文譯名）、`country_code`（ISO 3166-1 alpha-2，未對應留空）、
  `latitude`/`longitude`（十進位）、`feature_hint`（自然地物啟發式標記）
- **重生成**：
  `cargo run --release -- naer-prepare --input <原始CSV> --output data/vendor/naer/naer_place_names.csv`
- 設計與決策原因見 `docs/zh-tw/global-translation-processing.md` 與
  `docs/research/chinese-translation-sources.md`
