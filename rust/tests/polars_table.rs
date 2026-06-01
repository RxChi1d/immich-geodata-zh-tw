use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use immich_geodata_migration::pipeline::polars_table::{
    ADMIN1_COLUMNS, read_cities_dataframe, read_cities_rows, read_string_rows, write_cities_rows,
    write_geodata_rows_with_header, write_string_rows,
};
use immich_geodata_migration::pipeline::table::{
    CITIES_COLUMNS, GEODATA_COLUMNS, read_delimited, write_delimited,
};
use polars::prelude::DataType;

#[test]
fn reads_tw_minimal_cities_like_existing_parser() {
    assert_polars_matches_table(
        "fixtures/parity/tw_minimal/geoname/cities500.txt",
        b'\t',
        false,
        &CITIES_COLUMNS,
    );
}

#[test]
fn reads_cities_dataframe_with_legacy_typed_schema() {
    let path = repo_path("fixtures/parity/tw_minimal/geoname/cities500.txt");

    let df = read_cities_dataframe(&path, b'\t').unwrap();

    assert_eq!(df.column("population").unwrap().dtype(), &DataType::UInt32);
    assert_eq!(df.column("dem").unwrap().dtype(), &DataType::Int32);
    assert_eq!(
        df.column("modification_date").unwrap().dtype(),
        &DataType::Date
    );
    assert_eq!(df.column("dem").unwrap().null_count(), 2);
}

#[test]
fn reads_tw_minimal_admin1_like_existing_parser() {
    assert_polars_matches_table(
        "fixtures/parity/tw_minimal/geoname/admin1CodesASCII.txt",
        b'\t',
        false,
        &ADMIN1_COLUMNS,
    );
}

#[test]
fn reads_tw_minimal_extra_us_like_existing_parser() {
    assert_polars_matches_table(
        "fixtures/parity/tw_minimal/extra_data/US.txt",
        b'\t',
        false,
        &CITIES_COLUMNS,
    );
}

#[test]
fn reads_tw_minimal_geodata_header_like_existing_parser() {
    assert_polars_matches_table(
        "fixtures/parity/tw_minimal/geodata/tw_geodata.csv",
        b',',
        true,
        &GEODATA_COLUMNS,
    );
}

#[test]
fn reads_tw_minimal_metadata_header_like_existing_parser() {
    assert_polars_matches_table(
        "fixtures/parity/tw_minimal/meta_data/US.csv",
        b',',
        true,
        &GEODATA_COLUMNS,
    );
}

#[test]
fn reads_alternate_chinese_name_csv_header_like_existing_parser() {
    assert_polars_matches_table(
        "fixtures/parity/tw_minimal/alternate_chinese_name.csv",
        b',',
        true,
        &["geoname_id", "name"],
    );
}

#[test]
fn reads_quoted_tsv_field_with_doubled_quotes_like_existing_parser() {
    let path = write_temp_table(
        "quoted_doubled_quotes.tsv",
        "1\t\"Poselok \"\"Klyazminskoe\"\"\"\tName\n",
    );

    assert_temp_polars_matches_table(&path, b'\t', false, &["id", "name", "asciiname"]);
}

#[test]
fn reads_unquoted_embedded_quotes_like_existing_parser() {
    let path = write_temp_table(
        "unquoted_embedded_quotes.tsv",
        "1\tPoselok Turisticheskogo pansionata \"Klyazminskoe\"\tName\n",
    );

    assert_temp_polars_matches_table(&path, b'\t', false, &["id", "name", "asciiname"]);
}

#[test]
fn reads_delimiter_containing_quoted_field_like_existing_parser() {
    let path = write_temp_table("quoted_delimiter.tsv", "1\t\"A\tB\"\tName\n");

    assert_temp_polars_matches_table(&path, b'\t', false, &["id", "name", "asciiname"]);
}

#[test]
fn writes_admin1_tsv_like_existing_writer() {
    let rows = vec![
        vec![
            "TW.1".to_string(),
            "臺北市".to_string(),
            "臺北市".to_string(),
            "92000000".to_string(),
        ],
        vec![
            "ZZ.1".to_string(),
            "A\tB \"County\"".to_string(),
            "A\tB \"County\"".to_string(),
            "42".to_string(),
        ],
    ];
    assert_polars_writer_matches_table_writer(b'\t', false, &ADMIN1_COLUMNS, &rows);
}

#[test]
fn writes_cities_tsv_with_typed_schema_bytes_regression() {
    let rows = vec![vec![
        "1001".to_string(),
        "舊臺北".to_string(),
        "舊臺北".to_string(),
        String::new(),
        "25.04776000".to_string(),
        "121.53185000".to_string(),
        "P".to_string(),
        "PPL".to_string(),
        "TW".to_string(),
        String::new(),
        "OLD".to_string(),
        String::new(),
        String::new(),
        String::new(),
        "1000".to_string(),
        String::new(),
        String::new(),
        "Asia/Taipei".to_string(),
        "2024-01-01".to_string(),
    ]];
    let path = write_temp_table("writer_cities_typed.out", "");

    write_cities_rows(&path, b'\t', false, &rows).unwrap();

    assert_eq!(
        fs::read_to_string(&path).unwrap(),
        "1001\t舊臺北\t舊臺北\t\t25.04776000\t121.53185000\tP\tPPL\tTW\t\tOLD\t\t\t\t1000\t\t\tAsia/Taipei\t2024-01-01\n"
    );
    fs::remove_file(path).unwrap();
}

#[test]
fn writes_geodata_csv_with_header_like_polars() {
    let rows = vec![vec![
        "25.00000000".to_string(),
        "121.00000000".to_string(),
        "臺灣".to_string(),
        "臺北市".to_string(),
        "A,B".to_string(),
        String::new(),
        String::new(),
    ]];
    let path = write_temp_table("writer_geodata_polars.out", "");

    write_geodata_rows_with_header(&path, &rows).unwrap();

    assert_eq!(
        fs::read_to_string(&path).unwrap(),
        "latitude,longitude,country,admin_1,admin_2,admin_3,admin_4\n\
         25.0,121.0,臺灣,臺北市,\"A,B\",,\n"
    );
    fs::remove_file(path).unwrap();
}

fn assert_polars_matches_table(
    relative_path: &str,
    delimiter: u8,
    has_header: bool,
    columns: &[&str],
) {
    assert_rows_equal(repo_path(relative_path), delimiter, has_header, columns);
}

fn assert_temp_polars_matches_table(
    path: &Path,
    delimiter: u8,
    has_header: bool,
    columns: &[&str],
) {
    assert_rows_equal(path.to_path_buf(), delimiter, has_header, columns);
    fs::remove_file(path).unwrap();
}

fn assert_rows_equal(path: PathBuf, delimiter: u8, has_header: bool, columns: &[&str]) {
    let delimiter_char = char::from(delimiter);
    let expected = read_delimited(&path, delimiter_char, has_header).unwrap();
    let actual = if columns == CITIES_COLUMNS.as_slice() && !has_header {
        read_cities_rows(&path, delimiter).unwrap()
    } else {
        read_string_rows(&path, delimiter, has_header, columns).unwrap()
    };

    assert_eq!(actual.len(), expected.len(), "row count mismatch");
    assert_eq!(actual.first().map(Vec::len), expected.first().map(Vec::len));
    assert_eq!(actual, expected);
}

fn repo_path(relative_path: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join(relative_path)
}

fn write_temp_table(file_name: &str, content: &str) -> PathBuf {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let path = std::env::temp_dir().join(format!(
        "immich_geodata_polars_table_{}_{}_{}",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::Relaxed),
        file_name
    ));
    fs::write(&path, content).unwrap();
    path
}

fn assert_polars_writer_matches_table_writer(
    delimiter: u8,
    include_header: bool,
    columns: &[&str],
    rows: &[Vec<String>],
) {
    let delimiter_char = char::from(delimiter);
    let polars_path = write_temp_table("writer_polars.out", "");
    let table_path = write_temp_table("writer_table.out", "");
    let header = include_header.then_some(columns);

    write_string_rows(&polars_path, delimiter, include_header, columns, rows).unwrap();
    write_delimited(&table_path, delimiter_char, header, rows).unwrap();

    assert_eq!(
        fs::read_to_string(&polars_path).unwrap(),
        fs::read_to_string(&table_path).unwrap()
    );
    fs::remove_file(polars_path).unwrap();
    fs::remove_file(table_path).unwrap();
}
