//! Tests of the Cgb palette RAM registers BCPS/BCPD/OCPS/OCPD (E15_08).

use super::*;

/// Cgb bus over a 32 KiB zero cartridge.
fn cgb_bus() -> Bus {
    let mut bus = Bus::new(vec![0u8; 32 * 1024]);
    bus.cgb = true;
    bus.reset();
    bus
}

#[test]
fn e15_08_auto_increment_writes_two_bytes() {
    let mut bus = cgb_bus();
    bus.write(0xFF68, 0x80);
    bus.write(0xFF69, 0x1F);
    bus.write(0xFF69, 0x00);
    assert_eq!(bus.bg_pal[0], 0x1F);
    assert_eq!(bus.bg_pal[1], 0x00);
    // Index is now 2, auto-increment still set, unused bit 6 read as 1 (UNKNOWN - to confirm).
    assert_eq!(bus.read(0xFF68), 0xC2);
    assert_eq!(bus.peek(0xFF68), 0xC2);
}

#[test]
fn e15_08_without_auto_increment_the_index_stays() {
    let mut bus = cgb_bus();
    bus.write(0xFF68, 0x05);
    bus.write(0xFF69, 0xAB);
    assert_eq!(bus.read(0xFF68), 0x45);
    assert_eq!(bus.read(0xFF69), 0xAB);
    assert_eq!(bus.bg_pal[5], 0xAB);
    bus.write(0xFF69, 0xCD);
    assert_eq!(bus.bg_pal[5], 0xCD);
    assert_eq!(bus.bg_pal[6], 0xFF);
}

#[test]
fn e15_08_index_wraps_from_63_to_0() {
    let mut bus = cgb_bus();
    bus.write(0xFF6A, 0xBF); // OBJ, index 63, auto-increment
    bus.write(0xFF6B, 0x11);
    assert_eq!(bus.obj_pal[63], 0x11);
    assert_eq!(bus.read(0xFF6A), 0xC0); // index back to 0, bit 7 kept, bit 6 reads 1
    bus.write(0xFF6B, 0x22);
    assert_eq!(bus.obj_pal[0], 0x22);
    assert_eq!(bus.read(0xFF6A), 0xC1);
}

#[test]
fn e15_08_bg_and_obj_ram_are_independent() {
    let mut bus = cgb_bus();
    bus.write(0xFF68, 0x00);
    bus.write(0xFF69, 0x12);
    bus.write(0xFF6A, 0x00);
    bus.write(0xFF6B, 0x34);
    assert_eq!(bus.bg_pal[0], 0x12);
    assert_eq!(bus.obj_pal[0], 0x34);
    bus.write(0xFF68, 0x01);
    assert_eq!(bus.read(0xFF69), 0xFF);
    bus.write(0xFF6A, 0x00);
    assert_eq!(bus.read(0xFF6B), 0x34);
}

#[test]
fn e15_08_mode_3_blocks_data_access_but_not_the_index() {
    let mut bus = cgb_bus();
    bus.write(0xFF68, 0x80);
    bus.write(0xFF69, 0x77);
    bus.write(0xFF68, 0x80);
    bus.set_video_access(false, true);
    bus.set_video_write_access(false, true);
    assert_eq!(bus.read(0xFF69), 0xFF); // blocked read returns $FF
    assert_eq!(bus.peek(0xFF69), 0x77); // peek is not blocked
    bus.write(0xFF69, 0x55); // ignored, but the index still increments
    assert_eq!(bus.bg_pal[0], 0x77);
    assert_eq!(bus.read(0xFF68), 0xC1);
    bus.set_video_access(true, true);
    bus.set_video_write_access(true, true);
    assert_eq!(bus.read(0xFF69), 0xFF);
    bus.write(0xFF68, 0x00);
    assert_eq!(bus.read(0xFF69), 0x77);
}

#[test]
fn e15_08_initial_content_is_documented() {
    let bus = cgb_bus();
    assert!(bus.bg_pal.iter().all(|&b| b == 0xFF)); // note 11b: BG colors are white; $FF used
    assert_eq!(bus.obj_pal[0], 0x00); // note 11b: first byte of OBJ0 color 0 is $00
    assert!(bus.obj_pal[1..].iter().all(|&b| b == 0xFF)); // UNKNOWN - to confirm
    assert_eq!(bus.read(0xFF68), 0x40);
    assert_eq!(bus.read(0xFF6A), 0x40);
}

#[test]
fn e15_08_dmg_palette_registers_are_not_present() {
    let mut bus = Bus::new(vec![0u8; 32 * 1024]);
    bus.write(0xFF68, 0x80);
    bus.write(0xFF69, 0x12);
    bus.write(0xFF6A, 0x80);
    bus.write(0xFF6B, 0x34);
    for addr in [0xFF68u16, 0xFF69, 0xFF6A, 0xFF6B] {
        assert_eq!(bus.read(addr), 0xFF, "{addr:#06x}");
        assert_eq!(bus.peek(addr), 0xFF, "{addr:#06x}");
    }
    assert!(bus.bg_pal.iter().all(|&b| b == 0xFF));
    assert_eq!(bus.obj_pal[0], 0xFF);
}
