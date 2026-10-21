//! Executor for the `run` command (task C01_44): load the ROM bytes and run the
//! machine for at most `max_cycles` T-cycles. Serial bytes are echoed on stdout and the
//! run stops early on the expected text or the failure marker (C01_45), or when the CPU
//! records an unimplemented opcode (C01_46). With `--trace N`, one line per instruction
//! is printed on stderr (task C01_47).

use std::fs;
use std::io::Write;
use std::process::ExitCode;

use puce8gb_core::savestate::HEADER_LEN;
use puce8gb_core::{Dmg, LoadError, Machine, StateError};

use crate::run_args::RunArgs;
// Re-exported for later tasks (C01_47+); bin-only crate, so unused names need an allow.
#[allow(unused_imports)]
pub use crate::serial_scan::{scan_serial, Scan, FAIL_MARKER};

#[cfg(test)]
mod tests;

/// Exit code for an invalid save state file (E09_03); 2, 3, 4 and 64 are taken. UNKNOWN - to confirm.
const EXIT_BAD_STATE: u8 = 5;

/// Exit code for a rejected boot ROM (E14_05); 2, 3, 4, 5 and 64 are taken. UNKNOWN - to confirm.
const EXIT_BAD_BOOT: u8 = 6;

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
    /// The requested number of frames ran to completion (E03_16).
    FramesDone,
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

/// One instruction trace line in Gameboy-Doctor style (task C01_47): uppercase hex, two
/// digits for the registers and four for SP/PC, then the four bytes at PC. The flat
/// register list is fixed by the task signature.
#[allow(clippy::too_many_arguments)]
pub fn trace_line(
    a: u8,
    f: u8,
    b: u8,
    c: u8,
    d: u8,
    e: u8,
    h: u8,
    l: u8,
    sp: u16,
    pc: u16,
    mem: [u8; 4],
) -> String {
    let [m0, m1, m2, m3] = mem;
    format!(
        "A:{a:02X} F:{f:02X} B:{b:02X} C:{c:02X} D:{d:02X} E:{e:02X} H:{h:02X} L:{l:02X} \
         SP:{sp:04X} PC:{pc:04X} PCMEM:{m0:02X},{m1:02X},{m2:02X},{m3:02X}"
    )
}

/// Run a ROM image for at most `args.max_cycles` T-cycles (task C01_44). No file access:
/// the caller reads the bytes. Builds the machine and delegates to `run_machine`. With
/// `--ly-fixed HEX`, every $FF44 read returns that value (task C01_49).
pub fn run_rom(rom: &[u8], args: &RunArgs) -> Outcome {
    let mut dmg = match Dmg::new(rom) {
        Ok(dmg) => dmg,
        Err(_) => return Outcome::LoadError,
    };
    if let Some(v) = args.ly_fixed {
        dmg.debug_set_ly_override(Some(v)); // debug tool for trace comparison (C01_49)
    }
    run_machine(&mut dmg, args)
}

/// Run `frames` frames of a ROM image (E03_16 spec) and return the outcome with a copy of the
/// final framebuffer (empty on a load error). No file access: the caller reads and writes files.
pub fn run_frames(rom: &[u8], args: &RunArgs, frames: u64) -> (Outcome, Vec<u32>) {
    let mut dmg = match Dmg::new(rom) {
        Ok(dmg) => dmg,
        Err(_) => return (Outcome::LoadError, Vec::new()),
    };
    if let Some(v) = args.ly_fixed {
        dmg.debug_set_ly_override(Some(v));
    }
    for _ in 0..frames {
        dmg.run_frame();
    }
    (Outcome::FramesDone, dmg.framebuffer().to_vec())
}

/// Result of `run_frames_state` (E09_03): the outcome, the final framebuffer (empty on a load
/// error) and the serialized machine state when it was requested.
#[derive(Debug)]
pub struct FramesRun {
    pub outcome: Outcome,
    pub frame: Vec<u32>,
    pub state: Option<Vec<u8>>,
}

/// Like `run_frames`, with save states (E09_03): when `load` is given the machine is restored
/// from those bytes before the frames run; when `save` is true the state after the frames is
/// returned. An invalid state is a `StateError` and runs nothing. No file access.
pub fn run_frames_state(
    rom: &[u8],
    args: &RunArgs,
    frames: u64,
    load: Option<&[u8]>,
    save: bool,
) -> Result<FramesRun, StateError> {
    let mut dmg = match Dmg::new(rom) {
        Ok(dmg) => dmg,
        Err(_) => {
            return Ok(FramesRun {
                outcome: Outcome::LoadError,
                frame: Vec::new(),
                state: None,
            })
        }
    };
    if let Some(v) = args.ly_fixed {
        dmg.debug_set_ly_override(Some(v));
    }
    if let Some(bytes) = load {
        // A buffer shorter than the header cannot hold a state (E09_03): report BadSize
        // before the core's magic check, which would fire first on such buffers.
        if bytes.len() < HEADER_LEN {
            return Err(StateError::BadSize);
        }
        dmg.load_state(bytes)?;
    }
    for _ in 0..frames {
        dmg.run_frame();
    }
    let state = if save {
        Some(dmg.save_state_vec()?)
    } else {
        None
    };
    Ok(FramesRun {
        outcome: Outcome::FramesDone,
        frame: dmg.framebuffer().to_vec(),
        state,
    })
}

/// Run an already-built machine for at most `args.max_cycles` T-cycles (task C01_48): the
/// M-cycle loop of `run_rom`, split out so tests can pre-seed CPU state. Serial bytes are
/// echoed on stdout and the run stops early when the expected text or the failure marker
/// appears (C01_45), or when the CPU records an unimplemented opcode (C01_46); the serial
/// scan keeps priority over both. With `--trace N`, one line per instruction goes to stderr
/// at the start of each M-cycle, only on an instruction boundary (task C01_47).
pub fn run_machine(dmg: &mut Dmg, args: &RunArgs) -> Outcome {
    let mut collected = String::new();
    let mut buf = [0u8; 64];
    // Blargg cart-RAM result block (dmg_sound ROMs): bytes already scanned.
    let mut cart_buf = [0u8; 8192];
    let mut cart_len = 0usize;
    // Remaining traced instructions (task C01_47); 0 disables tracing.
    let mut trace_left = args.trace;
    // One M-cycle is 4 dots; the CPU runs every 4th dot (task C01_02).
    for _ in 0..(args.max_cycles / 4) {
        // Trace at the start of the M-cycle, before its ticks: one line per instruction,
        // never during its extra M-cycles (task C01_47).
        if trace_left > 0 && dmg.cpu.at_boundary() {
            let pc = dmg.cpu.pc;
            let mem = [
                dmg.bus.peek(pc),
                dmg.bus.peek(pc.wrapping_add(1)),
                dmg.bus.peek(pc.wrapping_add(2)),
                dmg.bus.peek(pc.wrapping_add(3)),
            ];
            eprintln!(
                "{}",
                trace_line(
                    dmg.cpu.a, dmg.cpu.f, dmg.cpu.b, dmg.cpu.c, dmg.cpu.d, dmg.cpu.e, dmg.cpu.h,
                    dmg.cpu.l, dmg.cpu.sp, pc, mem
                )
            );
            trace_left -= 1;
        }
        for _ in 0..4 {
            dmg.tick();
        }
        let n = dmg.take_serial_output(&mut buf);
        if n > 0 {
            // The serial echo is suppressed by `quiet` (E06_01); the scan still runs.
            if !args.quiet {
                print!("{}", String::from_utf8_lossy(&buf[..n]));
                let _ = std::io::stdout().flush();
            }
            match scan_serial(&mut collected, &buf[..n], args.expect_serial.as_deref()) {
                Scan::Found => return Outcome::Found(collected),
                Scan::Failed => return Outcome::Failed(collected),
                Scan::Continue => {}
            }
        }
        // Blargg cart-RAM result block (dmg_sound ROMs): feed the newly appeared bytes
        // into the same serial scan as $FF01 output (E04_06).
        let cn = dmg.bus.cart_result(&mut cart_buf);
        if cn > cart_len {
            // Cart-RAM echo suppressed by `quiet` too (E06_01); the scan still runs.
            if !args.quiet {
                print!("{}", String::from_utf8_lossy(&cart_buf[cart_len..cn]));
                let _ = std::io::stdout().flush();
            }
            match scan_serial(
                &mut collected,
                &cart_buf[cart_len..cn],
                args.expect_serial.as_deref(),
            ) {
                Scan::Found => return Outcome::Found(collected),
                Scan::Failed => return Outcome::Failed(collected),
                Scan::Continue => {}
            }
            cart_len = cn;
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
        Outcome::FramesDone => 0,
        Outcome::MaxCycles => {
            if args.expect_serial.is_some() {
                2
            } else {
                0
            }
        }
    }
}

/// Why `build_machine` failed (E14_05).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuildError {
    /// The ROM image cannot be loaded.
    BadRom,
    /// The boot ROM image is rejected by the core.
    BadBoot,
}

/// Builds the machine for the `run` command (E14_05): plain `Dmg::new` without a boot ROM,
/// `Dmg::with_boot_rom` with one. No file access: the caller reads the bytes.
pub fn build_machine(rom: &[u8], boot: Option<&[u8]>) -> Result<Dmg, BuildError> {
    match boot {
        None => Dmg::new(rom).map_err(|_| BuildError::BadRom),
        Some(boot) => Dmg::with_boot_rom(rom, boot).map_err(|err| {
            if err == LoadError::BadBootRom {
                BuildError::BadBoot
            } else {
                BuildError::BadRom
            }
        }),
    }
}

/// Runs a ROM with a boot ROM (E14_05): frames mode when `--frames` is given, else the
/// M-cycle loop of `run_machine`. Only this function and `execute` touch files.
fn execute_with_boot(args: &RunArgs, rom: &[u8], boot: &[u8]) -> ExitCode {
    let mut dmg = match build_machine(rom, Some(boot)) {
        Ok(dmg) => dmg,
        Err(BuildError::BadBoot) => {
            eprintln!("error: boot ROM rejected");
            return ExitCode::from(EXIT_BAD_BOOT);
        }
        Err(BuildError::BadRom) => {
            eprintln!("error: cannot load ROM");
            return ExitCode::from(3);
        }
    };
    if let Some(v) = args.ly_fixed {
        dmg.debug_set_ly_override(Some(v));
    }
    if let Some(frames) = args.frames {
        for _ in 0..frames {
            dmg.run_frame();
        }
        let frame = dmg.framebuffer().to_vec();
        if let Some(path) = &args.dump_ppm {
            let written =
                fs::File::create(path).and_then(|mut f| crate::ppm::write_ppm(&frame, &mut f));
            if let Err(err) = written {
                eprintln!("error: cannot write {path}: {err}");
                return ExitCode::from(3);
            }
        }
        if args.hash {
            println!("{}", crate::hash::hash_line(&frame));
        }
        return ExitCode::from(exit_code(&Outcome::FramesDone, args));
    }
    let outcome = run_machine(&mut dmg, args);
    if let Outcome::Unimplemented { opcode, pc } = &outcome {
        println!("{}", unimplemented_message(*opcode, *pc));
    }
    ExitCode::from(exit_code(&outcome, args))
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
    if let Some(boot_path) = &args.boot_rom {
        let boot = match fs::read(boot_path) {
            Ok(bytes) => bytes,
            Err(err) => {
                eprintln!("error: cannot read {boot_path}: {err}");
                return ExitCode::from(3);
            }
        };
        return execute_with_boot(args, &rom, &boot);
    }
    if let Some(frames) = args.frames {
        let load = match &args.load_state {
            Some(path) => match fs::read(path) {
                Ok(bytes) => Some(bytes),
                Err(err) => {
                    eprintln!("error: cannot read {path}: {err}");
                    return ExitCode::from(3);
                }
            },
            None => None,
        };
        let run = match run_frames_state(
            &rom,
            args,
            frames,
            load.as_deref(),
            args.save_state.is_some(),
        ) {
            Ok(run) => run,
            Err(err) => {
                eprintln!("error: invalid state file: {err:?}");
                return ExitCode::from(EXIT_BAD_STATE);
            }
        };
        if run.outcome == Outcome::LoadError {
            return ExitCode::from(exit_code(&run.outcome, args));
        }
        if let (Some(path), Some(state)) = (&args.save_state, &run.state) {
            if let Err(err) = fs::write(path, state) {
                eprintln!("error: cannot write {path}: {err}");
                return ExitCode::from(3);
            }
        }
        if let Some(path) = &args.dump_ppm {
            let written =
                fs::File::create(path).and_then(|mut f| crate::ppm::write_ppm(&run.frame, &mut f));
            if let Err(err) = written {
                eprintln!("error: cannot write {path}: {err}");
                return ExitCode::from(3);
            }
        }
        if args.hash {
            println!("{}", crate::hash::hash_line(&run.frame));
        }
        return ExitCode::from(exit_code(&run.outcome, args));
    }
    let outcome = run_rom(&rom, args);
    if let Outcome::Unimplemented { opcode, pc } = &outcome {
        println!("{}", unimplemented_message(*opcode, *pc));
    }
    ExitCode::from(exit_code(&outcome, args))
}
