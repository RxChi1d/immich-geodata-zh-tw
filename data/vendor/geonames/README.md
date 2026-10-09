# GeoNames `countryInfo.txt` vendored 資料

- **來源**：https://download.geonames.org/export/dump/countryInfo.txt
- **下載日期**：2026-10-09（保留原檔 CRLF 換行與檔頭註解，未經修改）
- **授權**：CC BY 4.0；attribution 見 `NOTICE.md` 的 GeoNames 條目
- **用途**：Immich v3.3.0 起以此檔取代 `i18n-iso-countries` 解析反向地理編碼的
  國名（只讀第 1、2、5 欄：alpha-2、alpha-3、Country）。`pack` 階段以
  `data/vendor/i18n-iso-countries/langs/en.json` 的繁中國名取代第 5 欄後，
  輸出為 release 的 `geodata/countryInfo.txt`。
- **何時需要更新**：只有 GeoNames 新增、移除或更名國家時。其他欄位的變動
  （人口、郵遞區號格式等）Immich 不讀，不影響結果。更新後若 `en.json` 缺少新
  代碼，`pack` 會報錯提醒補譯。
