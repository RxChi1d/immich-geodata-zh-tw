use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Manifest {
    pub name: String,
    pub stages: Vec<String>,
    pub countries: Vec<String>,
    pub extra_files: Vec<String>,
    pub base_geoname_id: i64,
    pub min_population: u32,
    pub modification_date: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fixture {
    pub root: PathBuf,
    pub manifest: Manifest,
}

impl Fixture {
    pub fn supports_stage(&self, stage: &str) -> bool {
        self.manifest.stages.iter().any(|item| item == stage)
    }
}

impl Manifest {
    fn default_stages() -> Vec<String> {
        [
            "extract",
            "transform_cities_schema",
            "admin1_load",
            "cities500_load",
            "translate",
            "pack",
        ]
        .iter()
        .map(|stage| (*stage).to_string())
        .collect()
    }
}

pub fn load_fixtures(
    fixtures_dir: &Path,
    fixture_name: Option<&str>,
) -> Result<Vec<Fixture>, String> {
    let fixture_dirs = if let Some(name) = fixture_name {
        vec![fixtures_dir.join(name)]
    } else {
        let mut dirs = Vec::new();
        for entry in fs::read_dir(fixtures_dir)
            .map_err(|error| format!("無法讀取 fixture 目錄 {}：{error}", fixtures_dir.display()))?
        {
            let path = entry
                .map_err(|error| format!("無法讀取 fixture 項目：{error}"))?
                .path();
            if path.is_dir() && path.join("manifest.json").exists() {
                dirs.push(path);
            }
        }
        dirs.sort();
        dirs
    };

    fixture_dirs
        .into_iter()
        .map(|root| {
            let manifest = read_manifest(&root.join("manifest.json"))?;
            Ok(Fixture { root, manifest })
        })
        .collect()
}

fn read_manifest(path: &Path) -> Result<Manifest, String> {
    let content = fs::read_to_string(path)
        .map_err(|error| format!("無法讀取 manifest {}：{error}", path.display()))?;
    Ok(Manifest {
        name: json_string(&content, "name")?,
        stages: json_optional_string_array(&content, "stages")
            .unwrap_or_else(Manifest::default_stages),
        countries: json_string_array(&content, "countries")?,
        extra_files: json_string_array(&content, "extra_files")?,
        base_geoname_id: json_i64(&content, "base_geoname_id")?,
        min_population: json_i64(&content, "min_population")? as u32,
        modification_date: json_string(&content, "modification_date")?,
    })
}

fn json_optional_string_array(content: &str, key: &str) -> Option<Vec<String>> {
    if !content.contains(&format!("\"{key}\"")) {
        return None;
    }
    json_string_array(content, key).ok()
}

fn json_string(content: &str, key: &str) -> Result<String, String> {
    let marker = format!("\"{key}\"");
    let start = content
        .find(&marker)
        .ok_or_else(|| format!("manifest 缺少欄位：{key}"))?;
    let after_colon = content[start..]
        .find(':')
        .map(|index| start + index + 1)
        .ok_or_else(|| format!("manifest 欄位格式錯誤：{key}"))?;
    let quote_start = content[after_colon..]
        .find('"')
        .map(|index| after_colon + index + 1)
        .ok_or_else(|| format!("manifest 字串欄位格式錯誤：{key}"))?;
    let quote_end = content[quote_start..]
        .find('"')
        .map(|index| quote_start + index)
        .ok_or_else(|| format!("manifest 字串欄位缺少結尾：{key}"))?;
    Ok(content[quote_start..quote_end].to_string())
}

fn json_i64(content: &str, key: &str) -> Result<i64, String> {
    let marker = format!("\"{key}\"");
    let start = content
        .find(&marker)
        .ok_or_else(|| format!("manifest 缺少欄位：{key}"))?;
    let after_colon = content[start..]
        .find(':')
        .map(|index| start + index + 1)
        .ok_or_else(|| format!("manifest 欄位格式錯誤：{key}"))?;
    let value: String = content[after_colon..]
        .chars()
        .skip_while(|char| char.is_whitespace())
        .take_while(|char| char.is_ascii_digit() || *char == '-')
        .collect();
    value
        .parse()
        .map_err(|error| format!("manifest 數字欄位格式錯誤 {key}={value:?}：{error}"))
}

fn json_string_array(content: &str, key: &str) -> Result<Vec<String>, String> {
    let marker = format!("\"{key}\"");
    let start = content
        .find(&marker)
        .ok_or_else(|| format!("manifest 缺少欄位：{key}"))?;
    let after_colon = content[start..]
        .find(':')
        .map(|index| start + index + 1)
        .ok_or_else(|| format!("manifest 欄位格式錯誤：{key}"))?;
    let array_start = content[after_colon..]
        .find('[')
        .map(|index| after_colon + index + 1)
        .ok_or_else(|| format!("manifest 陣列欄位格式錯誤：{key}"))?;
    let array_end = content[array_start..]
        .find(']')
        .map(|index| array_start + index)
        .ok_or_else(|| format!("manifest 陣列欄位缺少結尾：{key}"))?;
    let body = &content[array_start..array_end];
    Ok(body
        .split(',')
        .filter_map(|item| {
            let trimmed = item.trim();
            if trimmed.is_empty() {
                return None;
            }
            Some(trimmed.trim_matches('"').to_string())
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_manifest_fields() {
        let fixture = load_fixtures(Path::new("fixtures/parity"), Some("tw_minimal"))
            .unwrap()
            .remove(0);

        assert_eq!(fixture.manifest.name, "tw_minimal");
        assert_eq!(fixture.manifest.countries, vec!["TW"]);
        assert_eq!(fixture.manifest.base_geoname_id, 92_000_000);
    }
}
