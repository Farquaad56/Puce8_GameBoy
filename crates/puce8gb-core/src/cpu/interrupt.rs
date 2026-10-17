//! Interrupt control opcodes (task C01_16): DI ($F3). Task C01_30 adds RETI ($D9); task
//! C01_32 adds EI ($FB) with its one-instruction delay. Decoded by opcode bit fields per
//! decision C_00; no table.

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
            // DI (task C01_32): IME cleared immediately and any pending EI cancelled. No flag
            // affected; the fetch is the whole instruction (decision C_00), so no bus access.
            self.ime = false;
            self.ei_delay = 0;
            self.done();
            return true;
        }

        if self.opcode == 0xFB {
            // EI (task C01_32): IME is set only after the NEXT instruction completes, so a
            // one-instruction delay is armed here and resolved in done() (note 02b "EI : delai
            // d'une instruction"): done() decrements ei_delay at the end of each following
            // instruction and sets IME when it reaches 0. No flag affected; the fetch is the
            // whole instruction (decision C_00), so this handler calls done() itself.
            // Only arm the delay when none is pending: an EI executed while a previous EI is
            // still pending does not postpone IME (mooneye ei_sequence, FIX_03).
            if self.ei_delay == 0 {
                self.ei_delay = 2;
            }
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

    // ---- Task C01_32: EI delay, DI cancellation, dispatch, priority, RETI immediacy ----

    /// Address where test code is placed (work RAM; the ROM area $0000-$7FFF ignores writes).
    const CODE: u16 = 0xC000;

    /// Place `code` at CODE (work RAM) and return a CPU with PC there.
    fn cpu_at_code(bus: &mut crate::bus::Bus, code: &[u8]) -> Cpu {
        for (i, &b) in code.iter().enumerate() {
            bus.write(CODE.wrapping_add(i as u16), b);
        }
        Cpu {
            pc: CODE,
            ..Cpu::default()
        }
    }

    #[test]
    fn c01_32_ei_delay_takes_effect_after_next_instruction_then_dispatches() {
        let mut bus = testutil::new_bus();
        let mut cpu = cpu_at_code(&mut bus, &[0xFB, 0x00, 0x00]); // EI, NOP, NOP at CODE
        cpu.sp = 0xDFFE;
        bus.write(0xFFFF, 0x01); // IE: vblank enabled
        bus.io[0x0F] = 0x01; // IF bit 0 (vblank) requested

        cpu.tick(&mut bus); // EI fetch: IME must still be 0 (one-instruction delay)
        assert!(!cpu.ime);

        cpu.tick(&mut bus); // first NOP completes: the EI effect lands now
        assert!(cpu.ime);

        for _ in 0..5 {
            cpu.tick(&mut bus); // five M-cycles of interrupt entry (note 02b)
        }

        assert_eq!(cpu.pc, 0x0040); // vector of IF bit 0 (vblank)
        assert_eq!(cpu.sp, 0xDFFC); // SP = SP - 2
        assert_eq!(bus.read(0xDFFD), 0xC0); // pushed PC high byte at [SP-1]: the NOP not fetched is CODE+2
        assert_eq!(bus.read(0xDFFC), 0x02); // pushed PC low byte at [SP-2] = $C002
        assert_eq!(bus.read(0xFF0F), 0xE0); // IF bit 0 acknowledged (bits 5-7 read as 1)
        assert!(!cpu.ime); // IME cleared by the service of the interrupt
    }

    #[test]
    fn c01_32_di_cancels_pending_ei_and_blocks_dispatch() {
        let mut bus = testutil::new_bus();
        let mut cpu = cpu_at_code(&mut bus, &[0xFB, 0xF3, 0x00]); // EI, DI, NOP at CODE
        bus.write(0xFFFF, 0x01); // IE: vblank enabled
        bus.io[0x0F] = 0x01; // IF bit 0 (vblank) requested

        for _ in 0..3 {
            cpu.tick(&mut bus); // EI + DI + NOP: three M-cycles, no dispatch
        }

        assert_eq!(cpu.pc, CODE.wrapping_add(3)); // PC advanced past all three instructions
        assert!(!cpu.ime); // the pending EI was cancelled by the DI
    }

    #[test]
    fn c01_32_dispatch_takes_five_m_cycles_and_pushes_next_pc() {
        let mut bus = testutil::new_bus();
        let mut cpu = cpu_at_code(&mut bus, &[0x00]); // NOP at CODE
        cpu.sp = 0xDFFE;
        cpu.ime = true;
        bus.write(0xFFFF, 0x01); // IE: vblank enabled
        bus.io[0x0F] = 0x01; // IF bit 0 (vblank) requested

        for _ in 0..5 {
            cpu.tick(&mut bus); // the whole entry sequence is five M-cycles
        }

        assert_eq!(cpu.pc, 0x0040); // vector of IF bit 0
        assert_eq!(cpu.sp, 0xDFFC);
        assert_eq!(bus.read(0xDFFD), 0xC0); // high byte: dispatch happened before any fetch, so PC = CODE
        assert_eq!(bus.read(0xDFFC), 0x00); // low byte of the address not fetched ($C000)
        assert!(!cpu.ime);
    }

    #[test]
    fn c01_32_priority_lowest_bit_first() {
        let mut bus = testutil::new_bus();
        let mut cpu = cpu_at_code(&mut bus, &[0x00]); // NOP at CODE
        cpu.ime = true;
        bus.write(0xFFFF, 0x1F); // IE: all five sources enabled
        bus.io[0x0F] = 0x05; // IF: vblank (bit 0) + timer (bit 2) requested

        for _ in 0..5 {
            cpu.tick(&mut bus);
        }

        assert_eq!(cpu.pc, 0x0040); // bit 0 (vblank) wins over bit 2 (timer)
        assert_eq!(bus.io[0x0F], 0x04); // only the served bit is acknowledged: IF becomes $04
    }

    #[test]
    fn c01_32_reti_sets_ime_immediately_and_dispatches_right_after() {
        let mut bus = testutil::new_bus();
        let mut cpu = cpu_at_code(&mut bus, &[0xD9]); // RETI at CODE
        cpu.sp = 0xDFFC;
        bus.write(0xDFFC, 0x34); // low byte of the return address ($1234) at [SP]
        bus.write(0xDFFD, 0x12); // high byte at [SP]+1
        cpu.ime = false;
        bus.write(0xFFFF, 0x01); // IE: vblank enabled
        bus.io[0x0F] = 0x01; // IF bit 0 (vblank) requested

        for _ in 0..4 {
            cpu.tick(&mut bus); // RETI is four M-cycles (note 02b)
        }

        assert_eq!(cpu.pc, 0x1234); // PC popped from the stack like RET
        assert!(cpu.ime); // IME set immediately by RETI, no delay

        for _ in 0..5 {
            cpu.tick(&mut bus); // dispatch starts right after: five M-cycles
        }

        assert_eq!(cpu.pc, 0x0040); // vector of IF bit 0
        assert_eq!(bus.read(0xDFFD), 0x12); // pushed PC high byte at [SP-1] = $DFFD (PC was $1234)
        assert_eq!(bus.read(0xDFFC), 0x34); // pushed PC low byte at [SP-2] = $DFFC
    }

    #[test]
    fn c01_32_no_dispatch_when_ime_clear() {
        let mut bus = testutil::new_bus();
        let mut cpu = cpu_at_code(&mut bus, &[0x00]); // NOP at CODE
        cpu.ime = false;
        bus.write(0xFFFF, 0x01); // IE: vblank enabled
        bus.io[0x0F] = 0x01; // IF bit 0 (vblank) requested

        cpu.tick(&mut bus); // the NOP executes normally; no dispatch happens

        assert_eq!(cpu.pc, CODE.wrapping_add(1)); // PC advanced past the NOP
    }

    #[test]
    fn c01_32_if_register_bits5_to_7_read_as_one() {
        let mut bus = testutil::new_bus();
        bus.write(0xFF0F, 0x01); // IF: only bit 0 writable
        assert_eq!(bus.read(0xFF0F), 0xE1); // bits 5-7 read as 1 (task C01_32)

        bus.write(0xFFFF, 0xFF); // IE is a plain eight-bit register
        assert_eq!(bus.read(0xFFFF), 0xFF);
    }

    // ---- Task C01_36: blargg cpu_instrs 02 interrupts - per-source vector routing ----

    /// The ROM exercises every interrupt source; verify each IF bit routes to its
    /// documented vector (note 02b) in five M-cycles, clears IME and acknowledges the bit.
    #[test]
    fn c01_36_interrupt_vectors_route_to_correct_address() {
        let cases = [
            (0x01, 0x40), // vblank
            (0x02, 0x48), // stat
            (0x04, 0x50), // timer overflow
            (0x08, 0x58), // serial transfer end
            (0x10, 0x60), // joypad
        ];
        for (bit, vector) in cases {
            let mut bus = testutil::new_bus();
            let mut cpu = cpu_at_code(&mut bus, &[0x00]); // NOP at CODE
            cpu.sp = 0xDFFE;
            cpu.ime = true;
            bus.write(0xFFFF, bit); // IE: only this source enabled
            bus.io[0x0F] = bit; // IF: only this source requested

            for _ in 0..5 {
                cpu.tick(&mut bus); // five M-cycles of interrupt entry (note 02b)
            }

            assert_eq!(
                cpu.pc, vector,
                "IF bit {bit:#04x} must route to ${vector:04X}"
            );
            assert!(!cpu.ime); // IME cleared by the service of the interrupt
            assert_eq!(bus.io[0x0F] & 0x1F, 0); // the served IF bit is acknowledged
        }
    }

    // ---- FIX_03: an EI executed while a previous EI is still pending does not postpone IME (mooneye ei_sequence) ----

    #[test]
    fn fix_03_two_consecutive_ei_enable_ime_after_the_second() {
        let mut bus = testutil::new_bus();
        let mut cpu = Cpu::default();

        testutil::exec(&mut cpu, &mut bus, &[0xFB]); // EI at $C000: IME not set yet (one-instruction delay)
        assert!(!cpu.ime);

        testutil::exec(&mut cpu, &mut bus, &[0xFB]); // second EI while the first is still pending
        assert!(cpu.ime); // IME set as soon as the second EI completes; no extra postponement (FIX_03)
    }

    #[test]
    fn fix_03_single_ei_still_delays_by_one_instruction() {
        let mut bus = testutil::new_bus();
        let mut cpu = Cpu::default();

        testutil::exec(&mut cpu, &mut bus, &[0xFB]); // EI at $C000
        assert!(!cpu.ime); // IME not set until the next instruction completes (note 02b)

        testutil::exec(&mut cpu, &mut bus, &[0x00]); // NOP: the delayed effect lands here
        assert!(cpu.ime);
    }

    #[test]
    fn fix_03_ei_chain_dispatches_after_the_second_ei() {
        let mut bus = testutil::new_bus();
        bus.write(0xFFFF, 0x08); // IE: serial transfer end enabled
        bus.write(0xFF0F, 0x08); // IF: request pending (bit $08)
        let mut cpu = Cpu {
            sp: 0xDFFE,
            ..Cpu::default()
        };

        testutil::exec(&mut cpu, &mut bus, &[0xFB, 0xFB]); // EI at $C000 and $C001: the first one runs here
        assert!(!cpu.ime); // still pending after the first EI
        cpu.tick(&mut bus); // second EI at $C001 completes: IME set immediately (FIX_03)
        assert!(cpu.ime);

        for _ in 0..5 {
            cpu.tick(&mut bus); // five M-cycles of interrupt entry (note 02b)
        }

        assert_eq!(cpu.pc, 0x0058); // serial transfer end vector
        assert_eq!(bus.read(0xDFFC), 0x02); // pushed PC low byte ($C002 at sp - 2)
        assert_eq!(bus.read(0xDFFD), 0xC0); // pushed PC high byte (at sp - 1)
    }
}
