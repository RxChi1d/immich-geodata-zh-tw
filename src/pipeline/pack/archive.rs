//! release tree 的封存輸出：manifest、zip 與 tar.gz。
//!
//! 自 `pack.rs` 拆出——封存格式的細節（zip 的 local header 佈局、crc32 表、
//! tar 的 512 位元組區塊）與「要打包哪些檔案」是兩件事，混在同一檔會讓
//! pack 的主流程被格式細節淹沒。

use std::fs::{self, File};
use std::io::{BufWriter, Cursor, Write};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

fn release_files(root: &Path) -> Result<Vec<PathBuf>, String> {
    let mut files = Vec::new();
    collect_files(root, root, &mut files)?;
    files.sort();
    Ok(files)
}

fn collect_files(root: &Path, current: &Path, files: &mut Vec<PathBuf>) -> Result<(), String> {
    for entry in fs::read_dir(current)
        .map_err(|error| format!("無法讀取目錄 {}：{error}", current.display()))?
    {
        let path = entry
            .map_err(|error| format!("無法讀取目錄項目：{error}"))?
            .path();
        if path.is_dir() {
            collect_files(root, &path, files)?;
        } else {
            files.push(
                path.strip_prefix(root)
                    .map_err(|error| format!("無法計算相對路徑：{error}"))?
                    .to_path_buf(),
            );
        }
    }
    Ok(())
}

pub(super) struct ReleaseEntry {
    name: String,
    mode: u32,
    data: Vec<u8>,
    fnv_checksum: u64,
    crc32: u32,
}

pub(super) fn release_entries(root: &Path) -> Result<Vec<ReleaseEntry>, String> {
    release_files(root)?
        .into_iter()
        .map(|relative| {
            let path = root.join(&relative);
            let mode = file_mode(&path)?;
            let data = fs::read(&path)
                .map_err(|error| format!("無法讀取 release 檔案 {}：{error}", path.display()))?;
            let (fnv_checksum, crc32) = release_checksums(&data);
            Ok(ReleaseEntry {
                name: relative.to_string_lossy().replace('\\', "/"),
                mode,
                fnv_checksum,
                crc32,
                data,
            })
        })
        .collect()
}

pub(super) fn write_release_manifest(
    entries: &[ReleaseEntry],
    output: &Path,
) -> Result<(), String> {
    let file =
        File::create(output).map_err(|error| format!("無法寫入 release manifest：{error}"))?;
    let mut writer = BufWriter::new(file);
    for entry in entries {
        writeln!(
            writer,
            "{}\t{:o}\t{:016x}",
            entry.name, entry.mode, entry.fnv_checksum
        )
        .map_err(|error| format!("無法寫入 release manifest：{error}"))?;
    }
    writer
        .flush()
        .map_err(|error| format!("無法寫入 release manifest：{error}"))
}

#[cfg(unix)]
fn file_mode(path: &Path) -> Result<u32, String> {
    use std::os::unix::fs::PermissionsExt;

    Ok(fs::metadata(path)
        .map_err(|error| format!("無法讀取權限 {}：{error}", path.display()))?
        .permissions()
        .mode()
        & 0o777)
}

#[cfg(not(unix))]
fn file_mode(path: &Path) -> Result<u32, String> {
    let _ = path;
    Ok(0o644)
}

fn release_checksums(content: &[u8]) -> (u64, u32) {
    let crc_table = crc32_table();
    let mut fnv = 0xcbf29ce484222325_u64;
    let mut crc = 0xffff_ffff_u32;
    for byte in content {
        fnv ^= u64::from(*byte);
        fnv = fnv.wrapping_mul(0x100000001b3);
        let crc_index = ((crc ^ u32::from(*byte)) & 0xff) as usize;
        crc = (crc >> 8) ^ crc_table[crc_index];
    }
    (fnv, !crc)
}

pub(super) fn write_zip(entries: &[ReleaseEntry], output: &Path) -> Result<(), String> {
    let file = File::create(output)
        .map_err(|error| format!("無法建立 zip {}：{error}", output.display()))?;
    let mut file = BufWriter::new(file);
    let mut central = Vec::new();
    let mut offset = 0_u32;
    let dos_time = 0_u16;
    let dos_date: u16 = ((2026 - 1980) << 9) | (1 << 5) | 1;
    for entry in entries {
        let size = entry.data.len() as u32;
        let name_bytes = entry.name.as_bytes();

        let mut local = Vec::new();
        local.extend_from_slice(&0x04034b50_u32.to_le_bytes());
        local.extend_from_slice(&20_u16.to_le_bytes());
        local.extend_from_slice(&0_u16.to_le_bytes());
        local.extend_from_slice(&0_u16.to_le_bytes());
        local.extend_from_slice(&dos_time.to_le_bytes());
        local.extend_from_slice(&dos_date.to_le_bytes());
        local.extend_from_slice(&entry.crc32.to_le_bytes());
        local.extend_from_slice(&size.to_le_bytes());
        local.extend_from_slice(&size.to_le_bytes());
        local.extend_from_slice(&(name_bytes.len() as u16).to_le_bytes());
        local.extend_from_slice(&0_u16.to_le_bytes());
        local.extend_from_slice(name_bytes);
        file.write_all(&local)
            .map_err(|error| format!("無法寫入 zip local header：{error}"))?;
        file.write_all(&entry.data)
            .map_err(|error| format!("無法寫入 zip data：{error}"))?;

        central.extend_from_slice(&0x02014b50_u32.to_le_bytes());
        central.extend_from_slice(&20_u16.to_le_bytes());
        central.extend_from_slice(&20_u16.to_le_bytes());
        central.extend_from_slice(&0_u16.to_le_bytes());
        central.extend_from_slice(&0_u16.to_le_bytes());
        central.extend_from_slice(&dos_time.to_le_bytes());
        central.extend_from_slice(&dos_date.to_le_bytes());
        central.extend_from_slice(&entry.crc32.to_le_bytes());
        central.extend_from_slice(&size.to_le_bytes());
        central.extend_from_slice(&size.to_le_bytes());
        central.extend_from_slice(&(name_bytes.len() as u16).to_le_bytes());
        central.extend_from_slice(&0_u16.to_le_bytes());
        central.extend_from_slice(&0_u16.to_le_bytes());
        central.extend_from_slice(&0_u16.to_le_bytes());
        central.extend_from_slice(&0_u16.to_le_bytes());
        central.extend_from_slice(&0_u32.to_le_bytes());
        central.extend_from_slice(&offset.to_le_bytes());
        central.extend_from_slice(name_bytes);
        offset += local.len() as u32 + size;
    }
    let central_offset = offset;
    file.write_all(&central)
        .map_err(|error| format!("無法寫入 zip central directory：{error}"))?;
    let mut end = Vec::new();
    end.extend_from_slice(&0x06054b50_u32.to_le_bytes());
    end.extend_from_slice(&0_u16.to_le_bytes());
    end.extend_from_slice(&0_u16.to_le_bytes());
    let file_count = entries.len() as u16;
    end.extend_from_slice(&file_count.to_le_bytes());
    end.extend_from_slice(&file_count.to_le_bytes());
    end.extend_from_slice(&(central.len() as u32).to_le_bytes());
    end.extend_from_slice(&central_offset.to_le_bytes());
    end.extend_from_slice(&0_u16.to_le_bytes());
    file.write_all(&end)
        .map_err(|error| format!("無法寫入 zip end record：{error}"))?;
    file.flush()
        .map_err(|error| format!("無法寫入 zip end record：{error}"))
}

fn crc32_table() -> &'static [u32; 256] {
    static TABLE: OnceLock<[u32; 256]> = OnceLock::new();
    TABLE.get_or_init(|| {
        let mut table = [0_u32; 256];
        for (index, value) in table.iter_mut().enumerate() {
            let mut crc = index as u32;
            for _ in 0..8 {
                let mask = if crc & 1 == 1 { 0xedb8_8320 } else { 0 };
                crc = (crc >> 1) ^ mask;
            }
            *value = crc;
        }
        table
    })
}

pub(super) fn write_tar_gz(root: &Path, output: &Path) -> Result<(), String> {
    let entries = release_entries(root)?;
    let file = File::create(output)
        .map_err(|error| format!("無法建立 tar.gz {}：{error}", output.display()))?;
    let encoder = flate2::write::GzEncoder::new(file, flate2::Compression::default());
    let mut builder = tar::Builder::new(encoder);
    for entry in entries {
        let mut header = tar::Header::new_gnu();
        header
            .set_path(&entry.name)
            .map_err(|error| format!("無法設定 tar 路徑 {}：{error}", entry.name))?;
        header.set_size(entry.data.len() as u64);
        header.set_mode(entry.mode);
        header.set_uid(0);
        header.set_gid(0);
        header.set_mtime(1_767_225_600);
        header.set_cksum();
        builder
            .append(&header, Cursor::new(entry.data))
            .map_err(|error| format!("無法寫入 tar entry {}：{error}", entry.name))?;
    }
    let encoder = builder
        .into_inner()
        .map_err(|error| format!("無法完成 tar：{error}"))?;
    encoder
        .finish()
        .map_err(|error| format!("無法完成 gzip {}：{error}", output.display()))?;
    Ok(())
}
