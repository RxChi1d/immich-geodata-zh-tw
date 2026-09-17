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
    fn city_name(&self, city_keys: &[String]) -> String {
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

    fn field(&self, key: &str) -> Option<&String> {
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
fn redact_locationiq_failure(url: &str, failure: HttpFailure) -> HttpFailure {
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
    fn city_keys(&self, country: &str, config_path: &Path) -> Result<&[String], String> {
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
fn build_geodata_row(
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
