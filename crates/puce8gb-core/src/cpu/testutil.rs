//! Test helpers for CPU tests (decision C_00): place code in WRAM and run to the next
//! instruction boundary.

use super::Cpu;
use crate::bus::Bus;

/// A bus with a 32 KiB zero ROM in its power-up state (note 08): RAM zeroed, IO registers at
/// their post-boot values including IE = $00 and IF = $E1. CPU tests start from this so no
/// interrupt is pending unless a test sets one up explicitly (task C01_32).
pub fn new_bus() -> Bus {
    let mut bus = Bus::new(vec![0x00; 32 * 1024]);
    bus.reset();
    bus
}

/// Place `code` at $C000 through the bus, set PC there, tick until the next instruction
/// boundary and return the tick count. Panics on a runaway (more than 64 ticks).
#[allow(dead_code)] // used by the instruction group tests from C01_05 on (decision C_00)
pub fn exec(cpu: &mut Cpu, bus: &mut Bus, code: &[u8]) -> u32 {
    for (i, &b) in code.iter().enumerate() {
        bus.write(0xC000u16.wrapping_add(i as u16), b);
    }
    cpu.pc = 0xC000;

    let mut ticks = 0u32;
    loop {
        cpu.tick(bus);
        ticks += 1;
        assert!(
            ticks <= 64,
            "exec: runaway, no instruction boundary within 64 ticks"
        );
        if cpu.at_boundary() {
            return ticks;
        }
    }
}
