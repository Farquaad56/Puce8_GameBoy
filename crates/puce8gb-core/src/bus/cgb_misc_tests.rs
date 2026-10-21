//! Tests of RP $FF56, the undocumented registers $FF72-$FF75 and PCM12/PCM34 (E15_18).

use super::*;

/// Cgb bus over a 32 KiB zero cartridge, in color mode.
fn cgb_bus() -> Bus {
    let mut bus = Bus::new(vec![0u8; 32 * 1024]);
    bus.cgb = true;
    bus
}

#[test]
fn e15_18_rp_keeps_bits_7_6_and_0_and_reads_the_others_as_one() {
    let mut bus = cgb_bus();
    assert_eq!(bus.read(0xFF56), 0x3E); // post-boot value (note 11a)
    bus.write(0xFF56, 0xFF);
    assert_eq!(bus.read(0xFF56), 0xFF);
    bus.write(0xFF56, 0xC0);
    assert_eq!(bus.read(0xFF56), 0xFE); // bit 1 (no signal) reads 1, LED bit 0 clear
    bus.write(0xFF56, 0x01);
    assert_eq!(bus.read(0xFF56), 0x3F);
    bus.write(0xFF56, 0x02); // bit 1 is not writable
    assert_eq!(bus.read(0xFF56), 0x3E);
    bus.write(0xFF56, 0x00);
    assert_eq!(bus.read(0xFF56), 0x3E);
    assert_eq!(bus.peek(0xFF56), 0x3E);
}

#[test]
fn e15_18_ff72_and_ff73_are_plain_read_write_bytes() {
    let mut bus = cgb_bus();
    assert_eq!(bus.read(0xFF72), 0x00);
    assert_eq!(bus.read(0xFF73), 0x00);
    bus.write(0xFF72, 0xA5);
    bus.write(0xFF73, 0x5A);
    assert_eq!(bus.read(0xFF72), 0xA5);
    assert_eq!(bus.read(0xFF73), 0x5A);
    bus.write(0xFF72, 0xFF);
    assert_eq!(bus.read(0xFF72), 0xFF);
}

#[test]
fn e15_18_ff74_is_read_write_in_color_mode_and_locked_in_compat_mode() {
    let mut bus = cgb_bus();
    assert_eq!(bus.read(0xFF74), 0x00);
    bus.write(0xFF74, 0x5A);
    assert_eq!(bus.read(0xFF74), 0x5A);
    let mut compat = cgb_bus();
    compat.compat = true;
    compat.write(0xFF74, 0x5A); // ignored
    assert_eq!(compat.read(0xFF74), 0xFF);
    compat.compat = false;
    assert_eq!(compat.read(0xFF74), 0x00); // the write was ignored
}

#[test]
fn e15_18_ff75_keeps_only_bits_6_to_4() {
    let mut bus = cgb_bus();
    assert_eq!(bus.read(0xFF75), 0x8F);
    bus.write(0xFF75, 0xFF);
    assert_eq!(bus.read(0xFF75), 0xFF);
    bus.write(0xFF75, 0x10);
    assert_eq!(bus.read(0xFF75), 0x9F);
    bus.write(0xFF75, 0x8F); // bits 7 and 3-0 are not writable
    assert_eq!(bus.read(0xFF75), 0x8F);
}

#[test]
fn e15_18_pcm_registers_are_read_only() {
    let mut bus = cgb_bus();
    assert_eq!(bus.read(0xFF76), 0x00); // format UNKNOWN - to confirm, 0 is used
    assert_eq!(bus.read(0xFF77), 0x00);
    bus.write(0xFF76, 0xFF);
    bus.write(0xFF77, 0xFF);
    assert_eq!(bus.read(0xFF76), 0x00);
    assert_eq!(bus.read(0xFF77), 0x00);
}

#[test]
fn e15_18_dmg_model_keeps_reading_ff() {
    let mut bus = Bus::new(vec![0u8; 32 * 1024]);
    for addr in [0xFF56u16, 0xFF72, 0xFF73, 0xFF74, 0xFF75, 0xFF76, 0xFF77] {
        assert_eq!(bus.read(addr), 0xFF, "before write {addr:#06x}");
        bus.write(addr, 0x00);
        assert_eq!(bus.read(addr), 0xFF, "after write {addr:#06x}");
    }
}
