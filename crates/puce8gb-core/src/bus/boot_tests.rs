//! Tests of the boot ROM overlay (E14_02).

use super::*;

/// 32 KiB cartridge image filled with $A5 whose byte 0 is $AA.
fn cart_rom() -> Vec<u8> {
    let mut rom = vec![0xA5u8; 32 * 1024];
    rom[0] = 0xAA;
    rom
}

/// Bus with a synthetic 256-byte boot ROM filled with $C3 installed.
fn bus_with_boot() -> Bus {
    let mut bus = Bus::new(cart_rom());
    bus.set_boot_rom(&[0xC3u8; 256], 256)
        .expect("valid boot ROM");
    bus
}

#[test]
fn e14_02_boot_rom_overlays_low_range() {
    let bus = bus_with_boot();
    assert_eq!(bus.read(0x0000), 0xC3);
    assert_eq!(bus.read(0x00FF), 0xC3);
    assert_eq!(bus.peek(0x0000), 0xC3);
    assert_eq!(bus.read(0x0100), 0xA5);
}

#[test]
fn e14_02_nonzero_write_unmaps() {
    let mut bus = bus_with_boot();
    bus.write(0xFF50, 0x01);
    assert_eq!(bus.read(0x0000), 0xAA);
    assert_eq!(bus.peek(0x0000), 0xAA);
    bus.write(0xFF50, 0x00);
    assert_eq!(bus.read(0x0000), 0xAA);
}

#[test]
fn e14_02_zero_write_changes_nothing() {
    let mut bus = bus_with_boot();
    bus.write(0xFF50, 0x00);
    assert_eq!(bus.read(0x0000), 0xC3);
}

#[test]
fn e14_02_bad_size_is_rejected() {
    let mut bus = Bus::new(cart_rom());
    let result = bus.set_boot_rom(&[0xC3u8; 255], 256);
    assert_eq!(result, Err(BootRomError::BadSize));
    assert_eq!(bus.read(0x0000), 0xAA);
    assert!(!bus.boot_mapped);
}

#[test]
fn e14_02_remap_boot_brings_the_rom_back() {
    let mut bus = bus_with_boot();
    bus.write(0xFF50, 0x01);
    assert_eq!(bus.read(0x0000), 0xAA);
    bus.remap_boot();
    assert_eq!(bus.read(0x0000), 0xC3);
    let mut plain = Bus::new(cart_rom());
    plain.remap_boot();
    assert_eq!(plain.read(0x0000), 0xAA);
}

#[test]
fn e14_02_ff50_read_is_unchanged() {
    let mut bus = bus_with_boot();
    assert_eq!(bus.read(0xFF50), 0xFF);
    bus.write(0xFF50, 0x01);
    assert_eq!(bus.read(0xFF50), 0xFF);
}
