use std::time::Duration;

use reqwest::Url;

use crate::http::{HttpClient, HttpRequestPolicy};

pub const WDQS_URL: &str = "https://query.wikidata.org/sparql";
pub const WDACT_URL: &str = "https://www.wikidata.org/w/api.php";
pub const ZHWIKI_URL: &str = "https://zh.wikipedia.org/w/api.php";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WikidataClientOptions {
    pub source_lang: String,
    pub target_lang: String,
    pub fallback_langs: Vec<String>,
}

impl WikidataClientOptions {
    pub fn new(source_lang: impl Into<String>, target_lang: impl Into<String>) -> Self {
        let source_lang = source_lang.into();
        Self {
            fallback_langs: vec![
                "zh-hant".to_string(),
                "zh".to_string(),
                "en".to_string(),
                source_lang.clone(),
            ],
            source_lang,
            target_lang: target_lang.into(),
        }
    }
}

pub trait WikidataApi {
    fn search_entities_json(&self, name: &str, limit: usize) -> Result<String, String>;
    fn get_entities_json(
        &self,
        qids: &[String],
        props: &str,
        languages: &str,
    ) -> Result<String, String>;
    fn ask_p131_json(&self, candidate_qid: &str, parent_qid: &str) -> Result<String, String>;
    fn zhwiki_convert_title_json(&self, title: &str) -> Result<String, String>;
}

/// Wikidata API 與中文維基 API 的每請求節流間隔。
const WDACT_THROTTLE: Duration = Duration::from_millis(200);
/// WDQS（SPARQL）端點的每請求節流間隔。
/// Reason: WDQS 是 Wikimedia 各端點中速率限制最嚴格的，節流過短會頻繁
/// 觸發 429 與 Retry-After 等待，整體反而更慢；比照 Python 版設定 0.8 秒。
const WDQS_THROTTLE: Duration = Duration::from_millis(800);

/// Wikidata API（wbsearchentities / wbgetentities）與中文維基 API 的請求 policy。
pub fn wdact_request_policy() -> HttpRequestPolicy {
    HttpRequestPolicy {
        user_agent: "immich-geodata-zh-tw/1.0 (Rust Wikidata Translation Tool)".to_string(),
        timeout: Duration::from_secs(30),
        max_retries: 5,
        throttle_after_success: WDACT_THROTTLE,
        adaptive_throttle: true,
        ..HttpRequestPolicy::default()
    }
}

/// WDQS（SPARQL，P131 驗證）端點的請求 policy。
pub fn wdqs_request_policy() -> HttpRequestPolicy {
    HttpRequestPolicy {
        throttle_after_success: WDQS_THROTTLE,
        ..wdact_request_policy()
    }
}

#[derive(Debug, Clone)]
pub struct WikidataHttpClient {
    options: WikidataClientOptions,
    http: HttpClient,
    wdqs_http: HttpClient,
}

impl WikidataHttpClient {
    pub fn new(options: WikidataClientOptions) -> Result<Self, String> {
        Ok(Self {
            options,
            http: HttpClient::new(wdact_request_policy())?,
            wdqs_http: HttpClient::new(wdqs_request_policy())?,
        })
    }
}

impl WikidataApi for WikidataHttpClient {
    fn search_entities_json(&self, name: &str, limit: usize) -> Result<String, String> {
        self.http
            .get_text(search_entities_url(name, &self.options.source_lang, limit)?.as_str())
    }

    fn get_entities_json(
        &self,
        qids: &[String],
        props: &str,
        languages: &str,
    ) -> Result<String, String> {
        self.http
            .get_text(get_entities_url(qids, props, languages)?.as_str())
    }

    fn ask_p131_json(&self, candidate_qid: &str, parent_qid: &str) -> Result<String, String> {
        let query = format!("ASK {{ wd:{candidate_qid} (wdt:P131)+ wd:{parent_qid} . }}");
        self.wdqs_http.get_text(wdqs_url(&query)?.as_str())
    }

    fn zhwiki_convert_title_json(&self, title: &str) -> Result<String, String> {
        self.http
            .get_text(zhwiki_convert_title_url(title)?.as_str())
    }
}

pub fn search_entities_url(name: &str, source_lang: &str, limit: usize) -> Result<Url, String> {
    let mut url =
        Url::parse(WDACT_URL).map_err(|error| format!("Wikidata API URL 錯誤：{error}"))?;
    url.query_pairs_mut()
        .append_pair("format", "json")
        .append_pair("action", "wbsearchentities")
        .append_pair("search", name)
        .append_pair("language", source_lang)
        .append_pair("uselang", source_lang)
        .append_pair("type", "item")
        .append_pair("limit", &limit.to_string());
    Ok(url)
}

pub fn get_entities_url(qids: &[String], props: &str, languages: &str) -> Result<Url, String> {
    let mut url =
        Url::parse(WDACT_URL).map_err(|error| format!("Wikidata API URL 錯誤：{error}"))?;
    url.query_pairs_mut()
        .append_pair("format", "json")
        .append_pair("action", "wbgetentities")
        .append_pair("ids", &qids.join("|"))
        .append_pair("props", props)
        .append_pair("languages", languages);
    Ok(url)
}

pub fn wdqs_url(query: &str) -> Result<Url, String> {
    let mut url = Url::parse(WDQS_URL).map_err(|error| format!("WDQS URL 錯誤：{error}"))?;
    url.query_pairs_mut()
        .append_pair("query", query)
        .append_pair("format", "json");
    Ok(url)
}

pub fn zhwiki_convert_title_url(title: &str) -> Result<Url, String> {
    let mut url = Url::parse(ZHWIKI_URL).map_err(|error| format!("中文維基 URL 錯誤：{error}"))?;
    url.query_pairs_mut()
        .append_pair("action", "query")
        .append_pair("format", "json")
        .append_pair("converttitles", "1")
        .append_pair("titles", title);
    Ok(url)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wdqs_policy_throttles_more_aggressively_than_wdact_policy() {
        let wdact = wdact_request_policy();
        let wdqs = wdqs_request_policy();

        assert_eq!(wdact.throttle_after_success, Duration::from_millis(200));
        assert_eq!(wdqs.throttle_after_success, Duration::from_millis(800));
        // Wikimedia 端點啟用 AIMD 自適應節流；其他 HttpClient 使用者
        //（如 LocationIQ）維持預設關閉的固定節流。
        assert!(wdact.adaptive_throttle);
        assert!(wdqs.adaptive_throttle);
        assert!(!HttpRequestPolicy::default().adaptive_throttle);
        // 其餘設定（user agent、timeout、重試）兩者必須一致。
        assert_eq!(wdqs.user_agent, wdact.user_agent);
        assert_eq!(wdqs.timeout, wdact.timeout);
        assert_eq!(wdqs.max_retries, wdact.max_retries);
    }

    #[test]
    fn search_entities_url_matches_reference_request_contract() {
        let url = search_entities_url("서울특별시", "ko", 7).unwrap();
        let params: Vec<_> = url.query_pairs().collect();

        assert!(params.contains(&("action".into(), "wbsearchentities".into())));
        assert!(params.contains(&("search".into(), "서울특별시".into())));
        assert!(params.contains(&("language".into(), "ko".into())));
        assert!(params.contains(&("uselang".into(), "ko".into())));
        assert!(params.contains(&("type".into(), "item".into())));
        assert!(params.contains(&("limit".into(), "7".into())));
    }

    #[test]
    fn get_entities_url_uses_batch_contract() {
        let url = get_entities_url(
            &["Q1".to_string(), "Q2".to_string()],
            "labels|sitelinks",
            "zh-tw|zh",
        )
        .unwrap();
        let params: Vec<_> = url.query_pairs().collect();

        assert!(params.contains(&("action".into(), "wbgetentities".into())));
        assert!(params.contains(&("ids".into(), "Q1|Q2".into())));
        assert!(params.contains(&("props".into(), "labels|sitelinks".into())));
        assert!(params.contains(&("languages".into(), "zh-tw|zh".into())));
    }

    #[test]
    fn wdqs_url_matches_p131_ask_contract() {
        let url = wdqs_url("ASK { wd:Q123 (wdt:P131)+ wd:Q456 . }").unwrap();
        assert_eq!(url.host_str(), Some("query.wikidata.org"));
        assert!(url.as_str().contains("format=json"));
        assert!(url.as_str().contains("P131"));
    }

    #[test]
    fn zhwiki_convert_title_url_matches_reference_request_contract() {
        let url = zhwiki_convert_title_url("重庆市").unwrap();
        let params: Vec<_> = url.query_pairs().collect();

        assert!(params.contains(&("action".into(), "query".into())));
        assert!(params.contains(&("format".into(), "json".into())));
        assert!(params.contains(&("converttitles".into(), "1".into())));
        assert!(params.contains(&("titles".into(), "重庆市".into())));
    }
}
