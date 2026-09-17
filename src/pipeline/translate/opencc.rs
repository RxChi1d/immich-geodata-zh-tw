//! OpenCC 簡繁轉換與中文字串的判別。
//!
//! 自 `translate.rs` 拆出——轉換器的建立與快取、以及「這串字是簡體還是繁體」
//! 的判斷，是翻譯流程呼叫的工具，不屬於流程本身。

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};

use opencc_rust::Converter;

use crate::unicode_han::{includes_han, is_han_name};

#[derive(Debug, Clone)]
pub(super) struct OpenCcConverter {
    s2t_converter: Converter,
    t2s_converter: Converter,
    s2t: RefCell<HashMap<String, String>>,
    t2s: RefCell<HashMap<String, String>>,
}

impl OpenCcConverter {
    pub(super) fn new_lazy() -> Result<Self, String> {
        Ok(Self {
            s2t_converter: native_converter("s2t")?,
            t2s_converter: native_converter("t2s")?,
            s2t: RefCell::new(HashMap::new()),
            t2s: RefCell::new(HashMap::new()),
        })
    }

    pub(super) fn new(values: Vec<String>) -> Result<Self, String> {
        let mut unique: Vec<String> = values
            .into_iter()
            .filter(|value| !value.is_empty())
            .collect::<HashSet<_>>()
            .into_iter()
            .collect();
        unique.sort();
        let converter = Self::new_lazy()?;
        for value in unique {
            converter.s2t(&value);
            converter.t2s(&value);
        }
        Ok(converter)
    }

    pub(super) fn s2t(&self, text: &str) -> String {
        if let Some(converted) = self.s2t.borrow().get(text) {
            return converted.clone();
        }
        let converted = self.s2t_converter.convert(text);
        self.s2t
            .borrow_mut()
            .insert(text.to_string(), converted.clone());
        converted
    }

    pub(super) fn t2s(&self, text: &str) -> String {
        if let Some(converted) = self.t2s.borrow().get(text) {
            return converted.clone();
        }
        let converted = self.t2s_converter.convert(text);
        self.t2s
            .borrow_mut()
            .insert(text.to_string(), converted.clone());
        converted
    }
}

#[cfg(test)]
pub(super) fn run_opencc(
    config: &str,
    values: &[String],
) -> Result<HashMap<String, String>, String> {
    run_native_opencc(config, values)
}

#[cfg(test)]
pub(super) fn run_native_opencc(
    config: &str,
    values: &[String],
) -> Result<HashMap<String, String>, String> {
    if values.is_empty() {
        return Ok(HashMap::new());
    }

    let converter = native_converter(config)?;
    Ok(values
        .iter()
        .map(|value| (value.clone(), converter.convert(value)))
        .collect())
}

pub(super) fn native_converter(config: &str) -> Result<Converter, String> {
    match config {
        "s2t" => opencc_rust::presets::cn2t::converter("cn", "t"),
        "t2s" => opencc_rust::presets::t2cn::converter("t", "cn"),
        other => {
            return Err(format!(
                "不支援的 OpenCC native config：{other}；目前僅支援 s2t/t2s"
            ));
        }
    }
    .map_err(|error| format!("無法建立 OpenCC native converter：{error}"))
}

pub(super) fn translate_metadata_name(name: &str, converter: &OpenCcConverter) -> Option<String> {
    if !is_chinese_name(name) {
        None
    } else if is_simplified_chinese(name, converter) {
        Some(converter.s2t(name))
    } else {
        Some(name.to_string())
    }
}

pub(super) fn translate_alternate_name(name: &str, converter: &OpenCcConverter) -> String {
    if is_traditional_chinese(name, converter) {
        name.to_string()
    } else {
        converter.s2t(name)
    }
}

pub(super) fn extract_chinese_name(
    alternate_names: &str,
    converter: &OpenCcConverter,
) -> Option<String> {
    let mut simplified_candidate = None;
    let mut generic_candidate = None;

    for name in alternate_names.split(',') {
        if is_traditional_chinese(name, converter) {
            return Some(name.to_string());
        }
        if is_simplified_chinese(name, converter) && simplified_candidate.is_none() {
            simplified_candidate = Some(name.to_string());
        } else if includes_han(name) && generic_candidate.is_none() {
            generic_candidate = Some(name.to_string());
        }
    }

    simplified_candidate
        .map(|name| converter.s2t(&name))
        .or(generic_candidate)
}

pub(super) fn is_simplified_chinese(text: &str, converter: &OpenCcConverter) -> bool {
    is_chinese_name(text) && text == converter.t2s(text)
}

pub(super) fn is_traditional_chinese(text: &str, converter: &OpenCcConverter) -> bool {
    is_chinese_name(text) && text == converter.s2t(text)
}

pub(super) fn is_chinese_name(text: &str) -> bool {
    is_han_name(text)
}
