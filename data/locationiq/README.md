# data/locationiq

非 handler 國家的 LocationIQ 逆地理查詢產物，檔名為 `{國碼}.csv`
（ISO-3166-1 alpha-2，大寫）。

由 `immich-geodata locationiq --country-code <CC>` 逐點查詢產生。內容以
`(latitude, longitude)` 去重，重跑時自動跳過已查座標。查詢會消耗 LocationIQ
的付費額度，因此這些檔案納入 git 追蹤，讓進度跨執行存活。

目前只有馬來西亞（`MY.csv`）走此流程。TW/JP/KR/TH/ID 由官方圖資 handler 產生，
產物位於 `data/handler/`。

**本檔案不可刪除。** release workflow 以
`file_pattern: data/handler/* data/locationiq/*` 提交查詢進度，glob 匹配不到
檔案時 `git add` 會失敗（exit 128）。本目錄必須至少有一個會被 `*` 匹配的追蹤
檔案，dotfile 不算。

## 選國準則

locationiq 階段取 Nominatim 回應的 `city` 當城市名，`city` 為空時退回 `county`。
決定成敗的不是中文回應率，而是**回應的粒度是否為聚落**。OSM 聚落標記稀疏的
國家會退回行政區，城市名就變成轄區名。

新增國家前抽樣 30～40 點，同時統計中文回應率與粒度，兩者都合格才採用。

### 已評估國家

| 國家 | 中文回應率 | 結論 |
| :--- | :--- | :--- |
| 馬來西亞 MY | 86%（791 點全量） | **採用**。OSM 的 `name:zh` 是當地通用華文地名（八打靈再也、民丹莪、萬里茂），權威性高於機器翻譯。453 筆城市名由英文轉中文。 |
| 土耳其 TR | 67%（15 點） | 放棄。全數為 ilçe 轄區，如 Bostanbükü → 番紅花城。 |
| 義大利 IT | 53%（15 點） | 放棄。多數塌到 comune，且為機械音譯。 |
| 越南 VN | 52%（40 點） | 放棄。填補會指到數十公里外的省會，如 Núi Thành → 峴港市。 |
| 美國 US | 30%（30 點） | 放棄。 |
| 西班牙 ES | 30%（30 點） | 放棄。 |
| 澳洲 AU | 20%（30 點） | 放棄。 |
| 英國 GB | 18%（100 點） | 放棄。82% 回應為英文而被丟棄，其餘多為轄區層級，如 Yelverton → 西德文區。NAER 與 GeoNames 中文別名已覆蓋 27.9%。 |
| 紐西蘭 NZ | 17%（30 點） | 放棄。 |
| 荷蘭 NL | 13%（30 點） | 放棄。 |
| 菲律賓 PH | 10%（30 點） | 放棄。 |
| 柬埔寨 KH | 低（30 點） | 放棄。回應幾乎全為英文。 |
| 瑞士 CH | 0%（30 點） | 放棄。 |

LocationIQ 的價值來自華人社群的 OSM 標記密度。華人圈以外的國家，中文回應率與
粒度都不足。

### metadata 優先序的實測依據

`translate_cities_rows` 讓 GeoNames 中文別名優先於 LocationIQ metadata。以
馬來西亞 791 點做 A/B：

| | metadata 優先 | metadata 補位（現行） |
| :--- | :--- | :--- |
| 英文轉中文 | 453 | 453 |
| 既有中文名被改寫 | 70 | 1 |

metadata 優先時被改寫的例子：蕉賴 → 吉隆坡，浮羅山背／丹絨武雅／丹絨道光／
壟尾／武吉佔姆 → 喬治市。改為補位後增益完全保留，回歸幾乎消失。

## 新增或移除國家

流程與 workflow 需要的修改見 `CLAUDE.md` 的「非 handler 國家的維護規則」。
