# 第三方資料來源聲明

本專案使用以下第三方資料來源,特此聲明並表達感謝。

---

## 1. OpenStreetMap

- **來源**: OpenStreetMap contributors
- **授權**: Open Database License (ODbL) 1.0
- **使用方式**: 開發者為尚未建立專屬處理流程的國家產生 metadata 時，透過 LocationIQ API 取得反向地理編碼資料
- **資料範圍**: 臺灣、日本、南韓、泰國、印尼以外地區；早期版本的發布資料含此來源的衍生內容
- **授權連結**: https://opendatacommons.org/licenses/odbl/1-0/
- **資料來源**: https://www.openstreetmap.org/copyright

**授權聲明**:

> 本專案使用來自 [OpenStreetMap](https://www.openstreetmap.org/) 的資料,該資料依 [Open Database License (ODbL)](https://opendatacommons.org/licenses/odbl/1-0/) 提供。

---

## 2. GeoNames

- **來源**: GeoNames geographical database
- **授權**: Creative Commons Attribution 4.0 International (CC-BY 4.0)
- **使用方式**: 全球城市資料、行政區資料、地名翻譯
- **資料檔案**:
  - cities500.zip (全球城市資料)
  - admin1CodesASCII.txt (一級行政區資料)
  - admin2Codes.txt (二級行政區資料)
  - alternateNamesV2.zip (替代地名資料)
- **授權連結**: https://creativecommons.org/licenses/by/4.0/
- **資料來源**: https://www.geonames.org/

**授權聲明**:

> 本專案使用來自 [GeoNames](https://www.geonames.org/) 的資料,該資料依 [Creative Commons Attribution 4.0 International (CC-BY 4.0)](https://creativecommons.org/licenses/by/4.0/) 授權提供。

---

## 3. 中華民國國土測繪中心 (NLSC)

- **來源**: 內政部國土測繪中心開放資料平台
- **資料集**: 村(里)界 (TWD97經緯度)
- **授權**: 政府資料開放授權條款-第1版
- **使用方式**: 臺灣地區行政區邊界與地名資料
- **資料來源**: https://whgis-nlsc.moi.gov.tw/Opendata/Files.aspx

**資料來源標示**:

> 本專案使用中華民國內政部國土測繪中心提供之「村(里)界 (TWD97經緯度)」開放資料。

---

## 4. 国土数値情報（日本國土交通省）

- **來源**: 日本國土交通省 国土数値情報ダウンロードサイト
- **資料集**: 行政区域データ (世界測地系)
- **授權**: 国土数値情報 利用約款
- **授權連結**: https://nlftp.mlit.go.jp/ksj/other/agreement.html
- **使用方式**: 日本地區行政區邊界與地名資料
- **資料來源**: https://nlftp.mlit.go.jp/ksj/

**資料來源標示**:

> 本專案使用日本國土交通省提供之「行政区域データ」公開資料。

---

## 5. admdongkor

- **來源**: vuski/admdongkor (GitHub)，原始資料為南韓統計廳 SGIS 公開的行政洞界資料
- **資料集**: 南韓行政區邊界資料 (GeoJSON)
- **授權**: 邊界資料採 Creative Commons Attribution 4.0 (CC BY 4.0)；原始資料依南韓 KOGL 第 1 型（공공누리 제1유형，出處標示）開放，出處標示義務不因加工而免除
- **使用方式**: 南韓地區行政區邊界與地名資料
- **授權連結**: https://github.com/vuski/admdongkor/blob/master/LICENSE-DATA
- **資料來源**: https://github.com/vuski/admdongkor

**資料來源標示**:

> 本專案使用來自 [admdongkor](https://github.com/vuski/admdongkor) 專案的南韓行政區邊界資料，原始資料出自南韓統計廳 SGIS（統計地理資訊服務），依 [공공누리 제1유형](https://www.kogl.or.kr/info/licenseType1.do) 開放。

---

## 6. Thailand COD-AB

- **來源**: Humanitarian Data Exchange (HDX) - Thailand Subnational Administrative Boundaries
- **資料集**: Thailand administrative level 0-3 boundaries (COD-AB)
- **授權**: Creative Commons Attribution for Intergovernmental Organisations (CC BY-IGO)
- **使用方式**: 泰國地區行政區邊界與地名資料
- **授權連結**: https://creativecommons.org/licenses/by/3.0/igo/
- **資料來源**: https://data.humdata.org/dataset/cod-ab-tha

**授權聲明**:

> 本專案使用來自 [Humanitarian Data Exchange (HDX)](https://data.humdata.org/dataset/cod-ab-tha) 的泰國行政區邊界資料 (COD-AB)，該資料依 [Creative Commons Attribution for Intergovernmental Organisations (CC BY-IGO)](https://creativecommons.org/licenses/by/3.0/igo/) 授權提供。

---

## 7. 印尼地理空間資訊局（BIG，Badan Informasi Geospasial）

- **來源**: BIG 官方 ArcGIS REST 圖徵服務（FeatureServer，desa 村級圖徵）
- **資料集**: 印尼行政區 desa（村級）邊界，版本 TASWIL20230928，含 38 省全量資料
- **授權**: 印尼官方公開地理資料
- **使用方式**: 印尼地區行政區邊界與地名資料；本專案僅將其作為衍生加工輸入，輸出經反向地理編碼最佳化後的地名與代表座標資料，不散布、不重新發行原始向量圖資（polygon 邊界）
- **資料來源**: https://www.big.go.id/

**授權聲明**:

> 本專案使用印尼地理空間資訊局（[Badan Informasi Geospasial, BIG](https://www.big.go.id/)）官方發布的行政區邊界資料（版本 TASWIL20230928）作為衍生加工輸入，不散布原始向量圖資；原始圖資請逕向 BIG 官方服務取得。

---

## 8. 國家教育研究院《外國地名譯名》

- **來源**: 國家教育研究院《外國地名譯名》
- **資料集**: 政府資料開放平臺 dataset 15211
- **授權**: 政府資料開放授權條款-第1版 (OGDL 1.0，與 CC BY 4.0 相容)
- **使用方式**: 全球非 handler 地區（臺灣、日本、南韓、泰國、印尼以外）城市與行政區的官方臺灣譯名補洞與覆寫
- **授權連結**: https://data.gov.tw/license
- **資料來源**: https://data.gov.tw/dataset/15211

**授權聲明**:

> 本專案使用來自[國家教育研究院《外國地名譯名》](https://data.gov.tw/dataset/15211)的開放資料，該資料依[政府資料開放授權條款-第1版](https://data.gov.tw/license)授權提供。

---

## 9. Natural Earth

- **來源**: Natural Earth（透過 nvkelso/natural-earth-vector 發布）
- **資料集**: `ne_10m_admin_0_countries`（1:10m Admin 0 – Countries）
- **授權**: 公眾領域（Public Domain）
- **使用方式**: 國家邊界資料；該檔會原樣包含在本專案發布的 release 中
- **資料來源**: https://www.naturalearthdata.com/ 、 https://github.com/nvkelso/natural-earth-vector

**授權聲明**:

> Made with Natural Earth. Free vector and raster map data @ naturalearthdata.com.

---

## 10. Wikidata

- **來源**: Wikidata
- **授權**: Creative Commons CC0 1.0 Universal（公眾領域貢獻宣告）
- **使用方式**: 取得南韓、泰國、印尼行政區的繁體中文名稱，並以 P131（行政隸屬）逐級驗證譯名歸屬
- **授權連結**: https://creativecommons.org/publicdomain/zero/1.0/
- **資料來源**: https://www.wikidata.org/

---

## 11. 中文維基百科

- **來源**: 中文維基百科（zh.wikipedia.org）
- **授權**: Creative Commons Attribution-ShareAlike 4.0 (CC BY-SA 4.0)
- **使用方式**: 在 Wikidata 無適用標籤時，以條目標題與繁簡轉換 API 取得行政區的繁體中文名稱
- **授權連結**: https://creativecommons.org/licenses/by-sa/4.0/
- **資料來源**: https://zh.wikipedia.org/

**授權聲明**:

> 本專案自中文維基百科取用地名短語作為行政區譯名，內容依 [CC BY-SA 4.0](https://creativecommons.org/licenses/by-sa/4.0/) 提供。

---

## 12. i18n-iso-countries

- **來源**: [node-i18n-iso-countries](https://github.com/michaelwittig/node-i18n-iso-countries)（npm 套件 `i18n-iso-countries`）
- **授權**: MIT License，Copyright (c) 2016 widdix GmbH
- **使用方式**: 本專案修改 `langs/` 底下的語系檔（主要為 `en.json`），使 Immich 顯示繁體中文國名，並將修改後的副本包含在發布的 release 中
- **授權連結**: https://github.com/michaelwittig/node-i18n-iso-countries/blob/master/LICENSE
- **資料來源**: https://www.npmjs.com/package/i18n-iso-countries

**授權聲明**:

> 本專案散布的 `i18n-iso-countries` 為修改後的版本，原始套件依 MIT License 提供，授權全文包含於 `i18n-iso-countries/LICENSE`。

---

## 13. 其他參考資料

### 國家/地區中文譯名參考

- **中華民國經濟部國際貿易署**: 國家/地區中文名稱參考
- **中華民國外交部**: 國家/地區中文名稱參考

---

## 授權條款全文

如需查閱各授權條款的完整內容,請參考以下連結:

- **ODbL 1.0**: https://opendatacommons.org/licenses/odbl/1-0/
- **CC-BY 4.0**: https://creativecommons.org/licenses/by/4.0/legalcode
- **CC BY-IGO 3.0**: https://creativecommons.org/licenses/by/3.0/igo/legalcode
- **政府資料開放授權條款-第1版**: https://data.gov.tw/license
- **CC BY-SA 4.0**: https://creativecommons.org/licenses/by-sa/4.0/legalcode
- **CC0 1.0**: https://creativecommons.org/publicdomain/zero/1.0/legalcode
- **MIT License**: https://opensource.org/licenses/MIT

---

## 免責聲明

本專案對所使用的第三方資料來源表達最大的敬意與感謝。本專案依據各資料來源的授權條款合法使用資料,並已盡力確保所有署名與聲明的完整性。如有任何授權相關問題,請透過 GitHub Issues 聯繫我們。

本專案的程式碼與文件採用 GNU General Public License v3.0 (GPL-3.0) 授權,但所使用的第三方資料受各自授權條款約束。使用本專案時,請確保遵守所有相關的授權要求。

本專案發布的 Rust CLI binary 另包含編譯期相依套件,其授權條款以各套件的宣告為準,清單見 `Cargo.toml` 與 `Cargo.lock`。

---

**最後更新日期**: 2026-08-23
