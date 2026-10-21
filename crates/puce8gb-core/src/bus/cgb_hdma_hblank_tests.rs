//! Tests of the HBlank HDMA transfer, register $FF55 with bit 7 = 1 (E15_15).

use super::*;

/// Cgb bus over a 32 KiB zero cartridge.
fn cgb_bus() -> Bus {
    let mut bus = Bus::new(vec![0u8; 32 * 1024]);
    bus.cgb = true;
    bus
}

/// HDMA1-4 = $C1,$23,$01,$2F: source $C120, destination $8120.
fn set_regs(bus: &mut Bus) {
    bus.write(0xFF51, 0xC1);
    bus.write(0xFF52, 0x23);
    bus.write(0xFF53, 0x01);
    bus.write(0xFF54, 0x2F);
}

/// Byte i of the source area $C120.. is $10 + i.
fn fill_source(bus: &mut Bus, count: u16) {
    for i in 0..count {
        bus.write(0xC120 + i, 0x10 + i as u8);
    }
}

/// One full HBlank: the PPU leaves HBlank, then enters it again.
fn hblank(bus: &mut Bus) {
    bus.hdma_hblank_tick(false);
    bus.hdma_hblank_tick(true);
}

#[test]
fn e15_15_two_blocks_one_per_hblank() {
    let mut bus = cgb_bus();
    set_regs(&mut bus);
    fill_source(&mut bus, 48);
    bus.write(0xFF55, 0x81); // 2 blocks
    assert_eq!(bus.read(0xFF55), 0x01); // remaining blocks - 1, bit 7 = 0
    assert_eq!(bus.vram_bank(0, 0x8120), 0x00); // nothing copied before the first HBlank
    hblank(&mut bus);
    for i in 0..16u16 {
        assert_eq!(bus.vram_bank(0, 0x8120 + i), 0x10 + i as u8);
    }
    assert_eq!(bus.vram_bank(0, 0x8130), 0x00);
    assert_eq!(bus.read(0xFF55), 0x00);
    bus.hdma_hblank_tick(true); // still the same HBlank: no new block
    assert_eq!(bus.vram_bank(0, 0x8130), 0x00);
    hblank(&mut bus);
    for i in 0..32u16 {
        assert_eq!(bus.vram_bank(0, 0x8120 + i), 0x10 + i as u8);
    }
    assert_eq!(bus.vram_bank(0, 0x8140), 0x00); // not a third block
    assert_eq!(bus.read(0xFF55), 0xFF);
    hblank(&mut bus);
    assert_eq!(bus.vram_bank(0, 0x8140), 0x00); // finished: nothing more is copied
}

#[test]
fn e15_15_cancel_keeps_the_copied_block_and_sets_bit_7() {
    let mut bus = cgb_bus();
    set_regs(&mut bus);
    fill_source(&mut bus, 48);
    bus.write(0xFF55, 0x81);
    hblank(&mut bus);
    bus.write(0xFF55, 0x00); // cancel
    assert_eq!(bus.read(0xFF55), 0x80); // 1 block left: bit 7 set, count - 1 = 0
    hblank(&mut bus);
    assert_eq!(bus.vram_bank(0, 0x812F), 0x1F); // first block kept
    assert_eq!(bus.vram_bank(0, 0x8130), 0x00); // no second block after the cancel
}

#[test]
fn e15_15_no_copy_outside_hblank() {
    let mut bus = cgb_bus();
    set_regs(&mut bus);
    fill_source(&mut bus, 16);
    bus.write(0xFF55, 0x80);
    for _ in 0..100 {
        bus.hdma_hblank_tick(false); // VBlank, OAM scan, pixels or LCD off
    }
    assert_eq!(bus.vram_bank(0, 0x8120), 0x00);
    assert_eq!(bus.read(0xFF55), 0x00); // still armed: 1 block, count - 1 = 0
}

#[test]
fn e15_15_cpu_is_suspended_8_m_cycles_per_block() {
    let mut bus = cgb_bus();
    set_regs(&mut bus);
    bus.write(0xFF55, 0x80);
    assert!(!bus.hdma_stalled());
    hblank(&mut bus);
    for _ in 0..8 {
        assert!(bus.hdma_stalled());
    }
    assert!(!bus.hdma_stalled());
}

#[test]
fn e15_15_dmg_model_ignores_the_transfer() {
    let mut bus = Bus::new(vec![0u8; 32 * 1024]);
    set_regs(&mut bus);
    fill_source(&mut bus, 16);
    bus.write(0xFF55, 0x80);
    hblank(&mut bus);
    assert_eq!(bus.vram_bank(0, 0x8120), 0x00);
    assert_eq!(bus.read(0xFF55), 0xFF);
    assert!(!bus.hdma_stalled());
}
