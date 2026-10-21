//! Tests of the Cgb VRAM bank 1 through VBK $FF4F (E15_07).

use super::*;

/// Cgb bus over a 32 KiB zero cartridge.
fn cgb_bus() -> Bus {
    let mut bus = Bus::new(vec![0u8; 32 * 1024]);
    bus.cgb = true;
    bus
}

#[test]
fn e15_07_banks_keep_separate_data() {
    let mut bus = cgb_bus();
    bus.write(0xFF4F, 0);
    bus.write(0x8000, 0xAA);
    bus.write(0x9FFF, 0xA1);
    bus.write(0xFF4F, 1);
    bus.write(0x8000, 0xBB);
    bus.write(0x9FFF, 0xB1);
    assert_eq!(bus.read(0x8000), 0xBB);
    assert_eq!(bus.read(0x9FFF), 0xB1);
    assert_eq!(bus.peek(0x8000), 0xBB);
    bus.write(0xFF4F, 0);
    assert_eq!(bus.read(0x8000), 0xAA);
    assert_eq!(bus.read(0x9FFF), 0xA1);
}

#[test]
fn e15_07_ppu_accessor_sees_both_banks() {
    let mut bus = cgb_bus();
    bus.write(0x8000, 0xAA);
    bus.write(0xFF4F, 1);
    bus.write(0x8000, 0xBB);
    // The accessor ignores the current VBK value.
    assert_eq!(bus.vram_bank(0, 0x8000), 0xAA);
    assert_eq!(bus.vram_bank(1, 0x8000), 0xBB);
    bus.write(0xFF4F, 0);
    assert_eq!(bus.vram_bank(0, 0x8000), 0xAA);
    assert_eq!(bus.vram_bank(1, 0x8000), 0xBB);
    assert_eq!(bus.vram_bank(2, 0x8000), 0xFF);
    // Bank 0 is the array the PPU reads today.
    assert_eq!(bus.vram[0], 0xAA);
}

#[test]
fn e15_07_vbk_reads_back_with_unused_bits_set() {
    let mut bus = cgb_bus();
    assert_eq!(bus.read(0xFF4F), 0xFE); // note 11a: VBK reads bit 0 = bank, other bits 1
    bus.write(0xFF4F, 1);
    assert_eq!(bus.read(0xFF4F), 0xFF);
    assert_eq!(bus.peek(0xFF4F), 0xFF);
    bus.write(0xFF4F, 2); // only bit 0 matters
    assert_eq!(bus.read(0xFF4F), 0xFE);
}

#[test]
fn e15_07_blocking_applies_to_both_banks() {
    let mut bus = cgb_bus();
    bus.write(0xFF4F, 0);
    bus.write(0x8000, 0xAA);
    bus.write(0xFF4F, 1);
    bus.write(0x8000, 0xBB);
    bus.set_video_access(false, true);
    bus.set_video_write_access(false, true);
    assert_eq!(bus.read(0x8000), 0xFF); // bank 1 selected and blocked
    bus.write(0x8000, 0x11); // ignored
    bus.write(0xFF4F, 0);
    assert_eq!(bus.read(0x8000), 0xFF); // bank 0 selected and blocked
    bus.write(0x8000, 0x22); // ignored
    bus.set_video_access(true, true);
    bus.set_video_write_access(true, true);
    assert_eq!(bus.read(0x8000), 0xAA);
    bus.write(0xFF4F, 1);
    assert_eq!(bus.read(0x8000), 0xBB);
}

#[test]
fn e15_07_dmg_vbk_is_not_a_register() {
    let mut bus = Bus::new(vec![0u8; 32 * 1024]);
    bus.write(0x8000, 0xAA);
    bus.write(0xFF4F, 1);
    assert_eq!(bus.read(0x8000), 0xAA);
    assert_eq!(bus.read(0xFF4F), 0xFF);
    bus.write(0x8000, 0xBB);
    bus.write(0xFF4F, 0);
    assert_eq!(bus.read(0x8000), 0xBB);
}

#[test]
fn e15_07_reset_selects_bank_zero() {
    let mut bus = cgb_bus();
    bus.write(0xFF4F, 1);
    bus.reset();
    assert_eq!(bus.read(0xFF4F), 0xFE);
}
