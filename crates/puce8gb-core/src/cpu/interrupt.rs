//! Interrupt control opcodes (task C01_16): DI ($F3). Decoded by opcode bit fields per
//! decision C_00; no table. EI, RETI and interrupt dispatch arrive with task C01_32.

use super::Cpu;
use crate::bus::Bus;

impl Cpu {
    /// Interrupt control opcodes (task C01_16). Returns true when the current opcode is in
    /// this group, false otherwise. Timings and flag effects from note 02b / seed Opcodes.json:
    /// - DI ($F3): 4 T = 1 M-cycle, no flags affected; clears IME (interrupts disabled). The
    ///   fetch M-cycle is the whole instruction (decision C_00), so there is no bus access.
    pub(super) fn exec_interrupt_ops(&mut self, _bus: &mut Bus) -> bool {
        if self.opcode == 0xF3 {
            // DI: clear IME (note 02b "DI : efface IME"). No flag affected; the fetch is the
            // whole instruction.
            self.ime = false;
            self.done();
            return true;
        }

        false
    }
}

#[cfg(test)]
mod tests {
    use crate::cpu::{testutil, Cpu};

    #[test]
    fn c01_16_di_costs_one_tick_and_preserves_flags() {
        let mut bus = testutil::new_bus();
        let mut cpu = Cpu {
            f: 0xF0, // all four flags set
            ..Cpu::default()
        };

        let ticks = testutil::exec(&mut cpu, &mut bus, &[0xF3]); // DI

        assert_eq!(ticks, 1); // note 02b / seed Opcodes.json: 4 T = 1 M-cycle
        assert_eq!(cpu.pc, 0xC001); // PC advanced past the opcode
        assert_eq!(cpu.f, 0xF0); // flags unchanged (no flag affected)
    }

    #[test]
    fn c01_16_di_clears_ime() {
        let mut bus = testutil::new_bus();
        let mut cpu = Cpu {
            ime: true, // start with interrupts enabled
            ..Cpu::default()
        };

        testutil::exec(&mut cpu, &mut bus, &[0xF3]); // DI

        assert!(!cpu.ime); // IME cleared by di (note 02b)
    }

    #[test]
    fn c01_16_di_masks_f_low_nibble() {
        let mut bus = testutil::new_bus();
        let mut cpu = Cpu {
            f: 0xFF, // all four flags set plus a dirty low nibble
            ..Cpu::default()
        };

        testutil::exec(&mut cpu, &mut bus, &[0xF3]); // DI

        assert_eq!(cpu.f, 0xF0); // high nibble preserved, low nibble masked to zero
    }

    #[test]
    fn c01_16_di_is_not_recorded_unimplemented() {
        let mut bus = testutil::new_bus();
        let mut cpu = Cpu { ..Cpu::default() };

        testutil::exec(&mut cpu, &mut bus, &[0xF3]); // DI

        assert_eq!(cpu.unimplemented(), None); // $F3 is claimed by the interrupt group
    }
}
