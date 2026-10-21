//! Tests of the Cgb WRAM banking through SVBK $FF70 (E15_06).

use super::*;

/// Cgb bus over a 32 KiB zero cartridge.
fn cgb_bus() -> Bus {
    let mut bus = Bus::new(vec![0u8; 32 * 1024]);
    bus.cgb = true;
    bus
}

#[test]
fn e15_06_banks_keep_separate_data() {
    let mut bus = cgb_bus();
    bus.write(0xFF70, 1);
    bus.write(0xD000, 0x11);
    bus.write(0xFF70, 2);
    bus.write(0xD000, 0x22);
    bus.write(0xFF70, 7);
    bus.write(0xDFFF, 0x77);
    bus.write(0xFF70, 1);
    assert_eq!(bus.read(0xD000), 0x11);
    bus.write(0xFF70, 2);
    assert_eq!(bus.read(0xD000), 0x22);
    bus.write(0xFF70, 7);
    assert_eq!(bus.read(0xDFFF), 0x77);
    assert_eq!(bus.peek(0xDFFF), 0x77);
    bus.write(0xFF70, 1);
    assert_eq!(bus.read(0xDFFF), 0x00);
}

#[test]
fn e15_06_selecting_zero_selects_bank_one() {
    let mut bus = cgb_bus();
    bus.write(0xFF70, 1);
    bus.write(0xD000, 0x11);
    bus.write(0xFF70, 0);
    assert_eq!(bus.read(0xD000), 0x11);
    bus.write(0xD000, 0x33);
    bus.write(0xFF70, 1);
    assert_eq!(bus.read(0xD000), 0x33);
}

#[test]
fn e15_06_bank_zero_area_is_not_affected_by_svbk() {
    let mut bus = cgb_bus();
    bus.write(0xC000, 0xAA);
    bus.write(0xCFFF, 0xBB);
    for bank in 0u8..8 {
        bus.write(0xFF70, bank);
        assert_eq!(bus.read(0xC000), 0xAA, "bank {bank}");
        assert_eq!(bus.read(0xCFFF), 0xBB, "bank {bank}");
    }
    bus.write(0xFF70, 5);
    bus.write(0xC010, 0xCC);
    bus.write(0xFF70, 1);
    assert_eq!(bus.read(0xC010), 0xCC);
}

#[test]
fn e15_06_echo_follows_the_current_bank() {
    let mut bus = cgb_bus();
    bus.write(0xFF70, 3);
    bus.write(0xD123, 0x5A);
    assert_eq!(bus.read(0xF123), 0x5A);
    bus.write(0xF456, 0x6B);
    assert_eq!(bus.read(0xD456), 0x6B);
    bus.write(0xC010, 0x99);
    assert_eq!(bus.read(0xE010), 0x99);
    bus.write(0xFF70, 4);
    assert_eq!(bus.read(0xF123), 0x00);
    assert_eq!(bus.read(0xD123), 0x00);
    bus.write(0xFF70, 3);
    assert_eq!(bus.read(0xF123), 0x5A);
}

#[test]
fn e15_06_svbk_reads_back_with_unused_bits_set() {
    let mut bus = cgb_bus();
    assert_eq!(bus.read(0xFF70), 0xF8); // note 11a: SVBK $F8 after the boot ROM
    bus.write(0xFF70, 0x03);
    assert_eq!(bus.read(0xFF70), 0xFB);
    assert_eq!(bus.peek(0xFF70), 0xFB);
    bus.write(0xFF70, 0x0A); // only bits 0-2 are kept
    assert_eq!(bus.read(0xFF70), 0xFA);
    bus.write(0xFF70, 0x00);
    assert_eq!(bus.read(0xFF70), 0xF8);
}

#[test]
fn e15_06_dmg_ff70_is_not_a_register() {
    let mut bus = Bus::new(vec![0u8; 32 * 1024]);
    bus.write(0xD000, 0x11);
    bus.write(0xFF70, 2);
    assert_eq!(bus.read(0xD000), 0x11);
    assert_eq!(bus.read(0xFF70), 0xFF);
    bus.write(0xD000, 0x22);
    bus.write(0xFF70, 3);
    assert_eq!(bus.read(0xD000), 0x22);
    assert_eq!(bus.read(0xF000), 0x22);
}

#[test]
fn e15_06_reset_selects_bank_one_and_clears_banks() {
    let mut bus = cgb_bus();
    bus.write(0xFF70, 5);
    bus.write(0xD000, 0x44);
    bus.reset();
    assert_eq!(bus.read(0xFF70), 0xF8);
    assert_eq!(bus.read(0xD000), 0x00);
    bus.write(0xFF70, 5);
    assert_eq!(bus.read(0xD000), 0x00);
}
