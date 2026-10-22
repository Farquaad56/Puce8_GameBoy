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

#[test]
fn battery_hdma_general_transfers_chain_without_rewriting_registers() {
    let mut bus = cgb_bus();
    for i in 0..64u16 {
        bus.write(0xC000 + i, (i as u8) + 1); // $C000-$C03F: byte i is i + 1
    }
    bus.write(0xFF51, 0xC0);
    bus.write(0xFF52, 0x00);
    bus.write(0xFF53, 0x00);
    bus.write(0xFF54, 0x00);
    bus.write(0xFF55, 0x01); // general transfer of 2 blocks: $C000-$C01F -> $8000-$801F
    bus.write(0xFF55, 0x00); // 1 block from the advanced pointers: $C020-$C02F -> $8020-$802F
    for i in 0..32u16 {
        assert_eq!(bus.vram_bank(0, 0x8000 + i), (i as u8) + 1); // the first 32 bytes
    }
    for i in 0..16u16 {
        assert_eq!(bus.vram_bank(0, 0x8020 + i), (i as u8) + 0x21); // bytes $C020-$C02F
    }
}

#[test]
fn battery_hdma_hblank_blocks_advance_the_pointers() {
    let mut bus = cgb_bus();
    for i in 0..64u16 {
        bus.write(0xC000 + i, (i as u8) + 1); // $C000-$C03F: byte i is i + 1
    }
    bus.write(0xFF51, 0xC0);
    bus.write(0xFF52, 0x00);
    bus.write(0xFF53, 0x00);
    bus.write(0xFF54, 0x00);
    bus.write(0xFF55, 0x82); // arm an HBlank transfer of 3 blocks: $C000 -> $8000
    bus.hdma_hblank_tick(false);
    bus.hdma_hblank_tick(true); // one block copied: $C000-$C00F -> $8000-$800F, pointers advance to $C010/$8010
    assert_eq!(bus.read(0xFF55), 0x01); // still active, 2 blocks left (bit 7 = 0 while active)
    bus.write(0xFF55, 0x00); // cancel the armed transfer
    bus.write(0xFF55, 0x00); // 1 block from the advanced pointers: $C010-$C01F -> $8010-$801F
    for i in 0..16u16 {
        assert_eq!(bus.vram_bank(0, 0x8010 + i), (i as u8) + 0x11); // bytes $C010-$C01F
    }
}

#[test]
fn battery_hdma_destination_wraps_inside_vram() {
    let mut bus = cgb_bus();
    for i in 0..32u16 {
        bus.write(0xC000 + i, (i as u8) + 1); // $C000-$C01F: byte i is i + 1
    }
    bus.write(0xFF51, 0xC0);
    bus.write(0xFF52, 0x00);
    bus.write(0xFF53, 0x1F); // destination $9FF0 (masked with $1FF0)
    bus.write(0xFF54, 0xF0);
    bus.write(0xFF55, 0x00); // general transfer of 1 block: $C000-$C00F -> $9FF0-$9FFF
    for i in 0..16u16 {
        assert_eq!(bus.vram_bank(0, 0x9FF0 + i), (i as u8) + 1);
    }
    bus.write(0xFF55, 0x00); // second block from the advanced pointers: $C010-$C01F -> wraps to $8000-$800F
    for i in 0..16u16 {
        // UNKNOWN - to confirm (hardware behaviour past $9FFF is not documented)
        assert_eq!(bus.vram_bank(0, 0x8000 + i), (i as u8) + 0x11); // bytes $C010-$C01F
    }
}
