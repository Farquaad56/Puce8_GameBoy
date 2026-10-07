//! Executor for the `run` command (task C01_44): load the ROM bytes and run the
//! machine for at most `max_cycles` T-cycles. No serial output yet.

use std::fs;
use std::process::ExitCode;

use puce8gb_core::{Dmg, Machine};

use crate::run_args::RunArgs;

/// Outcome of a ROM execution (task C01_44).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    /// The ROM could not be loaded.
    LoadError,
    /// The machine ran for `max_cycles` T-cycles without the serial matching yet.
    MaxCycles,
}

/// Run a ROM image for at most `args.max_cycles` T-cycles (task C01_44). No file
/// access: the caller reads the bytes. Later tasks add one call each to the loop.
pub fn run_rom(rom: &[u8], args: &RunArgs) -> Outcome {
    let mut dmg = match Dmg::new(rom) {
        Ok(dmg) => dmg,
        Err(_) => return Outcome::LoadError,
    };
    // One M-cycle is 4 dots; the CPU runs every 4th dot (task C01_02).
    for _ in 0..(args.max_cycles / 4) {
        for _ in 0..4 {
            dmg.tick();
        }
    }
    Outcome::MaxCycles
}

/// Map an outcome to a process exit code (task C01_44): LoadError => 3; MaxCycles =>
/// 2 when serial output was expected, else 0.
pub fn exit_code(o: &Outcome, args: &RunArgs) -> u8 {
    match o {
        Outcome::LoadError => 3,
        Outcome::MaxCycles => {
            if args.expect_serial.is_some() {
                2
            } else {
                0
            }
        }
    }
}

/// Read the ROM file and run it (task C01_44). Only this function touches files.
pub fn execute(args: &RunArgs) -> ExitCode {
    let rom = match fs::read(&args.rom) {
        Ok(bytes) => bytes,
        Err(err) => {
            eprintln!("error: cannot read {}: {err}", args.rom);
            return ExitCode::from(3);
        }
    };
    let outcome = run_rom(&rom, args);
    ExitCode::from(exit_code(&outcome, args))
}

#[cfg(test)]
mod tests {
    use puce8gb_core::media::header::Header;

    use super::*;

    /// Synthetic ROM image of at least `MIN_ROM_LEN` bytes with `code` placed at the
    /// entry point $0100 and a valid stored header checksum (note 07a).
    fn test_rom(code: &[u8]) -> Vec<u8> {
        let mut r = vec![0x00u8; 0x150.max(0x100 + code.len())];
        for (i, b) in code.iter().enumerate() {
            r[0x100 + i] = *b;
        }
        r[0x14D] = Header::header_checksum(&r);
        r
    }

    fn args(max_cycles: u64, expect_serial: Option<&str>) -> RunArgs {
        RunArgs {
            rom: "game.gb".to_string(),
            max_cycles,
            expect_serial: expect_serial.map(String::from),
            trace: 0,
        }
    }

    #[test]
    fn c01_44_rom_too_short_is_load_error() {
        assert_eq!(run_rom(&[0u8; 100], &args(400, None)), Outcome::LoadError);
        assert_eq!(run_rom(&[], &args(400, None)), Outcome::LoadError);
    }

    #[test]
    fn c01_44_valid_rom_reaches_max_cycles() {
        // An all-zero ROM is a stream of NOPs at $0100: it runs to the budget.
        assert_eq!(
            run_rom(&test_rom(&[]), &args(400, None)),
            Outcome::MaxCycles
        );
    }

    #[test]
    fn c01_44_exit_code_mapping() {
        let a = args(400, None);
        assert_eq!(exit_code(&Outcome::LoadError, &a), 3);
        assert_eq!(exit_code(&Outcome::MaxCycles, &a), 0);
        let b = args(400, Some("PASS"));
        assert_eq!(exit_code(&Outcome::MaxCycles, &b), 2);
    }
}
