use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::time::Duration;

use reqwest::Url;

use crate::cli::RunOptions;
use crate::http::{HttpClient, HttpFailure, HttpRequestPolicy};
use crate::observability::ProgressReporter;
use crate::pipeline::fixtures::{Fixture, load_fixtures};
use crate::pipeline::polars_table::{
    read_cities_rows, read_geodata_rows_with_header, write_geodata_rows_with_header,
};
use crate::pipeline::table::{format_coordinate, read_delimited};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProductionLocationiqOptions {
    pub cities_file: PathBuf,
    pub output_file: PathBuf,
    pub country_code: String,
    pub batch_size: usize,
    pub qps: u32,
    pub api_key: String,
    pub overwrite: bool,
    /// 額度用完時是否視為正常結束。
    ///
    /// Reason: 本地補查與 CI 增量補查對「沒查完」的期待相反。本地是要把一國跑滿
    /// 才提交 CSV，中途停下必須讓流程失敗、不能繼續往下發布；CI 每週只補一段，
    /// 額度用完是預期中的結果，若讓它失敗，這一輪已查到的付費結果不會被 commit。
    pub allow_partial: bool,
}

/// LocationIQ 階段的結束狀態。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LocationiqOutcome {
    /// 所有待查座標都查完。
    Completed,
    /// 額度用完，已查結果已寫回輸出檔，剩餘座標留給下次執行。
    RateLimited,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocationiqAddress {
    pub country: String,
    pub state: String,
    pub city: String,
    pub county: String,
    pub suburb: String,
    pub neighbourhood: String,
}

pub trait ReverseGeocoder {
    /// Reason: 錯誤型別直接沿用 `HttpFailure` 而不另立一套。呼叫端唯一需要分辨的
    /// 是「被限速」與「其他失敗」，那正是 `HttpFailure` 已經表達的區分；多包一層
    /// 只是把同一個二分法換個名字再寫一次。
    fn reverse(
        &mut self,
        latitude: &str,
        longitude: &str,
    ) -> Result<Option<LocationiqAddress>, HttpFailure>;
}

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
        .append_pair("normalizeaddress", "1")
        .append_pair("normalizecity", "1")
        .append_pair("key", api_key);
    Ok(url)
}

fn redact_locationiq_key(url: &str) -> String {
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
        Err(_) => url.replace("key=", "key=***"),
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
        let body = self
            .http
            .get_text_detailed(url.as_str())
            .map_err(|error| match error {
                HttpFailure::RateLimited { .. } => error,
                other => HttpFailure::Other(format!(
                    "LocationIQ 查詢失敗 url={}：{other}",
                    redact_locationiq_key(url.as_str())
                )),
            })?;
        parse_locationiq_address(&body)
            .map(Some)
            .map_err(HttpFailure::Other)
    }
}

pub fn run(options: &RunOptions) -> Result<(), String> {
    let fixtures = load_fixtures(&options.fixtures_dir, options.fixture.as_deref())?;
    for fixture in fixtures {
        if !fixture.supports_stage("locationiq") {
            continue;
        }
        run_fixture(&fixture, options)?;
    }
    Ok(())
}

fn run_fixture(fixture: &Fixture, options: &RunOptions) -> Result<(), String> {
    let country = fixture
        .manifest
        .countries
        .first()
        .ok_or_else(|| "locationiq fixture 缺少 countries".to_string())?;
    let mut rows = read_existing_meta(&fixture.root.join("locationiq").join("existing_meta.csv"))?;
    let mut existing_coords: HashSet<(String, String)> = rows
        .iter()
        .map(|row| (row[0].clone(), row[1].clone()))
        .collect();
    let responses = read_responses(&fixture.root.join("locationiq").join("responses.csv"))?;
    let cities = read_cities_rows(
        &fixture
            .root
            .join("locationiq")
            .join("cities500_optimized.txt"),
        b'\t',
    )?;

    for city in cities {
        if city.len() < 19 || city[8] != *country {
            continue;
        }
        let latitude = format_coordinate(&city[4])?;
        let longitude = format_coordinate(&city[5])?;
        if existing_coords.contains(&(latitude.clone(), longitude.clone())) {
            continue;
        }
        let response = responses
            .get(&(latitude.clone(), longitude.clone()))
            .ok_or_else(|| format!("LocationIQ fixture 缺少座標回應：{latitude},{longitude}"))?;
        rows.push(build_geodata_row(response));
        existing_coords.insert((latitude, longitude));
    }

    rows.sort_by(|left, right| {
        left[2]
            .cmp(&right[2])
            .then(left[3].cmp(&right[3]))
            .then(left[4].cmp(&right[4]))
            .then(left[5].cmp(&right[5]))
            .then(left[6].cmp(&right[6]))
            .then(left[0].cmp(&right[0]))
    });

    let output = options
        .output_dir
        .join(&fixture.manifest.name)
        .join("locationiq")
        .join(format!("{country}.csv"));
    write_geodata_rows_with_header(&output, &rows)?;
    println!(
        "stage=locationiq fixture={} country={} rows={}",
        fixture.manifest.name,
        country,
        rows.len()
    );
    Ok(())
}

pub fn run_production(options: &ProductionLocationiqOptions) -> Result<LocationiqOutcome, String> {
    if options.overwrite && options.output_file.exists() {
        fs::remove_file(&options.output_file).map_err(|error| {
            format!(
                "無法刪除既有 LocationIQ metadata {}：{error}",
                options.output_file.display()
            )
        })?;
    }
    let mut client = LocationiqHttpClient::new(options.api_key.clone(), options.qps)?;
    run_production_with_client(options, &mut client)
}

pub fn run_production_with_client<C: ReverseGeocoder>(
    options: &ProductionLocationiqOptions,
    client: &mut C,
) -> Result<LocationiqOutcome, String> {
    let mut rows = read_existing_meta(&options.output_file)?;
    let mut existing_coords: HashSet<(String, String)> = rows
        .iter()
        .map(|row| (row[0].clone(), row[1].clone()))
        .collect();
    let cities = read_cities_rows(&options.cities_file, b'\t')?;
    let total = cities
        .iter()
        .filter(|city| {
            city.len() >= 19
                && city[8] == options.country_code
                && !existing_coords.contains(&(
                    format_coordinate(&city[4]).unwrap_or_default(),
                    format_coordinate(&city[5]).unwrap_or_default(),
                ))
        })
        .count() as u64;
    let progress = ProgressReporter::new("locationiq", total);
    progress.start();
    let mut processed = 0_u64;
    let mut batch = Vec::new();

    for city in cities {
        if city.len() < 19 || city[8] != options.country_code {
            continue;
        }
        let latitude = format_coordinate(&city[4])?;
        let longitude = format_coordinate(&city[5])?;
        if existing_coords.contains(&(latitude.clone(), longitude.clone())) {
            continue;
        }

        processed += 1;
        progress.step(processed);

        match client.reverse(&latitude, &longitude) {
            Ok(Some(address)) => {
                let response = vec![
                    latitude.clone(),
                    longitude.clone(),
                    address.country,
                    address.state,
                    address.city,
                    address.county,
                    address.suburb,
                    address.neighbourhood,
                ];
                batch.push(build_geodata_row(&response));
                existing_coords.insert((latitude, longitude));
                if batch.len() >= options.batch_size.max(1) {
                    rows.append(&mut batch);
                    save_metadata_rows(&options.output_file, &rows)?;
                }
            }
            Ok(None) => {
                println!(
                    "locationiq_skip country={} geoname_id={} latitude={} longitude={} reason=no_response",
                    options.country_code, city[0], latitude, longitude
                );
            }
            Err(error) => {
                if !batch.is_empty() {
                    rows.append(&mut batch);
                    save_metadata_rows(&options.output_file, &rows)?;
                }
                progress.finish();
                if let HttpFailure::RateLimited { body } = &error {
                    let done = processed.saturating_sub(1);
                    println!(
                        "stage=locationiq stop=rate_limited country={} done={done} remaining={} body={body}",
                        options.country_code,
                        total.saturating_sub(done)
                    );
                    // Reason: allow_partial 只容忍「這一輪有推進但沒查完」。第一筆
                    // 就被限速代表這一輪完全沒有進度——金鑰失效、帳號被限制，或當日
                    // 額度已被別處用光。若連這種情況也算正常結束，CI 會是綠的、
                    // nightly 照發，但 auto-commit 沒有變更可收因此不開 PR，整條補查
                    // 路線就此永久停擺且沒有任何錯誤訊息。誤判只有「同一把金鑰當天已
                    // 被本地跑光」一種，那本來就該重跑，代價遠小於沉默失效。
                    if options.allow_partial && done > 0 {
                        return Ok(LocationiqOutcome::RateLimited);
                    }
                    if done == 0 {
                        return Err(format!(
                            "LocationIQ 第一筆查詢就被限速，這一輪沒有任何進度。\
                             請確認金鑰是否有效、當日額度是否已被其他執行用光。body={body}"
                        ));
                    }
                    return Err(format!(
                        "LocationIQ 額度用完，已查結果保留在 {}（完成 {done}/{total}）。\
                         換金鑰或等額度重置後重跑會自動從第 {} 點續查；\
                         若要讓額度用完算正常結束，加上 --locationiq-allow-partial。body={body}",
                        options.output_file.display(),
                        done + 1
                    ));
                }
                return Err(format!(
                    "LocationIQ API 錯誤，已 flush 目前批次；geoname_id={} latitude={} longitude={}：{error}",
                    city[0], latitude, longitude
                ));
            }
        }
    }

    if !batch.is_empty() {
        rows.append(&mut batch);
        save_metadata_rows(&options.output_file, &rows)?;
    } else if !rows.is_empty() && !options.output_file.exists() {
        save_metadata_rows(&options.output_file, &rows)?;
    }
    progress.finish();
    println!(
        "stage=locationiq mode=production country={} output={} rows={}",
        options.country_code,
        options.output_file.display(),
        rows.len()
    );
    Ok(LocationiqOutcome::Completed)
}

fn read_existing_meta(path: &Path) -> Result<Vec<Vec<String>>, String> {
    if !path.exists() {
        return Ok(Vec::new());
    }
    let mut rows = read_geodata_rows_with_header(path)?;
    for row in &mut rows {
        row[0] = format_coordinate(&row[0])?;
        row[1] = format_coordinate(&row[1])?;
    }
    Ok(rows)
}

fn save_metadata_rows(path: &Path, rows: &[Vec<String>]) -> Result<(), String> {
    write_geodata_rows_with_header(path, rows)
}

fn read_responses(path: &Path) -> Result<HashMap<(String, String), Vec<String>>, String> {
    let rows = read_delimited(path, ',', true)?;
    let mut responses = HashMap::new();
    for mut row in rows {
        if row.len() != 8 {
            return Err(format!(
                "LocationIQ response 欄位數不符：{}",
                path.display()
            ));
        }
        row[0] = format_coordinate(&row[0])?;
        row[1] = format_coordinate(&row[1])?;
        responses.insert((row[0].clone(), row[1].clone()), row);
    }
    Ok(responses)
}

/// 將 LocationIQ 回應欄位轉為 geodata 列。
///
/// Reason: 有官方圖資 handler 的國家（TW/JP/KR/TH/ID）在 CLI 已由
/// `filter_country_codes_without_handler` 濾掉，不會進入本階段，因此這裡不做
/// 任何國家特化對應，一律沿用 LocationIQ 回應的行政區層級。
fn build_geodata_row(response: &[String]) -> Vec<String> {
    // Reason: LocationIQ 對部分國家只回傳 county 而不回傳 city，兩者同屬二級行政區。
    let admin2 = if response[4].is_empty() {
        response[5].clone()
    } else {
        response[4].clone()
    };

    vec![
        response[0].clone(),
        response[1].clone(),
        response[2].clone(),
        response[3].clone(),
        admin2,
        response[6].clone(),
        response[7].clone(),
    ]
}

fn parse_locationiq_address(body: &str) -> Result<LocationiqAddress, String> {
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
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    struct TestDir {
        path: PathBuf,
    }

    impl TestDir {
        fn new(name: &str) -> Self {
            let nanos = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let path = std::env::temp_dir().join(format!(
                "immich-geodata-locationiq-{name}-{}-{nanos}",
                std::process::id()
            ));
            fs::create_dir_all(&path).unwrap();
            Self { path }
        }
    }

    impl Drop for TestDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    struct StubClient {
        responses: Vec<Result<Option<LocationiqAddress>, HttpFailure>>,
    }

    impl ReverseGeocoder for StubClient {
        fn reverse(
            &mut self,
            _latitude: &str,
            _longitude: &str,
        ) -> Result<Option<LocationiqAddress>, HttpFailure> {
            self.responses.remove(0)
        }
    }

    /// 兩列 US 測試資料，第二列用來觸發指定的失敗。
    fn two_city_fixture(temp: &TestDir) -> PathBuf {
        let cities = temp.path.join("cities500_optimized.txt");
        fs::write(
            &cities,
            "1\tA\tA\t\t40.00000000\t-74.00000000\tP\tPPL\tUS\t\tNY\t\t\t\t0\t\t\tAmerica/New_York\t2026-01-01\n2\tB\tB\t\t41.00000000\t-73.00000000\tP\tPPL\tUS\t\tNY\t\t\t\t0\t\t\tAmerica/New_York\t2026-01-01\n",
        )
        .unwrap();
        cities
    }

    fn stub_success() -> Result<Option<LocationiqAddress>, HttpFailure> {
        Ok(Some(LocationiqAddress {
            country: "美國".to_string(),
            state: "紐約州".to_string(),
            city: "紐約".to_string(),
            county: String::new(),
            suburb: "曼哈頓".to_string(),
            neighbourhood: "蘇活區".to_string(),
        }))
    }

    fn rate_limit_options(
        cities: PathBuf,
        output: PathBuf,
        allow_partial: bool,
    ) -> ProductionLocationiqOptions {
        ProductionLocationiqOptions {
            cities_file: cities,
            output_file: output,
            country_code: "US".to_string(),
            batch_size: 10,
            qps: 1,
            api_key: "test".to_string(),
            overwrite: false,
            allow_partial,
        }
    }

    /// 額度用完且允許部分完成時，必須回報 RateLimited 並保留已查結果。
    #[test]
    fn production_locationiq_stops_gracefully_on_rate_limit_when_allowed() {
        let temp = TestDir::new("rate-limit-allowed");
        let cities = two_city_fixture(&temp);
        let output = temp.path.join("US.csv");
        let mut client = StubClient {
            responses: vec![
                stub_success(),
                Err(HttpFailure::RateLimited {
                    body: r#"{"error":"Rate Limited Day"}"#.to_string(),
                }),
            ],
        };

        let outcome = run_production_with_client(
            &rate_limit_options(cities, output.clone(), true),
            &mut client,
        )
        .unwrap();

        assert_eq!(outcome, LocationiqOutcome::RateLimited);
        let saved = fs::read_to_string(output).unwrap();
        assert!(saved.contains("紐約"), "已查結果必須保留");
    }

    /// 預設（本地補查）不得把「沒查完」當成功，否則會拿半套資料往下發布。
    #[test]
    fn production_locationiq_fails_on_rate_limit_by_default() {
        let temp = TestDir::new("rate-limit-default");
        let cities = two_city_fixture(&temp);
        let output = temp.path.join("US.csv");
        let mut client = StubClient {
            responses: vec![
                stub_success(),
                Err(HttpFailure::RateLimited {
                    body: r#"{"error":"Rate Limited Day"}"#.to_string(),
                }),
            ],
        };

        let error = run_production_with_client(
            &rate_limit_options(cities, output.clone(), false),
            &mut client,
        )
        .unwrap_err();

        assert!(
            error.contains("額度用完"),
            "錯誤訊息要說明是額度問題：{error}"
        );
        assert!(
            error.contains("--locationiq-allow-partial"),
            "錯誤訊息要指出 CI 該怎麼改：{error}"
        );
        let saved = fs::read_to_string(output).unwrap();
        assert!(saved.contains("紐約"), "失敗路徑也必須保留已查結果");
    }

    /// 第一筆就被限速代表這一輪零進度，即使開了 allow_partial 也必須失敗。
    ///
    /// Reason: 若這種情況算正常結束，CI 會綠、nightly 照發，但沒有新資料所以
    /// auto-commit 不開 PR——金鑰失效會變成永遠查不到的沉默失效。
    #[test]
    fn production_locationiq_fails_when_rate_limited_with_zero_progress() {
        let temp = TestDir::new("rate-limit-no-progress");
        let cities = two_city_fixture(&temp);
        let output = temp.path.join("US.csv");
        let mut client = StubClient {
            responses: vec![Err(HttpFailure::RateLimited {
                body: r#"{"error":"Rate Limited Day"}"#.to_string(),
            })],
        };

        let error =
            run_production_with_client(&rate_limit_options(cities, output, true), &mut client)
                .unwrap_err();

        assert!(
            error.contains("第一筆查詢就被限速"),
            "錯誤訊息要點出零進度：{error}"
        );
    }

    /// 非限速錯誤即使開了 allow_partial 也必須失敗。
    #[test]
    fn production_locationiq_still_fails_on_non_rate_limit_error_when_partial_allowed() {
        let temp = TestDir::new("rate-limit-other");
        let cities = two_city_fixture(&temp);
        let output = temp.path.join("US.csv");
        let mut client = StubClient {
            responses: vec![
                stub_success(),
                Err(HttpFailure::Other("金鑰無效".to_string())),
            ],
        };

        let error =
            run_production_with_client(&rate_limit_options(cities, output, false), &mut client);
        assert!(error.is_err());

        let mut client = StubClient {
            responses: vec![
                stub_success(),
                Err(HttpFailure::Other("金鑰無效".to_string())),
            ],
        };
        let temp = TestDir::new("rate-limit-other-allowed");
        let cities = two_city_fixture(&temp);
        let output = temp.path.join("US.csv");
        let error =
            run_production_with_client(&rate_limit_options(cities, output, true), &mut client)
                .unwrap_err();
        assert!(error.contains("金鑰無效"), "{error}");
    }

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

    #[test]
    fn production_locationiq_flushes_batch_before_abort() {
        let temp = TestDir::new("flush-error");
        let cities = temp.path.join("cities500_optimized.txt");
        fs::write(
            &cities,
            "1\tA\tA\t\t40.00000000\t-74.00000000\tP\tPPL\tUS\t\tNY\t\t\t\t0\t\t\tAmerica/New_York\t2026-01-01\n2\tB\tB\t\t41.00000000\t-73.00000000\tP\tPPL\tUS\t\tNY\t\t\t\t0\t\t\tAmerica/New_York\t2026-01-01\n",
        )
        .unwrap();
        let output = temp.path.join("US.csv");
        let mut client = StubClient {
            responses: vec![
                Ok(Some(LocationiqAddress {
                    country: "美國".to_string(),
                    state: "紐約州".to_string(),
                    city: "紐約".to_string(),
                    county: String::new(),
                    suburb: "曼哈頓".to_string(),
                    neighbourhood: "蘇活區".to_string(),
                })),
                Err(HttpFailure::Other("quota".to_string())),
            ],
        };
        let options = ProductionLocationiqOptions {
            cities_file: cities,
            output_file: output.clone(),
            country_code: "US".to_string(),
            batch_size: 10,
            qps: 1,
            api_key: "test".to_string(),
            overwrite: false,
            allow_partial: false,
        };

        let error = run_production_with_client(&options, &mut client).unwrap_err();

        assert!(error.contains("已 flush"));
        let saved = fs::read_to_string(output).unwrap();
        assert!(saved.contains("紐約"));
        assert!(saved.contains("曼哈頓"));
    }

    #[test]
    fn build_geodata_row_keeps_response_admin_levels() {
        // Reason: 舊版對 TW 會把直轄市層級整列上移，TW 改由官方圖資 handler 產生後
        // 這個階段不得再做任何國家特化搬移，否則會與 handler 產出的層級不一致。
        let response = vec![
            "25.03396400".to_string(),
            "121.56446800".to_string(),
            "臺灣".to_string(),
            "Taipei".to_string(),
            "臺北市".to_string(),
            String::new(),
            "信義區".to_string(),
            "西村里".to_string(),
        ];

        let row = build_geodata_row(&response);

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

    #[test]
    fn build_geodata_row_falls_back_to_county_when_city_is_empty() {
        let response = vec![
            "40.00000000".to_string(),
            "-74.00000000".to_string(),
            "美國".to_string(),
            "紐約州".to_string(),
            String::new(),
            "威徹斯特郡".to_string(),
            "郊區".to_string(),
            String::new(),
        ];

        let row = build_geodata_row(&response);

        assert_eq!(row[4], "威徹斯特郡");
        assert_eq!(row[5], "郊區");
        assert_eq!(row[6], "");
    }
}
