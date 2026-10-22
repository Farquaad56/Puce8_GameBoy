//! Suite runner execution (E06_01): runs manifest entries against ROM bytes, collects
//! per-entry results, and wires the `suite` subcommand into the CLI. File access is
//! limited to `collect_files`, `run_suite` and `execute`.

use std::fs;
use std::path::Path;
use std::process::ExitCode;

use puce8gb_core::Machine;

use crate::hash;
use crate::run::{self, Outcome};
use crate::run_args::RunArgs;
use crate::suite::{
    baseline_ok, mooneye_signature, parse_baseline, parse_manifest, parse_suite_args, rom_matches,
    Entry, Kind,
};

/// Result of running one manifest entry (E06_01 step 2a).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EntryResult {
    /// The entry passed.
    Pass,
    /// The entry failed with a reason.
    Fail(String),
    /// The entry used its whole cycle budget without deciding.
    Timeout,
}

/// Build the RunArgs for an entry: no ROM path (the bytes are passed in) and `quiet` so
/// no serial echo reaches stdout (E06_01 step 2a).
fn entry_args(entry: &Entry) -> RunArgs {
    RunArgs {
        rom: String::new(),
        max_cycles: entry.max_cycles,
        expect_serial: Some(entry.param.clone()),
        trace: 0,
        ly_fixed: None,
        frames: None,
        dump_ppm: None,
        hash: false,
        save_state: None,
        load_state: None,
        boot_rom: None,
        model: entry.model, // from the manifest model column, default dmg (E15_22)
        quiet: true,
    }
}

/// Parse a hash param `FRAMES:16HEX` (E06_01 step 2a): the frame count as u64, then
/// exactly 16 lowercase hex digits. None when malformed.
fn parse_hash_param(param: &str) -> Option<(u64, String)> {
    let (frames_raw, hex) = param.split_once(':')?;
    let frames = frames_raw.parse::<u64>().ok()?;
    if hex.len() != 16 || !hex.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f')) {
        return None;
    }
    Some((frames, hex.to_string()))
}

/// Run one manifest entry against ROM bytes (E06_01 step 2a). No file access: the caller
/// reads the ROM. Serial entries run with `quiet` so no serial echo reaches stdout.
pub fn run_entry(entry: &Entry, rom: &[u8]) -> EntryResult {
    match entry.kind {
        Kind::Serial => match run::run_rom(rom, &entry_args(entry)) {
            Outcome::Found(_) => EntryResult::Pass,
            Outcome::Failed(_) => EntryResult::Fail("failure marker in serial output".to_string()),
            Outcome::MaxCycles => EntryResult::Timeout,
            Outcome::Unimplemented { opcode, pc } => {
                EntryResult::Fail(run::unimplemented_message(opcode, pc))
            }
            Outcome::LoadError => EntryResult::Fail("cannot load ROM".to_string()),
            Outcome::FramesDone => EntryResult::Fail("unexpected outcome".to_string()),
        },
        Kind::Mooneye => {
            let mut dmg = match run::new_machine(rom, None, entry.model) {
                Ok(dmg) => dmg,
                Err(_) => return EntryResult::Fail("cannot load ROM".to_string()),
            };
            let mut last_ldbb: Option<[u8; 6]> = None;
            for _ in 0..(entry.max_cycles / 4) {
                // The mooneye programs park on a $40 byte (LD B,B) once the registers hold
                // their final values; check the signature at that boundary.
                if dmg.cpu.at_boundary() && dmg.bus.peek(dmg.cpu.pc) == 0x40 {
                    match mooneye_signature(
                        dmg.cpu.b, dmg.cpu.c, dmg.cpu.d, dmg.cpu.e, dmg.cpu.h, dmg.cpu.l,
                    ) {
                        Some(true) => return EntryResult::Pass,
                        Some(false) => {
                            return EntryResult::Fail("registers are all 0x42".to_string())
                        }
                        None => {
                            last_ldbb = Some([
                                dmg.cpu.b, dmg.cpu.c, dmg.cpu.d, dmg.cpu.e, dmg.cpu.h, dmg.cpu.l,
                            ]);
                        }
                    }
                }
                for _ in 0..4 {
                    dmg.tick();
                }
                if let Some((opcode, pc)) = dmg.cpu.unimplemented() {
                    return EntryResult::Fail(run::unimplemented_message(opcode, pc));
                }
            }
            match last_ldbb {
                Some(r) => EntryResult::Fail(format!(
                    "no signature, last LD B,B regs {:02X} {:02X} {:02X} {:02X} {:02X} {:02X}",
                    r[0], r[1], r[2], r[3], r[4], r[5]
                )),
                None => EntryResult::Timeout,
            }
        }
        Kind::Hash => match parse_hash_param(&entry.param) {
            None => EntryResult::Fail("bad hash param".to_string()),
            Some((frames, hex)) => {
                let mut args = entry_args(entry);
                args.frames = Some(frames);
                let (_, frame) = run::run_frames(rom, &args, frames);
                let line = hash::hash_line(&frame);
                if line == format!("hash: {hex}") {
                    EntryResult::Pass
                } else {
                    EntryResult::Fail(format!("got {line}"))
                }
            }
        },
    }
}

/// Recursively collect all file paths under `root` (E06_01 step 2a), as strings with '/'
/// separators, sorted. A missing root gives an empty Vec.
pub fn collect_files(root: &Path) -> Vec<String> {
    let mut out = Vec::new();
    collect_into(root, &mut out);
    out.sort();
    out
}

/// Recursive helper for `collect_files`: append every file under `dir`.
fn collect_into(dir: &Path, out: &mut Vec<String>) {
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(_) => return, // missing root (or unreadable dir): nothing to collect
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_into(&path, out);
        } else {
            out.push(path.to_string_lossy().replace('\\', "/"));
        }
    }
}

/// First ROM in `files` matching the entry's dir pattern and prefix (E06_01 step 2a).
pub fn find_rom(files: &[String], entry: &Entry) -> Option<String> {
    files
        .iter()
        .find(|path| rom_matches(path, &entry.dir_pattern, &entry.prefix))
        .cloned()
}

/// Run every manifest entry (or only the one named by `only`) against the ROM files
/// (E06_01 step 2a): one line per entry, then `(passed, total, lines)`.
pub fn run_suite(
    files: &[String],
    entries: &[Entry],
    only: Option<&str>,
) -> (usize, usize, Vec<String>) {
    let mut passed = 0;
    let mut total = 0;
    let mut lines = Vec::new();
    for entry in entries {
        if let Some(name) = only {
            if entry.name != name {
                continue;
            }
        }
        total += 1;
        let line = match find_rom(files, entry) {
            None => format!(
                "FAIL {}: ROM not found ({} {})",
                entry.name, entry.dir_pattern, entry.prefix
            ),
            Some(path) => match fs::read(&path) {
                Err(_) => format!("FAIL {}: cannot read ROM", entry.name),
                Ok(rom) => match run_entry(entry, &rom) {
                    EntryResult::Pass => {
                        passed += 1;
                        format!("PASS {}", entry.name)
                    }
                    EntryResult::Fail(reason) => format!("FAIL {}: {reason}", entry.name),
                    EntryResult::Timeout => format!("TIMEOUT {}", entry.name),
                },
            },
        };
        lines.push(line);
    }
    (passed, total, lines)
}

/// Suite exit code (E06_01 step 2a): with a baseline, pass when at least the baseline
/// number of tests passed; without one, every entry must pass.
pub fn exit_code_for(passed: usize, total: usize, baseline: Option<usize>) -> u8 {
    let ok = match baseline {
        Some(b) => baseline_ok(passed, b),
        None => passed == total,
    };
    if ok {
        0
    } else {
        1
    }
}

/// Run the `suite` subcommand (E06_01 step 2b): parse arguments, read the manifest and
/// optional baseline, run every entry against the ROMs under roms/test-roms, print one
/// line per entry plus a score. Argument or file errors exit 64; the suite itself exits
/// `exit_code_for` (0 or 1).
pub fn execute(args: &[String]) -> std::process::ExitCode {
    let parsed = match parse_suite_args(args) {
        Ok(parsed) => parsed,
        Err(msg) => {
            eprintln!("error: {msg}");
            return ExitCode::from(64);
        }
    };
    let manifest_text = match fs::read_to_string(&parsed.manifest) {
        Ok(text) => text,
        Err(_) => {
            eprintln!("error: cannot read manifest {}", parsed.manifest);
            return ExitCode::from(64);
        }
    };
    let entries = match parse_manifest(&manifest_text) {
        Ok(entries) => entries,
        Err(msg) => {
            eprintln!("error: {msg}");
            return ExitCode::from(64);
        }
    };
    // Model filter (E15_22): the default run keeps the Dmg entries, `--cgb` runs the Cgb ones.
    let entries = crate::suite::entries_for_model(entries, parsed.cgb);
    let baseline = match &parsed.baseline {
        Some(path) => match fs::read_to_string(path) {
            Ok(text) => match parse_baseline(&text) {
                Ok(n) => Some(n),
                Err(msg) => {
                    eprintln!("error: {msg}");
                    return ExitCode::from(64);
                }
            },
            Err(_) => {
                eprintln!("error: cannot read baseline {path}");
                return ExitCode::from(64);
            }
        },
        None => None,
    };
    let files = collect_files(Path::new("roms/test-roms"));
    let (passed, total, lines) = run_suite(&files, &entries, parsed.only.as_deref());
    if total == 0 {
        if let Some(name) = &parsed.only {
            eprintln!("error: no entry named {name}");
            return ExitCode::from(64);
        }
    }
    for line in &lines {
        println!("{line}");
    }
    println!("SUITE: {passed}/{total}");
    ExitCode::from(exit_code_for(passed, total, baseline))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Synthetic 32 KiB ROM image with `code` placed at the entry point $0100.
    fn prog(code: &[u8]) -> Vec<u8> {
        let mut r = vec![0x00u8; 0x8000];
        r[0x100..0x100 + code.len()].copy_from_slice(code);
        r
    }

    fn entry(kind: Kind, cycles: u64, param: &str) -> Entry {
        Entry {
            name: "t".to_string(),
            model: crate::run_args::ModelChoice::Dmg,
            dir_pattern: "d/".to_string(),
            prefix: "p-".to_string(),
            kind,
            max_cycles: cycles,
            param: param.to_string(),
        }
    }

    /// Mooneye pass program: B C D E H L = 3 5 8 13 21 34, then park on the $40 byte.
    const PASS_PROG: [u8; 15] = [
        0x06, 0x03, 0x0E, 0x05, 0x16, 0x08, 0x1E, 0x0D, 0x26, 0x15, 0x2E, 0x22, 0x40, 0x18, 0xFE,
    ];

    /// Mooneye fail program: every register is $42.
    const FAIL_PROG: [u8; 15] = [
        0x06, 0x42, 0x0E, 0x42, 0x16, 0x42, 0x1E, 0x42, 0x26, 0x42, 0x2E, 0x42, 0x40, 0x18, 0xFE,
    ];

    /// Mooneye wrong program: L = $23 instead of $22; the signature never matches.
    const WRONG_PROG: [u8; 15] = [
        0x06, 0x03, 0x0E, 0x05, 0x16, 0x08, 0x1E, 0x0D, 0x26, 0x15, 0x2E, 0x23, 0x40, 0x18, 0xFE,
    ];

    /// Serial program: send 'O' on the serial port (SB then SC with the internal clock).
    const SERIAL_PROG: [u8; 10] = [0x3E, 0x4F, 0xE0, 0x01, 0x3E, 0x81, 0xE0, 0x02, 0x18, 0xFE];

    #[test]
    fn e06_01_run_entry_mooneye_pass_fail_timeout() {
        assert_eq!(
            run_entry(&entry(Kind::Mooneye, 100_000, "-"), &prog(&PASS_PROG)),
            EntryResult::Pass
        );
        let r = run_entry(&entry(Kind::Mooneye, 100_000, "-"), &prog(&FAIL_PROG));
        assert!(matches!(r, EntryResult::Fail(_)));
        assert_eq!(
            run_entry(&entry(Kind::Mooneye, 100_000, "-"), &prog(&WRONG_PROG)),
            EntryResult::Fail("no signature, last LD B,B regs 03 05 08 0D 15 23".to_string())
        );
    }

    /// Program that parks on an infinite loop without ever executing LD B,B ($40).
    const NO_LDBB_PROG: [u8; 2] = [0x18, 0xFE];

    #[test]
    fn battery_ldbb_regs_reported() {
        assert_eq!(
            run_entry(&entry(Kind::Mooneye, 100_000, "-"), &prog(&NO_LDBB_PROG)),
            EntryResult::Timeout
        );
    }

    #[test]
    fn e06_01_run_entry_serial_pass_and_timeout() {
        assert_eq!(
            run_entry(&entry(Kind::Serial, 100_000, "O"), &prog(&SERIAL_PROG)),
            EntryResult::Pass
        );
        assert_eq!(
            run_entry(&entry(Kind::Serial, 100_000, "Z"), &prog(&SERIAL_PROG)),
            EntryResult::Timeout
        );
    }

    #[test]
    fn e06_01_run_entry_hash_pass_fail_bad_param() {
        let (_, frame) = run::run_frames(
            &prog(&[0x18, 0xFE]),
            &RunArgs {
                rom: String::new(),
                max_cycles: 100_000,
                expect_serial: None,
                trace: 0,
                ly_fixed: None,
                frames: Some(2),
                dump_ppm: None,
                hash: false,
                save_state: None,
                load_state: None,
                boot_rom: None,
                model: crate::run_args::ModelChoice::Auto,
                quiet: true,
            },
            2,
        );
        let h = hash::frame_hash(&frame);

        assert_eq!(
            run_entry(
                &entry(Kind::Hash, 100_000, &format!("2:{:016x}", h)),
                &prog(&[0x18, 0xFE])
            ),
            EntryResult::Pass
        );
        let r = run_entry(
            &entry(Kind::Hash, 100_000, "2:0000000000000000"),
            &prog(&[0x18, 0xFE]),
        );
        assert!(matches!(r, EntryResult::Fail(_)));
        assert_eq!(
            run_entry(&entry(Kind::Hash, 100_000, "zzz"), &prog(&[0x18, 0xFE])),
            EntryResult::Fail("bad hash param".to_string())
        );
    }

    #[test]
    fn e06_01_find_rom_first_match_sorted() {
        let mut files = vec![
            "x/d/p-b.gb".to_string(),
            "x/d/p-a.gb".to_string(),
            "x/other/p-c.gb".to_string(),
        ];
        files.sort();
        assert_eq!(
            find_rom(&files, &entry(Kind::Mooneye, 100_000, "-")),
            Some("x/d/p-a.gb".to_string())
        );
        let mut e = entry(Kind::Mooneye, 100_000, "-");
        e.dir_pattern = "nope/".to_string();
        assert_eq!(find_rom(&files, &e), None);
    }

    #[test]
    fn e06_01_run_suite_pass_and_missing_rom() {
        let dir = std::env::temp_dir().join(format!("puce8gb_e06_01_{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir); // stale files from a crashed run must not leak in
        fs::create_dir_all(dir.join("d")).expect("the test dir creates");
        // A full ROM image (not just the raw program bytes): Dmg::new needs >= MIN_ROM_LEN.
        fs::write(dir.join("d").join("p-x.gb"), prog(&PASS_PROG)).expect("the test ROM writes");
        let files = collect_files(&dir);

        let found = Entry {
            name: "found".to_string(),
            model: crate::run_args::ModelChoice::Dmg,
            dir_pattern: "d/".to_string(),
            prefix: "p-".to_string(),
            kind: Kind::Mooneye,
            max_cycles: 100_000,
            param: "-".to_string(),
        };
        let missing = Entry {
            name: "missing".to_string(),
            model: crate::run_args::ModelChoice::Dmg,
            dir_pattern: "zz/".to_string(),
            prefix: "q-".to_string(),
            kind: Kind::Mooneye,
            max_cycles: 100_000,
            param: "-".to_string(),
        };
        let entries = vec![found.clone(), missing.clone()];

        let (passed, total, lines) = run_suite(&files, &entries, None);
        assert_eq!((passed, total), (1, 2));
        assert_eq!(lines[0], "PASS found");
        assert!(lines[1].starts_with("FAIL missing: ROM not found"));

        let (passed, total, _lines) = run_suite(&files, &entries, Some("missing"));
        assert_eq!((passed, total), (0, 1));

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn e06_01_exit_code_for() {
        assert_eq!(exit_code_for(5, 5, None), 0);
        assert_eq!(exit_code_for(4, 5, None), 1);
        assert_eq!(exit_code_for(4, 5, Some(4)), 0);
        assert_eq!(exit_code_for(3, 5, Some(4)), 1);
    }
}
