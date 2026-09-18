//! LocationIQ 回應的地址結構，與各國城市名取哪個 address 欄位的設定。
//!
//! 自 `locationiq.rs` 拆出——「回應長什麼樣、城市名該取哪個欄位」與
//! 「怎麼發查詢、怎麼續跑」是兩件獨立的事。

use std::collections::HashMap;
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct LocationiqAddress {
    pub country: String,
    pub state: String,
    /// OSM 的 `district`，在馬來西亞是 daerah（縣）。
    ///
    /// Reason: 這個欄位先前從未被讀取，卻是 MY 唯一穩定落在第二級行政區的來源
    /// （實測 40 點：出現率 80%、中文率 100%）。它不是跨國通用的——VN 沒有這個
    /// 欄位、IT 要讀 `county`、GB 要讀 `city`，因此讀哪個欄位由
    /// `address_fields.json` 逐國指定。
    pub district: String,
    pub city: String,
    pub county: String,
    pub suburb: String,
    pub neighbourhood: String,
}

impl LocationiqAddress {
    /// 依該國設定的優先鏈取第一個有值的欄位當城市名。
    ///
    /// Reason: `state` 缺席時一律回空字串。這是擋「城市名塌到 admin1」的絆線——
    /// 越南的回應沒有 `state`，其 `city` 是省級直轄市（岘港市），100% 有值且
    /// 100% 是錯的層級，沒有上界的優先鏈會把它當成城市名寫出去。
    ///
    /// Reason: 這只是絆線，不是保證。`state` 存在但鏈的第一個命中偏粗的情況無法
    /// 結構性偵測——LocationIQ 不提供 `admin_level` 或 `place_rank`（實測
    /// `addressdetails`／`extratags`／`namedetails` 皆無）。真正的防線是新增國家
    /// 時的逐國抽樣，流程見 `data/locationiq/README.md`。
    pub(super) fn city_name(&self, city_keys: &[String]) -> String {
        if self.state.is_empty() {
            return String::new();
        }
        city_keys
            .iter()
            .filter_map(|key| self.field(key))
            .find(|value| !value.is_empty())
            .cloned()
            .unwrap_or_default()
    }

    pub(super) fn field(&self, key: &str) -> Option<&String> {
        match key {
            "district" => Some(&self.district),
            "city" => Some(&self.city),
            "county" => Some(&self.county),
            "suburb" => Some(&self.suburb),
            "neighbourhood" => Some(&self.neighbourhood),
            "state" => Some(&self.state),
            _ => None,
        }
    }
}

/// `city_keys` 可以填哪些欄位；與 `LocationiqAddress::field` 的 match arm 同源。
const SUPPORTED_CITY_KEYS: [&str; 6] = [
    "district",
    "city",
    "county",
    "suburb",
    "neighbourhood",
    "state",
];

#[derive(Debug, Clone, Default)]
pub struct AddressFields(HashMap<String, Vec<String>>);

impl AddressFields {
    /// 讀取 `address_fields.json`。
    ///
    /// 格式：`{"MY": {"city_keys": ["district", "city", "county"]}}`
    pub fn load(path: &Path) -> Result<Self, String> {
        let body = fs::read_to_string(path)
            .map_err(|error| format!("無法讀取 LocationIQ 欄位設定 {}：{error}", path.display()))?;
        // Reason: 手動走訪 serde_json::Value 而不加 `serde` derive 相依。整份設定
        // 只有一層巢狀、一個欄位，derive 省下的程式碼不足以換一個新的直接相依。
        let parsed: serde_json::Value = serde_json::from_str(&body)
            .map_err(|error| format!("{} 不是合法 JSON：{error}", path.display()))?;
        let object = parsed
            .as_object()
            .ok_or_else(|| format!("{} 的最外層必須是物件", path.display()))?;
        let mut fields = HashMap::with_capacity(object.len());
        for (country, value) in object {
            let keys = value
                .get("city_keys")
                .and_then(serde_json::Value::as_array)
                .ok_or_else(|| format!("{} 的 {country} 缺少 city_keys 陣列", path.display()))?;
            let keys: Vec<String> = keys
                .iter()
                .map(|key| {
                    key.as_str().map(str::to_string).ok_or_else(|| {
                        format!("{} 的 {country}.city_keys 必須都是字串", path.display())
                    })
                })
                .collect::<Result<_, _>>()?;
            if keys.is_empty() {
                return Err(format!(
                    "{} 的 {country}.city_keys 不得為空",
                    path.display()
                ));
            }
            // Reason: 不認得的 key 必須在載入時就擋下。`city_name` 走訪優先鏈時會
            // 跳過取不到的 key，拼錯（例如 `distict`）或填了本階段沒解析的欄位
            // （例如 VN/PH 的 `town`）都會靜默變成整國城市名全空——而且是在把該國
            // 額度花完之後才看得出來。這與「未登記就大聲中止」是同一條原則。
            if let Some(unknown) = keys
                .iter()
                .find(|key| LocationiqAddress::default().field(key).is_none())
            {
                return Err(format!(
                    "{} 的 {country}.city_keys 含無法解析的欄位 {unknown}。\
                     可用欄位：{}",
                    path.display(),
                    SUPPORTED_CITY_KEYS.join("、")
                ));
            }
            fields.insert(country.clone(), keys);
        }
        Ok(Self(fields))
    }

    /// 取該國的城市名欄位優先鏈；未登記即中止。
    ///
    /// Reason: 不提供預設順序。給了預設，下一個接上來的國家就會默默拿到錯的層級
    /// 而沒有任何訊號——那正是本次修正要根除的失敗模式（MY 沿用寫死的
    /// `city` → `county` 長達整個 #77 週期，直到有人去量才發現）。
    pub(super) fn city_keys(&self, country: &str, config_path: &Path) -> Result<&[String], String> {
        self.0.get(country).map(Vec::as_slice).ok_or_else(|| {
            format!(
                "LocationIQ 未登記 {country} 的 city_keys。\n\
                 抽樣指令（取該國任一座標，確認第二級行政區落在哪個 key）：\n  \
                 curl -s \"https://us1.locationiq.com/v1/reverse?lat=<lat>&lon=<lon>\
                 &format=json&accept-language=zh,en&normalizeaddress=0\
                 &normalizecity=0&key=$LOCATIONIQ_API_KEY\" | jq .address\n\
                 確認後將該國加入 {}；完整流程見 data/locationiq/README.md。",
                config_path.display()
            )
        })
    }
}

/// 將 LocationIQ 回應轉為 geodata 列。
///
/// Reason: 有官方圖資 handler 的國家（TW/JP/KR/TH/ID）在 CLI 已由
/// `filter_country_codes_without_handler` 濾掉，不會進入本階段，因此這裡不做
/// 任何國家特化對應，城市名一律由 `city_keys` 的優先鏈決定。
pub(super) fn build_geodata_row(
    latitude: &str,
    longitude: &str,
    address: &LocationiqAddress,
    city_keys: &[String],
) -> Vec<String> {
    vec![
        latitude.to_string(),
        longitude.to_string(),
        address.country.clone(),
        address.state.clone(),
        address.city_name(city_keys),
        address.suburb.clone(),
        address.neighbourhood.clone(),
    ]
}

pub(super) fn parse_locationiq_address(body: &str) -> Result<LocationiqAddress, String> {
    let response: serde_json::Value = serde_json::from_str(body)
        .map_err(|error| format!("LocationIQ 回應不是合法 JSON：{error}"))?;
    let address = response
        .get("address")
        .ok_or_else(|| "LocationIQ 回應缺少 address 物件".to_string())?;
    let field = |key: &str| {
        address
            .get(key)
            .and_then(serde_json::Value::as_str)
            .map(primary_name_variant)
            .unwrap_or_default()
    };
    Ok(LocationiqAddress {
        country: field("country"),
        state: field("state"),
        district: field("district"),
        city: field("city"),
        county: field("county"),
        suburb: field("suburb"),
        neighbourhood: field("neighbourhood"),
    })
}

/// 取 OSM 名稱的第一個變體。
///
/// Reason: OSM 的 `name:zh` 有時同時塞入簡繁兩種寫法，英國全境即如此
/// （`"country":"\u82f1\u56fd;\u82f1\u570b"` → `英国;英國`）。原樣保留會讓
/// 行政區名變成「英国;英國」這種不可用字串；只取第一個變體，繁化交給
/// translate 階段既有的 OpenCC s2t 統一處理，與歷史資料（泰國存為「泰国」）一致。
fn primary_name_variant(value: &str) -> String {
    value.split(';').next().unwrap_or(value).trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// LocationIQ 以 `\uXXXX` 逃逸回傳非 ASCII 名稱，且英國全境的 OSM `name:zh`
    /// 同時含簡繁兩種寫法。此測試以真實回應片段固定兩種行為。
    #[test]
    fn parse_locationiq_address_decodes_unicode_escapes_and_takes_first_variant() {
        // 真實回應片段：LocationIQ 以 \uXXXX 逃逸輸出非 ASCII，直接寫中文字元的
        // 測試無法覆蓋解碼路徑。
        let body = r#"{"place_id":"279655027","display_name":"x","address":{"city":"Royal Wootton Bassett","county":"Wiltshire","state":"\u82f1\u683c\u5170;\u82f1\u683c\u862d","country":"\u82f1\u56fd;\u82f1\u570b","country_code":"gb"}}"#;
        assert!(
            body.contains(r"\u82f1"),
            "測試輸入必須是逃逸形式，否則等於沒測"
        );
        let address = parse_locationiq_address(body).unwrap();
        assert_eq!(address.country, "英国");
        assert_eq!(address.state, "英格兰");
        assert_eq!(address.city, "Royal Wootton Bassett");
        assert_eq!(address.county, "Wiltshire");
        assert_eq!(address.suburb, "");
        assert_eq!(address.neighbourhood, "");
    }

    #[test]
    fn parse_locationiq_address_rejects_invalid_json() {
        assert!(parse_locationiq_address("not json").is_err());
        assert!(parse_locationiq_address(r#"{"lat":"1"}"#).is_err());
    }

    fn keys(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| value.to_string()).collect()
    }

    fn malaysia_address() -> LocationiqAddress {
        LocationiqAddress {
            country: "马来西亚".to_string(),
            state: "雪兰莪州".to_string(),
            district: "乌鲁冷岳县".to_string(),
            city: "加影".to_string(),
            ..LocationiqAddress::default()
        }
    }

    #[test]
    fn build_geodata_row_keeps_response_admin_levels() {
        // Reason: 舊版對 TW 會把直轄市層級整列上移，TW 改由官方圖資 handler 產生後
        // 這個階段不得再做任何國家特化搬移，否則會與 handler 產出的層級不一致。
        let address = LocationiqAddress {
            country: "臺灣".to_string(),
            state: "Taipei".to_string(),
            city: "臺北市".to_string(),
            suburb: "信義區".to_string(),
            neighbourhood: "西村里".to_string(),
            ..LocationiqAddress::default()
        };

        let row = build_geodata_row("25.03396400", "121.56446800", &address, &keys(&["city"]));

        assert_eq!(
            row,
            vec![
                "25.03396400",
                "121.56446800",
                "臺灣",
                "Taipei",
                "臺北市",
                "信義區",
                "西村里",
            ]
        );
    }

    /// MY 的實測形狀：`district` 是 daerah（縣）、`city` 是鎮，兩者都有值。
    ///
    /// Reason: 這一條是本次修正的核心——優先鏈把 `district` 排在 `city` 前面時取縣名。
    /// 若把順序改回 `city` 優先（即修正前的行為），本測試會取到「加影」而失敗。
    #[test]
    fn city_name_takes_district_before_city_for_malaysia() {
        let row = build_geodata_row(
            "2.93611000",
            "101.79217000",
            &malaysia_address(),
            &keys(&["district", "city", "county"]),
        );

        assert_eq!(row[4], "乌鲁冷岳县");
    }

    /// 鏈上前面的欄位沒值時往後退。
    #[test]
    fn city_name_falls_through_to_later_keys() {
        let address = LocationiqAddress {
            country: "意大利".to_string(),
            state: "西西里岛".to_string(),
            county: "卡塔尼亞".to_string(),
            ..LocationiqAddress::default()
        };

        let row = build_geodata_row(
            "37.86437240",
            "15.06540680",
            &address,
            &keys(&["district", "city", "county"]),
        );

        assert_eq!(row[4], "卡塔尼亞");
    }

    /// `state` 缺席時不產生城市名。
    ///
    /// Reason: 越南的回應沒有 `state`，其 `city` 是省級直轄市（岘港市）——100% 有值
    /// 且 100% 是錯的層級。沒有這道絆線，優先鏈會把 admin1 當成城市名寫出去。
    /// 移除 `city_name` 開頭的 state 檢查後，本測試會取到「岘港市」而失敗。
    #[test]
    fn city_name_is_empty_when_state_is_absent() {
        let address = LocationiqAddress {
            country: "越南".to_string(),
            state: String::new(),
            city: "岘港市".to_string(),
            ..LocationiqAddress::default()
        };

        let row = build_geodata_row(
            "15.83370170",
            "108.05170330",
            &address,
            &keys(&["district", "city", "county"]),
        );

        assert_eq!(row[4], "");
    }
}
