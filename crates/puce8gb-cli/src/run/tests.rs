//! Unit tests for the `run` executor (tasks C01_44, C01_45, C01_46).

use puce8gb_core::media::header::Header;

use super::*;

/// Synthetic ROM image of at least `MIN_ROM_LEN` bytes with `code` placed at the
/// entry point $0100 and a valid stored header checksum (note 07a). An empty `code`
/// yields an all-zero ROM: every byte is NOP, so it runs as a pure NOP stream (task C01_48);
/// its stored checksum stays zero rather than being recomputed. The base size is large
/// enough that a pure-NOP run of the test's max_cycles never reaches open bus past the image.
fn test_rom(code: &[u8]) -> Vec<u8> {
    let mut r = vec![0x00u8; 0x200.max(0x100 + code.len())];
    for (i, b) in code.iter().enumerate() {
        r[0x100 + i] = *b;
    }
    if !code.is_empty() {
        r[0x14D] = Header::header_checksum(&r);
    }
    r
}

fn args(max_cycles: u64, expect_serial: Option<&str>) -> RunArgs {
    RunArgs {
        rom: "game.gb".to_string(),
        max_cycles,
        expect_serial: expect_serial.map(String::from),
        trace: 0,
        ly_fixed: None,
        frames: None,
        dump_ppm: None,
        hash: false,
        save_state: None,
        load_state: None,
        boot_rom: None,
        quiet: false,
    }
}

/// Synthetic 256-byte boot ROM: JP $00FC at $0000, then LD A,$01 and LDH ($50),A at $00FC (E14_05).
fn e14_05_boot() -> Vec<u8> {
    let mut boot = vec![0x00u8; 256];
    boot[0x00] = 0xC3;
    boot[0x01] = 0xFC;
    boot[0x02] = 0x00;
    boot[0xFC] = 0x3E;
    boot[0xFD] = 0x01;
    boot[0xFE] = 0xE0;
    boot[0xFF] = 0x50;
    boot
}

#[test]
fn e14_05_boot_machine_hands_off_within_one_frame() {
    let rom = vec![0x00u8; 0x8000];
    let mut dmg = build_machine(&rom, Some(&e14_05_boot())).expect("valid boot ROM");
    assert_eq!(dmg.cpu.pc, 0x0000);
    dmg.run_frame();
    assert!(dmg.cpu.pc >= 0x0100, "pc {:04X}", dmg.cpu.pc);
    assert!(!dmg.bus.boot_mapped());
}

#[test]
fn e14_05_short_boot_rom_is_a_boot_error() {
    let rom = vec![0x00u8; 0x8000];
    assert_eq!(
        build_machine(&rom, Some(&[0u8; 10])).err(),
        Some(BuildError::BadBoot)
    );
}

#[test]
fn e14_05_bad_rom_is_not_a_boot_error() {
    assert_eq!(
        build_machine(&[0u8; 100], Some(&e14_05_boot())).err(),
        Some(BuildError::BadRom)
    );
}

#[test]
fn e14_05_without_boot_rom_the_plain_machine_is_built() {
    let rom = vec![0x00u8; 0x8000];
    let dmg = build_machine(&rom, None).expect("valid ROM");
    assert_eq!(dmg.cpu.pc, 0x0100);
    assert!(!dmg.bus.boot_mapped());
}

#[test]
fn e14_05_bad_boot_exit_code_is_6() {
    assert_eq!(EXIT_BAD_BOOT, 6);
}

#[test]
fn c01_44_rom_too_short_is_load_error() {
    assert_eq!(run_rom(&[0u8; 100], &args(400, None)), Outcome::LoadError);
    assert_eq!(run_rom(&[], &args(400, None)), Outcome::LoadError);
}

#[test]
fn c01_44_valid_rom_stops_on_unimplemented_opcode() {
    // NOP is implemented (C01_05), so an all-zero ROM no longer stops on its own. Seed the
    // unimplemented record through the test seam instead of executing a real opcode, so
    // this test survives later instruction groups (task C01_48).
    let mut dmg = Dmg::new(&test_rom(&[])).expect("valid ROM");
    dmg.cpu.debug_set_unimplemented(0x99, 0x0100);

    assert_eq!(
        run_machine(&mut dmg, &args(400, None)),
        Outcome::Unimplemented {
            opcode: 0x99,
            pc: 0x0100
        }
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

#[test]
fn c01_45_exit_code_mapping() {
    let a = args(400, None);
    assert_eq!(exit_code(&Outcome::Found("Passed".to_string()), &a), 0);
    assert_eq!(exit_code(&Outcome::Failed("Failed #3".to_string()), &a), 1);
    let b = args(400, Some("PASS"));
    assert_eq!(exit_code(&Outcome::MaxCycles, &b), 2);
}

#[test]
fn c01_45_serial_capture_without_instructions() {
    // No instruction executed: write SB then SC (internal clock) directly on the bus.
    let mut dmg = Dmg::new(&test_rom(&[])).expect("valid ROM");
    dmg.bus.write(0xFF01, b'A');
    dmg.bus.write(0xFF02, 0x81);
    let mut buf = [0u8; 64];
    assert_eq!(dmg.take_serial_output(&mut buf), 1);
    assert_eq!(buf[0], b'A');
}

#[test]
fn c01_46_unimplemented_message_format() {
    // Uppercase hex, exactly 2 digits for the opcode and 4 for the PC (task C01_46).
    assert_eq!(
        unimplemented_message(0x00, 0x0100),
        "UNIMPLEMENTED opcode 0x00 at PC=0x0100"
    );
    assert_eq!(
        unimplemented_message(0xFF, 0xC000),
        "UNIMPLEMENTED opcode 0xFF at PC=0xC000"
    );
}

#[test]
fn c01_46_check_unimplemented_none_and_some() {
    assert_eq!(check_unimplemented(None), None);
    assert_eq!(
        check_unimplemented(Some((0x99, 0xC000))),
        Some(Outcome::Unimplemented {
            opcode: 0x99,
            pc: 0xC000
        })
    );
}

#[test]
fn c01_46_exit_code_is_four() {
    let a = args(400, None);
    assert_eq!(
        exit_code(
            &Outcome::Unimplemented {
                opcode: 0x99,
                pc: 0xC000
            },
            &a
        ),
        4
    );
}

#[test]
fn c01_46_run_stops_on_unimplemented_opcode() {
    // The record is seeded through the test seam (task C01_48): no real opcode is
    // executed through the CPU, so this test survives later implementations. The run
    // stops on it at the first M-cycle check.
    let mut dmg = Dmg::new(&test_rom(&[])).expect("valid ROM");
    dmg.cpu.debug_set_unimplemented(0x99, 0x0100);

    assert_eq!(
        run_machine(&mut dmg, &args(400, None)),
        Outcome::Unimplemented {
            opcode: 0x99,
            pc: 0x0100
        }
    );
}

#[test]
fn c01_48_all_nop_rom_reaches_max_cycles() {
    // An all-zero ROM is a stream of NOPs; NOP stays implemented forever (task C01_48),
    // so the run reaches max_cycles without stopping.
    assert_eq!(
        run_rom(&test_rom(&[]), &args(400, None)),
        Outcome::MaxCycles
    );
}

#[test]
fn c01_47_trace_line_example() {
    // Exact Gameboy-Doctor style line (task C01_47): uppercase hex, two digits for the
    // registers, four for SP and PC, then the four bytes at PC.
    assert_eq!(
        trace_line(
            0x01,
            0xB0,
            0x00,
            0x13,
            0x00,
            0xD8,
            0x01,
            0x4D,
            0xFFFE,
            0x0100,
            [0x00, 0xC3, 0x50, 0x01],
        ),
        "A:01 F:B0 B:00 C:13 D:00 E:D8 H:01 L:4D SP:FFFE PC:0100 PCMEM:00,C3,50,01"
    );
}

#[test]
fn c01_47_trace_line_sp_pc_max() {
    // SP and PC at the wrapping edge $FFFF keep exactly four hex digits (task C01_47).
    assert_eq!(
        trace_line(
            0xFF,
            0x80,
            0xFE,
            0xFD,
            0xFC,
            0xFB,
            0xFA,
            0xF9,
            0xFFFF,
            0xFFFF,
            [0xEF, 0xEE, 0xED, 0xEC],
        ),
        "A:FF F:80 B:FE C:FD D:FC E:FB H:FA L:F9 SP:FFFF PC:FFFF PCMEM:EF,EE,ED,EC"
    );
}

#[test]
fn e03_16_run_frames_on_nop_rom_gives_a_blank_frame() {
    let rom = test_rom(&[]);
    let (outcome, frame) = run_frames(&rom, &args(0, None), 2);
    assert_eq!(outcome, Outcome::FramesDone);
    assert_eq!(frame.len(), 23040);
    for &px in &frame {
        assert_eq!(px, 0xFFFF_FFFF);
    }
}

#[test]
fn e03_16_run_frames_load_error() {
    let (outcome, frame) = run_frames(&[0u8; 100], &args(0, None), 1);
    assert_eq!(outcome, Outcome::LoadError);
    assert!(frame.is_empty());
}

#[test]
fn e03_16_frames_done_exit_code_is_zero() {
    let a = args(0, Some("X"));
    assert_eq!(exit_code(&Outcome::FramesDone, &a), 0);
}

#[test]
fn e03_07_hash_is_deterministic_across_two_runs() {
    // Two machines built from the same synthetic ROM run the same number of frames:
    // the framebuffer hash line must be identical (E03_07).
    let rom = test_rom(&[]);
    let (_, f1) = run_frames(&rom, &args(0, None), 2);
    let (_, f2) = run_frames(&rom, &args(0, None), 2);
    assert_eq!(crate::hash::hash_line(&f1), crate::hash::hash_line(&f2));
}

#[test]
fn e04_06_cart_ram_result_text_is_scanned_like_serial() {
    let mut rom = test_rom(&[]);
    rom[0x147] = 0x03; // MBC1 with battery-backed RAM
    rom[0x149] = 0x02; // 8 KiB of cartridge RAM
    let mut dmg = Dmg::new(&rom).expect("valid ROM");
    let args = args(400, Some("Passed"));
    // Blargg cart-RAM result block: signature $DE $B0 $61 at $A001-$A003, status not
    // running ($A000 != $80), zero-terminated text starting at $A004 (E04_06). The MBC1
    // RAM is enabled with a $0A write to 0000-1FFF before the block is written.
    dmg.bus.write(0x0000, 0x0A);
    dmg.bus.write(0xA001, 0xDE);
    dmg.bus.write(0xA002, 0xB0);
    dmg.bus.write(0xA003, 0x61);
    for (i, b) in b"Passed\0".iter().enumerate() {
        dmg.bus.write(0xA004 + i as u16, *b);
    }
    assert_eq!(
        run_machine(&mut dmg, &args),
        Outcome::Found("Passed".to_string())
    );
}

#[test]
fn e04_07_cart_ram_text_longer_than_256_bytes_is_scanned() {
    let mut rom = test_rom(&[]);
    rom[0x147] = 0x03; // MBC1 with battery-backed RAM
    rom[0x149] = 0x02; // 8 KiB of cartridge RAM
    let mut dmg = Dmg::new(&rom).expect("valid ROM");
    let args = args(400, Some("Passed"));
    // Blargg cart-RAM result block: signature $DE $B0 $61 at $A001-$A003, status not
    // running ($A000 != $80), zero-terminated text starting at $A004 (E04_07). The text
    // is longer than the old 256-byte buffer: the verdict must still be scanned. The MBC1
    // RAM is enabled with a $0A write to 0000-1FFF before the block is written.
    dmg.bus.write(0x0000, 0x0A);
    dmg.bus.write(0xA001, 0xDE);
    dmg.bus.write(0xA002, 0xB0);
    dmg.bus.write(0xA003, 0x61);
    let mut text = vec![b'A'; 300];
    text.extend_from_slice(b"Passed\0");
    for (i, b) in text.iter().enumerate() {
        dmg.bus.write(0xA004 + i as u16, *b);
    }
    match run_machine(&mut dmg, &args) {
        Outcome::Found(s) => assert!(s.ends_with("Passed")),
        other => panic!("unexpected outcome {other:?}"),
    }
}

#[test]
fn e04_06_cart_ram_failure_text_is_reported() {
    let mut rom = test_rom(&[]);
    rom[0x147] = 0x03; // MBC1 with battery-backed RAM
    rom[0x149] = 0x02; // 8 KiB of cartridge RAM
    let mut dmg = Dmg::new(&rom).expect("valid ROM");
    let args = args(400, Some("Passed"));
    // Same cart-RAM result block carrying the failure marker text (E04_06), written through
    // the bus with the MBC1 RAM enabled.
    dmg.bus.write(0x0000, 0x0A);
    dmg.bus.write(0xA001, 0xDE);
    dmg.bus.write(0xA002, 0xB0);
    dmg.bus.write(0xA003, 0x61);
    for (i, b) in b"Failed #3\0".iter().enumerate() {
        dmg.bus.write(0xA004 + i as u16, *b);
    }
    dmg.bus.write(0xA000, 0x03);
    assert_eq!(
        run_machine(&mut dmg, &args),
        Outcome::Failed("Failed #3".to_string())
    );
}

use puce8gb_core::StateError;

use crate::run_args::parse_run_args;

fn e09_03_to_args(list: &[&str]) -> Vec<String> {
    list.iter().map(|s| s.to_string()).collect()
}

fn e09_03_parse_err(list: &[&str]) -> String {
    match parse_run_args(&e09_03_to_args(list)) {
        Ok(_) => panic!("expected a parse error"),
        Err(msg) => msg,
    }
}

/// Program at $0100: a stripe pattern in VRAM tile 0, then a loop copying LY into SCX.
fn e09_03_rom() -> Vec<u8> {
    test_rom(&[
        0x21, 0x00, 0x80, // LD HL,$8000
        0x3E, 0xAA, // LD A,$AA
        0x0E, 0x20, // LD C,$20
        0x22, // LD (HL+),A
        0x0D, // DEC C
        0x20, 0xFC, // JR NZ,-4
        0xF0, 0x44, // LDH A,($44)
        0xE0, 0x43, // LDH ($43),A
        0x18, 0xFA, // JR -6
    ])
}

#[test]
fn e09_03_parse_save_and_load_state_flags() {
    let a = parse_run_args(&e09_03_to_args(&[
        "rom.gb",
        "--frames",
        "3",
        "--save-state",
        "s.bin",
        "--load-state",
        "l.bin",
    ]))
    .expect("parses");
    assert_eq!(a.save_state.as_deref(), Some("s.bin"));
    assert_eq!(a.load_state.as_deref(), Some("l.bin"));
    let b = parse_run_args(&e09_03_to_args(&["rom.gb", "--frames", "3"])).expect("parses");
    assert_eq!(b.save_state, None);
    assert_eq!(b.load_state, None);
}

#[test]
fn e09_03_state_flags_require_frames_and_a_value() {
    assert_eq!(
        e09_03_parse_err(&["rom.gb", "--save-state", "s.bin"]),
        "--save-state requires --frames"
    );
    assert_eq!(
        e09_03_parse_err(&["rom.gb", "--load-state", "l.bin"]),
        "--load-state requires --frames"
    );
    assert_eq!(
        e09_03_parse_err(&["rom.gb", "--frames", "1", "--save-state"]),
        "--save-state requires a value"
    );
    assert_eq!(
        e09_03_parse_err(&["rom.gb", "--frames", "1", "--load-state"]),
        "--load-state requires a value"
    );
}

#[test]
fn e09_03_run_frames_state_resumes_like_a_continuous_run() {
    let rom = e09_03_rom();
    let a = args(0, None);
    let full = run_frames_state(&rom, &a, 4, None, false).expect("runs");
    assert!(
        full.frame.iter().any(|&p| p != full.frame[0]),
        "flat picture"
    );
    let first = run_frames_state(&rom, &a, 2, None, true).expect("runs");
    let state = first.state.expect("state requested");
    let second = run_frames_state(&rom, &a, 2, Some(&state), false).expect("runs");
    assert_eq!(second.outcome, Outcome::FramesDone);
    assert_eq!(second.frame, full.frame);
    assert!(second.state.is_none());
}

#[test]
fn e09_03_run_frames_state_rejects_a_bad_state() {
    let rom = e09_03_rom();
    let a = args(0, None);
    let err = run_frames_state(&rom, &a, 1, Some(&[0u8; 4]), false).expect_err("bad state");
    assert_eq!(err, StateError::BadSize);
    let first = run_frames_state(&rom, &a, 1, None, true).expect("runs");
    let mut state = first.state.expect("state requested");
    state[0] ^= 0xFF;
    let err = run_frames_state(&rom, &a, 1, Some(&state), false).expect_err("bad magic");
    assert_eq!(err, StateError::BadMagic);
}

#[test]
fn e09_03_cli_save_state_then_load_state_round_trip() {
    let dir = std::env::temp_dir().join(format!("puce8gb_e09_03_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("temp dir");
    let rom_path = dir.join("rom.gb");
    let state_path = dir.join("state.bin");
    let full_path = dir.join("full.ppm");
    let resumed_path = dir.join("resumed.ppm");
    std::fs::write(&rom_path, e09_03_rom()).expect("rom file");
    let path_of = |p: &std::path::Path| p.to_string_lossy().into_owned();

    let mut full = args(0, None);
    full.quiet = true;
    full.rom = path_of(&rom_path);
    full.frames = Some(4);
    full.dump_ppm = Some(path_of(&full_path));
    let _ = execute(&full);

    let mut save = args(0, None);
    save.quiet = true;
    save.rom = path_of(&rom_path);
    save.frames = Some(2);
    save.save_state = Some(path_of(&state_path));
    let _ = execute(&save);
    let state_len = std::fs::metadata(&state_path).expect("state file").len();
    assert!(state_len > 100_000, "state length {state_len}");

    let mut resume = args(0, None);
    resume.quiet = true;
    resume.rom = path_of(&rom_path);
    resume.frames = Some(2);
    resume.load_state = Some(path_of(&state_path));
    resume.dump_ppm = Some(path_of(&resumed_path));
    let _ = execute(&resume);

    let expected = std::fs::read(&full_path).expect("full ppm");
    let actual = std::fs::read(&resumed_path).expect("resumed ppm");
    assert!(!expected.is_empty());
    assert_eq!(actual, expected);
    let _ = std::fs::remove_dir_all(&dir);
}
