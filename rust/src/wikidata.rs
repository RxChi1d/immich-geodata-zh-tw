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

#[derive(Debug, Clone)]
pub struct WikidataHttpClient {
    options: WikidataClientOptions,
    http: HttpClient,
}

impl WikidataHttpClient {
    pub fn new(options: WikidataClientOptions) -> Result<Self, String> {
        let policy = HttpRequestPolicy {
            user_agent: "immich-geodata-zh-tw/1.0 (Rust Wikidata Translation Tool)".to_string(),
            timeout: Duration::from_secs(30),
            max_retries: 5,
            ..HttpRequestPolicy::default()
        };
        Ok(Self {
            options,
            http: HttpClient::new(policy)?,
        })
    }

    pub fn search_entities_json(&self, name: &str, limit: usize) -> Result<String, String> {
        self.http
            .get_text(search_entities_url(name, &self.options.source_lang, limit)?.as_str())
    }

    pub fn ask_p131_json(&self, candidate_qid: &str, parent_qid: &str) -> Result<String, String> {
        let query = format!("ASK {{ wd:{candidate_qid} (wdt:P131)+ wd:{parent_qid} . }}");
        self.http.get_text(wdqs_url(&query)?.as_str())
    }

    pub fn zhwiki_convert_title_json(&self, title: &str) -> Result<String, String> {
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
