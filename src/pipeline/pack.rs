use std::fs;
use std::path::{Path, PathBuf};
use std::time::Instant;

use crate::cli::RunOptions;
use crate::pipeline::fixtures::{Fixture, load_fixtures};
use crate::pipeline::translate;

mod archive;

use archive::{release_entries, write_release_manifest, write_tar_gz, write_zip};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProductionPackOptions {
    pub output_dir: PathBuf,
    pub data_dir: PathBuf,
    pub project_dir: PathBuf,
    pub release_date: String,
    pub profile: bool,
}

pub fn run(options: &RunOptions) -> Result<(), String> {
    translate::run(options)?;

    let fixtures = load_fixtures(&options.fixtures_dir, options.fixture.as_deref())?;
    for fixture in fixtures {
        if !fixture.supports_stage("pack") {
            continue;
        }
        run_fixture(&fixture, options)?;
    }
    Ok(())
}

fn run_fixture(fixture: &Fixture, options: &RunOptions) -> Result<(), String> {
    if fixture.root.join("pack_sources").exists() {
        return run_production_fixture(fixture, options);
    }

    let fixture_output = options.output_dir.join(&fixture.manifest.name);
    let translate_output = fixture_output.join("translate");
    let pack_output = fixture_output.join("pack").join("release").join("geodata");
    fs::create_dir_all(&pack_output)
        .map_err(|error| format!("無法建立 pack 輸出目錄 {}：{error}", pack_output.display()))?;
    fs::copy(
        translate_output.join("admin1CodesASCII_translated.txt"),
        pack_output.join("admin1CodesASCII.txt"),
    )
    .map_err(|error| format!("無法複製 admin1CodesASCII：{error}"))?;
    fs::copy(
        translate_output.join("cities500_translated.txt"),
        pack_output.join("cities500.txt"),
    )
    .map_err(|error| format!("無法複製 cities500：{error}"))?;
    fs::write(
        pack_output.join("geodata-date.txt"),
        format!("{}\n", fixture.manifest.modification_date),
    )
    .map_err(|error| format!("無法寫入 geodata-date.txt：{error}"))?;
    println!(
        "stage=pack fixture={} output=release/geodata",
        fixture.manifest.name
    );
    Ok(())
}

fn run_production_fixture(fixture: &Fixture, options: &RunOptions) -> Result<(), String> {
    let source = fixture.root.join("pack_sources");
    let output = options.output_dir.join(&fixture.manifest.name).join("pack");
    if output.exists() {
        fs::remove_dir_all(&output).map_err(|error| format!("無法清理 pack output：{error}"))?;
    }
    let release = output.join("release");
    let geodata = release.join("geodata");
    fs::create_dir_all(&geodata).map_err(|error| format!("無法建立 release geodata：{error}"))?;

    copy_file(
        &source
            .join("geoname_data")
            .join("ne_10m_admin_0_countries.geojson"),
        &geodata.join("ne_10m_admin_0_countries.geojson"),
    )?;
    copy_file(
        &source
            .join("output")
            .join("admin1CodesASCII_translated.txt"),
        &geodata.join("admin1CodesASCII.txt"),
    )?;
    copy_file(
        &source.join("geoname_data").join("admin2Codes.txt"),
        &geodata.join("admin2Codes.txt"),
    )?;
    copy_file(
        &source.join("output").join("cities500_translated.txt"),
        &geodata.join("cities500.txt"),
    )?;
    copy_file(&source.join("LICENSE"), &release.join("LICENSE"))?;
    copy_file(&source.join("NOTICE.md"), &release.join("NOTICE.md"))?;
    copy_dir(
        &source.join("i18n-iso-countries"),
        &release.join("i18n-iso-countries"),
    )?;
    fs::write(
        geodata.join("geodata-date.txt"),
        fixture.manifest.modification_date.as_bytes(),
    )
    .map_err(|error| format!("無法寫入 geodata-date.txt：{error}"))?;

    let entries = release_entries(&release)?;
    write_release_manifest(&entries, &output.join("release-tree.manifest"))?;
    write_zip(&entries, &output.join("release.zip"))?;
    write_tar_gz(&release, &output.join("release.tar.gz"))?;
    println!(
        "stage=pack fixture={} output=release archives=true",
        fixture.manifest.name
    );
    Ok(())
}

pub fn run_production(options: &ProductionPackOptions) -> Result<(), String> {
    let mut profile = PackProfile::new(options.profile);
    profile.time("remove_old_releases", || {
        remove_old_releases(&options.output_dir)
    })?;
    let release = options.output_dir.join("release");
    let geodata = release.join("geodata");
    profile.time("copy_release_tree", || {
        fs::create_dir_all(&geodata).map_err(|error| {
            format!(
                "無法建立 production release geodata {}：{error}",
                geodata.display()
            )
        })?;

        copy_file(
            &options.data_dir.join("ne_10m_admin_0_countries.geojson"),
            &geodata.join("ne_10m_admin_0_countries.geojson"),
        )?;
        copy_file(
            &options.output_dir.join("admin1CodesASCII_translated.txt"),
            &geodata.join("admin1CodesASCII.txt"),
        )?;
        copy_file(
            &options.data_dir.join("admin2Codes.txt"),
            &geodata.join("admin2Codes.txt"),
        )?;
        copy_file(
            &options.output_dir.join("cities500_translated.txt"),
            &geodata.join("cities500.txt"),
        )?;
        copy_file(
            &options.project_dir.join("LICENSE"),
            &release.join("LICENSE"),
        )?;
        copy_file(
            &options.project_dir.join("NOTICE.md"),
            &release.join("NOTICE.md"),
        )?;
        // Reason: 來源改為 data/vendor/，但 release 內的目的地必須維持
        // `i18n-iso-countries`——update_data.sh 依該名稱把 langs/ 掛進使用者
        // 容器的 node_modules，改名會讓所有既有部署失效。
        copy_dir(
            &options
                .project_dir
                .join("data")
                .join("vendor")
                .join("i18n-iso-countries"),
            &release.join("i18n-iso-countries"),
        )?;
        fs::write(
            geodata.join("geodata-date.txt"),
            options.release_date.as_bytes(),
        )
        .map_err(|error| format!("無法寫入 geodata-date.txt：{error}"))?;
        Ok(())
    })?;

    let entries = profile.time("release_entries", || release_entries(&release))?;
    profile.time("write_manifest", || {
        write_release_manifest(&entries, &options.output_dir.join("release-tree.manifest"))
    })?;
    profile.time("write_zip", || {
        write_zip(&entries, &options.output_dir.join("release.zip"))
    })?;
    profile.time("write_tar_gz", || {
        write_tar_gz(&release, &options.output_dir.join("release.tar.gz"))
    })?;
    println!(
        "stage=pack mode=production output={} archives=true",
        options.output_dir.display()
    );
    profile.print();
    Ok(())
}

struct PackProfile {
    enabled: bool,
    started: Instant,
    timings: Vec<(&'static str, u128)>,
}

impl PackProfile {
    fn new(enabled: bool) -> Self {
        Self {
            enabled,
            started: Instant::now(),
            timings: Vec::new(),
        }
    }

    fn time<T, F>(&mut self, name: &'static str, action: F) -> Result<T, String>
    where
        F: FnOnce() -> Result<T, String>,
    {
        let started = Instant::now();
        let result = action();
        if self.enabled {
            self.timings.push((name, started.elapsed().as_millis()));
        }
        result
    }

    fn print(&self) {
        if !self.enabled {
            return;
        }
        let mut line = format!(
            "profile stage=pack.detail total_ms={}",
            self.started.elapsed().as_millis()
        );
        for (name, elapsed_ms) in &self.timings {
            line.push_str(&format!(" {name}_ms={elapsed_ms}"));
        }
        println!("{line}");
    }
}

fn remove_old_releases(output_dir: &Path) -> Result<(), String> {
    fs::create_dir_all(output_dir)
        .map_err(|error| format!("無法建立 output 目錄 {}：{error}", output_dir.display()))?;
    for entry in fs::read_dir(output_dir)
        .map_err(|error| format!("無法讀取 output 目錄 {}：{error}", output_dir.display()))?
    {
        let entry = entry.map_err(|error| format!("無法讀取 output 項目：{error}"))?;
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if !name.starts_with("release") {
            continue;
        }
        let path = entry.path();
        if path.is_dir() {
            fs::remove_dir_all(&path)
                .map_err(|error| format!("無法刪除舊 release 目錄 {}：{error}", path.display()))?;
        } else {
            fs::remove_file(&path)
                .map_err(|error| format!("無法刪除舊 release 檔案 {}：{error}", path.display()))?;
        }
    }
    Ok(())
}

fn copy_file(source: &Path, destination: &Path) -> Result<(), String> {
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("無法建立目錄 {}：{error}", parent.display()))?;
    }
    fs::copy(source, destination).map_err(|error| {
        format!(
            "無法複製 {} 到 {}：{error}",
            source.display(),
            destination.display()
        )
    })?;
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
            copy_file(&source_path, &destination_path)?;
        }
    }
    Ok(())
}
