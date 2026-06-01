use immich_geodata_migration::observability::{DependencyLogger, ProgressReporter};
use immich_geodata_migration::pipeline::locationiq::{LocationiqHttpClient, build_locationiq_url};
use immich_geodata_migration::unicode_han::{includes_han, is_han, is_han_name};

#[test]
fn unicode_han_matches_regex_script_extensions_cases() {
    for character in ['漢', '𠀀', '㐀', '々', '〆', '〇', '〡', '·', '・'] {
        assert!(
            is_han(character),
            "{character} should match Script_Extensions=Han"
        );
    }
    assert!(!is_han('A'));
    assert!(!is_han('é'));
    assert!(includes_han("São・Tomé"));
    assert!(is_han_name("𠀀-臺"));
    assert!(!is_han_name("São Tomé"));
}

#[test]
fn locationiq_url_encodes_coordinates_and_redacts_key_for_logs() {
    let url = build_locationiq_url("25.03300000", "121.56540000", "secret key").unwrap();

    assert!(url.as_str().contains("lat=25.03300000"));
    assert!(url.as_str().contains("lon=121.56540000"));
    assert!(url.as_str().contains("accept-language=zh%2Cen"));
    assert!(url.as_str().contains("key=secret+key"));
    assert!(!format!("{url:?}").contains("***"));
}

#[test]
fn locationiq_http_client_exposes_redacted_url_for_observability() {
    let client = LocationiqHttpClient::new("secret-token".to_string(), 2).unwrap();
    let redacted = client.redacted_url("25.0", "121.0").unwrap();

    assert!(redacted.contains("key=***"));
    assert!(!redacted.contains("secret-token"));
}

#[test]
fn dependency_logger_keeps_loguru_style_levels() {
    let logger = DependencyLogger::new("test-stage");

    assert_eq!(logger.info("rows=1"), "level=INFO stage=test-stage rows=1");
    assert_eq!(
        logger.warning("missing=1"),
        "level=WARNING stage=test-stage missing=1"
    );
    assert_eq!(
        logger.error("failed=true"),
        "level=ERROR stage=test-stage failed=true"
    );
}

#[test]
fn progress_reporter_has_ci_safe_tqdm_replacement_lines() {
    let progress = ProgressReporter::new("locationiq", 3);

    assert_eq!(
        progress.started_line(),
        "progress_start stage=locationiq total=3"
    );
    assert_eq!(
        progress.step_line(2),
        "progress stage=locationiq current=2 total=3"
    );
    assert_eq!(
        progress.finished_line(),
        "progress_finish stage=locationiq total=3"
    );
}
