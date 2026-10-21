//! Tests of the general-purpose HDMA transfer, registers $FF51-$FF55 (E15_14).

use super::*;

/// Cgb bus over a 32 KiB zero cartridge.
fn cgb_bus() -> Bus {
    let mut bus = Bus::new(vec![0u8; 32 * 1024]);
    bus.cgb = true;
    bus
}

/// HDMA1-4 = $C1,$23,$01,$2F: source $C120, destination ((0x01 << 8) | 0x2F) & 0x1FF0 = $8120.
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

#[test]
fn e15_14_length_3_copies_64_bytes_from_c120_to_8120() {
    let mut bus = cgb_bus();
    set_regs(&mut bus);
    fill_source(&mut bus, 80);
    bus.write(0xFF55, 0x03);
    for i in 0..64u16 {
        assert_eq!(bus.vram_bank(0, 0x8120 + i), 0x10 + i as u8);
    }
    assert_eq!(bus.vram_bank(0, 0x811F), 0x00); // byte before the first one untouched
    assert_eq!(bus.vram_bank(0, 0x8160), 0x00); // byte after the last one untouched
    assert_eq!(bus.read(0xFF55), 0xFF); // finished
}

#[test]
fn e15_14_length_0_copies_exactly_16_bytes() {
    let mut bus = cgb_bus();
    set_regs(&mut bus);
    fill_source(&mut bus, 32);
    bus.write(0xFF55, 0x00);
    for i in 0..16u16 {
        assert_eq!(bus.vram_bank(0, 0x8120 + i), 0x10 + i as u8);
    }
    assert_eq!(bus.vram_bank(0, 0x8130), 0x00); // not 17 bytes
    assert_eq!(bus.read(0xFF55), 0xFF);
}

#[test]
fn e15_14_destination_bank_is_the_one_selected_by_vbk() {
    let mut bus = cgb_bus();
    set_regs(&mut bus);
    fill_source(&mut bus, 16);
    bus.write(0xFF4F, 1);
    bus.write(0xFF55, 0x00);
    assert_eq!(bus.vram_bank(1, 0x8120), 0x10);
    assert_eq!(bus.vram_bank(1, 0x812F), 0x1F);
    assert_eq!(bus.vram_bank(0, 0x8120), 0x00);
}

#[test]
fn e15_14_cpu_is_suspended_8_m_cycles_per_block() {
    let mut bus = cgb_bus();
    assert!(!bus.hdma_stalled());
    set_regs(&mut bus);
    bus.write(0xFF55, 0x03); // 4 blocks
    for _ in 0..32 {
        assert!(bus.hdma_stalled());
    }
    assert!(!bus.hdma_stalled());
}

#[test]
fn e15_14_dmg_model_ignores_the_registers() {
    let mut bus = Bus::new(vec![0u8; 32 * 1024]);
    set_regs(&mut bus);
    fill_source(&mut bus, 16);
    bus.write(0xFF55, 0x00);
    assert_eq!(bus.vram_bank(0, 0x8120), 0x00);
    assert_eq!(bus.read(0xFF55), 0xFF);
    assert!(!bus.hdma_stalled());
}
