//! GeoNames alternateNamesV2 的讀取、篩選與轉列。
//!
//! 自 `translate.rs` 拆出——這一組函式從頭到尾只處理 alternate names 這一個
//! 資料來源，與 cities500／admin1 的翻譯流程沒有共用狀態。

use std::fs::File;
use std::path::Path;
use std::sync::Arc;

use polars::prelude::*;

use crate::pipeline::polars_table::{
    read_alternate_name_rows_with_header, write_alternate_name_rows_with_header,
};

pub(super) fn build_alternate_names_dataframe(
    data_dir: &Path,
    output: &Path,
) -> Result<DataFrame, String> {
    let rows = build_alternate_name_rows_from_data_dir(data_dir)?;
    write_alternate_name_rows_with_header(output, &rows)?;
    alternate_name_rows_to_dataframe(&rows)
}

pub(super) fn build_alternate_name_rows_from_data_dir(
    data_dir: &Path,
) -> Result<Vec<Vec<String>>, String> {
    let source = data_dir.join("alternateNamesV2.txt");
    if !source.exists() {
        return Err(format!(
            "替代名稱檔案不存在：{}；請先執行 prepare 或指定 --alternate-name-file",
            source.display()
        ));
    }
    build_alternate_name_rows(&source)
}

pub(super) fn build_alternate_name_rows(source: &Path) -> Result<Vec<Vec<String>>, String> {
    let priority = [
        "zh-Hant", "zh-TW", "zh-HK", "zh", "zh-Hans", "zh-CN", "zh-SG",
    ];
    let df = read_alternate_names_v2_dataframe(source)?;
    let mut filtered = filter_chinese_alternate_names(&df, &priority)?;
    let priority_values = alternate_name_priority_values(&filtered, &priority)?;
    filtered
        .with_column(UInt32Chunked::from_vec("priority".into(), priority_values).into_column())
        .map_err(|error| format!("無法建立 alternateNamesV2 priority 欄位：{error}"))?;

    let sorted = filtered
        .sort(
            ["priority"],
            SortMultipleOptions::new().with_maintain_order(true),
        )
        .map_err(|error| format!("Polars alternateNamesV2 priority sort 失敗：{error}"))?;
    let subset = ["geoname_id".to_string()];
    let selected = sorted
        .unique_stable(Some(&subset), UniqueKeepStrategy::First, None)
        .and_then(|df| df.select(["geoname_id", "name"]))
        .map_err(|error| format!("Polars alternateNamesV2 group/select 失敗：{error}"))?;
    alternate_name_dataframe_to_rows(&selected)
}

pub(super) fn read_alternate_names_v2_dataframe(source: &Path) -> Result<DataFrame, String> {
    let file = File::open(source)
        .map_err(|error| format!("無法開啟 alternateNamesV2 {}：{error}", source.display()))?;
    let projection = Arc::new(vec![1_usize, 2, 3, 4]);
    let dtype_overwrite = Arc::new(vec![
        DataType::String,
        DataType::String,
        DataType::String,
        DataType::String,
    ]);
    let mut df = CsvReadOptions::default()
        .with_has_header(false)
        .with_projection(Some(projection))
        .with_dtype_overwrite(Some(dtype_overwrite))
        .map_parse_options(|parse_options| {
            parse_options
                .with_separator(b'\t')
                .with_missing_is_null(true)
                .with_null_values(Some(NullValues::AllColumnsSingle("\\N".into())))
        })
        .into_reader_with_file_handle(file)
        .finish()
        .map_err(|error| {
            format!(
                "Polars 無法讀取 alternateNamesV2 {}：{error}",
                source.display()
            )
        })?;
    df.set_column_names(&["geoname_id", "lang", "name", "is_preferred_name"])
        .map_err(|error| format!("Polars alternateNamesV2 欄位命名失敗：{error}"))?;
    Ok(df)
}

pub(super) fn alternate_name_priority_values(
    df: &DataFrame,
    priority: &[&str],
) -> Result<Vec<u32>, String> {
    let langs = df
        .column("lang")
        .and_then(|column| column.str())
        .map_err(|error| format!("Polars alternateNamesV2 lang 欄位錯誤：{error}"))?;
    let preferred = df
        .column("is_preferred_name")
        .map_err(|error| format!("Polars alternateNamesV2 is_preferred_name 欄位錯誤：{error}"))?;
    let fallback = u32::try_from(priority.len() + 1)
        .map_err(|error| format!("alternateNamesV2 priority 長度過大：{error}"))?;
    Ok((0..df.height())
        .map(|index| {
            if is_preferred_alternate_name(preferred, index) {
                0
            } else {
                langs
                    .get(index)
                    .and_then(|lang| {
                        priority
                            .iter()
                            .position(|candidate| *candidate == lang)
                            .and_then(|priority_index| u32::try_from(priority_index + 1).ok())
                    })
                    .unwrap_or(fallback)
            }
        })
        .collect())
}

pub(super) fn is_preferred_alternate_name(column: &Column, index: usize) -> bool {
    match column.get(index) {
        Ok(AnyValue::Int64(1))
        | Ok(AnyValue::Int32(1))
        | Ok(AnyValue::UInt64(1))
        | Ok(AnyValue::UInt32(1))
        | Ok(AnyValue::String("1")) => true,
        Ok(AnyValue::StringOwned(value)) => value.as_str() == "1",
        _ => false,
    }
}

pub(super) fn filter_chinese_alternate_names(
    df: &DataFrame,
    priority: &[&str],
) -> Result<DataFrame, String> {
    let langs = df
        .column("lang")
        .and_then(|column| column.str())
        .map_err(|error| format!("Polars alternateNamesV2 lang 欄位錯誤：{error}"))?;
    let mask = BooleanChunked::from_iter_options(
        "is_chinese".into(),
        (0..df.height()).map(|index| langs.get(index).map(|lang| priority.contains(&lang))),
    );
    df.filter(&mask)
        .map_err(|error| format!("Polars alternateNamesV2 中文語言篩選失敗：{error}"))
}

pub(super) fn alternate_name_dataframe_to_rows(df: &DataFrame) -> Result<Vec<Vec<String>>, String> {
    let geoname_ids = df
        .column("geoname_id")
        .and_then(|column| column.str())
        .map_err(|error| format!("Polars alternateNamesV2 geoname_id 欄位錯誤：{error}"))?;
    let names = df
        .column("name")
        .and_then(|column| column.str())
        .map_err(|error| format!("Polars alternateNamesV2 name 欄位錯誤：{error}"))?;
    let mut rows = (0..df.height())
        .map(|index| {
            vec![
                geoname_ids.get(index).unwrap_or_default().to_string(),
                names
                    .get(index)
                    .unwrap_or_default()
                    .replace("桃園縣", "桃園市"),
            ]
        })
        .collect::<Vec<Vec<String>>>();
    rows.sort_by(|left, right| left[0].cmp(&right[0]));
    Ok(rows)
}

pub(super) fn load_alternate_names_dataframe(path: &Path) -> Result<DataFrame, String> {
    if !path.exists() {
        return empty_alternate_names_dataframe();
    }
    alternate_name_rows_to_dataframe(&read_alternate_name_rows_with_header(path)?)
}

pub(super) fn empty_alternate_names_dataframe() -> Result<DataFrame, String> {
    DataFrame::new(
        0,
        vec![
            Series::new("geoname_id".into(), Vec::<String>::new()).into(),
            Series::new("name".into(), Vec::<String>::new()).into(),
        ],
    )
    .map_err(|error| format!("無法建立空 alternate-name DataFrame：{error}"))
}

pub(super) fn alternate_name_rows_to_dataframe(rows: &[Vec<String>]) -> Result<DataFrame, String> {
    DataFrame::new(
        rows.len(),
        vec![
            Series::new(
                "geoname_id".into(),
                rows.iter()
                    .map(|row| row.first().cloned().unwrap_or_default())
                    .collect::<Vec<_>>(),
            )
            .into(),
            Series::new(
                "name".into(),
                rows.iter()
                    .map(|row| row.get(1).cloned().unwrap_or_default())
                    .collect::<Vec<_>>(),
            )
            .into(),
        ],
    )
    .map_err(|error| format!("無法建立 alternate-name DataFrame：{error}"))
}
