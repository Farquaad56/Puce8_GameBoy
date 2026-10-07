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
    }
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
