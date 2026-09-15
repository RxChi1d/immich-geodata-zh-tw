# 印尼 kecamatan 繁中譯名對照表

`kecamatan_zh.csv` 供 `src/pipeline/extract/indonesia_kecamatan.rs` 於 extract
階段查表，替 `admin_3`（Immich 顯示的城市名）補上中文。查無譯名時回退 BIG
官方印尼文——**多數 kecamatan 本來就沒有中文名，那是預期結果，不是缺陷**。

## 欄位

| 欄位 | 說明 |
| :--- | :--- |
| `name` | BIG 圖資 `WADMKC` 原文，查表的 key |
| `name_zh` | 繁中譯名（handler 端會再過一次安全簡轉繁） |
| `latitude` / `longitude` | 該行政單位的 medoid（離平均最近的**實際**代表點） |
| `source` | `naer` / `wikidata` / `osm` |

**查表要同時比對名稱與座標**（門檻 15 km）。kecamatan 名稱在全國不唯一——
`Bandung` 既是西爪哇的萬隆市，也是數個郡的名字——只比對名稱會把萬隆的譯名
套到別省的同名郡上。因此**同名的不同行政單位各自一列**（286 個相異名共 329 列），
handler 端的 lookup 在同名多筆中取最近者。

座標必須是**實際存在的代表點**，不能用平均值。Amahai 的 30 個代表點散布在馬魯古
的離島之間，平均座標離最近的點 98 km，15 km 驗證因此全數落空——最初版用平均座標
時有 49 筆譯名完全沒套用。改存 medoid 後距離為 0 必定命中，再由 handler 的逐單位
判定把名字傳給整組列。

## 為什麼需要這張表

Immich 顯示的城市名取自 kecamatan（理由見
[City 層級的選擇條件](../../../docs/zh-tw/city-level-criteria.md)），但既有的
Wikidata translator 拿不到這一層的譯名：它以 `P31` 類別白名單過濾候選，而中文
標籤多半掛在**同名的聚落實體**上而非 kecamatan 實體（`乌布` 在 `P31=town` 的
`Q210654`，kecamatan `Q3274172` 沒有中文標籤）。

NAER 譯名表也用不上——`naer_lookup.rs` 對 handler 國家一律跳過，因為 handler
名稱來自官方圖資屬權威值，而 NAER 按名稱比對會誤配（`Alas` 會對到「阿拉斯海峽」）。

## 來源與優先序

**NAER（國教院官方）→ Wikidata → OSM `name:zh`**

| 來源 | 收錄條件 |
| :--- | :--- |
| NAER | 名稱正規化相符 + 座標 15 km 內 + 排除自然地物（`feature_hint`） |
| Wikidata | 原文 label **全等** + `(wdt:P131)+` 遞移驗證到所屬 kabupaten |
| OSM | `name:zh` + 座標 15 km 內，**且前兩者皆無**，**且該中文字串本身出現在 Wikidata 同名實體的中文標籤中**（比對前去層級字尾） |

Wikidata 用「原文 label 全等」而非 `P31` 類別白名單，是比照南韓 handler 的
作法——類別白名單會擋掉 `乌布`（`P31=town`），而全等比對讓車站、機關、選區
自然落選，正確性交給 P131 驗證。P131 也會自動擋掉語言與族群實體
（`Banjar → 班查語` 沒有 P131）與上一層行政區（`Bandung → 萬隆縣` 的 P131
指向省而非 kabupaten）。

### OSM 為什麼排在最後且需要佐證

OSM 的印尼 `name:zh` 品質兩極：爪哇、加里曼丹、蘇門答臘等華人聚居久的地方是
真正通行的地名（喃吧哇、嘉薄棉、日巴拉），峇里島則有一批只存在於 OSM 的機器
產物。最明確的證據是 `Sidemen → 伴奏者`——那是把英文樂手術語丟進翻譯器的結果，
不是地名；同一批還有 `Abiansemal → 阿比安塞玛尔`、`Banjarangkan → 班加兰坎`
這類逐音節硬轉，在 Wikidata 與 NAER 都查無此名。

因此 OSM 只在其他來源都沒有時才採用，且**佐證要逐字**：OSM 的中文字串本身要
出現在 Wikidata 同名實體的中文標籤中。實測 89 個 OSM 獨有名稱中 18 個通過。

`Sidemen` 正是「存在性佐證不夠」的反例——Wikidata 上確實有它的中文名
（`喜德門`），所以任何只檢查「有沒有中文」的規則都會放行，但 OSM 的字串
（`伴奏者`）仍然是錯的。逐字比對才擋得住。

比對前要去層級字尾，否則同一個名字會被誤判為衝突：`卡朗阿森` 與
`卡朗阿森县`、`克隆孔` 與 `克隆孔县` 都是同一個地方。去字尾後仍不一致的
12 筆全數剔除，包含 `Sidemen`（伴奏者 vs 喜德門）與 `Lubuk Pakam`
（吧敢 vs 朗塞斯顿——後者是澳洲的朗塞斯頓，Wikidata 那筆本身就是錯配）。

## 重建方式

表由離線腳本產生，不在 extract 當下打外部 API，避免譯名隨 Wikidata／OSM 當下
狀態漂移。重建需要：

1. `data/handler/id_geodata.csv` 的 `admin_3` 欄（kecamatan 原文與座標）
2. Wikidata SPARQL（WDQS）
3. Overpass API（OSM）
4. `data/vendor/naer/naer_place_names.csv`

不需要 BIG 原始圖資。改表之後要重跑 `extract --country ID` 才會寫進
`data/handler/id_geodata.csv`，該步驟才需要圖資。

## 現況

329 列、286 個相異 kecamatan 名，來源分布 Wikidata 175、NAER 127、OSM 27
（同名多列會重複計入來源）。實際輸出 286/286 全數套用，零遺漏。

覆蓋集中在會拍照的地方：

| 省 | kecamatan 有中文 | 點覆蓋 |
| :--- | ---: | ---: |
| 雅加達 | 39/44 | 249/417（59.7%） |
| 巴釐省 | 15/57 | 266/793（33.5%） |
| 西加里曼丹省 | 43/173 | 632/2,391（26.4%） |
| 中爪哇省 | 30/553 | 708/9,085（7.8%） |
| 全國 | **286/6,907** | **5,433/107,961（5.0%）** |

譯名必須是純中文：`Cileunyi區`、`Tangaran區`、`Tekarang區` 這類 Wikidata 的
「原文＋層級字」label 已在建表時剔除，handler 的 `is_valid_chinese_translation`
是第二道防線。

## 已知限制

- 覆蓋率低是資料現實：華文世界對外國地名的命名大致到第二級行政區為止，
  第三級沒有系統性的譯名。南韓的 읍면동、泰國的 ตำบล 同樣是 0%。
- 峇里島刻意犧牲覆蓋率換正確性。不採用未經佐證的機器音譯是明確裁決，
  寧可顯示 `Abiansemal` 也不顯示查無來源的音譯。
