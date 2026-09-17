use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;
use std::path::PathBuf;

use crate::cli::RunOptions;
use crate::http::HttpFailure;
use crate::observability::ProgressReporter;
use crate::pipeline::fixtures::{Fixture, load_fixtures};
use crate::pipeline::polars_table::{
    read_cities_rows, read_geodata_rows_with_header, write_geodata_rows_with_header,
};
use crate::pipeline::table::{format_coordinate, read_delimited};

mod address;
mod client;

pub use address::{AddressFields, LocationiqAddress};
use address::{build_geodata_row, parse_locationiq_address};
pub use client::{LocationiqHttpClient, build_locationiq_url};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProductionLocationiqOptions {
    pub cities_file: PathBuf,
    pub output_file: PathBuf,
    pub country_code: String,
    pub batch_size: usize,
    pub qps: u32,
    pub api_key: String,
    pub overwrite: bool,
    /// `address_fields.json` 的路徑。
    ///
    /// Reason: production 讀 `data/locationiq/`，fixture 讀自己 root 下的同名檔，
    /// 兩邊走同一段查表邏輯，fixture 才能真的覆蓋「未登記國家即中止」這條路徑。
    pub address_fields_file: PathBuf,
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
    let address_fields_file = fixture.root.join("locationiq").join("address_fields.json");
    let address_fields = AddressFields::load(&address_fields_file)?;
    let city_keys = address_fields
        .city_keys(country, &address_fields_file)?
        .to_vec();
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
        let address = responses
            .get(&(latitude.clone(), longitude.clone()))
            .ok_or_else(|| format!("LocationIQ fixture 缺少座標回應：{latitude},{longitude}"))?;
        rows.push(build_geodata_row(
            &latitude, &longitude, address, &city_keys,
        ));
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
    // Reason: 設定驗證必須在刪檔之前。`--overwrite` 正是「改了 city_keys 要重查」
    // 的用法，而那時最可能同時發生的失誤就是國家沒登記或國碼打錯——先刪再驗會
    // 讓既有的付費查詢結果在報錯前就消失。
    AddressFields::load(&options.address_fields_file)?
        .city_keys(&options.country_code, &options.address_fields_file)?;
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
    // Reason: 設定查表放在最前面，未登記的國家在任何一次付費查詢之前就中止。
    let address_fields = AddressFields::load(&options.address_fields_file)?;
    let city_keys = address_fields
        .city_keys(&options.country_code, &options.address_fields_file)?
        .to_vec();
    let city_keys = city_keys.as_slice();
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
    let mut not_found = 0_u64;
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
                batch.push(build_geodata_row(
                    &latitude, &longitude, &address, city_keys,
                ));
                existing_coords.insert((latitude, longitude));
                if batch.len() >= options.batch_size.max(1) {
                    rows.append(&mut batch);
                    save_metadata_rows(&options.output_file, &rows)?;
                }
            }
            Ok(None) => {
                not_found += 1;
                println!(
                    "locationiq_skip country={} geoname_id={} latitude={} longitude={} reason=not_found",
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
                    // Reason: 重試耗盡的 429 一律當成額度用完，不解析 body 區分是
                    // 每分鐘還是每日上限——那是外部 API 未文件化的字串，比對錯了
                    // 會把綠燈變紅燈。誤判的後果也不對稱：分鐘上限被誤當每日上限
                    // 只是這一輪少補一些點（已查結果照常提交，下次接續），不是停擺。
                    //
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
    // Reason: `not_found` 進摘要行而不是設成失敗條件。這些座標不寫進 metadata，
    // 每輪都會重查；待查清空後剩下的正好全是查不到的點，任何「全是 404 就失敗」
    // 的規則都會在那一刻起每週紅燈。數字留在日誌供人判讀即可。
    println!(
        "stage=locationiq mode=production country={} output={} rows={} not_found={not_found}",
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

/// 讀 fixture 的 `responses.csv`。
///
/// 欄位：latitude,longitude,country,state,district,city,county,suburb,neighbourhood
fn read_responses(path: &Path) -> Result<HashMap<(String, String), LocationiqAddress>, String> {
    let rows = read_delimited(path, ',', true)?;
    let mut responses = HashMap::new();
    for row in rows {
        if row.len() != 9 {
            return Err(format!(
                "LocationIQ response 欄位數不符：{}",
                path.display()
            ));
        }
        let latitude = format_coordinate(&row[0])?;
        let longitude = format_coordinate(&row[1])?;
        let address = LocationiqAddress {
            country: row[2].clone(),
            state: row[3].clone(),
            district: row[4].clone(),
            city: row[5].clone(),
            county: row[6].clone(),
            suburb: row[7].clone(),
            neighbourhood: row[8].clone(),
        };
        responses.insert((latitude, longitude), address);
    }
    Ok(responses)
}

/// 各國要讀哪些 address 欄位當城市名。
///
/// Reason: OSM 的 address schema 逐國不同，沒有通用的欄位名。實測 5 國：
/// MY 的 `district` 是 daerah（出現率 80%）、IT 的 `county` 是 provincia、
/// GB 的 `city` 是 local authority；但 VN 沒有 `district`，其 `city` 是省級
/// 直轄市，PH 三個欄位全無。寫死任何一條順序都會在某些國家拿到錯層級。
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

    /// 查不到的座標只跳過該點，不中止整輪，且不寫進 metadata（下輪會重試）。
    #[test]
    fn production_locationiq_skips_point_without_response() {
        let temp = TestDir::new("not-found-skip");
        let cities = two_city_fixture(&temp);
        let fields = address_fields_file(&temp, "US", "\"city\"");
        let output = temp.path.join("US.csv");
        let mut client = StubClient {
            responses: vec![Ok(None), stub_success()],
        };

        let outcome = run_production_with_client(
            &rate_limit_options(cities, output.clone(), fields, false),
            &mut client,
        )
        .unwrap();

        assert_eq!(outcome, LocationiqOutcome::Completed);
        let saved = fs::read_to_string(output).unwrap();
        assert!(
            !saved.contains("40.0,-74.0"),
            "查不到的座標不得寫進 metadata，否則下一輪不會重試：{saved}"
        );
        assert!(
            saved.contains("41.0,-73.0"),
            "同一輪後續的查詢必須照常寫入：{saved}"
        );
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
            ..LocationiqAddress::default()
        }))
    }

    /// 寫一份只登記 `country` 的 `address_fields.json`，回傳其路徑。
    fn address_fields_file(temp: &TestDir, country: &str, keys: &str) -> PathBuf {
        let path = temp.path.join("address_fields.json");
        fs::write(
            &path,
            format!(r#"{{"{country}": {{"city_keys": [{keys}]}}}}"#),
        )
        .unwrap();
        path
    }

    fn rate_limit_options(
        cities: PathBuf,
        output: PathBuf,
        address_fields: PathBuf,
        allow_partial: bool,
    ) -> ProductionLocationiqOptions {
        ProductionLocationiqOptions {
            cities_file: cities,
            output_file: output,
            address_fields_file: address_fields,
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
        let fields = address_fields_file(&temp, "US", "\"city\"");
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
            &rate_limit_options(cities, output.clone(), fields, true),
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
        let fields = address_fields_file(&temp, "US", "\"city\"");
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
            &rate_limit_options(cities, output.clone(), fields, false),
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
        let fields = address_fields_file(&temp, "US", "\"city\"");
        let output = temp.path.join("US.csv");
        let mut client = StubClient {
            responses: vec![Err(HttpFailure::RateLimited {
                body: r#"{"error":"Rate Limited Day"}"#.to_string(),
            })],
        };

        let error = run_production_with_client(
            &rate_limit_options(cities, output, fields, true),
            &mut client,
        )
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
        let fields = address_fields_file(&temp, "US", "\"city\"");
        let output = temp.path.join("US.csv");
        let mut client = StubClient {
            responses: vec![
                stub_success(),
                Err(HttpFailure::Other("金鑰無效".to_string())),
            ],
        };

        let error = run_production_with_client(
            &rate_limit_options(cities, output, fields, false),
            &mut client,
        );
        assert!(error.is_err());

        let mut client = StubClient {
            responses: vec![
                stub_success(),
                Err(HttpFailure::Other("金鑰無效".to_string())),
            ],
        };
        let temp = TestDir::new("rate-limit-other-allowed");
        let cities = two_city_fixture(&temp);
        let fields = address_fields_file(&temp, "US", "\"city\"");
        let output = temp.path.join("US.csv");
        let error = run_production_with_client(
            &rate_limit_options(cities, output, fields, true),
            &mut client,
        )
        .unwrap_err();
        assert!(error.contains("金鑰無效"), "{error}");
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
                    suburb: "曼哈頓".to_string(),
                    neighbourhood: "蘇活區".to_string(),
                    ..LocationiqAddress::default()
                })),
                Err(HttpFailure::Other("quota".to_string())),
            ],
        };
        let options = ProductionLocationiqOptions {
            cities_file: cities,
            output_file: output.clone(),
            address_fields_file: address_fields_file(&temp, "US", "\"city\""),
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
    fn address_fields_rejects_unregistered_country() {
        let temp = TestDir::new("address-fields-unregistered");
        let path = address_fields_file(&temp, "MY", "\"district\"");

        let error = AddressFields::load(&path)
            .unwrap()
            .city_keys("VN", &path)
            .unwrap_err();

        assert!(error.contains("未登記 VN"), "訊息應點名國碼：{error}");
        assert!(
            error.contains("address_fields.json"),
            "訊息應指出要改哪個檔：{error}"
        );
    }

    /// 未登記的國家必須在任何一次付費查詢之前中止。
    ///
    /// Reason: 診斷訊息若靠「先查一筆再印出實際欄位」取得，就等於讓沒設定好的國家
    /// 花掉額度。這裡以「stub 一次都沒被呼叫」斷言那條順序。
    #[test]
    fn production_locationiq_aborts_before_any_query_when_country_unregistered() {
        let temp = TestDir::new("unregistered-country");
        let cities = two_city_fixture(&temp);
        let output = temp.path.join("US.csv");
        let fields = address_fields_file(&temp, "MY", "\"district\"");
        let mut client = StubClient {
            responses: vec![stub_success(), stub_success()],
        };

        let error = run_production_with_client(
            &rate_limit_options(cities, output.clone(), fields, false),
            &mut client,
        )
        .unwrap_err();

        assert!(error.contains("未登記 US"), "訊息應點名國碼：{error}");
        assert_eq!(
            client.responses.len(),
            2,
            "未登記的國家不得發出任何查詢，stub 回應應原封不動"
        );
        assert!(!output.exists(), "中止時不得寫出輸出檔");
    }

    /// `city_keys` 填了本階段無法解析的欄位時，載入就要失敗。
    ///
    /// Reason: `city_name` 走訪優先鏈時會跳過取不到的 key。沒有這道驗證，拼錯
    /// （`distict`）或填了未解析的欄位（VN/PH 的 `town`）會讓整國城市名全空，
    /// 而且要等額度花完才看得出來。
    #[test]
    fn address_fields_rejects_unsupported_key() {
        let temp = TestDir::new("address-fields-unsupported-key");
        let path = address_fields_file(&temp, "PH", "\"town\"");

        let error = AddressFields::load(&path).unwrap_err();

        assert!(error.contains("town"), "訊息應點名該欄位：{error}");
        assert!(error.contains("district"), "訊息應列出可用欄位：{error}");
    }

    /// `--overwrite` 必須先驗設定再刪檔。
    ///
    /// Reason: `--overwrite` 正是「改了 city_keys 要重查」的用法，而那時最容易
    /// 同時發生的失誤就是國家沒登記或國碼打錯。先刪再驗會讓既有的付費查詢結果
    /// 在報錯之前就消失。
    #[test]
    fn overwrite_validates_config_before_deleting_existing_results() {
        let temp = TestDir::new("overwrite-validates-first");
        let cities = two_city_fixture(&temp);
        let output = temp.path.join("US.csv");
        fs::write(
            &output,
            "latitude,longitude,country,admin_1,admin_2,admin_3,admin_4\n",
        )
        .unwrap();
        let fields = address_fields_file(&temp, "MY", "\"district\"");

        let mut options = rate_limit_options(cities, output.clone(), fields, false);
        options.overwrite = true;
        let error = run_production(&options).unwrap_err();

        assert!(error.contains("未登記 US"), "訊息應點名國碼：{error}");
        assert!(output.exists(), "設定驗證失敗時不得刪除既有的付費查詢結果");
    }

    #[test]
    fn address_fields_rejects_empty_city_keys() {
        let temp = TestDir::new("address-fields-empty");
        let path = address_fields_file(&temp, "MY", "");

        let error = AddressFields::load(&path).unwrap_err();

        assert!(error.contains("不得為空"), "{error}");
    }
}
