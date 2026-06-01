use std::fs::File;
use std::path::Path;
use std::sync::Arc;

use polars::prelude::*;

use crate::pipeline::table::{CITIES_COLUMNS, GEODATA_COLUMNS};

pub const ADMIN1_COLUMNS: [&str; 4] = ["id", "name", "asciiname", "geoname_id"];
pub const ALTERNATE_NAME_COLUMNS: [&str; 2] = ["geoname_id", "name"];

#[derive(Debug, Clone, Copy)]
enum FixedSchema {
    Cities,
    Admin1,
    Geodata,
    AlternateName,
}

impl FixedSchema {
    fn columns(self) -> &'static [&'static str] {
        match self {
            Self::Cities => &CITIES_COLUMNS,
            Self::Admin1 => &ADMIN1_COLUMNS,
            Self::Geodata => &GEODATA_COLUMNS,
            Self::AlternateName => &ALTERNATE_NAME_COLUMNS,
        }
    }

    fn dtype(self, index: usize) -> DataType {
        match self {
            Self::Cities => match self.columns()[index] {
                "population" => DataType::UInt32,
                "dem" => DataType::Int32,
                "modification_date" => DataType::Date,
                _ => DataType::String,
            },
            Self::Geodata => match self.columns()[index] {
                "latitude" | "longitude" => DataType::Float64,
                _ => DataType::String,
            },
            Self::Admin1 | Self::AlternateName => DataType::String,
        }
    }

    fn schema(self) -> Schema {
        Schema::from_iter(
            self.columns()
                .iter()
                .enumerate()
                .map(|(index, name)| Field::new(PlSmallStr::from_str(name), self.dtype(index))),
        )
    }

    fn empty_string_is_null(self) -> bool {
        matches!(self, Self::Cities | Self::Geodata)
    }
}

pub fn read_cities_rows(path: &Path, delimiter: u8) -> Result<Vec<Vec<String>>, String> {
    let df = read_fixed_dataframe(path, delimiter, false, FixedSchema::Cities)?;
    dataframe_to_rows(&df, FixedSchema::Cities.columns())
}

pub fn read_admin1_rows(path: &Path) -> Result<Vec<Vec<String>>, String> {
    let df = read_fixed_dataframe(path, b'\t', false, FixedSchema::Admin1)?;
    dataframe_to_rows(&df, FixedSchema::Admin1.columns())
}

pub fn read_geodata_rows_with_header(path: &Path) -> Result<Vec<Vec<String>>, String> {
    let df = read_fixed_dataframe(path, b',', true, FixedSchema::Geodata)?;
    dataframe_to_rows(&df, FixedSchema::Geodata.columns())
}

pub fn read_alternate_name_rows_with_header(path: &Path) -> Result<Vec<Vec<String>>, String> {
    let df = read_fixed_dataframe(path, b',', true, FixedSchema::AlternateName)?;
    dataframe_to_rows(&df, FixedSchema::AlternateName.columns())
}

pub fn read_cities_dataframe(path: &Path, delimiter: u8) -> Result<DataFrame, String> {
    read_fixed_dataframe(path, delimiter, false, FixedSchema::Cities)
}

pub fn cities_rows_to_dataframe(rows: &[Vec<String>]) -> Result<DataFrame, String> {
    rows_to_fixed_dataframe(rows, FixedSchema::Cities)
}

pub fn admin1_rows_to_dataframe(rows: &[Vec<String>]) -> Result<DataFrame, String> {
    rows_to_fixed_dataframe(rows, FixedSchema::Admin1)
}

pub fn cities_dataframe_to_rows(df: &DataFrame) -> Result<Vec<Vec<String>>, String> {
    let selected = df
        .select(CITIES_COLUMNS)
        .map_err(|error| format!("Polars cities DataFrame 欄位選取失敗：{error}"))?;
    dataframe_to_rows(&selected, FixedSchema::Cities.columns())
}

pub fn admin1_dataframe_to_rows(df: &DataFrame) -> Result<Vec<Vec<String>>, String> {
    let selected = df
        .select(ADMIN1_COLUMNS)
        .map_err(|error| format!("Polars admin1 DataFrame 欄位選取失敗：{error}"))?;
    dataframe_to_rows(&selected, FixedSchema::Admin1.columns())
}

pub fn write_cities_rows(
    path: &Path,
    delimiter: u8,
    include_header: bool,
    rows: &[Vec<String>],
) -> Result<(), String> {
    write_fixed_rows(path, delimiter, include_header, FixedSchema::Cities, rows)
}

pub fn write_admin1_rows(path: &Path, rows: &[Vec<String>]) -> Result<(), String> {
    write_fixed_rows(path, b'\t', false, FixedSchema::Admin1, rows)
}

pub fn write_geodata_rows_with_header(path: &Path, rows: &[Vec<String>]) -> Result<(), String> {
    write_fixed_rows(path, b',', true, FixedSchema::Geodata, rows)
}

pub fn write_alternate_name_rows_with_header(
    path: &Path,
    rows: &[Vec<String>],
) -> Result<(), String> {
    write_fixed_rows(path, b',', true, FixedSchema::AlternateName, rows)
}

pub fn read_string_rows(
    path: &Path,
    delimiter: u8,
    has_header: bool,
    columns: &[&str],
) -> Result<Vec<Vec<String>>, String> {
    let df = read_dataframe(path, delimiter, has_header, columns, string_schema(columns))?;
    dataframe_to_rows(&df, columns)
}

fn read_fixed_dataframe(
    path: &Path,
    delimiter: u8,
    has_header: bool,
    fixed_schema: FixedSchema,
) -> Result<DataFrame, String> {
    read_dataframe(
        path,
        delimiter,
        has_header,
        fixed_schema.columns(),
        fixed_schema.schema(),
    )
}

fn read_dataframe(
    path: &Path,
    delimiter: u8,
    has_header: bool,
    columns: &[&str],
    schema: Schema,
) -> Result<DataFrame, String> {
    let file = File::open(path)
        .map_err(|error| format!("無法開啟 Polars 表格 {}：{error}", path.display()))?;
    CsvReadOptions::default()
        .with_has_header(has_header)
        .with_columns(Some(
            columns
                .iter()
                .map(|name| PlSmallStr::from_str(name))
                .collect(),
        ))
        .with_schema(Some(Arc::new(schema)))
        .map_parse_options(|parse_options| {
            parse_options
                .with_separator(delimiter)
                .with_missing_is_null(true)
                .with_try_parse_dates(true)
        })
        .into_reader_with_file_handle(file)
        .finish()
        .map_err(|error| format!("Polars 無法讀取表格 {}：{error}", path.display()))
}

pub fn write_string_rows(
    path: &Path,
    delimiter: u8,
    include_header: bool,
    columns: &[&str],
    rows: &[Vec<String>],
) -> Result<(), String> {
    let df = rows_to_string_dataframe(rows, columns)?;
    write_dataframe(path, delimiter, include_header, df)
}

fn write_fixed_rows(
    path: &Path,
    delimiter: u8,
    include_header: bool,
    fixed_schema: FixedSchema,
    rows: &[Vec<String>],
) -> Result<(), String> {
    let df = rows_to_fixed_dataframe(rows, fixed_schema)?;
    write_dataframe(path, delimiter, include_header, df)
}

fn write_dataframe(
    path: &Path,
    delimiter: u8,
    include_header: bool,
    mut df: DataFrame,
) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|error| format!("無法建立 Polars 輸出目錄 {}：{error}", parent.display()))?;
    }

    let mut file = File::create(path)
        .map_err(|error| format!("無法建立 Polars 表格 {}：{error}", path.display()))?;
    CsvWriter::new(&mut file)
        .include_header(include_header)
        .with_separator(delimiter)
        .finish(&mut df)
        .map_err(|error| format!("Polars 無法寫入表格 {}：{error}", path.display()))
}

fn string_schema(columns: &[&str]) -> Schema {
    Schema::from_iter(
        columns
            .iter()
            .map(|name| Field::new(PlSmallStr::from_str(name), DataType::String)),
    )
}

fn rows_to_string_dataframe(rows: &[Vec<String>], columns: &[&str]) -> Result<DataFrame, String> {
    for (index, row) in rows.iter().enumerate() {
        if row.len() != columns.len() {
            return Err(format!(
                "Polars 表格第 {index} 列欄位數不符：expected={} actual={}",
                columns.len(),
                row.len()
            ));
        }
    }

    let polars_columns = columns
        .iter()
        .enumerate()
        .map(|(index, name)| {
            let values: Vec<String> = rows.iter().map(|row| row[index].clone()).collect();
            Series::new((*name).into(), values).into()
        })
        .collect::<Vec<Column>>();
    DataFrame::new(rows.len(), polars_columns)
        .map_err(|error| format!("無法建立 Polars 表格 DataFrame：{error}"))
}

fn rows_to_fixed_dataframe(
    rows: &[Vec<String>],
    fixed_schema: FixedSchema,
) -> Result<DataFrame, String> {
    let columns = fixed_schema.columns();
    for (row_index, row) in rows.iter().enumerate() {
        if row.len() != columns.len() {
            return Err(format!(
                "Polars 表格第 {row_index} 列欄位數不符：expected={} actual={}",
                columns.len(),
                row.len()
            ));
        }
    }

    let polars_columns = columns
        .iter()
        .enumerate()
        .map(|(column_index, name)| {
            build_column(
                rows,
                column_index,
                name,
                fixed_schema.dtype(column_index),
                fixed_schema.empty_string_is_null(),
            )
        })
        .collect::<Result<Vec<Column>, String>>()?;
    DataFrame::new(rows.len(), polars_columns)
        .map_err(|error| format!("無法建立 Polars typed DataFrame：{error}"))
}

fn build_column(
    rows: &[Vec<String>],
    index: usize,
    name: &str,
    dtype: DataType,
    empty_string_is_null: bool,
) -> Result<Column, String> {
    match dtype {
        DataType::String => {
            if empty_string_is_null {
                Ok(StringChunked::from_iter_options(
                    name.into(),
                    rows.iter().map(|row| non_empty_value(&row[index])),
                )
                .into_series()
                .into())
            } else {
                let values: Vec<String> = rows.iter().map(|row| row[index].clone()).collect();
                Ok(Series::new(name.into(), values).into())
            }
        }
        DataType::UInt32 => {
            let values = rows
                .iter()
                .enumerate()
                .map(|(row_index, row)| parse_optional_u32(&row[index], name, row_index));
            Ok(UInt32Chunked::from_iter_options(
                name.into(),
                values.collect::<Result<Vec<_>, _>>()?.into_iter(),
            )
            .into_series()
            .into())
        }
        DataType::Int32 => {
            let values = rows
                .iter()
                .enumerate()
                .map(|(row_index, row)| parse_optional_i32(&row[index], name, row_index));
            Ok(Int32Chunked::from_iter_options(
                name.into(),
                values.collect::<Result<Vec<_>, _>>()?.into_iter(),
            )
            .into_series()
            .into())
        }
        DataType::Float64 => {
            let values = rows
                .iter()
                .enumerate()
                .map(|(row_index, row)| parse_optional_f64(&row[index], name, row_index));
            Ok(Float64Chunked::from_iter_options(
                name.into(),
                values.collect::<Result<Vec<_>, _>>()?.into_iter(),
            )
            .into_series()
            .into())
        }
        DataType::Date => {
            let values = rows
                .iter()
                .enumerate()
                .map(|(row_index, row)| parse_optional_date(&row[index], name, row_index));
            Ok(Int32Chunked::from_iter_options(
                name.into(),
                values.collect::<Result<Vec<_>, _>>()?.into_iter(),
            )
            .into_date()
            .into_series()
            .into())
        }
        other => Err(format!("不支援的固定 schema 欄位型別 {name}: {other:?}")),
    }
}

fn dataframe_to_rows(df: &DataFrame, columns: &[&str]) -> Result<Vec<Vec<String>>, String> {
    if df.width() != columns.len() {
        return Err(format!(
            "Polars 表格欄位數不符：expected={} actual={}",
            columns.len(),
            df.width()
        ));
    }

    let mut rows = Vec::with_capacity(df.height());
    for row_index in 0..df.height() {
        let mut row = Vec::with_capacity(columns.len());
        for name in columns {
            let column = df
                .column(name)
                .map_err(|error| format!("Polars DataFrame 缺少欄位 {name}：{error}"))?;
            row.push(format_any_value(column.get(row_index).map_err(
                |error| format!("Polars 欄位 {name} 第 {row_index} 列讀取失敗：{error}"),
            )?));
        }
        rows.push(row);
    }
    Ok(rows)
}

fn format_any_value(value: AnyValue<'_>) -> String {
    match value {
        AnyValue::Null => String::new(),
        AnyValue::String(value) => value.to_string(),
        AnyValue::StringOwned(value) => value.to_string(),
        AnyValue::UInt32(value) => value.to_string(),
        AnyValue::Int32(value) => value.to_string(),
        AnyValue::Float64(value) => value.to_string(),
        AnyValue::Date(days) => format_date(days),
        other => other.to_string(),
    }
}

fn non_empty_value(value: &str) -> Option<&str> {
    (!value.is_empty()).then_some(value)
}

fn parse_optional_u32(value: &str, name: &str, row_index: usize) -> Result<Option<u32>, String> {
    if value.is_empty() {
        Ok(None)
    } else {
        value.parse::<u32>().map(Some).map_err(|error| {
            format!("Polars 欄位 {name} 第 {row_index} 列無法解析 UInt32 值 {value:?}：{error}")
        })
    }
}

fn parse_optional_i32(value: &str, name: &str, row_index: usize) -> Result<Option<i32>, String> {
    if value.is_empty() {
        Ok(None)
    } else {
        value.parse::<i32>().map(Some).map_err(|error| {
            format!("Polars 欄位 {name} 第 {row_index} 列無法解析 Int32 值 {value:?}：{error}")
        })
    }
}

fn parse_optional_f64(value: &str, name: &str, row_index: usize) -> Result<Option<f64>, String> {
    if value.is_empty() {
        Ok(None)
    } else {
        value.parse::<f64>().map(Some).map_err(|error| {
            format!("Polars 欄位 {name} 第 {row_index} 列無法解析 Float64 值 {value:?}：{error}")
        })
    }
}

fn parse_optional_date(value: &str, name: &str, row_index: usize) -> Result<Option<i32>, String> {
    if value.is_empty() {
        Ok(None)
    } else {
        parse_date(value).map(Some).map_err(|error| {
            format!("Polars 欄位 {name} 第 {row_index} 列無法解析 Date 值 {value:?}：{error}")
        })
    }
}

fn parse_date(value: &str) -> Result<i32, String> {
    let mut parts = value.split('-');
    let year = parts
        .next()
        .ok_or_else(|| "缺少年份".to_string())?
        .parse::<i32>()
        .map_err(|error| format!("年份格式錯誤：{error}"))?;
    let month = parts
        .next()
        .ok_or_else(|| "缺少月份".to_string())?
        .parse::<i32>()
        .map_err(|error| format!("月份格式錯誤：{error}"))?;
    let day = parts
        .next()
        .ok_or_else(|| "缺少日期".to_string())?
        .parse::<i32>()
        .map_err(|error| format!("日期格式錯誤：{error}"))?;
    if parts.next().is_some() || !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return Err("必須是 YYYY-MM-DD".to_string());
    }
    let days = days_from_civil(year, month, day);
    if format_date(days) != value {
        return Err("日期不存在".to_string());
    }
    Ok(days)
}

fn days_from_civil(year: i32, month: i32, day: i32) -> i32 {
    let year = year - i32::from(month <= 2);
    let era = if year >= 0 { year } else { year - 399 } / 400;
    let yoe = year - era * 400;
    let month = month + if month > 2 { -3 } else { 9 };
    let doy = (153 * month + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

fn format_date(days: i32) -> String {
    let days = days + 719_468;
    let era = if days >= 0 { days } else { days - 146_096 } / 146_097;
    let doe = days - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let mut year = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = mp + if mp < 10 { 3 } else { -9 };
    year += i32::from(month <= 2);
    format!("{year:04}-{month:02}-{day:02}")
}
