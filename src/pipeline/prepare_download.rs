use std::fs;
use std::fs::File;
use std::io;
use std::path::Path;

use crate::http::HttpClient;
use crate::pipeline::prepare::ProductionPrepareOptions;

pub const GEONAMES_BASE_URL: &str = "https://download.geonames.org/export/dump";
pub const NATURAL_EARTH_URL: &str = "https://raw.githubusercontent.com/nvkelso/natural-earth-vector/master/geojson/ne_10m_admin_0_countries.geojson";

pub fn run_production(options: &ProductionPrepareOptions) -> Result<(), String> {
    run_production_with_downloader(options, &download_file)
}

fn run_production_with_downloader<F>(
    options: &ProductionPrepareOptions,
    downloader: &F,
) -> Result<(), String>
where
    F: Fn(&str, &Path) -> Result<(), String>,
{
    let extra_data_dir = options.target_dir.join("extra_data");
    if options.update && options.target_dir.exists() {
        fs::remove_dir_all(&options.target_dir).map_err(|error| {
            format!(
                "無法清理 prepare 目錄 {}：{error}",
                options.target_dir.display()
            )
        })?;
    }

    fs::create_dir_all(&extra_data_dir).map_err(|error| {
        format!(
            "無法建立 prepare 目錄 {}：{error}",
            extra_data_dir.display()
        )
    })?;

    download_zip_member_if_missing(
        &format!("{}/cities500.zip", options.geonames_base_url),
        &options.target_dir.join("cities500.zip"),
        &options.target_dir.join("cities500.txt"),
        downloader,
    )?;

    for country in &options.countries {
        download_zip_member_if_missing(
            &format!("{}/{}.zip", options.geonames_base_url, country),
            &extra_data_dir.join(format!("{country}.zip")),
            &extra_data_dir.join(format!("{country}.txt")),
            downloader,
        )?;
    }

    download_plain_if_missing(
        &format!("{}/admin1CodesASCII.txt", options.geonames_base_url),
        &options.target_dir.join("admin1CodesASCII.txt"),
        downloader,
    )?;
    download_plain_if_missing(
        &format!("{}/admin2Codes.txt", options.geonames_base_url),
        &options.target_dir.join("admin2Codes.txt"),
        downloader,
    )?;
    download_plain_if_missing(
        &options.natural_earth_url,
        &options.target_dir.join("ne_10m_admin_0_countries.geojson"),
        downloader,
    )?;
    download_zip_member_if_missing(
        &format!("{}/alternateNamesV2.zip", options.geonames_base_url),
        &options.target_dir.join("alternateNamesV2.zip"),
        &options.target_dir.join("alternateNamesV2.txt"),
        downloader,
    )?;

    println!(
        "stage=prepare mode=production target={} countries={}",
        options.target_dir.display(),
        options.countries.join(",")
    );
    Ok(())
}

fn download_plain_if_missing<F>(url: &str, output_path: &Path, downloader: &F) -> Result<(), String>
where
    F: Fn(&str, &Path) -> Result<(), String>,
{
    if output_path.exists() {
        println!("prepare_skip file={}", output_path.display());
        return Ok(());
    }
    downloader(url, output_path)
}

fn download_zip_member_if_missing<F>(
    url: &str,
    zip_path: &Path,
    required_file: &Path,
    downloader: &F,
) -> Result<(), String>
where
    F: Fn(&str, &Path) -> Result<(), String>,
{
    if required_file.exists() {
        println!("prepare_skip file={}", required_file.display());
        return Ok(());
    }
    downloader(url, zip_path)?;
    extract_required_zip_member(zip_path, required_file)?;
    if !required_file.exists() {
        return Err(format!(
            "解壓後缺少必要檔案 {}，請確認來源檔案格式是否改變",
            required_file.display()
        ));
    }
    fs::remove_file(zip_path)
        .map_err(|error| format!("無法刪除暫存 zip {}：{error}", zip_path.display()))?;
    Ok(())
}

fn download_file(url: &str, output_path: &Path) -> Result<(), String> {
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("無法建立下載目錄 {}：{error}", parent.display()))?;
    }
    let temp_path = output_path.with_extension("download");
    let client = HttpClient::with_default_policy()?;
    let bytes = client.get_bytes(url)?;
    fs::write(&temp_path, bytes)
        .map_err(|error| format!("無法寫入暫存下載檔案 {}：{error}", temp_path.display()))?;

    fs::rename(&temp_path, output_path).map_err(|error| {
        format!(
            "無法保存下載檔案 {} 到 {}：{error}",
            temp_path.display(),
            output_path.display()
        )
    })?;
    println!(
        "prepare_download url={url} output={}",
        output_path.display()
    );
    Ok(())
}

fn extract_required_zip_member(zip_path: &Path, required_file: &Path) -> Result<(), String> {
    let expected_name = required_file
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| format!("無法判斷必要 zip member：{}", required_file.display()))?;
    let source = File::open(zip_path)
        .map_err(|error| format!("無法開啟 zip {}：{error}", zip_path.display()))?;
    let mut archive = zip::ZipArchive::new(source)
        .map_err(|error| format!("zip 解壓失敗 {}：{error}", zip_path.display()))?;
    let mut matching_index = None;
    for index in 0..archive.len() {
        let file = archive
            .by_index(index)
            .map_err(|error| format!("zip 解壓失敗 {}：{error}", zip_path.display()))?;
        let Some(path) = file.enclosed_name() else {
            continue;
        };
        if path.file_name().and_then(|name| name.to_str()) == Some(expected_name) {
            matching_index = Some(index);
            break;
        }
    }
    let Some(index) = matching_index else {
        return Err(format!(
            "解壓後缺少必要檔案 {}，請確認來源檔案格式是否改變",
            required_file.display()
        ));
    };
    if let Some(parent) = required_file.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("無法建立解壓目錄 {}：{error}", parent.display()))?;
    }
    let mut source = archive
        .by_index(index)
        .map_err(|error| format!("zip 解壓失敗 {}：{error}", zip_path.display()))?;
    if source.is_dir() {
        return Err(format!("zip member 不是檔案：{expected_name}"));
    }
    let temp_path = required_file.with_extension("extract");
    let mut output = File::create(&temp_path)
        .map_err(|error| format!("無法建立解壓暫存檔案 {}：{error}", temp_path.display()))?;
    io::copy(&mut source, &mut output)
        .map_err(|error| format!("無法解壓 {}：{error}", required_file.display()))?;
    fs::rename(&temp_path, required_file).map_err(|error| {
        format!(
            "無法保存解壓檔案 {} 到 {}：{error}",
            temp_path.display(),
            required_file.display()
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::path::PathBuf;
    use std::sync::{Arc, Mutex};
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
                "immich-geodata-prepare-{name}-{}-{nanos}",
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

    fn production_options(temp: &TestDir) -> ProductionPrepareOptions {
        ProductionPrepareOptions {
            target_dir: temp.path.join("geoname_data"),
            countries: vec!["TW".to_string(), "JP".to_string()],
            update: false,
            geonames_base_url: "https://fixture.test".to_string(),
            natural_earth_url: "https://fixture.test/natural.geojson".to_string(),
        }
    }

    fn success_fixture() -> FixtureDownloader {
        FixtureDownloader::new(vec![
            (
                "https://fixture.test/cities500.zip",
                Ok(zip_bytes("cities500.txt", b"city\n")),
            ),
            (
                "https://fixture.test/TW.zip",
                Ok(zip_bytes("TW.txt", b"tw\n")),
            ),
            (
                "https://fixture.test/JP.zip",
                Ok(zip_bytes("JP.txt", b"jp\n")),
            ),
            (
                "https://fixture.test/admin1CodesASCII.txt",
                Ok(b"admin1\n".to_vec()),
            ),
            (
                "https://fixture.test/admin2Codes.txt",
                Ok(b"admin2\n".to_vec()),
            ),
            (
                "https://fixture.test/natural.geojson",
                Ok(b"{\"type\":\"FeatureCollection\"}\n".to_vec()),
            ),
            (
                "https://fixture.test/alternateNamesV2.zip",
                Ok(zip_bytes("alternateNamesV2.txt", b"alternate\n")),
            ),
        ])
    }

    #[test]
    fn production_prepare_downloads_expected_files_and_skips_existing() {
        let temp = TestDir::new("success");
        let fixture = success_fixture();
        let options = production_options(&temp);

        run_production_with_downloader(&options, &|url, path| fixture.download(url, path)).unwrap();

        assert_eq!(
            fs::read_to_string(options.target_dir.join("cities500.txt")).unwrap(),
            "city\n"
        );
        assert_eq!(
            fs::read_to_string(options.target_dir.join("extra_data").join("TW.txt")).unwrap(),
            "tw\n"
        );
        assert_eq!(
            fs::read_to_string(options.target_dir.join("admin1CodesASCII.txt")).unwrap(),
            "admin1\n"
        );
        assert!(!options.target_dir.join("cities500.zip").exists());
        assert!(!options.target_dir.join("alternateNamesV2.zip").exists());
        assert_eq!(fixture.request_count(), 7);

        run_production_with_downloader(&options, &|url, path| fixture.download(url, path)).unwrap();

        assert_eq!(fixture.request_count(), 7);
    }

    #[test]
    fn production_prepare_update_rebuilds_target_dir() {
        let temp = TestDir::new("update");
        let fixture = success_fixture();
        let mut options = production_options(&temp);
        fs::create_dir_all(&options.target_dir).unwrap();
        fs::write(options.target_dir.join("stale.txt"), "stale").unwrap();
        options.update = true;

        run_production_with_downloader(&options, &|url, path| fixture.download(url, path)).unwrap();

        assert!(!options.target_dir.join("stale.txt").exists());
        assert_eq!(
            fs::read_to_string(options.target_dir.join("extra_data").join("JP.txt")).unwrap(),
            "jp\n"
        );
    }

    #[test]
    fn production_prepare_reports_bad_http_status() {
        let temp = TestDir::new("http-status");
        let fixture = FixtureDownloader::new(vec![(
            "https://fixture.test/cities500.zip",
            Err("下載失敗（HTTP status 404）：https://fixture.test/cities500.zip".to_string()),
        )]);
        let mut options = production_options(&temp);
        options.countries = vec!["TW".to_string()];

        let error =
            run_production_with_downloader(&options, &|url, path| fixture.download(url, path))
                .unwrap_err();

        assert!(error.contains("HTTP status 404"));
        assert!(!options.target_dir.join("cities500.txt").exists());
    }

    #[test]
    fn production_prepare_reports_bad_zip() {
        let temp = TestDir::new("bad-zip");
        let fixture = FixtureDownloader::new(vec![(
            "https://fixture.test/cities500.zip",
            Ok(b"bad zip".to_vec()),
        )]);
        let mut options = production_options(&temp);
        options.countries = vec!["TW".to_string()];

        let error =
            run_production_with_downloader(&options, &|url, path| fixture.download(url, path))
                .unwrap_err();

        assert!(error.contains("zip 解壓失敗"));
    }

    #[test]
    fn production_prepare_reports_missing_extracted_file() {
        let temp = TestDir::new("missing-member");
        let fixture = FixtureDownloader::new(vec![(
            "https://fixture.test/cities500.zip",
            Ok(zip_bytes("unexpected.txt", b"unexpected\n")),
        )]);
        let mut options = production_options(&temp);
        options.countries = vec!["TW".to_string()];

        let error =
            run_production_with_downloader(&options, &|url, path| fixture.download(url, path))
                .unwrap_err();

        assert!(error.contains("解壓後缺少必要檔案"));
        assert!(error.contains("cities500.txt"));
    }

    struct FixtureDownloader {
        files: HashMap<String, Result<Vec<u8>, String>>,
        requests: Arc<Mutex<Vec<String>>>,
    }

    impl FixtureDownloader {
        fn new(files: Vec<(&str, Result<Vec<u8>, String>)>) -> Self {
            Self {
                files: files
                    .into_iter()
                    .map(|(url, result)| (url.to_string(), result))
                    .collect(),
                requests: Arc::new(Mutex::new(Vec::new())),
            }
        }

        fn download(&self, url: &str, path: &Path) -> Result<(), String> {
            self.requests.lock().unwrap().push(url.to_string());
            match self.files.get(url) {
                Some(Ok(content)) => fs::write(path, content)
                    .map_err(|error| format!("無法寫入 fixture 下載檔案：{error}")),
                Some(Err(error)) => Err(error.clone()),
                None => Err(format!("下載失敗（HTTP status 404）：{url}")),
            }
        }

        fn request_count(&self) -> usize {
            self.requests.lock().unwrap().len()
        }
    }

    fn zip_bytes(name: &str, data: &[u8]) -> Vec<u8> {
        let mut bytes = Vec::new();
        let name_bytes = name.as_bytes();
        let crc = crc32(data);
        bytes.extend_from_slice(&0x04034b50_u32.to_le_bytes());
        bytes.extend_from_slice(&20_u16.to_le_bytes());
        bytes.extend_from_slice(&0_u16.to_le_bytes());
        bytes.extend_from_slice(&0_u16.to_le_bytes());
        bytes.extend_from_slice(&0_u16.to_le_bytes());
        bytes.extend_from_slice(&0_u16.to_le_bytes());
        bytes.extend_from_slice(&crc.to_le_bytes());
        bytes.extend_from_slice(&(data.len() as u32).to_le_bytes());
        bytes.extend_from_slice(&(data.len() as u32).to_le_bytes());
        bytes.extend_from_slice(&(name_bytes.len() as u16).to_le_bytes());
        bytes.extend_from_slice(&0_u16.to_le_bytes());
        bytes.extend_from_slice(name_bytes);
        bytes.extend_from_slice(data);
        let central_offset = bytes.len() as u32;
        bytes.extend_from_slice(&0x02014b50_u32.to_le_bytes());
        bytes.extend_from_slice(&20_u16.to_le_bytes());
        bytes.extend_from_slice(&20_u16.to_le_bytes());
        bytes.extend_from_slice(&0_u16.to_le_bytes());
        bytes.extend_from_slice(&0_u16.to_le_bytes());
        bytes.extend_from_slice(&0_u16.to_le_bytes());
        bytes.extend_from_slice(&0_u16.to_le_bytes());
        bytes.extend_from_slice(&crc.to_le_bytes());
        bytes.extend_from_slice(&(data.len() as u32).to_le_bytes());
        bytes.extend_from_slice(&(data.len() as u32).to_le_bytes());
        bytes.extend_from_slice(&(name_bytes.len() as u16).to_le_bytes());
        bytes.extend_from_slice(&0_u16.to_le_bytes());
        bytes.extend_from_slice(&0_u16.to_le_bytes());
        bytes.extend_from_slice(&0_u16.to_le_bytes());
        bytes.extend_from_slice(&0_u16.to_le_bytes());
        bytes.extend_from_slice(&0_u32.to_le_bytes());
        bytes.extend_from_slice(&0_u32.to_le_bytes());
        bytes.extend_from_slice(name_bytes);
        let central_size = bytes.len() as u32 - central_offset;
        bytes.extend_from_slice(&0x06054b50_u32.to_le_bytes());
        bytes.extend_from_slice(&0_u16.to_le_bytes());
        bytes.extend_from_slice(&0_u16.to_le_bytes());
        bytes.extend_from_slice(&1_u16.to_le_bytes());
        bytes.extend_from_slice(&1_u16.to_le_bytes());
        bytes.extend_from_slice(&central_size.to_le_bytes());
        bytes.extend_from_slice(&central_offset.to_le_bytes());
        bytes.extend_from_slice(&0_u16.to_le_bytes());
        bytes
    }

    fn crc32(data: &[u8]) -> u32 {
        let mut crc = 0xffff_ffff_u32;
        for byte in data {
            crc ^= u32::from(*byte);
            for _ in 0..8 {
                let mask = if crc & 1 == 1 { 0xedb8_8320 } else { 0 };
                crc = (crc >> 1) ^ mask;
            }
        }
        !crc
    }
}
