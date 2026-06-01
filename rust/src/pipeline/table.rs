use std::fs::{self, File};
use std::io::{BufWriter, Write};
use std::path::Path;

pub const CITIES_COLUMNS: [&str; 19] = [
    "geoname_id",
    "name",
    "asciiname",
    "alternatenames",
    "latitude",
    "longitude",
    "feature_class",
    "feature_code",
    "country_code",
    "cc2",
    "admin1_code",
    "admin2_code",
    "admin3_code",
    "admin4_code",
    "population",
    "elevation",
    "dem",
    "timezone",
    "modification_date",
];

pub const GEODATA_COLUMNS: [&str; 7] = [
    "latitude",
    "longitude",
    "country",
    "admin_1",
    "admin_2",
    "admin_3",
    "admin_4",
];

pub fn read_delimited(
    path: &Path,
    delimiter: char,
    has_header: bool,
) -> Result<Vec<Vec<String>>, String> {
    let content = fs::read_to_string(path)
        .map_err(|error| format!("無法讀取表格 {}：{error}", path.display()))?;
    let mut lines = content.lines();
    if has_header {
        lines.next();
    }
    Ok(lines
        .filter(|line| !line.is_empty())
        .map(|line| parse_delimited_line(line, delimiter))
        .collect())
}

pub fn read_csv_with_header(path: &Path) -> Result<Vec<Vec<String>>, String> {
    read_delimited(path, ',', true)
}

pub fn write_delimited(
    path: &Path,
    delimiter: char,
    header: Option<&[&str]>,
    rows: &[Vec<String>],
) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("無法建立輸出目錄 {}：{error}", parent.display()))?;
    }

    let file =
        File::create(path).map_err(|error| format!("無法寫入 {}：{error}", path.display()))?;
    let mut writer = BufWriter::new(file);
    if let Some(header) = header {
        write_header_row(&mut writer, delimiter, header)
            .map_err(|error| format!("無法寫入 {}：{error}", path.display()))?;
    }
    for row in rows {
        write_data_row(&mut writer, delimiter, row)
            .map_err(|error| format!("無法寫入 {}：{error}", path.display()))?;
    }
    writer
        .flush()
        .map_err(|error| format!("無法寫入 {}：{error}", path.display()))
}

fn parse_delimited_line(line: &str, delimiter: char) -> Vec<String> {
    let mut fields = Vec::new();
    let mut field = String::new();
    let mut chars = line.chars().peekable();
    let mut in_quotes = false;
    while let Some(char) = chars.next() {
        match char {
            '"' if in_quotes && chars.peek() == Some(&'"') => {
                field.push('"');
                chars.next();
            }
            '"' if in_quotes => in_quotes = false,
            '"' if field.is_empty() => in_quotes = true,
            char if char == delimiter && !in_quotes => {
                fields.push(field);
                field = String::new();
            }
            _ => field.push(char),
        }
    }
    fields.push(field);
    fields
}

#[cfg(test)]
fn escape_delimited_value(value: &str, delimiter: char) -> String {
    if value.contains(delimiter) || value.contains('"') || value.contains('\n') {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value.to_string()
    }
}

fn write_header_row(
    writer: &mut BufWriter<File>,
    delimiter: char,
    row: &[&str],
) -> std::io::Result<()> {
    for (index, value) in row.iter().enumerate() {
        if index > 0 {
            write_char(writer, delimiter)?;
        }
        write_escaped_delimited_value(writer, value, delimiter)?;
    }
    writer.write_all(b"\n")
}

fn write_data_row(
    writer: &mut BufWriter<File>,
    delimiter: char,
    row: &[String],
) -> std::io::Result<()> {
    for (index, value) in row.iter().enumerate() {
        if index > 0 {
            write_char(writer, delimiter)?;
        }
        write_escaped_delimited_value(writer, value, delimiter)?;
    }
    writer.write_all(b"\n")
}

fn write_escaped_delimited_value(
    writer: &mut BufWriter<File>,
    value: &str,
    delimiter: char,
) -> std::io::Result<()> {
    if !value.contains(delimiter) && !value.contains('"') && !value.contains('\n') {
        return writer.write_all(value.as_bytes());
    }

    writer.write_all(b"\"")?;
    for char in value.chars() {
        if char == '"' {
            writer.write_all(b"\"\"")?;
        } else {
            write_char(writer, char)?;
        }
    }
    writer.write_all(b"\"")
}

fn write_char(writer: &mut BufWriter<File>, value: char) -> std::io::Result<()> {
    let mut buffer = [0_u8; 4];
    writer.write_all(value.encode_utf8(&mut buffer).as_bytes())
}

pub fn format_coordinate(value: &str) -> Result<String, String> {
    let parsed: f64 = value
        .parse()
        .map_err(|error| format!("座標格式錯誤 {value:?}：{error}"))?;
    Ok(format!("{parsed:.8}"))
}

pub fn parse_i64(value: &str, field: &str) -> Result<i64, String> {
    value
        .parse()
        .map_err(|error| format!("{field} 格式錯誤 {value:?}：{error}"))
}

pub fn parse_u32(value: &str, field: &str) -> Result<u32, String> {
    value
        .parse()
        .map_err(|error| format!("{field} 格式錯誤 {value:?}：{error}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escape_tsv_value_quotes_embedded_quotes_like_polars() {
        assert_eq!(
            escape_delimited_value("Poselok Turisticheskogo pansionata \"Klyazminskoe\"", '\t',),
            "\"Poselok Turisticheskogo pansionata \"\"Klyazminskoe\"\"\""
        );
    }

    #[test]
    fn escape_delimited_value_quotes_delimiter() {
        assert_eq!(escape_delimited_value("A\tB", '\t'), "\"A\tB\"");
        assert_eq!(escape_delimited_value("A,B", ','), "\"A,B\"");
    }

    #[test]
    fn parse_tsv_line_unescapes_embedded_quotes() {
        assert_eq!(
            parse_delimited_line("1\t\"Poselok \"\"Klyazminskoe\"\"\"\tName", '\t',),
            vec!["1", "Poselok \"Klyazminskoe\"", "Name"]
        );
    }

    #[test]
    fn parse_tsv_line_preserves_unquoted_embedded_quotes() {
        assert_eq!(
            parse_delimited_line(
                "1\tPoselok Turisticheskogo pansionata \"Klyazminskoe\"\tName",
                '\t',
            ),
            vec![
                "1",
                "Poselok Turisticheskogo pansionata \"Klyazminskoe\"",
                "Name",
            ]
        );
    }
}
