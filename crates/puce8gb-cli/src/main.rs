//! puce8gb-cli: reads ROM files from disk (the core only receives bytes) and prints
//! cartridge information. `info <rom>` exits 3 on any load error (bad file or bad ROM).

use std::fs;
use std::process::ExitCode;

use puce8gb_core::media::header::{Cartridge, Header};

/// Exit code for a load error: the file could not be read or is not a valid cartridge.
const EXIT_LOAD_ERROR: u8 = 3;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        None => {
            println!("{}", puce8gb_core::version());
            ExitCode::SUCCESS
        }
        Some("info") if args.len() == 2 => info(&args[1]),
        _ => {
            eprintln!("usage: puce8gb-cli [info <rom>]");
            ExitCode::from(2)
        }
    }
}

/// Print the cartridge header of `<rom>` (task E01_04): title, type, ROM/RAM size and
/// checksum status. Returns exit code 3 on any load error (bad file or bad ROM).
fn info(path: &str) -> ExitCode {
    let rom = match fs::read(path) {
        Ok(bytes) => bytes,
        Err(err) => {
            eprintln!("error: cannot read {path}: {err}");
            return ExitCode::from(EXIT_LOAD_ERROR);
        }
    };
    match Cartridge::new(rom) {
        Ok(cart) => {
            let title = String::from_utf8_lossy(&cart.header.title)
                .trim_end_matches('\0')
                .to_string();
            println!(
                "Title: {}",
                if title.is_empty() { "(none)" } else { &title }
            );
            println!(
                "Type: {} (code ${:02X})",
                cartridge_type_name(cart.header.cartridge_type),
                cart.header.cartridge_type
            );
            match Cartridge::rom_size_bytes(cart.header.rom_size_code) {
                Some(bytes) => println!(
                    "ROM size: {} KiB (code ${:02X})",
                    bytes / 1024,
                    cart.header.rom_size_code
                ),
                None => println!(
                    "ROM size: unknown (code ${:02X})",
                    cart.header.rom_size_code
                ),
            }
            match ram_size_bytes(cart.header.ram_size_code) {
                Some(0) => println!("RAM size: none (code ${:02X})", cart.header.ram_size_code),
                Some(bytes) => println!(
                    "RAM size: {} KiB (code ${:02X})",
                    bytes / 1024,
                    cart.header.ram_size_code
                ),
                None => println!(
                    "RAM size: unknown (code ${:02X})",
                    cart.header.ram_size_code
                ),
            }
            let checksum = if cart.header.checksum_ok {
                "ok"
            } else {
                "invalid"
            };
            println!(
                "Checksum: {} (${:02X} stored, ${:02X} computed)",
                checksum,
                cart.header.header_checksum,
                Header::header_checksum(&cart.rom)
            );
            ExitCode::SUCCESS
        }
        Err(err) => {
            eprintln!("error: cannot load cartridge: {err:?}");
            ExitCode::from(EXIT_LOAD_ERROR)
        }
    }
}

/// Name of the cartridge type at $0147 (note 07a "Type de cartouche").
fn cartridge_type_name(code: u8) -> &'static str {
    match code {
        0x00 => "ROM-only",
        0x01 => "MBC1",
        0x02 => "MBC1+RAM",
        0x03 => "MBC1+RAM+BATTERY",
        0x05 => "MBC2",
        0x06 => "MBC2+BATTERY",
        0x08 => "ROM+RAM (never used)",
        0x09 => "ROM+RAM+BATTERY (never used)",
        0x0B..=0x0D => "MMM01",
        0x0F..=0x13 => "MBC3 (+TIMER/+BATTERY)",
        0x15..=0x17 => "MBC4 (CONFLIT, note 07a)",
        0x19..=0x1E => "MBC5 (+RUMBLE/+RAM/+BATTERY)",
        0x20 => "MBC6",
        0x22 => "MBC7+SENSOR+RUMBLE+RAM+BATTERY",
        0xFC => "POCKET CAMERA",
        0xFD => "BANDAI TAMA5",
        0xFE => "HuC3",
        0xFF => "HuC1+RAM+BATTERY",
        _ => "unknown",
    }
}

/// External RAM size in bytes for the code at $0149 (note 07a "Taille RAM"):
/// $00 = none, $02 = 8 KiB, $03 = 32 KiB, $04 = 128 KiB, $05 = 64 KiB. The code $01 is a
/// CONFLIT between sources (note 07a) and every other code is unknown: both return None.
fn ram_size_bytes(code: u8) -> Option<usize> {
    match code {
        0x00 => Some(0),
        0x02 => Some(8 * 1024),
        0x03 => Some(32 * 1024),
        0x04 => Some(128 * 1024),
        0x05 => Some(64 * 1024),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use puce8gb_core::media::header::CartridgeLoadError;

    /// Synthetic ROM image of at least `MIN_ROM_LEN` bytes with the given header fields
    /// and a valid stored checksum at $014D (note 07a).
    fn rom_with(len: usize, title: &[u8], cart_type: u8, rom_size: u8, ram_size: u8) -> Vec<u8> {
        let mut r = vec![0x00u8; len.max(0x150)];
        for (i, b) in title.iter().enumerate() {
            if i < 16 {
                r[0x134 + i] = *b;
            }
        }
        r[0x147] = cart_type;
        r[0x148] = rom_size;
        r[0x149] = ram_size;
        r[0x14D] = Header::header_checksum(&r);
        r
    }

    #[test]
    fn e01_04_cli_info_fields_for_rom_only() {
        let cart = Cartridge::new(rom_with(32 * 1024, b"MY GAME", 0x00, 0x00, 0x00))
            .expect("a ROM-only cartridge loads");
        assert_eq!(
            String::from_utf8_lossy(&cart.header.title).trim_end_matches('\0'),
            "MY GAME"
        );
        assert_eq!(cartridge_type_name(cart.header.cartridge_type), "ROM-only");
        assert_eq!(
            Cartridge::rom_size_bytes(cart.header.rom_size_code),
            Some(32 * 1024)
        );
        assert_eq!(ram_size_bytes(cart.header.ram_size_code), Some(0));
        assert!(cart.header.checksum_ok);
    }

    #[test]
    fn e01_04_cli_info_fields_for_rom_only_with_ram() {
        let cart = Cartridge::new(rom_with(32 * 1024, b"MY GAME", 0x00, 0x00, 0x02))
            .expect("a ROM-only cartridge with RAM loads");
        assert_eq!(ram_size_bytes(cart.header.ram_size_code), Some(8 * 1024));
    }

    #[test]
    fn e01_04_cli_ram_size_codes() {
        assert_eq!(ram_size_bytes(0x00), Some(0));
        assert_eq!(ram_size_bytes(0x02), Some(8 * 1024));
        assert_eq!(ram_size_bytes(0x03), Some(32 * 1024));
        assert_eq!(ram_size_bytes(0x04), Some(128 * 1024));
        assert_eq!(ram_size_bytes(0x05), Some(64 * 1024));
        // $01 is a CONFLIT between sources (note 07a): unknown, not guessed.
        assert_eq!(ram_size_bytes(0x01), None);
        assert_eq!(ram_size_bytes(0xFF), None);
    }

    #[test]
    fn e01_04_cli_cartridge_type_names() {
        assert_eq!(cartridge_type_name(0x00), "ROM-only");
        assert_eq!(cartridge_type_name(0x01), "MBC1");
        assert_eq!(cartridge_type_name(0x03), "MBC1+RAM+BATTERY");
        assert_eq!(cartridge_type_name(0xFF), "HuC1+RAM+BATTERY");
        assert_eq!(cartridge_type_name(0x42), "unknown");
    }

    #[test]
    fn e01_04_cli_load_error_variants() {
        // A ROM shorter than the header is a load error (the CLI maps it to exit 3).
        assert!(matches!(
            Cartridge::new(vec![0u8; 100]),
            Err(CartridgeLoadError::TooShort)
        ));
    }
}
