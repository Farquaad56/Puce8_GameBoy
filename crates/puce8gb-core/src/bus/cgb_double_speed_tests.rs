//! Tests of the HDMA pause length in double speed (E15_17, note 11c).

use super::*;

/// Cgb bus over a 32 KiB zero cartridge.
fn cgb_bus() -> Bus {
    let mut bus = Bus::new(vec![0u8; 32 * 1024]);
    bus.cgb = true;
    bus
}

#[test]
fn e15_17_general_hdma_pauses_8_m_cycles_per_block_in_normal_speed() {
    let mut bus = cgb_bus();
    bus.write(0xFF55, 0x00);
    for _ in 0..8 {
        assert!(bus.hdma_stalled());
    }
    assert!(!bus.hdma_stalled());
}

#[test]
fn e15_17_general_hdma_pauses_16_m_cycles_per_block_in_double_speed() {
    let mut bus = cgb_bus();
    bus.double_speed = true;
    bus.write(0xFF55, 0x00);
    for _ in 0..16 {
        assert!(bus.hdma_stalled());
    }
    assert!(!bus.hdma_stalled());
}

#[test]
fn e15_17_hblank_hdma_pauses_16_m_cycles_per_block_in_double_speed() {
    let mut bus = cgb_bus();
    bus.double_speed = true;
    bus.write(0xFF55, 0x80);
    bus.hdma_hblank_tick(false);
    bus.hdma_hblank_tick(true);
    for _ in 0..16 {
        assert!(bus.hdma_stalled());
    }
    assert!(!bus.hdma_stalled());
}
