//! Tests of KEY1 $FF4D and the STOP speed switch bus side (E15_16).

use super::*;

/// Cgb bus over a 32 KiB zero cartridge.
fn cgb_bus() -> Bus {
    let mut bus = Bus::new(vec![0u8; 32 * 1024]);
    bus.cgb = true;
    bus
}

#[test]
fn e15_16_key1_unused_bits_read_one_and_bit_7_is_read_only() {
    let mut bus = cgb_bus();
    assert_eq!(bus.read(0xFF4D), 0x7E);
    bus.write(0xFF4D, 0x01);
    assert_eq!(bus.read(0xFF4D), 0x7F);
    bus.write(0xFF4D, 0x00);
    assert_eq!(bus.read(0xFF4D), 0x7E);
    bus.write(0xFF4D, 0xFE); // bit 7 and the unused bits are not writable
    assert_eq!(bus.read(0xFF4D), 0x7E);
}

#[test]
fn e15_16_armed_switch_toggles_speed_pauses_2050_m_cycles_and_resets_div() {
    let mut bus = cgb_bus();
    bus.write(0xFF4D, 0x01);
    bus.stop_speed_switch();
    assert!(bus.double_speed());
    assert_eq!(bus.read(0xFF4D), 0xFE); // bit 7 = 1, bit 0 back to 0
    assert_eq!(bus.read(0xFF04), 0x00);
    for _ in 0..2050 {
        assert!(bus.hdma_stalled());
    }
    assert!(!bus.hdma_stalled());
    bus.write(0xFF4D, 0x01);
    bus.stop_speed_switch(); // back to normal speed
    assert!(!bus.double_speed());
    assert_eq!(bus.read(0xFF4D), 0x7E);
}

#[test]
fn e15_16_stop_without_arming_changes_nothing() {
    let mut bus = cgb_bus();
    bus.stop_speed_switch();
    assert!(!bus.double_speed());
    assert!(!bus.hdma_stalled());
    assert_eq!(bus.read(0xFF4D), 0x7E);
}

#[test]
fn e15_16_dmg_model_has_no_key1() {
    let mut bus = Bus::new(vec![0u8; 32 * 1024]);
    bus.write(0xFF4D, 0x01);
    assert_eq!(bus.read(0xFF4D), 0xFF);
    bus.stop_speed_switch();
    assert!(!bus.double_speed());
    assert!(!bus.hdma_stalled());
}
