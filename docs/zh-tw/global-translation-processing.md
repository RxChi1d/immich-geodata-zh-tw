# 全球（非 handler 地區）翻譯處理邏輯

> 本文件說明本專案如何處理「非 handler 國家」（即臺灣、日本、南韓、泰國、
> 印尼以外的全球地區）的中文翻譯，是 README 中「其他地區」優化章節的詳細
> 版本。

## 背景：兩條翻譯路徑

本專案的地名翻譯依資料來源分為兩條獨立路徑：

- **handler 國家（TW / JP / KR / TH / ID，清單由 extract handler 單一
  來源決定）**：以官方圖資 handler 為核心，並使用 Wikidata translator
  （名稱搜尋 → zh / zh-tw / zh-hant label → P131 上級行政區鏈驗證消歧 →
  OpenCC 字元級簡繁轉換）產生高品質的繁中翻譯。臺灣用語來自 Wikidata 的
  zh-tw label 與 handler 內建對照表，OpenCC 只負責字元層級的簡轉繁。
  此路徑不在本文件討論範圍。
- **全球非 handler 地區（translate 階段）**：正式 release 只跑上述
  handler 國家，**LocationIQ 流程不在 production release 中**。因此全球
  其餘所有國家的 cities500 與 admin1 記錄，原本的中文名僅來自 GeoNames
  alternateNames 的 zh 系語言列與 cities500 內嵌中文，再經 OpenCC 轉換，
  **完全沒有 Wikidata 參與**。

### 非 handler 地區的實際基線

由於上述原因，非 handler 地區的翻譯涵蓋率遠低於一般直覺。2026-06 的全量
（非抽樣）實測結果如下：

| 層級 | 記錄數 | 現行有中文名（基線） |
|---|---:|---:|
| cities500（非 handler 全球） | 229,760 | 46,290（20.1%） |
| admin1（非 handler 全球） | 3,720 | 2,211（59.4%） |

這意味著全球絕大多數城市記錄在沒有額外翻譯來源時只能回退英文，這也是
本專案引入國教院（NAER）官方譯名作為補強層的原因。上表為離線實測結果，
不隨程式碼自動驗證；重算需重跑 prepare 與 translate 階段。完整研究與
實測過程見
[中文地名翻譯來源替代方案評估](../research/chinese-translation-sources.md)。

## 翻譯優先序

非 handler 地區在 translate 階段的翻譯來源依以下優先序決定。NAER 的介入
位置依其**信心分級**而不同——高信心可覆寫既有譯名，中信心僅在其他來源
皆無結果時補洞，低信心則完全不使用。

### cities500（城市層級）

1. **NAER 官方譯名（高信心）**：覆寫層級插入優先序首位
2. LocationIQ metadata（既有；載入 `data/locationiq/{國碼}.csv`。handler
   國家的 `data/handler/{國碼小寫}_geodata.csv` 位於不同目錄，由 enhance 階段以
   明確檔名消費，不參與此處查表。正式 release 目前無非 handler 國家，因此此層
   為 no-op）
3. GeoNames alternateNames zh 系 + OpenCC（既有）
4. 內嵌中文 alternatenames（既有）
5. **NAER 官方譯名（中信心）**：僅在上述 2–4 皆無結果時補洞
6. fallback 原名（既有）

### admin1（一級行政區層級）

admin1 第一版採保守策略，NAER **僅補洞、不覆寫**：

1. GeoNames alternateNames（既有，優先序不變）
2. **NAER lookup**：僅在既有來源無中文名時使用
3. fallback（既有）

admin1 的匹配條件與 city 不同，下方的信心分級表不適用於此路徑：

- 候選國碼必須與 admin1 code 的國碼前綴完全一致，不接受空國碼候選。
- 以旗下 cities500 的平均座標為近似質心，距離門檻放寬為 300 km。
- 容差內若次近候選的譯名與最近者不同、且兩者距離差不到 5 km，直接放棄
  補洞。
- 不讀 `feature_hint`，也不計算信心等級——此路徑一律僅補洞，降權與否對
  結果沒有差別。
- 找不到國碼一致的候選屬常態（多數 admin1 不在 NAER 詞典中），不計入
  拒絕計數。

### 信心分級（confidence tiers）

NAER 在 cities500 命中後依下表判定信心等級（實作見
`src/pipeline/naer_lookup.rs`）：

| 等級 | 條件（全部成立） | 權限 |
|---|---|---|
| 高 | 國碼一致、距離 ≤ 15 km、`feature_hint=false`、無歧義（容差內唯一譯名，或最近與次近差 ≥ 5 km） | 覆寫既有譯名 + 補洞 |
| 中 | 命中但有任一弱化訊號：無國碼一致候選而改用空國碼候選、`feature_hint=true`、近距歧義 | 僅補洞（既有來源皆無中文名時才使用） |
| 低 | 國碼不符且無空國碼候選可降級、容差內無候選（距離 > 15 km）、座標畸形無法解析 | 拒絕 |

- admin1 第一版一律僅補洞，待品質報告量化錯配率後再評估開放覆寫。
- 容差常數 `NAER_CITY_DISTANCE_KM = 15.0` 的依據：NAER 座標精度為
  ±1 角分（約 2 km），加上城市中心點偏移緩衝；實測此容差下美國錯配率
  趨近 0。

## NAER 資料來源與授權

- **來源**：國家教育研究院《外國地名譯名》，政府資料開放平臺
  [dataset 15211](https://data.gov.tw/dataset/15211)
- **授權**：政府資料開放授權條款－第 1 版（OGDL 1.0），與
  CC BY 4.0 相容；發佈物需保留 attribution（見 [NOTICE.md](../../NOTICE.md)）
- **資料規模**：原始 dataset 64,487 筆、涵蓋 700 國家／地區（離線實測值，
  見[研究文檔](../research/chinese-translation-sources.md)）；經
  `naer-prepare` 清理後的 vendored 檔為 64,075 筆，差額 412 筆是座標不可
  解析、座標欄為空或名稱不可用而丟棄的列
- **vendored 檔**：清理後的 6 欄 CSV 存放於 `data/vendor/naer/naer_place_names.csv`，
  對應 192 個 ISO 3166-1 alpha-2 國碼，另有 1,798 筆國名未對應
  （`country_code` 留空）；欄位說明見
  [`data/vendor/naer/README.md`](../../data/vendor/naer/README.md)

## 為什麼選 NAER（研究結論摘要）

對非 handler 地區（佔 release 絕大多數記錄）而言，NAER 是效益最大的補強
來源。相對於 GeoNames alternateNames 基線，NAER 帶來以下效益（2026-06
離線實測，未隨程式碼驗證）：

- **cities500**：補洞 +16,380 筆（+7.1 pp，相對 +35%），覆蓋率自
  20.1% 提升至 27.3%
- **admin1**：補洞 +494 筆（+13.3 pp），覆蓋率自 59.4% 提升至 72.7%；
  救回包含 `Dubai → 杜拜`、`Andorra la Vella → 老安道爾` 等高曝光條目
- **品質覆寫潛力**：另有 9,415 筆 cities500 記錄現行雖有中文（多來自
  `zh` 簡體列經 OpenCC 機械轉換），但 NAER 同時有官方臺灣譯名，可作品質
  覆寫

選擇 NAER 而非其他候選來源的原因：

- **GeoNames `zh-Hant` 不足以作為繁體補洞來源**：全球 cities500 範圍內
  `zh-Hant` 僅 1,338 筆、`zh-TW` 僅 283 筆，填充率最高的美國也只有 3.3%。
- **OSM `name:zh-Hant` 授權排除**：ODbL 的 share-alike 條款會使批次萃取
  的譯名構成 Derivative Database，發佈時被迫採用同授權，與本專案發佈
  模式衝突；且其國外地名 `name:zh-Hant` 填充率稀疏。
- **Unicode CLDR 範圍排除**：僅涵蓋國家／地區層級顯示名稱，無城市級
  gazetteer。
- **樂詞網授權排除**：版權所有，未開放。

NAER 的核心優勢在於它是官方審譯標準（前國立編譯館經 220 次以上會議
審譯），品質優於 GeoNames 簡轉繁的機械轉換：疑似簡體字僅 0.03%、中文名
欄位無空值。完整評估見
[研究文檔](../research/chinese-translation-sources.md)。

## 為什麼採 runtime join 而非預先 crosswalk

NAER 譯名在 translate 階段以 **runtime 動態 join**（名稱正規化 + 座標
消歧）接入，而不是預先建立一份 NAER ↔ GeoNames 的 geonameid crosswalk
檔。原因：

- 本專案在 nightly auto-update 下，GeoNames cities500 每日都可能變動；
  預先 join 的 crosswalk 會持續過時，且需額外維護同步機制。
- runtime join 會在每次 release 時針對當下的 cities500 重新匹配，
  自動適應新增或變動的城市。
- vendored 的 `naer_place_names.csv` 對 GeoNames 零耦合（不含任何 GeoNames
  資料），git diff 可審查，更新時無需考慮 GeoNames 版本相依。

## 為什麼 admin1 僅補洞

admin1 是高曝光層級（每張照片的行政區顯示都會用到），且本專案目前**尚未
量化** NAER 在 admin1 的覆寫錯配率。在缺乏實測錯配率的情況下開放覆寫，
風險高於效益——若 NAER 覆寫了原本正確的 GeoNames 譯名而造成回退，影響
範圍大且難以察覺。

此外，admin1 的座標消歧只能以該 admin1 旗下 cities500 城市的平均座標
近似質心（`admin1CodesASCII.txt` 本身無座標），這個近似受城市分布不均
（離島、跨日期變更線）影響。在補洞模式下，即使消歧失敗也只是放棄補洞、
不會破壞既有譯名，錯配代價有限。

因此第一版一律僅補洞、不覆寫，待首次品質報告量化錯配率後再評估是否
開放高信心覆寫。

## vendored 檔更新流程

NAER 原始資料的下載與清理屬於**離線路徑**，不在 release path 上，僅在
資料來源更新時手動執行：

```bash
# 1. 自 opendata.naer.edu.tw 手動下載 dataset 15211 的最新 CSV
# 2. 執行 naer-prepare 子指令清理並輸出 vendored 檔
cargo run --release -- naer-prepare \
  --input <原始CSV路徑> \
  --output data/vendor/naer/naer_place_names.csv

# 3. 檢視統計報告與 git diff，確認無異常後 commit
```

`naer-prepare` 會完成下列前處理：

- **座標解析鏈**：HTML entity 還原 → 標籤移除 → 撇號字元統一 → 度分制
  轉十進位 → 範圍驗證（|lat| ≤ 90、|lon| ≤ 180）。實測成功率約 99.4%。
- **名稱正規化**（`name_norm`，匹配 key）：去除括號註記（圓括號、全形
  括號與方括號，含 `〔〕`）、取逗號前段、NFKD 變音符號折疊、小寫、
  壓縮連續空白。
- **中文名清理**（`name_zh`）：剝離同一組括號註記（`科魯涅(科倫納)` →
  `科魯涅`）。
- **國碼對應**（`country_code`）：以 i18n-iso-countries 的 zh-tw 對照表
  加 alias 表（如 `韓國→KR`、`剛果{金夏沙}→CD`）對應為 ISO 3166-1
  alpha-2；無法對應者留空（依信心分級降為中信心，僅補洞）。
- **自然地物啟發式**（`feature_hint`）：英文名含地物標記（`R.`、`Bay`、
  `Mt.`、`Cape`、`Island` 等）或中文譯名以地形字尾（河／灣／島／山／湖／
  角／峽 等）結尾時標記為 `true`。僅作降權依據、不刪列：
  `San Francisco → 舊金山` 這類字尾撞型的城市仍會被標記為 `true`，但只是
  降為中信心（僅補洞），不會從詞典中消失。

失敗列處理：

- **丟棄**：座標不可解析（`coordinate_failures`）、座標欄為空
  （`coordinate_empty`）、名稱不可用——正規化後 `name_norm` 或 `name_zh`
  為空、或 `name_zh` 含逗號（兩者皆計入 `name_failures`）。
- **保留**：國名未對應的列（`country_code` 留空）；座標解析成功但為
  (0,0) 的列僅計入 `suspicious_zero_coordinates`，不刪列。

輸出前依欄位排序以利 git diff 審查，並偵測同 `(name_norm, country_code)`
但相距不到 5 km 卻譯名不同的列，計入報告的 `conflicts`。

## 品質報告解讀

translate 階段會輸出單行 NAER 統計 log（驗收 gate 之一），以 `key=value`
形式呈現、空白分隔，方便 grep/awk 解析。完整欄位如下：

**採用計數**

- `city_fill`：city 既無中文名、NAER 補洞數。
- `city_override`：city 既有中文名、NAER 高信心覆寫數。
- `city_demoted_kept_existing`：city 既有中文名、NAER 為中信心 → 保留既有數。
- `admin1_fill`：admin1 既無中文名、NAER 補洞數。

**拒絕計數（依原因分類）**

- `city_rejected_distance`：有候選但座標全部超出 15 km 容差。
- `city_rejected_country`：有候選但國碼全不符、且無空國碼候選可降級。
- `admin1_rejected_no_centroid`：有候選但該 admin1 無質心（無轄下城市）可驗證。
- `admin1_rejected_distance`：有候選但質心驗證全部超過 300 km 門檻。
- `admin1_rejected_ambiguous`：距離合格但近距存在不同譯名、質心無法消歧。

> 註：handler 國家跳過與「name 完全無候選」為常態，不計入拒絕。

**距離分布摘要（被採用匹配的消歧距離，單位 km）**

- city：`city_dist_0_1km`（[0,1)）、`city_dist_1_5km`（[1,5)）、
  `city_dist_5_15km`（[5,15]）。
- admin1：`admin1_dist_0_1km`（[0,1)）、`admin1_dist_1_5km`（[1,5)）、
  `admin1_dist_5km_plus`（≥5；admin1 質心為近似值、容差較寬，超過 5 km
  一律歸入此桶以共用同一摘要結構）。

驗收時與離線實測的預期量級對照（來源同上節，尚未以品質報告 log 校準，
首次產出報告後應改以實際值為基準）：

- cities 補洞 ≈ 16,380
- cities 覆寫上限 ≈ 9,415（信心分級降級後實際值會更低，以首次品質報告
  確立基準）
- admin1 補洞 ≤ 494（名稱匹配估計值；加上國碼與質心驗證後會更低）

若實際數字偏離上述量級（例如補洞數遠低於預期，或覆寫數爆量），即為
品質警訊，應檢查 vendored 檔、正規化邏輯或信心分級條件是否異常，而非
直接放行。
