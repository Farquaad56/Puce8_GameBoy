//! ROM header title, battery save path and window title (E07_06).

use std::path::{Path, PathBuf};

const TITLE_START: usize = 0x134;
/// End (exclusive) of the title field on DMG cartridges.
const TITLE_END_DMG: usize = 0x144;
/// End (exclusive) of the title when the CGB flag is set: the manufacturer code follows.
const TITLE_END_CGB: usize = 0x13F;
const CGB_FLAG: usize = 0x143;

/// Title from the cartridge header: printable ASCII up to the first other byte, trimmed.
pub fn rom_title(rom: &[u8]) -> String {
    if rom.len() < TITLE_END_DMG {
        return String::new();
    }
    let end = if matches!(rom[CGB_FLAG], 0x80 | 0xC0) {
        TITLE_END_CGB
    } else {
        TITLE_END_DMG
    };
    let mut title = String::new();
    for &byte in &rom[TITLE_START..end] {
        if !(0x20..=0x7E).contains(&byte) {
            break;
        }
        title.push(char::from(byte));
    }
    title.trim().to_string()
}

/// Battery RAM file: the ROM path with the extension replaced by `sav`.
pub fn save_path(rom_path: &Path) -> PathBuf {
    rom_path.with_extension("sav")
}

/// Window title for a ROM title (empty title gives the bare application name).
pub fn window_title(title: &str) -> String {
    if title.is_empty() {
        String::from("Puce8 GameBoy")
    } else {
        format!("Puce8 GameBoy - {title}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rom_with(title: &[u8]) -> Vec<u8> {
        let mut rom = vec![0u8; 0x150];
        rom[TITLE_START..TITLE_START + title.len()].copy_from_slice(title);
        rom
    }

    #[test]
    fn e07_06_title_is_read_from_the_header() {
        assert_eq!(rom_title(&rom_with(b"TETRIS")), "TETRIS");
    }

    #[test]
    fn e07_06_title_stops_at_a_non_printable_byte_and_is_trimmed() {
        assert_eq!(rom_title(&rom_with(&[b'A', 0x00, b'B'])), "A");
        assert_eq!(rom_title(&rom_with(b"AB  ")), "AB");
    }

    #[test]
    fn e07_06_cgb_title_is_limited_to_11_characters() {
        let mut rom = rom_with(b"ABCDEFGHIJKLMNOP");
        rom[CGB_FLAG] = 0x80;
        assert_eq!(rom_title(&rom), "ABCDEFGHIJK");
    }

    #[test]
    fn e07_06_short_rom_has_an_empty_title() {
        assert_eq!(rom_title(&[0u8; 0x100]), "");
    }

    #[test]
    fn e07_06_save_path_replaces_the_extension() {
        assert_eq!(
            save_path(Path::new("roms/game.gb")),
            PathBuf::from("roms/game.sav")
        );
        assert_eq!(save_path(Path::new("game")), PathBuf::from("game.sav"));
        assert_eq!(
            save_path(Path::new("a/game.tar.gb")),
            PathBuf::from("a/game.tar.sav")
        );
    }

    #[test]
    fn e07_06_window_title_includes_the_rom_title() {
        assert_eq!(window_title(""), "Puce8 GameBoy");
        assert_eq!(window_title("TETRIS"), "Puce8 GameBoy - TETRIS");
    }
}
