//! ROM extraction from .zip archives (E13_07).

use std::io::{Cursor, Read};

/// Largest accepted ROM entry, uncompressed (8 MiB).
pub const MAX_ROM_BYTES: u64 = 8_388_608;

fn is_rom_name(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    lower.ends_with(".gb") || lower.ends_with(".gbc") || lower.ends_with(".sgb")
}

/// Returns (entry name, bytes) of the first ROM entry (.gb, .gbc, .sgb) in archive order.
pub fn extract_rom(zip_bytes: &[u8]) -> Result<(String, Vec<u8>), String> {
    let mut archive = zip::ZipArchive::new(Cursor::new(zip_bytes))
        .map_err(|_| "not a valid zip file".to_string())?;
    for index in 0..archive.len() {
        let Ok(mut entry) = archive.by_index(index) else {
            continue;
        };
        let Ok(name) = entry.name() else {
            continue;
        };
        if entry.is_dir() || !is_rom_name(&name) {
            continue;
        }
        if entry.size() > MAX_ROM_BYTES {
            return Err("ROM entry too large".to_string());
        }
        let name = name.to_string();
        let mut data = Vec::new();
        entry
            .by_ref()
            .take(MAX_ROM_BYTES + 1)
            .read_to_end(&mut data)
            .map_err(|_| "not a valid zip file".to_string())?;
        if data.len() as u64 > MAX_ROM_BYTES {
            return Err("ROM entry too large".to_string());
        }
        return Ok((name, data));
    }
    Err("no ROM in archive".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn build_zip(entries: &[(&str, &[u8])]) -> Vec<u8> {
        let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Stored);
        for (name, data) in entries {
            writer.start_file(*name, options).unwrap();
            writer.write_all(data).unwrap();
        }
        writer.finish().unwrap().into_inner()
    }

    #[test]
    fn e13_07_extracts_first_rom_entry() {
        let zip_bytes = build_zip(&[("readme.txt", b"hello"), ("game.GB", &[1, 2, 3, 4])]);
        let (name, data) = extract_rom(&zip_bytes).unwrap();
        assert_eq!(name, "game.GB");
        assert_eq!(data, vec![1, 2, 3, 4]);
    }

    #[test]
    fn e13_07_no_rom_in_archive() {
        let zip_bytes = build_zip(&[("a.txt", b"text")]);
        assert_eq!(
            extract_rom(&zip_bytes),
            Err("no ROM in archive".to_string())
        );
    }

    #[test]
    fn e13_07_not_a_zip() {
        assert_eq!(
            extract_rom(b"not a zip"),
            Err("not a valid zip file".to_string())
        );
    }

    #[test]
    fn e13_07_rom_name_matching() {
        assert!(is_rom_name("A.GBC"));
        assert!(is_rom_name("dir/b.sgb"));
        assert!(!is_rom_name("c.gb.bak"));
        assert!(!is_rom_name("d"));
    }
}
