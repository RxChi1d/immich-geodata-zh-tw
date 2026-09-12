# data/locationiq

非 handler 國家的 LocationIQ 逆地理查詢產物，檔名為 `{國碼}.csv`
（ISO-3166-1 alpha-2，大寫）。

由 `immich-geodata locationiq --country-code <CC>` 逐點查詢產生。內容以
`(latitude, longitude)` 去重，重跑時自動跳過已查座標。查詢會消耗 LocationIQ
的付費額度，因此這些檔案納入 git 追蹤，讓進度跨執行存活。

## 額度與節流

免費方案同時有三條限制：**2 req/s、60 req/min、5,000 req/day**。前兩條互相
矛盾——照 2 req/s 打滿是 120 req/min，必然撞上分鐘上限，因此節流以較嚴的
60 req/min 為準（`--locationiq-qps` 預設 1，間隔 1020 ms ≈ 58.8 req/min）。

額度用完時（HTTP 429，response body 為 `Rate Limited Second` / `Minute` /
`Day`）行為依情境分開：

| 情境 | 旗標 | 行為 |
| :--- | :--- | :--- |
| 本地補查 | 預設 | 保留已查結果後**中止**，不讓半套資料往下發布 |
| CI 增量補查 | `--locationiq-allow-partial` | 保留已查結果後**正常結束**，nightly 照常發布，剩餘座標留待下次 |

`--locationiq-allow-partial` 只容忍「有推進但沒查完」。第一筆就被限速代表這一輪
零進度（金鑰失效、帳號被限制、或當日額度已被其他執行用光），一律失敗——否則
nightly 會照發、auto-commit 因無變更而不開 PR，整條補查路線沉默停擺。

本地跑滿一個國家的作法是重跑同一道指令——已查座標會自動跳過，所以換一把
金鑰或等額度重置後接續即可：

```bash
cargo run --release -- release --country-code MY --locationiq-api-key <key>
```

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

## admin1 修正與 `{CC}_admin1_fixes.csv`

LocationIQ 回應的 `admin_1` 用來修正 GeoNames 標錯的一級行政區。判定方式是
以 Natural Earth 10m admin-1 的 `gn_id` 對接 `admin1CodesASCII` 第 4 欄，學出
「LocationIQ 行政區名稱 → admin1 代碼」的映射，再要求 NE 與 LocationIQ 指向
同一個代碼才修正。

**不要改用 NE 的 `gn_a1_code` 欄位。** 它直接寫著代碼看似省事，但實測全球有
42 筆與 `admin1CodesASCII` 不符（越南最嚴重）。`gn_id` 是 GeoNames 永久識別碼，
且以 ID 對接時，上游把行政區名稱寫錯也不影響結果。

**目標代碼查不到就跳過，絕不自造 admin1 記錄。** geonameid 是全球共用識別碼，
自造的 ID 可能撞上上游現有的記錄，或撞上上游日後新增的記錄——後者今天沒事，
下次更新才爆，且沒有任何訊號。

### 兩個配套機制目的不同

| 機制 | 作用 | 擋得住壞資料嗎 |
| :--- | :--- | :--- |
| `{CC}_admin1_fixes.csv` | 留下紀錄，可跨週 diff | 否 |
| 修正量 5% 上限 | 異常時直接中止 build | 是 |

`auto-update.yaml` 的順序是「建置 → 發布 nightly → 提交檔案 → 開 PR」，nightly
在任何人看到 PR 之前就出貨了，所以人工 review 不是關卡。真正的煞車是
`admin1_apply` 的 5% 上限。MY 實測修正量為 8/740 = 1.08%。

紀錄檔是**唯寫**的：程式永遠不讀回它。手動編輯不會有任何效果，下次執行就被
覆蓋。發現修正有誤時，要改的是判定條件，不是這個檔案——一旦程式會讀它，它就
變成人工核准清單，把這條自動路線變成半手動。

檔案只收「任一可判定來源與原值不符」的列，含採納與拒絕。兩來源都同意原值的
點不收——它們要先變成分歧列才可能出事，而那本身就是一列新增；全部收進來只會
讓該看的列淹沒在雜訊裡。

### 沒有距邊界門檻

曾有一版要求候選點距 NE 多邊形邊界 ≥2 km。實測在 MY 上，該規則的真陽性攔截數
為 0，卻砍掉 6 筆兩來源一致的修正（Setapak 0.31 km、SS2 1.55 km、Bandar Utama
1.15 km 等，其中 4 筆已人工查核為真實上游錯誤）。它想擋的 9 筆「NE 獨排眾議」
案例全部已由「兩來源必須一致」攔下。

Reason: GeoNames 標錯行政區的地方本來就集中在邊界——吉隆坡是嵌在雪蘭莪裡的
243 km² 飛地，八打靈再也整片貼著界線。用「離邊界遠」當安全條件，等於排除這個
功能存在的理由。`boundary_km` 仍寫進紀錄檔供診斷，但不參與裁決。

## 新增或移除國家

流程與 workflow 需要的修改見 `CLAUDE.md` 的「非 handler 國家的維護規則」。
