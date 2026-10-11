//! Interrupt control opcodes (task C01_16): DI ($F3). Task C01_30 adds RETI ($D9). EI and
//! interrupt dispatch arrive with task C01_32. Decoded by opcode bit fields per decision
//! C_00; no table.

use super::Cpu;
use crate::bus::Bus;

impl Cpu {
    /// Interrupt control opcodes (tasks C01_16, C01_30). Returns true when the current
    /// opcode is in this group, false otherwise. Timings and flag effects from note 02b /
    /// seed Opcodes.json; byte order of the popped return address confirmed by trace
    /// comparison against real hardware (task C01_30): a pop reads the low byte from [SP]
    /// and the high byte from [SP]+1.
    /// - DI ($F3): 4 T = 1 M-cycle, no flags affected; clears IME (interrupts disabled). The
    ///   fetch M-cycle is the whole instruction (decision C_00), so there is no bus access.
    /// - RETI ($D9; block 3, y == 3, z == 1): 16 T = 4 M-cycles, no flags affected. Pops the
    ///   return address like RET: the low byte is read from [SP] on step 2 and the high byte
    ///   from [SP]+1 on step 3; PC is set to the popped value and SP incremented by two on
    ///   step 4, where IME is also re-enabled (note 02b: equivalent of an EI immediately
    ///   followed by a RET).
    pub(super) fn exec_interrupt_ops(&mut self, bus: &mut Bus) -> bool {
        if self.opcode == 0xF3 {
            // DI: clear IME (note 02b "DI : efface IME"). No flag affected; the fetch is the
            // whole instruction.
            self.ime = false;
            self.done();
            return true;
        }

        // RETI ($D9): block 3, y == 3, z == 1. Four M-cycles total (note 02b).
        if x_is(self.opcode, 3) && y_is(self.opcode, 3) && z_is(self.opcode, 1) {
            match self.step {
                // Step 1 is the fetch M-cycle (decision C_00): no bus access yet.
                1 => return true,
                // Step 2: read the low byte of the return address from [SP]; SP not advanced yet.
                2 => {
                    self.lo = bus.read(self.sp);
                    return true;
                }
                // Step 3: read the high byte of the return address from [SP]+1.
                3 => {
                    self.hi = bus.read(self.sp.wrapping_add(1));
                    return true;
                }
                // Step 4: PC = popped value, SP += 2 (register writes), re-enable IME, then end.
                _ => {
                    let value = ((self.hi as u16) << 8) | self.lo as u16;
                    self.pc = value; // RETI: PC = (SP) (register write, no bus access)
                    self.sp = self.sp.wrapping_add(2);
                    self.ime = true; // RETI re-enables interrupts (note 02b)
                    self.done();
                    return true;
                }
            }
        }

        false
    }
}

// ---- Opcode bit-field helpers (decision C_00: x = op>>6, y = (op>>3)&7, z = op&7) ----

fn x_is(op: u8, v: u8) -> bool {
    op >> 6 == v
}
fn y_is(op: u8, v: u8) -> bool {
    (op >> 3) & 7 == v
}
fn z_is(op: u8, v: u8) -> bool {
    op & 7 == v
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

    // ---- RETI ($D9; note 02b "RETI : ret + IME=1, cout 4 M-cycles"; seed Opcodes.json: [16 T = 4 M]) ----

    #[test]
    fn c01_30_reti_costs_four_ticks_and_pops_return_address() {
        let mut bus = testutil::new_bus();
        // Seed the stack with a return address (low byte at [SP], high byte at [SP]+1).
        let sp = 0xC100u16;
        bus.write(sp, 0x56); // low byte of the return address ($7856) at [SP]
        bus.write(sp.wrapping_add(1), 0x78); // high byte at [SP]+1
        let mut cpu = Cpu {
            sp,
            f: 0x80, // Z set
            ..Cpu::default()
        };

        let ticks = testutil::exec(&mut cpu, &mut bus, &[0xD9]); // RETI

        assert_eq!(ticks, 4); // note 02b / seed Opcodes.json: 16 T = 4 M-cycles
        assert_eq!(cpu.pc, 0x7856); // PC set to the popped return address (like RET)
        assert_eq!(cpu.sp, 0xC102); // SP incremented by two (0xC100 + 2)
        assert!(cpu.ime); // IME re-enabled by reti (note 02b)
        assert_eq!(cpu.f, 0x80); // flags unchanged (no flag affected)
    }

    #[test]
    fn c01_30_reti_sets_ime_from_disabled() {
        let mut bus = testutil::new_bus();
        let sp = 0xC100u16;
        bus.write(sp, 0x56); // low byte of the return address ($7856) at [SP]
        bus.write(sp.wrapping_add(1), 0x78); // high byte at [SP]+1
        let mut cpu = Cpu {
            sp,
            ime: false, // start with interrupts disabled (as after a DI)
            ..Cpu::default()
        };

        testutil::exec(&mut cpu, &mut bus, &[0xD9]); // RETI

        assert!(cpu.ime); // IME set by reti even when it started clear (note 02b)
    }

    #[test]
    fn c01_30_reti_masks_f_low_nibble() {
        let mut bus = testutil::new_bus();
        let sp = 0xC100u16;
        bus.write(sp, 0x56); // low byte of the return address ($7856) at [SP]
        bus.write(sp.wrapping_add(1), 0x78); // high byte at [SP]+1
        let mut cpu = Cpu {
            sp,
            f: 0xFF, // all four flags set plus a dirty low nibble
            ..Cpu::default()
        };

        testutil::exec(&mut cpu, &mut bus, &[0xD9]); // RETI

        assert_eq!(cpu.f, 0xF0); // high nibble preserved, low nibble masked to zero
    }

    #[test]
    fn c01_30_reti_is_not_recorded_unimplemented() {
        let mut bus = testutil::new_bus();
        let sp = 0xC100u16;
        bus.write(sp, 0x56); // low byte of the return address ($7856) at [SP]
        bus.write(sp.wrapping_add(1), 0x78); // high byte at [SP]+1
        let mut cpu = Cpu {
            sp,
            ..Cpu::default()
        };

        testutil::exec(&mut cpu, &mut bus, &[0xD9]); // RETI

        assert_eq!(cpu.unimplemented(), None); // $D9 is claimed by the interrupt group
    }
}
