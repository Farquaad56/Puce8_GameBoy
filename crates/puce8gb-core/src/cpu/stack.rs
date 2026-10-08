//! Stack transfers (task C01_09): PUSH rr and POP rr for rr in BC/DE/HL/AF. Decoded by opcode
//! bit fields per decision C_00; no table.

use super::Cpu;
use crate::bus::Bus;

impl Cpu {
    /// Stack transfers (task C01_09). Returns true when the current opcode is in this group,
    /// false otherwise. Timings and flag effects from note 02c (seed Opcodes.json):
    /// - PUSH rr ($C5/$D5/$E5/$F5; block 3, z == 5, y even so y>>1 selects BC/DE/HL/AF):
    ///   1 byte, 16 T = 4 M-cycles, no flags. SP is decremented by two on step 2 (a register
    ///   write, so no bus access); the high byte of rr is stored at (SP) on step 3 and the low
    ///   byte at (SP)+1 on step 4 (one bus write each). The high-byte-first order is deduced
    ///   from standard SM83/GB behavior; see open_questions.md. At most one bus access per
    ///   M-cycle (decision C_00).
    /// - POP rr ($C1/$D1/$E1/$F1; block 3, z == 1, y even so y>>1 selects BC/DE/HL/AF):
    ///   1 byte, 12 T = 3 M-cycles. The high byte is read from (SP) on step 2 and the low byte
    ///   from (SP)+1 on step 3 (one bus read each); rr is set and SP is advanced by two on
    ///   step 3. No flags are affected except that POP AF restores F verbatim from the stack;
    ///   the low nibble of F is then re-masked to zero at instruction end (decision C_00).
    pub(super) fn exec_stack(&mut self, bus: &mut Bus) -> bool {
        let op = self.opcode;

        // PUSH rr: block 3, z == 5, y even. Four M-cycles total.
        if x_is(op, 3) && z_is(op, 5) && (y_of(op) & 1) == 0 {
            match self.step {
                // Step 1 is the fetch M-cycle (decision C_00): no bus access yet.
                1 => return true,
                // Step 2: SP = SP - 2 (a register write, so no bus access).
                2 => {
                    self.sp = self.sp.wrapping_sub(2);
                    return true;
                }
                // Step 3: store the high byte of rr at (SP) (one bus access).
                3 => {
                    let value = self.pair(y_of(op));
                    bus.write(self.sp, (value >> 8) as u8);
                    return true;
                }
                // Step 4: store the low byte of rr at (SP)+1 (one bus access), then end.
                _ => {
                    let value = self.pair(y_of(op));
                    bus.write(self.sp.wrapping_add(1), value as u8);
                    self.done();
                    return true;
                }
            }
        }

        // POP rr: block 3, z == 1, y even. Three M-cycles total.
        if x_is(op, 3) && z_is(op, 1) && (y_of(op) & 1) == 0 {
            match self.step {
                // Step 1 is the fetch M-cycle (decision C_00): no bus access yet.
                1 => return true,
                // Step 2: read the high byte from (SP); SP is not advanced yet.
                2 => {
                    self.hi = bus.read(self.sp);
                    return true;
                }
                // Step 3: read the low byte from (SP)+1, set rr, advance SP by two, then end.
                _ => {
                    let value = ((self.hi as u16) << 8) | bus.read(self.sp.wrapping_add(1)) as u16;
                    self.store_pair(y_of(op), value);
                    self.sp = self.sp.wrapping_add(2);
                    self.done();
                    return true;
                }
            }
        }

        false
    }

    /// The 16-bit value of the register pair selected by bit-field index y>>1: BC/DE/HL/AF.
    fn pair(&self, y: u8) -> u16 {
        match y >> 1 {
            0 => ((self.b as u16) << 8) | self.c as u16, // BC
            1 => ((self.d as u16) << 8) | self.e as u16, // DE
            2 => ((self.h as u16) << 8) | self.l as u16, // HL
            _ => ((self.a as u16) << 8) | self.f as u16, // AF
        }
    }

    /// Store a 16-bit value into the register pair selected by bit-field index y>>1: BC/DE/HL/AF.
    /// For AF the low byte is stored verbatim; done() re-masks the F low nibble to zero.
    fn store_pair(&mut self, y: u8, value: u16) {
        match y >> 1 {
            0 => {
                self.b = (value >> 8) as u8;
                self.c = value as u8; // BC
            }
            1 => {
                self.d = (value >> 8) as u8;
                self.e = value as u8; // DE
            }
            2 => {
                self.h = (value >> 8) as u8;
                self.l = value as u8; // HL
            }
            _ => {
                self.a = (value >> 8) as u8;
                self.f = value as u8; // AF: restored verbatim, low nibble re-masked by done()
            }
        }
    }
}

// ---- Opcode bit-field helpers (decision C_00: x = op>>6, y = (op>>3)&7, z = op&7) ----

fn x_is(op: u8, v: u8) -> bool {
    op >> 6 == v
}
fn z_is(op: u8, v: u8) -> bool {
    op & 7 == v
}
fn y_of(op: u8) -> u8 {
    (op >> 3) & 7
}

#[cfg(test)]
mod tests {
    use crate::cpu::{testutil, Cpu};

    // A WRAM data cell kept clear of the $C000 code area that testutil::exec uses. The stack
    // pointer is set to DATA so PUSH writes to DATA-2/DATA-1 and POP reads from DATA/DATA+1.
    const DATA: u16 = 0xC100;

    // ---- Family 1: PUSH rr (note 02c seed Opcodes.json: [16 T = 4 M], no flags) ----

    #[test]
    fn c01_09_push_bc_costs_four_ticks() {
        let mut bus = testutil::new_bus();
        let mut cpu = Cpu {
            b: 0x12,
            c: 0x34,
            sp: DATA,
            f: 0x80, // Z set
            ..Cpu::default()
        };

        let ticks = testutil::exec(&mut cpu, &mut bus, &[0xC5]); // PUSH BC

        assert_eq!(ticks, 4); // note 02c: 16 T = 4 M-cycles
        assert_eq!(bus.peek(DATA.wrapping_sub(2)), 0x12); // high byte of BC stored at (SP)
        assert_eq!(bus.peek(DATA.wrapping_sub(1)), 0x34); // low byte of BC stored at (SP)+1
        assert_eq!(cpu.sp, DATA.wrapping_sub(2)); // SP decremented by two
        assert_eq!(cpu.pc, 0xC001); // PC advanced past the opcode
        assert_eq!(cpu.f, 0x80); // flags unchanged (Z preserved)
    }

    #[test]
    fn c01_09_push_de_stores_pair() {
        let mut bus = testutil::new_bus();
        let mut cpu = Cpu {
            d: 0xCD,
            e: 0xAB,
            sp: DATA,
            ..Cpu::default()
        };

        let ticks = testutil::exec(&mut cpu, &mut bus, &[0xD5]); // PUSH DE

        assert_eq!(ticks, 4); // note 02c: 16 T = 4 M-cycles
        assert_eq!(bus.peek(DATA.wrapping_sub(2)), 0xCD); // high byte of DE stored at (SP)
        assert_eq!(bus.peek(DATA.wrapping_sub(1)), 0xAB); // low byte of DE stored at (SP)+1
        assert_eq!(cpu.sp, DATA.wrapping_sub(2));
    }

    #[test]
    fn c01_09_push_hl_stores_pair() {
        let mut bus = testutil::new_bus();
        let mut cpu = Cpu {
            h: 0x87,
            l: 0x65,
            sp: DATA,
            ..Cpu::default()
        };

        let ticks = testutil::exec(&mut cpu, &mut bus, &[0xE5]); // PUSH HL

        assert_eq!(ticks, 4); // note 02c: 16 T = 4 M-cycles
        assert_eq!(bus.peek(DATA.wrapping_sub(2)), 0x87); // high byte of HL stored at (SP)
        assert_eq!(bus.peek(DATA.wrapping_sub(1)), 0x65); // low byte of HL stored at (SP)+1
        assert_eq!(cpu.sp, DATA.wrapping_sub(2));
    }

    #[test]
    fn c01_09_push_af_stores_a_high_f_low() {
        let mut bus = testutil::new_bus();
        let mut cpu = Cpu {
            a: 0x34,
            f: 0x12, // clean low nibble (C set)
            sp: DATA,
            ..Cpu::default()
        };

        let ticks = testutil::exec(&mut cpu, &mut bus, &[0xF5]); // PUSH AF

        assert_eq!(ticks, 4); // note 02c: 16 T = 4 M-cycles
        assert_eq!(bus.peek(DATA.wrapping_sub(2)), 0x34); // A (high byte) stored at (SP)
        assert_eq!(bus.peek(DATA.wrapping_sub(1)), 0x12); // F (low byte, verbatim) stored at (SP)+1
        assert_eq!(cpu.sp, DATA.wrapping_sub(2));
        assert_eq!(cpu.a, 0x34); // A unchanged
        assert_eq!(cpu.f, 0x10); // no flag affected; F low nibble re-masked to zero at instruction end (C_00)
    }

    #[test]
    fn c01_09_push_masks_f_low_nibble() {
        // Flag boundary: a dirty low nibble of F is re-masked at instruction end; the high
        // nibble (the four real flags) is preserved because no flag is affected.
        let mut bus = testutil::new_bus();
        let mut cpu = Cpu {
            b: 0x12,
            c: 0x34,
            sp: DATA,
            f: 0xFF, // all four flags set plus a dirty low nibble
            ..Cpu::default()
        };

        let ticks = testutil::exec(&mut cpu, &mut bus, &[0xC5]); // PUSH BC

        assert_eq!(ticks, 4);
        assert_eq!(cpu.f, 0xF0); // high nibble preserved, low nibble masked to zero
    }

    #[test]
    fn c01_09_push_sp_wraps_at_bottom_of_memory() {
        // Edge case: SP = $0001; PUSH decrements it past $0000 and wraps to $FFFF. The high
        // byte is stored at (SP) = $FFFF (the IE cell); the low-byte write to $0000 lands in
        // ROM, which is read-only (decision A_06), so it is dropped by the bus.
        let mut bus = testutil::new_bus();
        let mut cpu = Cpu {
            b: 0x12,
            c: 0x34,
            sp: 0x0001,
            ..Cpu::default()
        };

        let ticks = testutil::exec(&mut cpu, &mut bus, &[0xC5]); // PUSH BC

        assert_eq!(ticks, 4); // note 02c: 16 T = 4 M-cycles
        assert_eq!(cpu.sp, 0xFFFF); // SP wrapped past $0000
        assert_eq!(bus.peek(0xFFFF), 0x12); // high byte of BC stored at (SP) = $FFFF
        assert_eq!(bus.peek(0x0000), 0x00); // low-byte write to $0000 dropped (ROM read-only)
    }

    // ---- Family 2: POP rr (note 02c seed Opcodes.json: [12 T = 3 M], no flags except AF) ----

    #[test]
    fn c01_09_pop_bc_costs_three_ticks() {
        let mut bus = testutil::new_bus();
        bus.write(DATA, 0x12); // high byte at (SP)
        bus.write(DATA.wrapping_add(1), 0x34); // low byte at (SP)+1
        let mut cpu = Cpu {
            b: 0x00,
            c: 0x00,
            sp: DATA,
            f: 0x80, // Z set
            ..Cpu::default()
        };

        let ticks = testutil::exec(&mut cpu, &mut bus, &[0xC1]); // POP BC

        assert_eq!(ticks, 3); // note 02c: 12 T = 3 M-cycles
        assert_eq!((cpu.b as u16) << 8 | cpu.c as u16, 0x1234); // BC now holds the stack value
        assert_eq!(cpu.sp, DATA.wrapping_add(2)); // SP advanced by two
        assert_eq!(cpu.pc, 0xC001); // PC advanced past the opcode
        assert_eq!(cpu.f, 0x80); // flags unchanged (Z preserved)
    }

    #[test]
    fn c01_09_pop_de_loads_pair() {
        let mut bus = testutil::new_bus();
        bus.write(DATA, 0xCD); // high byte at (SP)
        bus.write(DATA.wrapping_add(1), 0xAB); // low byte at (SP)+1
        let mut cpu = Cpu {
            d: 0x00,
            e: 0x00,
            sp: DATA,
            ..Cpu::default()
        };

        let ticks = testutil::exec(&mut cpu, &mut bus, &[0xD1]); // POP DE

        assert_eq!(ticks, 3); // note 02c: 12 T = 3 M-cycles
        assert_eq!((cpu.d as u16) << 8 | cpu.e as u16, 0xCDAB); // DE now holds the stack value
        assert_eq!(cpu.sp, DATA.wrapping_add(2));
    }

    #[test]
    fn c01_09_pop_hl_loads_pair() {
        let mut bus = testutil::new_bus();
        bus.write(DATA, 0x87); // high byte at (SP)
        bus.write(DATA.wrapping_add(1), 0x65); // low byte at (SP)+1
        let mut cpu = Cpu {
            h: 0x00,
            l: 0x00,
            sp: DATA,
            ..Cpu::default()
        };

        let ticks = testutil::exec(&mut cpu, &mut bus, &[0xE1]); // POP HL

        assert_eq!(ticks, 3); // note 02c: 12 T = 3 M-cycles
        assert_eq!((cpu.h as u16) << 8 | cpu.l as u16, 0x8765); // HL now holds the stack value
        assert_eq!(cpu.sp, DATA.wrapping_add(2));
    }

    #[test]
    fn c01_09_pop_af_restores_flags() {
        let mut bus = testutil::new_bus();
        bus.write(DATA, 0x34); // A (high byte) at (SP)
        bus.write(DATA.wrapping_add(1), 0x50); // F (low byte) at (SP)+1: N and C set, clean nibble
        let mut cpu = Cpu {
            a: 0x00,
            f: 0x80, // Z set before the POP
            sp: DATA,
            ..Cpu::default()
        };

        let ticks = testutil::exec(&mut cpu, &mut bus, &[0xF1]); // POP AF

        assert_eq!(ticks, 3); // note 02c: 12 T = 3 M-cycles
        assert_eq!(cpu.a, 0x34); // A restored from the stack high byte
        assert_eq!(cpu.f, 0x50); // F restored verbatim from the stack low byte (N and C set)
        assert_eq!(cpu.sp, DATA.wrapping_add(2));
    }

    #[test]
    fn c01_09_pop_af_masks_f_low_nibble() {
        // Flag boundary: POP AF restores F verbatim from the stack; a dirty low nibble in the
        // stored value is re-masked to zero at instruction end. The high nibble survives.
        let mut bus = testutil::new_bus();
        bus.write(DATA, 0x34); // A (high byte) at (SP)
        bus.write(DATA.wrapping_add(1), 0xFF); // F (low byte) with all bits set
        let mut cpu = Cpu {
            a: 0x00,
            f: 0x80,
            sp: DATA,
            ..Cpu::default()
        };

        let ticks = testutil::exec(&mut cpu, &mut bus, &[0xF1]); // POP AF

        assert_eq!(ticks, 3);
        assert_eq!(cpu.a, 0x34);
        assert_eq!(cpu.f, 0xF0); // restored verbatim then masked: high nibble kept, low nibble zero
    }
}
