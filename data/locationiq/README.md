# data/locationiq

非 handler 國家的 LocationIQ 逆地理查詢產物與設定。

| 檔案 | 內容 |
| :--- | :--- |
| `{國碼}.csv` | 逐點查詢結果（ISO-3166-1 alpha-2，大寫） |
| `address_fields.json` | 各國要從回應的哪個欄位取城市名 |
| `README.md` | 本檔 |

由 `immich-geodata locationiq --country-code <CC>` 產生。內容以
`(latitude, longitude)` 去重，重跑時自動跳過已查座標。查詢會消耗 LocationIQ
的付費額度，因此這些檔案納入 git 追蹤，讓進度跨執行存活。

目前只有馬來西亞（`MY.csv`）走此流程。TW/JP/KR/TH/ID 由官方圖資 handler 產生，
產物位於 `data/handler/`。

**本目錄的檔案不可全數刪除。** release workflow 以
`file_pattern: data/handler/* data/locationiq/*` 提交查詢進度，glob 匹配不到
檔案時 `git add` 會失敗（exit 128）。本目錄必須至少有一個會被 `*` 匹配的追蹤
檔案，dotfile 不算。

## 城市名取哪一個行政層級

Immich 只渲染三個字串：country / state / city。state 固定取 GeoNames admin1。

**city 是「包含該座標的行政單位」的名稱，取一般使用者會用來說明自己在哪裡的
最細層級。** handler 國家以官方多邊形做 point-in-polygon 決定這個單位；非 handler
國家沒有官方圖資，只能以 LocationIQ 的回應逼近。

既有 handler 的實際選擇：

| 國 | admin1（→ state） | city | 單位數 |
| :--- | :--- | :--- | ---: |
| TW | 縣市 | 鄉鎮市區 | 357 |
| JP | 都道府県 | 市町村 | 1,716 |
| KR | 시도 | 시군구 | 235 |
| TH | 府 | amphoe | 913 |
| ID | 省 | kabupaten/kota | 514 |

**多個座標共用一個 city 名是設計，不是缺陷。** 印尼平均 211 個座標共用一個
kabupaten 名、日本 73 個、臺灣 22 個。把撞名率當品質指標會判定所有 handler 國家
都嚴重缺陷。真正的缺陷是**名字跨出該座標所屬的單位**。

**不要綁定 GeoNames 的層級編號。** 同一個 `admin2_code` 在各國是完全不同的東西：
JP 的市町村、KR 的시군구、ID 的 kabupaten/kota 是自治體，但 DE 的 Regierungsbezirk
只有 19 個且部分邦沒設，FR 的 département 是行政分區而非地名。

### admin1 失效時先修 admin1

GeoNames 的 TW admin1 是已廢止的省（只有 4 個值）。TW handler 因此把整體下推
一級，使 admin1 = 縣市、city = 鄉鎮市區。**這是 admin1 的規則，不是 city 的**：
先確認 admin1 有實質意義，再決定 city。

同類情況：GB 的 admin1 是英格蘭／蘇格蘭等 5 個構成國，當 state 過粗。尚未處理。

## address_fields.json

指定各國要從 LocationIQ 回應的哪些欄位取城市名，依序取第一個有值者：

```json
{
  "MY": {
    "city_keys": ["district", "city", "county"]
  }
}
```

可用的 key：`district`、`city`、`county`、`suburb`、`neighbourhood`、`state`。

### 為什麼是逐國設定

LocationIQ 的 address 物件直接反映 OSM 的標記慣例，各國的第二級行政區落在不同
的 key。實測 5 國：

| 國 | 第二級行政區在哪個 key | 備註 |
| :--- | :--- | :--- |
| MY | `district`（daerah） | 出現率 80%、中文率 100%；砂拉越／沙巴無 `district`，該處的 `city` 就是當地的縣 |
| IT | `county`（provincia） | 中文（卡塔尼亞） |
| GB | `city`（local authority） | 中文（卡迪夫） |
| VN | **無** | `city` 是省級直轄市（岘港市），`town` 才是 commune 但為越南文 |
| PH | **無** | 三個 key 皆無，`town` 是 municipality 但為英文 |

寫死任何一條順序都會在某些國家取得錯誤層級，因此不提供預設值：**設定檔沒有
登記的國家，locationiq 階段在發出任何查詢之前中止**，錯誤訊息附抽樣指令。

### 查詢參數

請求一律帶 `normalizeaddress=0` 與 `normalizecity=0`，兩者都不可開啟：

- `normalizeaddress=1` 回傳固定欄位清單，**其中不含 `district`**。實測同一座標
  開啟後 `district` 整個消失。
- `normalizecity=1` 在 `city` 缺席時，依序把 `city_district`、`locality`、`town`、
  `borough`、`municipality`、`village`、`hamlet`、`quarter`、`neighbourhood` 的
  第一個有值者提升成 `city`。九個層級塞進同一欄，層級無法預期——馬來西亞實測，
  同一個 daerah 內最多出現 9 種不同名字。

### state 絆線

`state` 欄位缺席時一律不產生城市名。越南的回應沒有 `state`，其 `city` 是省級
直轄市，100% 有值且 100% 是錯的層級；沒有上界的優先鏈會把 admin1 當成城市名
寫出去。

**這只是絆線，不是保證。** `state` 存在但優先鏈的第一個命中偏粗的情況無法結構性
偵測——LocationIQ 不提供 `admin_level` 或 `place_rank`（`addressdetails`、
`extratags`、`namedetails` 皆無）。真正的防線是下一節的逐國抽樣。

### 空白優於錯層級

優先鏈全數落空時，該列的城市名留白，translate 階段退回 GeoNames 原名——與沒跑
過 LocationIQ 的結果相同。錯層級則會靜默污染，因此**寧可留白也不要為了補覆蓋率
而延長優先鏈**。

馬來西亞 19,903 點中有 695 筆（3.5%）留白，集中在 OSM 未標 `district` 的地區：
玻璃市 221（該州只有一個 daerah）、登嘉樓 184、沙巴 134、砂拉越 116。三類補法
都經過評估後放棄：

- `region`：在砂拉越是省／division（加帛省、美里省），比縣粗一級；在沙巴多數
  也是 division（内陆省、斗湖省），少數才是縣（吧巴 = Papar）。**同一個 key 在
  同一國指向兩個層級，單一優先鏈表達不了**，且塌到 division 時 `state` 絆線擋
  不住（加帛省 ≠ 砂拉越州）。
- `state_district`：只在部分沙巴點出現，值為 `West Coast Division`，是英文的
  division。
- `village` / `town`：玻璃市、登嘉樓、吉隆坡的留白點只有這兩者，屬聚落級，且
  中文率僅 29%／12%。

注意「`district` 與 `region` 在馬來西亞從不共存」（100 點中 83 筆只有 district、
8 筆只有 region、9 筆兩者皆無）這項觀察，只證明補上不會動到已有值的列，**沒有**
證明補上之後拿到的是正確層級。

### 改動設定後必須重查

`city_keys` 一改，該國既有列的城市名語意就跟新查的列不一致。**在本地以
`--overwrite` 重查該國再提交**；CI 只做增量，不會修正既有列，也不得帶
`--overwrite`。

## 新增國家的評估流程

### 1. 抽樣看該國的回應長什麼樣

取該國任意座標，關閉兩個正規化參數，觀察完整的 address 物件：

```bash
curl -s "https://us1.locationiq.com/v1/reverse?lat=<lat>&lon=<lon>\
&format=json&accept-language=zh,en&normalizeaddress=0&normalizecity=0\
&key=$LOCATIONIQ_API_KEY" | jq .address
```

找出裝著該國第二級行政區、且值為中文的 key。找不到就是不採用該國——越南與
菲律賓都卡在這一步。

### 2. 驗收

取同一個 GeoNames `admin2_code` 內的多個座標（建議 5～6 個單位、各 10 點），
量兩項：

| 指標 | 合格條件 |
| :--- | :--- |
| **一致率**：同一單位內的座標是否取得同一個名字 | 越高越好 |
| **覆蓋率**：優先鏈取得非空值的比例 | 越高越好 |

馬來西亞實測（60 點、6 個 daerah）：

| | 舊設定（`normalizecity` 提升鏈） | `district` → `city` → `county` |
| :--- | ---: | ---: |
| 覆蓋率（100 點） | 84 | **93** |
| 一致的 daerah | **0/6** | **4/6** |

全量重查後（19,903 點）以新舊 CSV 重疊的 775 座標複驗，趨勢一致：同一 daerah
內名字一致的單位由 42/139 升到 107/136，其中有 3 點以上的單位由 8/84 升到
60/83；覆蓋率 96.5%，143/144 個 daerah 有對應名字。

`admin2_code` 在此只用來**分組**，不當 ground truth。它會過時——馬來西亞的
金寶（Kampar）2011 年自近打（Kinta）分出、武吉玛邦 2015 年自加帛分出，GeoNames
皆仍用舊碼。因此不一致的組要逐筆人工判斷是 LocationIQ 錯還是上游過時，上述
2 個不一致的 daerah 就都屬後者。

**不要量「中文回應率」，也不要量撞名率。** 前者量的是有沒有翻譯、不是對不對；
後者會判定所有 handler 國家都嚴重缺陷。

### 3. 登記與 workflow

1. 在 `address_fields.json` 加入該國與其 `city_keys`
2. 在 `release.yaml` 與 `auto-update.yaml` 的 `--country-code` 加上該國，
   並在 step 層的 `env:` 注入 `LOCATIONIQ_API_KEY`
3. 在本地跑滿該國再提交 CSV，CI 只做增量
4. 更新本檔的「已評估國家」

移除國家（例如改用 handler）與其餘 workflow 細節見 `CLAUDE.md` 的
「非 handler 國家的維護規則」。

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

## 已評估國家

下表的百分比是**舊指標**（中文回應率），且是在 `normalizecity=1` 讀 `city` 的
舊設定下量的，保留作為歷史紀錄。

| 國家 | 中文回應率（舊指標） | 結論 |
| :--- | :--- | :--- |
| 馬來西亞 MY | 86%（791 點全量） | **採用**。OSM 的 `name:zh` 是當地通用華文地名（八打靈再也、民丹莪、萬里茂），權威性高於機器翻譯。 |
| 土耳其 TR | 67%（15 點） | 放棄。全數為 ilçe 轄區，如 Bostanbükü → 番紅花城。 |
| 義大利 IT | 53%（15 點） | 放棄。**結論待複驗**——新設定下 `county` 是中文的 provincia（卡塔尼亞），落在正確層級。 |
| 越南 VN | 52%（40 點） | 放棄。**跨 admin1**——Núi Thành 在廣南省，卻取得直轄市峴港的名字。新設定下亦無可用 key。 |
| 美國 US | 30%（30 點） | 放棄。 |
| 西班牙 ES | 30%（30 點） | 放棄。 |
| 澳洲 AU | 20%（30 點） | 放棄。 |
| 英國 GB | 18%（100 點） | 放棄。NAER 與 GeoNames 中文別名已覆蓋 27.9%。**結論待複驗**——新設定下 `city` 是中文的 local authority（卡迪夫）。 |
| 紐西蘭 NZ | 17%（30 點） | 放棄。 |
| 荷蘭 NL | 13%（30 點） | 放棄。 |
| 菲律賓 PH | 10%（30 點） | 放棄。三個 key 皆無，回應全為英文。 |
| 柬埔寨 KH | 低（30 點） | 放棄。回應幾乎全為英文。 |
| 瑞士 CH | 0%（30 點） | 放棄。 |

LocationIQ 的價值來自華人社群的 OSM 標記密度。華人圈以外的國家，中文覆蓋通常
不足；標記較密的國家值得依前一節的流程重新評估。

## metadata 優先序的實測依據

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
