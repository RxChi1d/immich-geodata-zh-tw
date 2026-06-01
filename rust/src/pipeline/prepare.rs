use std::fs;
use std::path::Path;
use std::path::PathBuf;

use crate::cli::RunOptions;
use crate::pipeline::fixtures::{Fixture, load_fixtures};
use crate::pipeline::prepare_download;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProductionPrepareOptions {
    pub target_dir: PathBuf,
    pub countries: Vec<String>,
    pub update: bool,
    pub geonames_base_url: String,
    pub natural_earth_url: String,
}

impl ProductionPrepareOptions {
    pub fn new(target_dir: PathBuf, countries: Vec<String>, update: bool) -> Self {
        Self {
            target_dir,
            countries,
            update,
            geonames_base_url: prepare_download::GEONAMES_BASE_URL.to_string(),
            natural_earth_url: prepare_download::NATURAL_EARTH_URL.to_string(),
        }
    }
}

pub fn run(options: &RunOptions) -> Result<(), String> {
    let fixtures = load_fixtures(&options.fixtures_dir, options.fixture.as_deref())?;
    for fixture in fixtures {
        if !fixture.supports_stage("prepare") {
            continue;
        }
        run_fixture(&fixture, options)?;
    }
    Ok(())
}

pub fn run_production(options: &ProductionPrepareOptions) -> Result<(), String> {
    prepare_download::run_production(options)
}

fn run_fixture(fixture: &Fixture, options: &RunOptions) -> Result<(), String> {
    let source = fixture.root.join("prepare_sources").join("geoname_data");
    let destination = options
        .output_dir
        .join(&fixture.manifest.name)
        .join("prepare")
        .join("geoname_data");
    if destination.exists() {
        fs::remove_dir_all(&destination)
            .map_err(|error| format!("無法清理 prepare output：{error}"))?;
    }
    copy_dir(&source, &destination)?;
    println!(
        "stage=prepare fixture={} source={}",
        fixture.manifest.name,
        source.display()
    );
    Ok(())
}

fn copy_dir(source: &Path, destination: &Path) -> Result<(), String> {
    fs::create_dir_all(destination)
        .map_err(|error| format!("無法建立目錄 {}：{error}", destination.display()))?;
    for entry in fs::read_dir(source)
        .map_err(|error| format!("無法讀取目錄 {}：{error}", source.display()))?
    {
        let entry = entry.map_err(|error| format!("無法讀取目錄項目：{error}"))?;
        let source_path = entry.path();
        let destination_path = destination.join(entry.file_name());
        if source_path.is_dir() {
            copy_dir(&source_path, &destination_path)?;
        } else {
            fs::copy(&source_path, &destination_path).map_err(|error| {
                format!(
                    "無法複製 {} 到 {}：{error}",
                    source_path.display(),
                    destination_path.display()
                )
            })?;
        }
    }
    Ok(())
}
