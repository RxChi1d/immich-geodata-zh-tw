use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::time::Duration;

use reqwest::Url;

use crate::cli::RunOptions;
use crate::http::{HttpClient, HttpRequestPolicy};
use crate::observability::ProgressReporter;
use crate::pipeline::fixtures::{Fixture, load_fixtures};
use crate::pipeline::polars_table::{
    read_cities_rows, read_geodata_rows_with_header, read_string_rows,
    write_geodata_rows_with_header,
};
use crate::pipeline::table::{format_coordinate, read_delimited};

const MUNICIPALITIES: [&str; 9] = [
    "臺北市",
    "新北市",
    "桃園市",
    "臺中市",
    "臺南市",
    "高雄市",
    "基隆市",
    "新竹市",
    "嘉義市",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProductionLocationiqOptions {
    pub cities_file: PathBuf,
    pub output_file: PathBuf,
    pub country_code: String,
    pub batch_size: usize,
    pub qps: u32,
    pub api_key: String,
    pub overwrite: bool,
    pub tw_admin1_map: Option<PathBuf>,
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
    fn reverse(
        &mut self,
        latitude: &str,
        longitude: &str,
    ) -> Result<Option<LocationiqAddress>, String>;
}

pub struct LocationiqHttpClient {
    api_key: String,
    http: HttpClient,
}

impl LocationiqHttpClient {
    pub fn new(api_key: String, qps: u32) -> Result<Self, String> {
        let delay_ms = 1020_u64 / u64::from(qps.max(1));
        let policy = HttpRequestPolicy {
            user_agent: "immich-geodata-zh-tw-rust/0.1 LocationIQ".to_string(),
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
    ) -> Result<Option<LocationiqAddress>, String> {
        let url = build_locationiq_url(latitude, longitude, &self.api_key)?;
        let body = self.http.get_text(url.as_str()).map_err(|error| {
            format!(
                "LocationIQ 查詢失敗 url={}：{error}",
                redact_locationiq_key(url.as_str())
            )
        })?;
        Ok(Some(parse_locationiq_address(&body)?))
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
    let admin1_map = read_admin1_map(&fixture.root.join("locationiq").join("tw_admin1_map.csv"))?;
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
        rows.push(build_geodata_row(country, &city, response, &admin1_map)?);
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

pub fn run_production(options: &ProductionLocationiqOptions) -> Result<(), String> {
    if options.overwrite && options.output_file.exists() {
        fs::remove_file(&options.output_file).map_err(|error| {
            format!(
                "無法刪除既有 LocationIQ metadata {}：{error}",
                options.output_file.display()
            )
        })?;
    }
    let admin1_map = match &options.tw_admin1_map {
        Some(path) if path.exists() => read_admin1_map(path)?,
        _ => HashMap::new(),
    };
    let mut client = LocationiqHttpClient::new(options.api_key.clone(), options.qps)?;
    run_production_with_client(options, &admin1_map, &mut client)
}

pub fn run_production_with_client<C: ReverseGeocoder>(
    options: &ProductionLocationiqOptions,
    admin1_map: &HashMap<String, String>,
    client: &mut C,
) -> Result<(), String> {
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
                batch.push(build_geodata_row(
                    &options.country_code,
                    &city,
                    &response,
                    admin1_map,
                )?);
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
    Ok(())
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

fn read_admin1_map(path: &Path) -> Result<HashMap<String, String>, String> {
    let rows = read_string_rows(path, b',', true, &["new_id", "name"])?;
    Ok(rows
        .into_iter()
        .filter(|row| row.len() >= 2)
        .map(|row| (row[0].clone(), row[1].clone()))
        .collect())
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

fn build_geodata_row(
    country: &str,
    city: &[String],
    response: &[String],
    admin1_map: &HashMap<String, String>,
) -> Result<Vec<String>, String> {
    let latitude = response[0].clone();
    let longitude = response[1].clone();
    let response_country = response[2].clone();
    let mut admin1 = response[3].clone();
    let mut admin2 = if response[4].is_empty() {
        response[5].clone()
    } else {
        response[4].clone()
    };
    let mut admin3 = response[6].clone();
    let mut admin4 = response[7].clone();

    if country == "TW" {
        let key = format!("TW.{}", city[10]);
        if let Some(mapped) = admin1_map.get(&key) {
            admin1 = mapped.clone();
        }
        if MUNICIPALITIES.contains(&admin2.as_str()) {
            admin2 = admin3;
            admin3 = admin4;
            admin4 = String::new();
        }
    }

    Ok(vec![
        latitude,
        longitude,
        response_country,
        admin1,
        admin2,
        admin3,
        admin4,
    ])
}

fn parse_locationiq_address(body: &str) -> Result<LocationiqAddress, String> {
    let address = json_object(body, "address")
        .ok_or_else(|| "LocationIQ 回應缺少 address 物件".to_string())?;
    Ok(LocationiqAddress {
        country: json_string(&address, "country").unwrap_or_default(),
        state: json_string(&address, "state").unwrap_or_default(),
        city: json_string(&address, "city").unwrap_or_default(),
        county: json_string(&address, "county").unwrap_or_default(),
        suburb: json_string(&address, "suburb").unwrap_or_default(),
        neighbourhood: json_string(&address, "neighbourhood").unwrap_or_default(),
    })
}

fn json_object(body: &str, key: &str) -> Option<String> {
    let marker = format!("\"{key}\"");
    let start = body.find(&marker)?;
    let open_offset = body[start..].find('{')?;
    let open = start + open_offset;
    let mut depth = 0_i32;
    for (offset, char) in body[open..].char_indices() {
        match char {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(body[open..open + offset + 1].to_string());
                }
            }
            _ => {}
        }
    }
    None
}

fn json_string(body: &str, key: &str) -> Option<String> {
    let marker = format!("\"{key}\"");
    let start = body.find(&marker)?;
    let colon_offset = body[start..].find(':')?;
    let after_colon = start + colon_offset + 1;
    let quote_offset = body[after_colon..].find('"')?;
    let value_start = after_colon + quote_offset + 1;
    let mut value = String::new();
    let mut escaped = false;
    for char in body[value_start..].chars() {
        if escaped {
            value.push(char);
            escaped = false;
            continue;
        }
        match char {
            '\\' => escaped = true,
            '"' => return Some(value),
            _ => value.push(char),
        }
    }
    None
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
        responses: Vec<Result<Option<LocationiqAddress>, String>>,
    }

    impl ReverseGeocoder for StubClient {
        fn reverse(
            &mut self,
            _latitude: &str,
            _longitude: &str,
        ) -> Result<Option<LocationiqAddress>, String> {
            self.responses.remove(0)
        }
    }

    #[test]
    fn production_locationiq_flushes_batch_before_abort() {
        let temp = TestDir::new("flush-error");
        let cities = temp.path.join("cities500_optimized.txt");
        fs::write(
            &cities,
            "1\tA\tA\t\t25.00000000\t121.00000000\tP\tPPL\tTW\t\t01\t\t\t\t0\t\t\tAsia/Taipei\t2026-01-01\n2\tB\tB\t\t24.00000000\t120.00000000\tP\tPPL\tTW\t\t02\t\t\t\t0\t\t\tAsia/Taipei\t2026-01-01\n",
        )
        .unwrap();
        let output = temp.path.join("TW.csv");
        let mut admin1 = HashMap::new();
        admin1.insert("TW.01".to_string(), "臺北市".to_string());
        let mut client = StubClient {
            responses: vec![
                Ok(Some(LocationiqAddress {
                    country: "臺灣".to_string(),
                    state: "Taiwan".to_string(),
                    city: "臺北市".to_string(),
                    county: String::new(),
                    suburb: "信義區".to_string(),
                    neighbourhood: "西村里".to_string(),
                })),
                Err("quota".to_string()),
            ],
        };
        let options = ProductionLocationiqOptions {
            cities_file: cities,
            output_file: output.clone(),
            country_code: "TW".to_string(),
            batch_size: 10,
            qps: 2,
            api_key: "test".to_string(),
            overwrite: false,
            tw_admin1_map: None,
        };

        let error = run_production_with_client(&options, &admin1, &mut client).unwrap_err();

        assert!(error.contains("已 flush"));
        let saved = fs::read_to_string(output).unwrap();
        assert!(saved.contains("臺北市"));
        assert!(saved.contains("信義區"));
    }
}
