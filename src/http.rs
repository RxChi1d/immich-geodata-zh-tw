use std::fmt;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use reqwest::StatusCode;
use reqwest::blocking::{Client, Response};
use reqwest::header::RETRY_AFTER;

/// HTTP 請求失敗的原因。
///
/// Reason: 呼叫端需要區分「被伺服器限速」與其他失敗。前者是可預期的額度耗盡，
/// 已取得的結果應保留；後者（金鑰錯誤、網路中斷、格式異常）代表流程有問題，
/// 必須往上拋。純字串錯誤無法讓呼叫端安全地做這個判斷。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HttpFailure {
    /// 429 且重試已耗盡。
    ///
    /// `body` 保留伺服器回應內容：LocationIQ 以 body 區分
    /// `Rate Limited Second` / `Rate Limited Minute` / `Rate Limited Day`，
    /// 那是唯一能判斷撞到哪一條限制的資訊。
    RateLimited {
        body: String,
    },
    Other(String),
}

/// 錯誤訊息中保留的 response body 上限，避免 HTML 錯誤頁灌爆日誌。
const FAILURE_BODY_LIMIT: usize = 200;

impl fmt::Display for HttpFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RateLimited { body } => {
                write!(formatter, "HTTP 請求被限速 status=429 body={body}")
            }
            Self::Other(message) => write!(formatter, "{message}"),
        }
    }
}

impl From<HttpFailure> for String {
    fn from(failure: HttpFailure) -> Self {
        failure.to_string()
    }
}

/// 截斷 response body，只保留診斷所需的開頭。
fn truncate_body(body: &str) -> String {
    let trimmed = body.trim();
    match trimmed.char_indices().nth(FAILURE_BODY_LIMIT) {
        Some((index, _)) => format!("{}…", &trimmed[..index]),
        None => trimmed.to_string(),
    }
}

#[derive(Debug, Clone)]
pub struct HttpRequestPolicy {
    pub user_agent: String,
    /// 單次請求的總逾時（含 response body 讀取）。
    pub timeout: Duration,
    /// 建立連線階段的逾時。
    ///
    /// Reason: `timeout` 同時涵蓋連線與 body 下載，大檔下載必須放寬總逾時，
    /// 但連線階段不應跟著放寬——否則伺服器無回應時，每次重試都要等滿整個
    /// 總逾時才失敗。拆成獨立的連線逾時可讓「連不上」快速失敗、「下載慢」
    /// 有足夠時間完成。
    pub connect_timeout: Duration,
    pub max_retries: usize,
    pub base_backoff: Duration,
    pub throttle_after_success: Duration,
    pub sleep_between_retries: bool,
    /// 啟用 AIMD 自適應節流（429 倍增、連續成功回落）。
    ///
    /// Reason: AIMD 參數是針對 Wikimedia 的滾動視窗配額調校的；
    /// LocationIQ 等以 qps 精確計費的端點應維持固定節流，
    /// 避免一次 429 讓吞吐長期低於付費方案額度。
    pub adaptive_throttle: bool,
}

/// 單次重試等待的上限，用於箝制伺服器回報的 `Retry-After`。
///
/// Reason: `Retry-After` 由伺服器決定，日配額型的端點可能回報到重置為止的秒數
/// （例如 3600）。照著睡會讓 job 卡住數小時後被 CI 的時限砍掉——連帶砍掉
/// 「限速就優雅停止」那條路徑，已 flush 的付費查詢結果也不會被 commit。等超過
/// 一分鐘的重試不如直接結束這一輪，由續跑機制接手。
const RETRY_AFTER_MAX: Duration = Duration::from_secs(60);

/// 自適應節流上限，避免 429 連發時節流無限增長。
const ADAPTIVE_THROTTLE_MAX: Duration = Duration::from_secs(8);
/// 連續成功達此次數後，節流向基準值回落一半。
const ADAPTIVE_DECAY_SUCCESSES: u32 = 25;

/// 自適應節流狀態：429 時倍增節流、連續成功後緩慢回落。
///
/// Reason: Wikimedia API 的速率限制是以 IP 計的滾動視窗配額，固定
/// delay 無法保證不觸頂；改為 AIMD（加性回落、乘性增長）讓請求
/// 間隔自動收斂到伺服器實際允許的速率。
#[derive(Debug)]
struct AdaptiveThrottle {
    base: Duration,
    current: Duration,
    consecutive_successes: u32,
}

impl AdaptiveThrottle {
    fn new(base: Duration) -> Self {
        Self {
            base,
            current: base,
            consecutive_successes: 0,
        }
    }

    /// 成功後回傳本次應等待的節流時間，並在連續成功後逐步回落。
    fn on_success(&mut self) -> Duration {
        let delay = self.current;
        self.consecutive_successes += 1;
        if self.consecutive_successes >= ADAPTIVE_DECAY_SUCCESSES && self.current > self.base {
            self.current = self.base.max(self.current / 2);
            self.consecutive_successes = 0;
        }
        delay
    }

    /// 收到 429 時倍增節流（至少 1 秒，最多 ADAPTIVE_THROTTLE_MAX）。
    fn on_rate_limited(&mut self) {
        self.consecutive_successes = 0;
        let doubled = self.current.saturating_mul(2).max(Duration::from_secs(1));
        self.current = doubled.min(ADAPTIVE_THROTTLE_MAX);
    }

    fn current(&self) -> Duration {
        self.current
    }
}

impl Default for HttpRequestPolicy {
    fn default() -> Self {
        Self {
            user_agent: "immich-geodata/0.1".to_string(),
            timeout: Duration::from_secs(30),
            connect_timeout: Duration::from_secs(10),
            max_retries: 5,
            base_backoff: Duration::from_secs(2),
            throttle_after_success: Duration::ZERO,
            sleep_between_retries: true,
            adaptive_throttle: false,
        }
    }
}

#[derive(Debug, Clone)]
pub struct HttpClient {
    client: Client,
    policy: HttpRequestPolicy,
    // Reason: 以 Arc 共享，clone 出去的 client 仍回報到同一份節流狀態，
    // 避免多份節流各自為政、合計超過伺服器允許的速率。
    throttle: Arc<Mutex<AdaptiveThrottle>>,
}

impl HttpClient {
    pub fn new(policy: HttpRequestPolicy) -> Result<Self, String> {
        let client = Client::builder()
            .user_agent(policy.user_agent.clone())
            .timeout(policy.timeout)
            .connect_timeout(policy.connect_timeout)
            .build()
            .map_err(|error| format!("無法建立 HTTP client：{error}"))?;
        let throttle = Arc::new(Mutex::new(AdaptiveThrottle::new(
            policy.throttle_after_success,
        )));
        Ok(Self {
            client,
            policy,
            throttle,
        })
    }

    pub fn with_default_policy() -> Result<Self, String> {
        Self::new(HttpRequestPolicy::default())
    }

    pub fn get_text(&self, url: &str) -> Result<String, String> {
        self.get_text_detailed(url).map_err(String::from)
    }

    /// 與 `get_text` 相同，但保留 `HttpFailure` 以便呼叫端辨識限速。
    pub fn get_text_detailed(&self, url: &str) -> Result<String, HttpFailure> {
        let response = self.get_response(url)?;
        response
            .text()
            .map_err(|error| HttpFailure::Other(format!("無法讀取 HTTP 文字回應 {url}：{error}")))
    }

    pub fn get_bytes(&self, url: &str) -> Result<Vec<u8>, String> {
        let response = self.get_response(url).map_err(String::from)?;
        response
            .bytes()
            .map(|bytes| bytes.to_vec())
            .map_err(|error| format!("無法讀取 HTTP 位元組回應 {url}：{error}"))
    }

    fn get_response(&self, url: &str) -> Result<Response, HttpFailure> {
        let mut last_error: Option<HttpFailure> = None;
        let attempts = self.policy.max_retries.max(1);
        for attempt in 1..=attempts {
            match self
                .client
                .get(url)
                .header("accept", "application/json")
                .send()
            {
                Ok(response) if response.status().is_success() => {
                    let delay = if self.policy.adaptive_throttle {
                        self.throttle
                            .lock()
                            .map(|mut throttle| throttle.on_success())
                            .unwrap_or(self.policy.throttle_after_success)
                    } else {
                        self.policy.throttle_after_success
                    };
                    if !delay.is_zero() {
                        thread::sleep(delay);
                    }
                    return Ok(response);
                }
                Ok(response) => {
                    let status = response.status();
                    if !is_retryable_status(status) || attempt == attempts {
                        // Reason: 限速要讓呼叫端可辨識，且 body 帶著 LocationIQ
                        // 「撞到哪一條限制」的唯一線索，不能在這裡丟掉。
                        if status == StatusCode::TOO_MANY_REQUESTS {
                            let body = response.text().unwrap_or_default();
                            return Err(HttpFailure::RateLimited {
                                body: truncate_body(&body),
                            });
                        }
                        return Err(HttpFailure::Other(format!(
                            "HTTP 請求失敗 status={status} url={url}"
                        )));
                    }
                    let retry_after = response
                        .headers()
                        .get(RETRY_AFTER)
                        .and_then(|value| value.to_str().ok())
                        .and_then(|value| value.parse::<u64>().ok())
                        .map(Duration::from_secs);
                    if self.policy.adaptive_throttle
                        && status == StatusCode::TOO_MANY_REQUESTS
                        && let Ok(mut throttle) = self.throttle.lock()
                    {
                        throttle.on_rate_limited();
                        // Reason: 只記錄狀態與等待時間，不含 URL，避免洩漏 query string 中的 API key。
                        eprintln!(
                            "http_retry status={status} attempt={attempt} retry_after={retry_after:?} throttle={:?}",
                            throttle.current()
                        );
                    } else {
                        eprintln!(
                            "http_retry status={status} attempt={attempt} retry_after={retry_after:?}"
                        );
                    }
                    last_error = Some(HttpFailure::Other(format!(
                        "HTTP 請求暫時失敗 status={status} url={url}"
                    )));
                    self.sleep_before_retry(attempt, retry_after);
                }
                Err(error) => {
                    last_error = Some(HttpFailure::Other(format!(
                        "HTTP 請求失敗 url={url}：{error}"
                    )));
                    if attempt == attempts {
                        break;
                    }
                    self.sleep_before_retry(attempt, None);
                }
            }
        }
        Err(last_error.unwrap_or_else(|| HttpFailure::Other(format!("HTTP 請求失敗 url={url}"))))
    }

    /// 本次重試前應等待的時間。抽成純函式讓箝制行為可以不真的睡就測到。
    fn retry_delay(&self, attempt: usize, retry_after: Option<Duration>) -> Duration {
        retry_after
            .map(|delay| delay.min(RETRY_AFTER_MAX))
            .unwrap_or_else(|| {
                let multiplier = u32::try_from(attempt).unwrap_or(u32::MAX).max(1);
                self.policy.base_backoff.saturating_mul(multiplier)
            })
    }

    fn sleep_before_retry(&self, attempt: usize, retry_after: Option<Duration>) {
        if !self.policy.sleep_between_retries {
            return;
        }
        let delay = self.retry_delay(attempt, retry_after);
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
    fn adaptive_throttle_doubles_on_429_and_caps_at_max() {
        let mut throttle = AdaptiveThrottle::new(Duration::from_millis(200));

        throttle.on_rate_limited();
        // 第一次 429：倍增後低於 1 秒，提升至下限 1 秒。
        assert_eq!(throttle.current(), Duration::from_secs(1));
        for _ in 0..10 {
            throttle.on_rate_limited();
        }
        // 連續 429 後收斂在上限。
        assert_eq!(throttle.current(), ADAPTIVE_THROTTLE_MAX);
    }

    #[test]
    fn adaptive_throttle_decays_back_to_base_after_consecutive_successes() {
        let mut throttle = AdaptiveThrottle::new(Duration::from_millis(200));
        throttle.on_rate_limited();
        assert_eq!(throttle.current(), Duration::from_secs(1));

        for _ in 0..ADAPTIVE_DECAY_SUCCESSES {
            throttle.on_success();
        }
        assert_eq!(throttle.current(), Duration::from_millis(500));

        // 再持續成功，最終回落到基準值且不低於基準。
        for _ in 0..(ADAPTIVE_DECAY_SUCCESSES * 3) {
            throttle.on_success();
        }
        assert_eq!(throttle.current(), Duration::from_millis(200));
    }

    #[test]
    fn adaptive_throttle_resets_success_streak_on_429() {
        let mut throttle = AdaptiveThrottle::new(Duration::from_millis(200));
        throttle.on_rate_limited();
        for _ in 0..(ADAPTIVE_DECAY_SUCCESSES - 1) {
            throttle.on_success();
        }
        // 一次 429 重置成功計數並再倍增。
        throttle.on_rate_limited();
        assert_eq!(throttle.current(), Duration::from_secs(2));
        throttle.on_success();
        assert_eq!(throttle.current(), Duration::from_secs(2));
    }

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

    /// 429 重試耗盡時必須回報 `RateLimited` 並保留 response body。
    ///
    /// Reason: LocationIQ 只在 body 區分 `Rate Limited Second` / `Minute` / `Day`，
    /// 過去這裡把 body 丟掉，撞到哪一條限制只能靠日誌時間戳反推。
    #[test]
    fn exhausted_429_reports_rate_limited_with_body() {
        let body = r#"{"error":"Rate Limited Day"}"#;
        let replies = (0..HttpRequestPolicy::default().max_retries)
            .map(|_| HttpReply::new(429, "", body))
            .collect();
        let server = TestServer::new(replies);
        let client = test_client();

        let failure = client.get_text_detailed(&server.url()).unwrap_err();

        assert_eq!(
            failure,
            HttpFailure::RateLimited {
                body: body.to_string()
            }
        );
        assert!(failure.to_string().contains("Rate Limited Day"));
    }

    /// 伺服器回報的 `Retry-After` 必須被箝制，否則 job 會睡到被 CI 時限砍掉。
    #[test]
    fn retry_delay_caps_server_reported_retry_after() {
        let client = HttpClient::new(HttpRequestPolicy {
            base_backoff: Duration::from_secs(2),
            ..HttpRequestPolicy::default()
        })
        .unwrap();

        assert_eq!(
            client.retry_delay(1, Some(Duration::from_secs(3600))),
            RETRY_AFTER_MAX,
            "日配額型的 Retry-After 必須被箝制"
        );
        // 低於上限時照伺服器的值走。
        assert_eq!(
            client.retry_delay(1, Some(Duration::from_secs(5))),
            Duration::from_secs(5)
        );
        // 沒有 Retry-After 時維持原本的線性退避。
        assert_eq!(client.retry_delay(3, None), Duration::from_secs(6));
    }

    #[test]
    fn truncate_body_caps_length_without_splitting_characters() {
        let long = "額".repeat(FAILURE_BODY_LIMIT + 10);
        let truncated = truncate_body(&long);
        assert!(truncated.ends_with('…'));
        assert_eq!(truncated.chars().count(), FAILURE_BODY_LIMIT + 1);
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
