//! puce8gb-cli: reads ROM files from disk (the core only receives bytes). `info <rom>`
//! prints cartridge information and exits 3 on any load error; `run` parses its
//! arguments (exit 64 when bad) but does not execute a ROM yet.

mod info;
mod run;
mod run_args;
mod serial_scan;

use std::process::ExitCode;

/// Exit code for bad `run` arguments (2 is reserved for timeout).
const EXIT_BAD_RUN_ARGS: u8 = 64;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        None => {
            println!("{}", puce8gb_core::version());
            ExitCode::SUCCESS
        }
        Some("info") if args.len() == 2 => info::info(&args[1]),
        Some("run") => match run_args::parse_run_args(&args[1..]) {
            Ok(parsed) => run::execute(&parsed),
            Err(msg) => {
                eprintln!("error: {msg}");
                ExitCode::from(EXIT_BAD_RUN_ARGS)
            }
        },
        _ => {
            eprintln!(
                "usage: puce8gb-cli [info <rom> | run <rom> [--max-cycles N] \
                 [--expect-serial TEXT] [--trace N]]"
            );
            ExitCode::from(2)
        }
    }
}

#[cfg(test)]
mod tests {
    use puce8gb_core::media::header::{Cartridge, CartridgeLoadError, Header};

    use crate::info::{cartridge_type_name, ram_size_bytes};

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
