//! Tests of the Cgb OPRI register $FF6C (E15_11).

use super::*;

/// Cgb bus over a 32 KiB cartridge whose CGB flag byte $0143 is `flag`, after a reset.
fn cgb_bus(flag: u8) -> Bus {
    let mut rom = vec![0u8; 32 * 1024];
    rom[0x0143] = flag;
    let mut bus = Bus::new(rom);
    bus.cgb = true;
    bus.reset();
    bus
}

#[test]
fn e15_11_initial_value_follows_the_cartridge_flag() {
    let bus = cgb_bus(0x80); // Cgb cartridge: CGB style priority
    assert_eq!(bus.read(0xFF6C), 0xFE);
    assert!(!bus.opri_dmg_style());
    let bus = cgb_bus(0xC0);
    assert!(!bus.opri_dmg_style());
    let bus = cgb_bus(0x00); // DMG-only cartridge: DMG style priority
    assert_eq!(bus.read(0xFF6C), 0xFF);
    assert!(bus.opri_dmg_style());
}

#[test]
fn e15_11_write_read_round_trip_keeps_bit_0_only() {
    let mut bus = cgb_bus(0x80);
    bus.write(0xFF6C, 0x01);
    assert_eq!(bus.read(0xFF6C), 0xFF);
    assert_eq!(bus.peek(0xFF6C), 0xFF);
    assert!(bus.opri_dmg_style());
    bus.write(0xFF6C, 0x00);
    assert_eq!(bus.read(0xFF6C), 0xFE);
    assert!(!bus.opri_dmg_style());
    bus.write(0xFF6C, 0xFE); // bit 0 clear: the other bits are not stored
    assert_eq!(bus.read(0xFF6C), 0xFE);
    bus.write(0xFF6C, 0xFF);
    assert_eq!(bus.read(0xFF6C), 0xFF);
}

#[test]
fn e15_11_dmg_model_has_no_opri() {
    let mut bus = Bus::new(vec![0u8; 32 * 1024]);
    bus.write(0xFF6C, 0x01);
    assert_eq!(bus.read(0xFF6C), 0xFF);
    assert_eq!(bus.peek(0xFF6C), 0xFF);
    assert_eq!(bus.opri, 0);
    assert!(!bus.opri_dmg_style());
}
