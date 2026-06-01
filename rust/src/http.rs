use std::thread;
use std::time::Duration;

use reqwest::StatusCode;
use reqwest::blocking::{Client, Response};
use reqwest::header::RETRY_AFTER;

#[derive(Debug, Clone)]
pub struct HttpRequestPolicy {
    pub user_agent: String,
    pub timeout: Duration,
    pub max_retries: usize,
    pub base_backoff: Duration,
    pub throttle_after_success: Duration,
    pub sleep_between_retries: bool,
}

impl Default for HttpRequestPolicy {
    fn default() -> Self {
        Self {
            user_agent: "immich-geodata-zh-tw-rust/0.1".to_string(),
            timeout: Duration::from_secs(30),
            max_retries: 5,
            base_backoff: Duration::from_secs(2),
            throttle_after_success: Duration::ZERO,
            sleep_between_retries: true,
        }
    }
}

#[derive(Debug, Clone)]
pub struct HttpClient {
    client: Client,
    policy: HttpRequestPolicy,
}

impl HttpClient {
    pub fn new(policy: HttpRequestPolicy) -> Result<Self, String> {
        let client = Client::builder()
            .user_agent(policy.user_agent.clone())
            .timeout(policy.timeout)
            .build()
            .map_err(|error| format!("無法建立 HTTP client：{error}"))?;
        Ok(Self { client, policy })
    }

    pub fn with_default_policy() -> Result<Self, String> {
        Self::new(HttpRequestPolicy::default())
    }

    pub fn get_text(&self, url: &str) -> Result<String, String> {
        let response = self.get_response(url)?;
        response
            .text()
            .map_err(|error| format!("無法讀取 HTTP 文字回應 {url}：{error}"))
    }

    pub fn get_bytes(&self, url: &str) -> Result<Vec<u8>, String> {
        let response = self.get_response(url)?;
        response
            .bytes()
            .map(|bytes| bytes.to_vec())
            .map_err(|error| format!("無法讀取 HTTP 位元組回應 {url}：{error}"))
    }

    fn get_response(&self, url: &str) -> Result<Response, String> {
        let mut last_error: Option<String> = None;
        let attempts = self.policy.max_retries.max(1);
        for attempt in 1..=attempts {
            match self
                .client
                .get(url)
                .header("accept", "application/json")
                .send()
            {
                Ok(response) if response.status().is_success() => {
                    if !self.policy.throttle_after_success.is_zero() {
                        thread::sleep(self.policy.throttle_after_success);
                    }
                    return Ok(response);
                }
                Ok(response) => {
                    let status = response.status();
                    if !is_retryable_status(status) || attempt == attempts {
                        return Err(format!("HTTP 請求失敗 status={status} url={url}"));
                    }
                    let retry_after = response
                        .headers()
                        .get(RETRY_AFTER)
                        .and_then(|value| value.to_str().ok())
                        .and_then(|value| value.parse::<u64>().ok())
                        .map(Duration::from_secs);
                    last_error = Some(format!("HTTP 請求暫時失敗 status={status} url={url}"));
                    self.sleep_before_retry(attempt, retry_after);
                }
                Err(error) => {
                    last_error = Some(format!("HTTP 請求失敗 url={url}：{error}"));
                    if attempt == attempts {
                        break;
                    }
                    self.sleep_before_retry(attempt, None);
                }
            }
        }
        Err(last_error.unwrap_or_else(|| format!("HTTP 請求失敗 url={url}")))
    }

    fn sleep_before_retry(&self, attempt: usize, retry_after: Option<Duration>) {
        if !self.policy.sleep_between_retries {
            return;
        }
        let delay = retry_after.unwrap_or_else(|| {
            let multiplier = u32::try_from(attempt).unwrap_or(u32::MAX).max(1);
            self.policy.base_backoff.saturating_mul(multiplier)
        });
        if !delay.is_zero() {
            thread::sleep(delay);
        }
    }
}

fn is_retryable_status(status: StatusCode) -> bool {
    matches!(
        status,
        StatusCode::TOO_MANY_REQUESTS
            | StatusCode::INTERNAL_SERVER_ERROR
            | StatusCode::BAD_GATEWAY
            | StatusCode::SERVICE_UNAVAILABLE
            | StatusCode::GATEWAY_TIMEOUT
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::{TcpListener, TcpStream};
    use std::sync::{Arc, Mutex};

    #[test]
    fn retries_429_then_returns_body() {
        let server = TestServer::new(vec![
            HttpReply::new(429, "Retry-After: 0\r\n", "rate limited"),
            HttpReply::new(200, "", "ok"),
        ]);
        let client = test_client();

        let body = client.get_text(&server.url()).unwrap();

        assert_eq!(body, "ok");
        assert_eq!(server.request_count(), 2);
    }

    #[test]
    fn does_not_retry_non_retryable_status() {
        let server = TestServer::new(vec![HttpReply::new(404, "", "missing")]);
        let client = test_client();

        let error = client.get_text(&server.url()).unwrap_err();

        assert!(error.contains("404"));
        assert_eq!(server.request_count(), 1);
    }

    fn test_client() -> HttpClient {
        let policy = HttpRequestPolicy {
            sleep_between_retries: false,
            base_backoff: Duration::ZERO,
            ..HttpRequestPolicy::default()
        };
        HttpClient::new(policy).unwrap()
    }

    struct HttpReply {
        status: u16,
        headers: String,
        body: String,
    }

    impl HttpReply {
        fn new(status: u16, headers: &str, body: &str) -> Self {
            Self {
                status,
                headers: headers.to_string(),
                body: body.to_string(),
            }
        }
    }

    struct TestServer {
        url: String,
        requests: Arc<Mutex<usize>>,
    }

    impl TestServer {
        fn new(replies: Vec<HttpReply>) -> Self {
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            let url = format!("http://{}", listener.local_addr().unwrap());
            let requests = Arc::new(Mutex::new(0));
            let request_count = Arc::clone(&requests);
            std::thread::spawn(move || {
                for reply in replies {
                    let (mut stream, _) = listener.accept().unwrap();
                    drain_request(&mut stream);
                    *request_count.lock().unwrap() += 1;
                    write_reply(&mut stream, &reply);
                }
            });
            Self { url, requests }
        }

        fn url(&self) -> String {
            self.url.clone()
        }

        fn request_count(&self) -> usize {
            *self.requests.lock().unwrap()
        }
    }

    fn drain_request(stream: &mut TcpStream) {
        let mut buffer = [0_u8; 1024];
        let _ = stream.read(&mut buffer);
    }

    fn write_reply(stream: &mut TcpStream, reply: &HttpReply) {
        let response = format!(
            "HTTP/1.1 {} Test\r\n{}Content-Length: {}\r\nConnection: close\r\n\r\n{}",
            reply.status,
            reply.headers,
            reply.body.len(),
            reply.body
        );
        stream.write_all(response.as_bytes()).unwrap();
    }
}
