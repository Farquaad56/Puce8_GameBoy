//! Executor for the `run` command (task C01_44): load the ROM bytes and run the
//! machine for at most `max_cycles` T-cycles. Serial bytes are echoed on stdout and the
//! run stops early on the expected text or the failure marker (C01_45), or when the CPU
//! records an unimplemented opcode (C01_46).

use std::fs;
use std::io::Write;
use std::process::ExitCode;

use puce8gb_core::{Dmg, Machine};

use crate::run_args::RunArgs;
// Re-exported for later tasks (C01_47+); bin-only crate, so unused names need an allow.
#[allow(unused_imports)]
pub use crate::serial_scan::{scan_serial, Scan, FAIL_MARKER};

#[cfg(test)]
mod tests;

/// Outcome of a ROM execution (task C01_44).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// The ROM could not be loaded.
    LoadError,
    /// The machine ran for `max_cycles` T-cycles without the serial matching yet.
    MaxCycles,
    /// The expected text was found in the collected serial output (task C01_45).
    Found(String),
    /// The failure marker was found in the collected serial output (task C01_45).
    Failed(String),
    /// The CPU recorded an unimplemented opcode: `opcode` at address `pc` (C01_46).
    Unimplemented { opcode: u8, pc: u16 },
}

/// Message printed for an unimplemented opcode (task C01_46): uppercase hex, 2 digits
/// for the opcode and 4 for the PC.
pub fn unimplemented_message(opcode: u8, pc: u16) -> String {
    format!("UNIMPLEMENTED opcode 0x{:02X} at PC=0x{:04X}", opcode, pc)
}

/// Map a CPU unimplement record (task C01_46) to an outcome.
pub fn check_unimplemented(rec: Option<(u8, u16)>) -> Option<Outcome> {
    rec.map(|(opcode, pc)| Outcome::Unimplemented { opcode, pc })
}

/// Run a ROM image for at most `args.max_cycles` T-cycles (task C01_44). No file access:
/// the caller reads the bytes. Serial bytes are echoed on stdout and the run stops early
/// when the expected text or the failure marker appears (C01_45), or when the CPU records
/// an unimplemented opcode (C01_46); the serial scan keeps priority over both.
pub fn run_rom(rom: &[u8], args: &RunArgs) -> Outcome {
    let mut dmg = match Dmg::new(rom) {
        Ok(dmg) => dmg,
        Err(_) => return Outcome::LoadError,
    };
    let mut collected = String::new();
    let mut buf = [0u8; 64];
    // One M-cycle is 4 dots; the CPU runs every 4th dot (task C01_02).
    for _ in 0..(args.max_cycles / 4) {
        for _ in 0..4 {
            dmg.tick();
        }
        let n = dmg.take_serial_output(&mut buf);
        if n > 0 {
            print!("{}", String::from_utf8_lossy(&buf[..n]));
            let _ = std::io::stdout().flush();
            match scan_serial(&mut collected, &buf[..n], args.expect_serial.as_deref()) {
                Scan::Found => return Outcome::Found(collected),
                Scan::Failed => return Outcome::Failed(collected),
                Scan::Continue => {}
            }
        }
        // Once per M-cycle after the ticks: stop on an unimplemented opcode (C01_46).
        if let Some(outcome) = check_unimplemented(dmg.cpu.unimplemented()) {
            return outcome;
        }
    }
    Outcome::MaxCycles
}

/// Map an outcome to a process exit code (task C01_45): LoadError => 3; Found => 0;
/// Failed => 1; MaxCycles => 2 when serial output was expected, else 0. An unimplemented
/// opcode stops the run with exit code 4 (decision C_00).
pub fn exit_code(o: &Outcome, args: &RunArgs) -> u8 {
    match o {
        Outcome::LoadError => 3,
        Outcome::Found(_) => 0,
        Outcome::Failed(_) => 1,
        Outcome::Unimplemented { .. } => 4,
        Outcome::MaxCycles => {
            if args.expect_serial.is_some() {
                2
            } else {
                0
            }
        }
    }
}

/// Read the ROM file and run it (task C01_44). Only this function touches files; an
/// unimplemented opcode is reported on stdout before exit (C01_46).
pub fn execute(args: &RunArgs) -> ExitCode {
    let rom = match fs::read(&args.rom) {
        Ok(bytes) => bytes,
        Err(err) => {
            eprintln!("error: cannot read {}: {err}", args.rom);
            return ExitCode::from(3);
        }
    };
    let outcome = run_rom(&rom, args);
    if let Outcome::Unimplemented { opcode, pc } = &outcome {
        println!("{}", unimplemented_message(*opcode, *pc));
    }
    ExitCode::from(exit_code(&outcome, args))
}
