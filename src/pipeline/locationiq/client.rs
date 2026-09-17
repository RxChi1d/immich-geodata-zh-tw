//! LocationIQ 的 HTTP 呼叫端：URL 組裝、節流與錯誤遮蔽。
//!
//! 自 `locationiq.rs` 拆出。這裡的每一條路徑都碰得到帶 API key 的 URL，
//! 集中在單一檔案才看得出遮蔽有沒有漏掉。

use std::time::Duration;

use reqwest::Url;

use crate::http::{HttpClient, HttpFailure, HttpRequestPolicy};
use crate::pipeline::locationiq::{LocationiqAddress, ReverseGeocoder, parse_locationiq_address};

pub struct LocationiqHttpClient {
    api_key: String,
    http: HttpClient,
}

impl LocationiqHttpClient {
    pub fn new(api_key: String, qps: u32) -> Result<Self, String> {
        let delay_ms = 1020_u64 / u64::from(qps.max(1));
        let policy = HttpRequestPolicy {
            user_agent: "immich-geodata/0.1 LocationIQ".to_string(),
            throttle_after_success: Duration::from_millis(delay_ms),
            ..HttpRequestPolicy::default()
        };
        Ok(Self {
            api_key,
            http: HttpClient::new(policy)
                .map_err(|error| format!("無法建立 LocationIQ HTTP client：{error}"))?,
        })
    }

    pub fn redacted_url(&self, latitude: &str, longitude: &str) -> Result<String, String> {
        Ok(redact_locationiq_key(
            build_locationiq_url(latitude, longitude, &self.api_key)?.as_str(),
        ))
    }
}

pub fn build_locationiq_url(latitude: &str, longitude: &str, api_key: &str) -> Result<Url, String> {
    let mut url = Url::parse("https://us1.locationiq.com/v1/reverse")
        .map_err(|error| format!("LocationIQ base URL 錯誤：{error}"))?;
    url.query_pairs_mut()
        .append_pair("lat", latitude)
        .append_pair("lon", longitude)
        .append_pair("format", "json")
        .append_pair("accept-language", "zh,en")
        // Reason: 兩個正規化都必須關閉，否則拿不到正確層級。
        // `normalizeaddress=1` 回傳的是固定欄位清單（name/house_number/road/
        // neighbourhood/suburb/city/county/state/postcode/country_code），**不含
        // `district`**——實測同一座標開啟後 `district` 整個消失。
        // `normalizecity=1` 則在 `city` 缺席時，依序把 city_district → locality →
        // town → borough → municipality → village → hamlet → quarter →
        // neighbourhood 的第一個有值者提升成 `city`，九個層級塞進同一欄，使層級
        // 無法預期（MY 實測：同一個 daerah 內最多出現 9 種不同名字）。
        .append_pair("normalizeaddress", "0")
        .append_pair("normalizecity", "0")
        .append_pair("key", api_key);
    Ok(url)
}

/// 把 HTTP 層的錯誤轉成帶遮蔽 URL 的 LocationIQ 錯誤。
///
/// Reason: `HttpFailure::Other` 的訊息由 HTTP 層組出，內嵌未遮蔽的原始 URL
/// （query string 含 `key=<API key>`）。只遮外層的 `url=` 不夠——金鑰無效時的
/// 401 會把完整金鑰印進日誌，而那正是額度錯誤訊息要使用者去查的路徑。這裡把
/// 內層訊息中的原始 URL 一併換成遮蔽版本。限速錯誤原樣往上傳，呼叫端要靠它
/// 辨識額度用完，且它的訊息不含 URL。
pub(super) fn redact_locationiq_failure(url: &str, failure: HttpFailure) -> HttpFailure {
    match failure {
        // Reason: 這兩個變體都必須原樣往上傳，呼叫端要靠變體本身分辨處置方式
        // （限速＝額度用完、404＝這個座標查不到）。包成 Other 會讓分辨失效。
        // 它們的訊息本來就不含 URL，沒有遮蔽需求。
        HttpFailure::RateLimited { .. } | HttpFailure::NotFound { .. } => failure,
        other => {
            let redacted = redact_locationiq_key(url);
            HttpFailure::Other(format!(
                "LocationIQ 查詢失敗 url={redacted}：{}",
                other.to_string().replace(url, redacted.as_str())
            ))
        }
    }
}

pub(super) fn redact_locationiq_key(url: &str) -> String {
    match Url::parse(url) {
        Ok(mut parsed) => {
            let pairs: Vec<(String, String)> = parsed
                .query_pairs()
                .map(|(key, value)| {
                    let value = if key == "key" {
                        "***".to_string()
                    } else {
                        value.into_owned()
                    };
                    (key.into_owned(), value)
                })
                .collect();
            parsed.query_pairs_mut().clear().extend_pairs(pairs);
            parsed.to_string()
        }
        // Reason: 舊寫法 `url.replace("key=", "key=***")` 只是在 `key=` 後面插入
        // 星號，金鑰原樣留在後面（`key=***pk.secret`），等於沒有遮蔽。`key` 是
        // build_locationiq_url 附加的最後一個參數，截到它為止即可保留診斷資訊
        // 又不外洩金鑰。
        Err(_) => match url.split_once("key=") {
            Some((head, _)) => format!("{head}key=***"),
            None => url.to_string(),
        },
    }
}

impl ReverseGeocoder for LocationiqHttpClient {
    fn reverse(
        &mut self,
        latitude: &str,
        longitude: &str,
    ) -> Result<Option<LocationiqAddress>, HttpFailure> {
        let url =
            build_locationiq_url(latitude, longitude, &self.api_key).map_err(HttpFailure::Other)?;
        let body = match self.http.get_text_detailed(url.as_str()) {
            Ok(body) => body,
            // Reason: LocationIQ 對無法逆地理編碼的座標（外海、無定義區域）回
            // 404 `Unable to geocode`，那是這一個座標的屬性，不是流程出錯。
            // 併進錯誤會讓單一查不到的座標中止整條 release——而
            // `--locationiq-allow-partial` 只容忍限速，擋不住它。回 `Ok(None)`
            // 讓呼叫端跳過該點；該座標不會寫進 metadata，故下一輪仍會重試，
            // 暫時性的 404 不會被永久記成「查過了」。
            Err(HttpFailure::NotFound { .. }) => return Ok(None),
            Err(error) => return Err(redact_locationiq_failure(url.as_str(), error)),
        };
        parse_locationiq_address(&body)
            .map(Some)
            .map_err(HttpFailure::Other)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// HTTP 層錯誤內嵌的原始 URL 含 API key，包裝後不得外洩。
    ///
    /// Reason: 只遮外層 `url=` 時，`{other}` 仍帶著完整金鑰；金鑰無效的 401
    /// 是最常觸發的路徑，也正是錯誤訊息叫使用者去查的那一種。
    #[test]
    fn locationiq_failure_redacts_api_key_from_nested_http_message() {
        let url = build_locationiq_url("25.0", "121.5", "pk.secret123").unwrap();
        let failure = redact_locationiq_failure(
            url.as_str(),
            HttpFailure::Other(format!("HTTP 請求失敗 status=401 url={url}")),
        );

        let message = failure.to_string();
        assert!(!message.contains("pk.secret123"), "金鑰不得出現：{message}");
        assert!(message.contains("key=***"), "應保留遮蔽後的 URL：{message}");
    }

    /// `Url::parse` 失敗的退路也必須真的遮掉金鑰。
    ///
    /// Reason: 舊寫法 `replace("key=", "key=***")` 只是插入星號，金鑰原樣留在
    /// 後面（`key=***pk.secret123`），看起來有遮蔽但完全沒有。
    #[test]
    fn redact_key_on_unparseable_url_does_not_leak() {
        let redacted = redact_locationiq_key("not a url ?lat=1&key=pk.secret123");
        assert!(
            !redacted.contains("pk.secret123"),
            "金鑰不得出現：{redacted}"
        );
        assert!(redacted.contains("key=***"), "應留下遮蔽標記：{redacted}");
    }

    /// 限速錯誤必須原樣往上傳，否則呼叫端無法辨識額度用完。
    #[test]
    fn locationiq_failure_passes_rate_limited_through() {
        let url = build_locationiq_url("25.0", "121.5", "pk.secret123").unwrap();
        let failure = redact_locationiq_failure(
            url.as_str(),
            HttpFailure::RateLimited {
                body: r#"{"error":"Rate Limited Day"}"#.to_string(),
            },
        );

        assert!(matches!(failure, HttpFailure::RateLimited { .. }));
    }

    /// 404 必須原樣往上傳，否則 `reverse` 的跳過對應永遠不會觸發。
    ///
    /// Reason: 這是整條 404 處置最容易被改壞的一環——只要 404 在這裡塌成
    /// `Other`，`reverse` 的 `Err(NotFound) => Ok(None)` 就再也配不到，一個查不到
    /// 的座標又會中止整條 release，而且沒有任何編譯錯誤提示。
    #[test]
    fn locationiq_failure_passes_not_found_through() {
        let url = build_locationiq_url("25.0", "121.5", "pk.secret123").unwrap();
        let failure = redact_locationiq_failure(
            url.as_str(),
            HttpFailure::NotFound {
                body: r#"{"error":"Unable to geocode"}"#.to_string(),
            },
        );

        assert!(matches!(failure, HttpFailure::NotFound { .. }));
        assert!(
            !failure.to_string().contains("pk.secret123"),
            "金鑰不得出現：{failure}"
        );
    }
}
