---
name: immich-geodata-country-handler
description: 當使用者明確提出要為 immich-geodata-zh-tw 新增某個國家的 handler 時使用（例如「新增越南 handler」「加入菲律賓支援」「為某國寫 extract handler」），包含其前期規劃與研究。涵蓋來源選擇、schema 研究、座標與投影策略實驗、特殊行政結構盤點，以及 Wikidata translator 的搜尋語言研究、國家 QID 查證、P131 驗證設計與翻譯品質驗收。僅更新既有國家圖資或跑 release 時不適用（改用 immich-geodata-source-release）。
version: 1.0.0
license: MIT
---

# immich-geodata-zh-tw 新增國家 handler

此 project skill 沉澱 TW/JP/KR/TH 四個 handler 的實作經驗，用於新增
國家的資料處理 handler。整體步驟以 `CLAUDE.md`「擴充新國家」章節為準；
此 skill 補充各階段的決策方法論與必要實驗，避免重蹈過往踩過的坑。

## 事實來源

- `CLAUDE.md` 擴充新國家章節：handler 結構、geoname_id、schema 與 gates。
- `docs/zh-tw/{taiwan,japan,south-korea,thailand}-admin-processing.md`：
  各國欄位語意、名稱與座標策略的完整先例。
- `rust/src/pipeline/extract/handlers.rs`、`korea_wikidata.rs`、
  `thailand_wikidata.rs`、`types.rs`：既有實作參考。

## 階段一：來源與 schema 研究

1. **官方來源優先**：國家測繪機構（TW=NLSC、JP=MLIT N03）、權威社群
   彙整（KR=admdongkor）、國際組織（TH=OCHA COD-AB）。確認授權可用。
2. **先讀 schema 再寫程式**：盤點各級行政區欄位、語言欄位（官方英文/
   原文是否齊備，影響名稱策略的 fallback 設計）、官方代表點欄位。
3. **欄位 → admin 層級對應先做紙上設計**，並與既有 handler 交叉比對
   驗證合理性。JP 與 KR 是最佳對照組——兩者都是「多層municipal結構
   壓成 admin1/admin2」的案例：
   - JP：N03_001（都道府縣）→ admin1；N03_003（郡）/N03_004（市町村）
     /N03_005（政令市區）依結構壓成 admin2，並非欄位直接照搬。
   - KR：sidonm → admin1；sggnm（市郡區）→ admin2，世宗需特例。
   - 共同教訓：哪些欄位「有用」取決於 Immich 顯示需求（admin1/admin2
     兩級），不是來源有什麼就提取什麼；壓層邏輯要對照真實案例驗證。
4. **盤點特殊行政結構**（每個國家都有，先找出來再設計欄位對應）：
   - JP：郡/町同名（需 duplicate gun towns 消歧）、政令市單/雙層差異。
   - KR：世宗特別自治市無 admin2（需正規化特例）。
   - TH：曼谷 admin2 是 khet 不是 amphoe（instance-of 類別不同）。
   - 同名行政區（同國不同 parent 下）必須以 parent 鏈區分。

## 階段二：座標與投影策略（以實驗決定）

1. **座標策略**：Immich 用單點最近距離模型。若來源提供官方代表點，
   以「polygon 內取樣點當真實 GPS → 比較命中率」的實驗決定採用幾何
   中心或官方代表點（TH 先例：幾何中心 76.18% > 官方代表點 74.30%）。
2. **投影策略**：比較 dynamic UTM 與該國單一等積投影的 centroid 差異
   與命中率。差異在公尺級以下時，選簡單的單一投影（TH 先例：Albers
   與 dynamic UTM 命中率差 <0.002%，採 Albers）；國土東西跨度大時
   dynamic UTM 才有優勢（JP/KR 先例）。
3. multipart polygon 每個部分各出一列是預期行為，文件需說明列數
   與行政區數不一致的原因。

## 階段三：名稱策略

### 0. 先判斷是否需要 Wikidata translator

第一個分岔是「來源名稱能否直接作為中文輸出」：

- **漢字圈來源**（JP 漢字、TW 繁中）：名稱可直接採用或僅需輕量轉換，
  **不需要** Wikidata translator——JP handler 即為先例，重點工作落在
  欄位壓層與同名消歧，而非翻譯。
- **非漢字語系**（KR 韓文、TH 泰文/英文）：需要完整翻譯管線，
  進行以下必要研究。

### 1. 國家 QID 查證（hardcode，不做執行期查詢）

1. 以即時 API 查詢確認國家 QID 的 label（如 Q869=泰國、Q884=大韓民國）。
2. 寫入 handler 常數並附中文名註解。QID 為 Wikidata 永久識別碼，
   合併情境下舊 QID 仍會 redirect，hardcode 比執行期搜尋穩定。

### 2. 搜尋語言研究（必做，禁止憑直覺選擇）

主要搜尋語言「英文 vs 該國原文」必須以抽樣實驗決定，按層級分別判斷：

1. **取樣**：admin1 全取；admin2 取「結構性同名類別」全取（如泰國的
   เมืองX 首府縣）＋ 隨機 ≥50 個（固定 seed 以便重現）。
2. **對照測試**：對每個樣本分別以候選語言呼叫 `wbsearchentities`
   （limit=7），統計「正確或可 P131 驗證的實體出現在候選中」的比率。
3. **決策**：命中率高的語言為該層級主要語言；另一語言若能補回主語言的
   失敗案例，設計為驗證後備（fallback 必須配 P131＋instance-of 過濾）。
4. **先例數據**：KR 原文（韓文）搜尋即高鑑別度；TH 125 樣本實驗英文
   命中 100%、泰文僅 4–12%——原文不一定比較準，鑑別度取決於該國
   實體在 Wikidata 上的標籤慣例，必須實測。

### 3. P131 驗證設計（標準規則，不可繞過）

1. `TranslationDataset` 的 `country_qid` 為必填；admin1 對國家 QID、
   admin2 對所屬 admin1 QID 逐級驗證，無「盲取第一名」路徑。
2. 推行前以 WDQS 一次性驗證該國全部 admin1 可通過
   `(wdt:P131)+ <國家QID>`，避免標準規則使正確翻譯回退。
3. 若設計 fallback 搜尋，查證該國 admin1/admin2 的 instance-of 類別
   QID（以實體 P31 反查），作為候選過濾集。
4. 視需要加 keyword 排除 filter（KR 先例：議會、政府機關等非行政區
   實體常與行政區同名）。

### 4. 低層級行政區（admin3 以下）

預設不建 Wikidata cache（量大、歧義風險高、翻譯收益低），採官方英文
／原文回退（TH admin3 先例）。要翻譯需先做小樣本錯配率評估。

## 階段四：驗收

1. 真實資料 extract 後統計各級翻譯覆蓋率，抽查未翻譯項是
   「Wikidata 無中文標籤／無可驗證實體」而非程式缺陷。
2. 補上 fixture 與 Wikidata stub（如 `TH_wikidata_stub.json`），
   讓測試不依賴即時網路；執行 Rust gates。
3. 文件比照 `docs/zh-tw/<country>-admin-processing.md` 撰寫（含英文版），
   座標/投影/搜尋語言的選擇必須附上實驗數據。
4. 同步更新 CLI country parsing、handler routing 與
   `immich-geodata-source-release` skill 的來源清單。

## Guardrails

- Wikidata/Wikimedia 速率限制以 IP 計：一次只跑一個國家的
  extract／實驗，腳本必須尊重 `Retry-After`，否則數據會被 429 污染。
- 抽樣實驗結果與 seed 記錄於 PR 描述或文件，確保可重現。
- 不要把 `CLAUDE.md` 的步驟複製成第二份手冊；此 skill 只補充
  決策方法論與實驗流程。
