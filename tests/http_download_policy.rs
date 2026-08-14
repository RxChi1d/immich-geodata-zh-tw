//! 驗證大檔下載的 HTTP 逾時設定，避免逾時值退回到 API 呼叫用的預設值。

use std::net::TcpListener;
use std::thread;
use std::time::{Duration, Instant};

use immich_geodata::http::{HttpClient, HttpRequestPolicy};
use immich_geodata::pipeline::prepare_download::download_request_policy;

/// GeoNames 最大的下載檔 `alternateNamesV2.zip` 約 200 MB。
const LARGEST_DOWNLOAD_BYTES: u64 = 200 * 1024 * 1024;
/// 下載逾時必須寬鬆到能容忍 1 MB/s 的上游速率。
const MIN_TOLERATED_BYTES_PER_SEC: u64 = 1024 * 1024;
/// 單一檔案下載在最壞情況（伺服器接受連線後不回應）耗用的時間上限。
const WORST_CASE_LIMIT: Duration = Duration::from_secs(20 * 60);

#[test]
fn download_policy_tolerates_slow_upstream() {
    let policy = download_request_policy();
    let tolerated_bytes = MIN_TOLERATED_BYTES_PER_SEC * policy.timeout.as_secs();
    assert!(
        tolerated_bytes >= LARGEST_DOWNLOAD_BYTES,
        "下載逾時 {:?} 無法在 1 MB/s 下載完 200 MB 檔案",
        policy.timeout
    );
}

#[test]
fn download_policy_bounds_worst_case_duration() {
    let policy = download_request_policy();
    // Reason: 逾時同時涵蓋等待 headers 與讀取 body，伺服器無回應時每次嘗試
    // 都會耗滿逾時；重試次數必須讓總耗時維持在可接受範圍。
    let worst_case = policy.timeout * u32::try_from(policy.max_retries.max(1)).unwrap();
    assert!(
        worst_case <= WORST_CASE_LIMIT,
        "最壞情況耗時 {worst_case:?} 超過上限 {WORST_CASE_LIMIT:?}"
    );
}

#[test]
fn download_policy_keeps_fast_connect_timeout() {
    let policy = download_request_policy();
    assert_eq!(
        policy.connect_timeout,
        HttpRequestPolicy::default().connect_timeout,
        "下載 policy 不應放寬連線逾時"
    );
}

/// 伺服器接受 TCP 連線後不回應：驗證此情境由總逾時控制，而非連線逾時。
///
/// Reason: 這個行為正是重試次數必須壓低的原因；若 reqwest 未來提供獨立的
/// read timeout，此測試會失敗並提示可以重新調整重試次數。
#[test]
fn stalled_server_fails_at_total_timeout_not_connect_timeout() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("無法綁定測試 listener");
    let addr = listener.local_addr().expect("無法取得測試 listener 位址");
    thread::spawn(move || {
        if let Ok((stream, _)) = listener.accept() {
            thread::sleep(Duration::from_secs(10));
            drop(stream);
        }
    });

    let policy = HttpRequestPolicy {
        timeout: Duration::from_millis(600),
        connect_timeout: Duration::from_millis(50),
        max_retries: 1,
        sleep_between_retries: false,
        ..HttpRequestPolicy::default()
    };
    let client = HttpClient::new(policy).expect("無法建立測試 HTTP client");

    let started = Instant::now();
    let error = client
        .get_bytes(&format!("http://{addr}/stalled"))
        .expect_err("停滯的伺服器應回傳錯誤");
    let elapsed = started.elapsed();

    assert!(error.contains("HTTP 請求失敗"), "非預期錯誤訊息：{error}");
    assert!(
        elapsed >= Duration::from_millis(500),
        "連線已建立，不應由連線逾時提前中斷（耗時 {elapsed:?}）"
    );
    assert!(
        elapsed < Duration::from_secs(3),
        "應於總逾時後失敗（耗時 {elapsed:?}）"
    );
}
