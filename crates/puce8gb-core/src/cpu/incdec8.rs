//! 8-bit INC/DEC (task C01_12): INC r and DEC r for r in B C D E H L A, plus INC (HL)
//! and DEC (HL). Decoded by opcode bit fields per decision C_00; no table.

use super::Cpu;
use crate::bus::Bus;

impl Cpu {
    /// 8-bit increments/decrements with flags (task C01_12). Returns true when the current
    /// opcode is in this group, false otherwise. Timings and flag effects from note 02c:
    /// - INC r / DEC r (block 0, z == 4 / z == 5): 4 T = 1 M-cycle; Z set iff the result is
    ///   zero, N cleared for INC / set for DEC, H on a low-nibble carry/borrow, C untouched.
    /// - INC (HL) ($34) / DEC (HL) ($35): 12 T = 3 M-cycles; step 2 reads (HL), step 3
    ///   writes it back (at most one bus access per decision C_00).
    pub(super) fn exec_incdec8(&mut self, bus: &mut Bus) -> bool {
        let op = self.opcode;

        // Block 0, z == 4 (INC r8) or z == 5 (DEC r8); y selects the register, y == 6 is (HL).
        if x_is(op, 0) && (z_is(op, 4) || z_is(op, 5)) {
            let dec = z_is(op, 5);

            // INC (HL) / DEC (HL): three M-cycles total.
            if y_is(op, 6) {
                match self.step {
                    // Step 1 is the fetch M-cycle (decision C_00): no bus access yet.
                    1 => return true,
                    // Step 2: read (HL) and latch it for step 3.
                    2 => {
                        self.lo = bus.read(self.hl());
                        return true;
                    }
                    _ => {
                        let old = self.lo;
                        let new = if dec {
                            old.wrapping_sub(1)
                        } else {
                            old.wrapping_add(1)
                        };
                        bus.write(self.hl(), new);
                        self.f = (self.f & 0x10) | incdec8_flags(old, new, dec);
                        self.done();
                        return true;
                    }
                }
            }

            // INC r / DEC r: the fetch M-cycle is the whole instruction (register work only).
            let old = self.read_r8(y_of(op));
            let new = if dec {
                old.wrapping_sub(1)
            } else {
                old.wrapping_add(1)
            };
            self.store_r8(y_of(op), new);
            self.f = (self.f & 0x10) | incdec8_flags(old, new, dec);
            self.done();
            return true;
        }

        false
    }
}

/// Flags after an 8-bit INC/DEC (note 02a): Z set iff the result is zero, N set for DEC and
/// cleared for INC, H on a low-nibble carry (INC) or borrow (DEC), C untouched. Pure so it
/// can be tested without a bus.
fn incdec8_flags(old: u8, new: u8, dec: bool) -> u8 {
    let mut f = 0u8;
    if new == 0x00 {
        f |= 0x80; // Z
    }
    if dec {
        f |= 0x40; // N set for the subtraction
    }
    if (dec && old & 0x0F == 0x00) || (!dec && old & 0x0F == 0x0F) {
        f |= 0x20; // H: half carry/borrow of the low four bits
    }
    f
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
fn y_of(op: u8) -> u8 {
    (op >> 3) & 7
}

#[cfg(test)]
mod tests {
    use super::incdec8_flags;
    use crate::cpu::{testutil, Cpu};

    // ---- Family 1: INC r (note 02c seed Opcodes.json: $04/$0C/$14/$1C/$24/$2C/$3C,
    // 4 T = 1 M-cycle, flags z0h-; pandocs historical L2184) ----

    #[test]
    fn c01_12_inc_r_b_costs_one_tick_and_keeps_c() {
        let mut bus = testutil::new_bus();
        let mut cpu = Cpu {
            b: 0x35,
            f: 0xFF, // all flags set plus a dirty low nibble
            ..Cpu::default()
        };

        let ticks = testutil::exec(&mut cpu, &mut bus, &[0x04]); // INC B

        assert_eq!(ticks, 1);
        assert_eq!(cpu.b, 0x36); // incremented
        assert_eq!(cpu.pc, 0xC001); // PC advanced past the opcode
        assert_eq!(cpu.f, 0x10); // only C survives; Z/N/H clear, low nibble masked
    }

    #[test]
    fn c01_12_inc_r_wrap_ff_to_zero_sets_z_and_h() {
        let mut bus = testutil::new_bus();
        let mut cpu = Cpu {
            a: 0xFF,
            ..Cpu::default()
        };

        let ticks = testutil::exec(&mut cpu, &mut bus, &[0x3C]); // INC A

        assert_eq!(ticks, 1);
        assert_eq!(cpu.a, 0x00); // wrapped around
        assert_eq!(cpu.f, 0xA0); // Z set (result zero) and H set (low nibble $0F overflowed)
    }

    #[test]
    fn c01_12_inc_r_half_carry_boundary_sets_h_only() {
        let mut bus = testutil::new_bus();
        let mut cpu = Cpu {
            h: 0x0F,
            f: 0x10, // C set, must survive
            ..Cpu::default()
        };

        let ticks = testutil::exec(&mut cpu, &mut bus, &[0x24]); // INC H

        assert_eq!(ticks, 1);
        assert_eq!(cpu.h, 0x10); // $0F + 1 crosses the low-nibble boundary
        assert_eq!(cpu.f, 0x30); // H set, Z and N clear, C preserved
    }

    #[test]
    fn c01_12_inc_r_no_half_carry_clears_h() {
        let mut bus = testutil::new_bus();
        let mut cpu = Cpu {
            d: 0x7E,
            f: 0x20, // H set before the instruction, must be cleared
            ..Cpu::default()
        };

        let ticks = testutil::exec(&mut cpu, &mut bus, &[0x14]); // INC D

        assert_eq!(ticks, 1);
        assert_eq!(cpu.d, 0x7F);
        assert_eq!(cpu.f, 0x00); // Z/N/H all clear (no low-nibble carry)
    }

    // ---- Family 2: DEC r (note 02c seed Opcodes.json: $05/$0D/$15/$1D/$25/$2D/$3D,
    // 4 T = 1 M-cycle, flags z1h-; pandocs historical L2186) ----

    #[test]
    fn c01_12_dec_r_a_costs_one_tick_and_sets_n() {
        let mut bus = testutil::new_bus();
        let mut cpu = Cpu {
            a: 0x5A,
            f: 0xC0, // Z and N set before the instruction; Z must be recomputed (C clear)
            ..Cpu::default()
        };

        let ticks = testutil::exec(&mut cpu, &mut bus, &[0x3D]); // DEC A

        assert_eq!(ticks, 1);
        assert_eq!(cpu.a, 0x59); // decremented
        assert_eq!(cpu.pc, 0xC001);
        assert_eq!(cpu.f, 0x40); // Z cleared (result non-zero), N set, H clear, C clear
    }

    #[test]
    fn c01_12_dec_r_wrap_zero_to_ff_sets_n_and_h() {
        let mut bus = testutil::new_bus();
        let mut cpu = Cpu {
            b: 0x00,
            f: 0x80, // Z set before the instruction; must be cleared
            ..Cpu::default()
        };

        let ticks = testutil::exec(&mut cpu, &mut bus, &[0x05]); // DEC B

        assert_eq!(ticks, 1);
        assert_eq!(cpu.b, 0xFF); // wrapped around
        assert_eq!(cpu.f, 0x60); // Z clear, N set, H set (low nibble $00 underflowed)
    }

    #[test]
    fn c01_12_dec_r_half_carry_boundary_sets_h() {
        let mut bus = testutil::new_bus();
        let mut cpu = Cpu {
            l: 0x10,
            f: 0x30, // H and C set before the instruction; C must survive
            ..Cpu::default()
        };

        let ticks = testutil::exec(&mut cpu, &mut bus, &[0x2D]); // DEC L

        assert_eq!(ticks, 1);
        assert_eq!(cpu.l, 0x0F); // $10 - 1 borrows from bit 4: the low nibble underflows
        assert_eq!(cpu.f, 0x70); // Z clear, N set, H set (bit-3 borrow), C preserved
    }

    #[test]
    fn c01_12_dec_l_half_carry_boundary_sets_h_with_c_only() {
        let mut bus = testutil::new_bus();
        let mut cpu = Cpu {
            l: 0x10,
            f: 0x10, // C set before the instruction; it must survive
            ..Cpu::default()
        };

        let ticks = testutil::exec(&mut cpu, &mut bus, &[0x2D]); // DEC L

        assert_eq!(ticks, 1);
        assert_eq!(cpu.l, 0x0F); // $10 - 1 borrows into bit 3 (low nibble was $00)
        assert_eq!(cpu.f, 0x70); // Z clear, N set, H set, C preserved
    }

    #[test]
    fn c01_12_dec_r_no_half_carry_clears_h() {
        let mut bus = testutil::new_bus();
        let mut cpu = Cpu {
            l: 0x2A,
            f: 0x30, // H and C set before the instruction; C must survive
            ..Cpu::default()
        };

        let ticks = testutil::exec(&mut cpu, &mut bus, &[0x2D]); // DEC L

        assert_eq!(ticks, 1);
        assert_eq!(cpu.l, 0x29); // low nibble $A -> $9: no borrow at bit 3
        assert_eq!(cpu.f, 0x50); // Z clear, N set, H cleared, C preserved
    }

    #[test]
    fn c01_12_dec_b_half_carry_boundary_sets_h() {
        let mut bus = testutil::new_bus();
        let mut cpu = Cpu {
            b: 0x10,
            f: 0x10, // C set before the instruction; it must survive
            ..Cpu::default()
        };

        let ticks = testutil::exec(&mut cpu, &mut bus, &[0x05]); // DEC B

        assert_eq!(ticks, 1);
        assert_eq!(cpu.b, 0x0F); // $10 - 1 borrows through bit 3 (low nibble was $0)
        assert_eq!(cpu.f, 0x70); // Z clear, N set, H set, C preserved
    }

    #[test]
    fn c01_12_dec_r_half_carry_set_when_low_nibble_zero() {
        let mut bus = testutil::new_bus();
        let mut cpu = Cpu {
            b: 0x20,
            f: 0x10, // C set before the instruction; it must survive
            ..Cpu::default()
        };

        let ticks = testutil::exec(&mut cpu, &mut bus, &[0x05]); // DEC B

        assert_eq!(ticks, 1);
        assert_eq!(cpu.b, 0x1F); // low nibble $00 underflows: borrow at bit 3
        assert_eq!(cpu.f, 0x70); // Z clear, N set, H set, C preserved
    }

    // ---- Family 3: INC (HL) ($34; note 02c spot-check L52: 12 T = 3 M-cycles, z0h-) ----

    #[test]
    fn c01_12_inc_hl_costs_three_ticks_and_keeps_c() {
        let mut bus = testutil::new_bus();
        bus.write(0xC100, 0x80);
        let mut cpu = Cpu {
            h: 0xC1,
            l: 0x00,
            f: 0x10, // C set, must survive
            ..Cpu::default()
        };

        let ticks = testutil::exec(&mut cpu, &mut bus, &[0x34]); // INC (HL)

        assert_eq!(ticks, 3);
        assert_eq!(bus.read(0xC100), 0x81); // memory incremented
        assert_eq!(cpu.pc, 0xC001); // PC advanced past the opcode only
        assert_eq!(cpu.f, 0x10); // Z/N/H clear, C preserved
    }

    #[test]
    fn c01_12_inc_hl_wrap_ff_to_zero_sets_z_and_h() {
        let mut bus = testutil::new_bus();
        bus.write(0xC100, 0xFF);
        let mut cpu = Cpu {
            h: 0xC1,
            l: 0x00,
            ..Cpu::default()
        };

        let ticks = testutil::exec(&mut cpu, &mut bus, &[0x34]); // INC (HL)

        assert_eq!(ticks, 3);
        assert_eq!(bus.read(0xC100), 0x00); // wrapped around in memory
        assert_eq!(cpu.f, 0xA0); // Z set and H set (low nibble $0F overflowed)
    }

    // ---- Family 4: DEC (HL) ($35; note 02c seed Opcodes.json: 12 T = 3 M-cycles, z1h-) ----

    #[test]
    fn c01_12_dec_hl_costs_three_ticks_and_sets_n() {
        let mut bus = testutil::new_bus();
        bus.write(0xC100, 0x81);
        let mut cpu = Cpu {
            h: 0xC1,
            l: 0x00,
            f: 0x80, // Z set before the instruction; must be cleared
            ..Cpu::default()
        };

        let ticks = testutil::exec(&mut cpu, &mut bus, &[0x35]); // DEC (HL)

        assert_eq!(ticks, 3);
        assert_eq!(bus.read(0xC100), 0x80); // memory decremented
        assert_eq!(cpu.pc, 0xC001);
        assert_eq!(cpu.f, 0x40); // Z cleared (result non-zero), N set, H clear (no bit-3 borrow)
    }

    #[test]
    fn c01_12_dec_hl_sets_z_when_result_is_zero() {
        let mut bus = testutil::new_bus();
        bus.write(0xC100, 0x01);
        let mut cpu = Cpu {
            h: 0xC1,
            l: 0x00,
            f: 0x20, // H set before the instruction; must be cleared
            ..Cpu::default()
        };

        let ticks = testutil::exec(&mut cpu, &mut bus, &[0x35]); // DEC (HL)

        assert_eq!(ticks, 3);
        assert_eq!(bus.read(0xC100), 0x00); // $01 - 1 reaches zero
        assert_eq!(cpu.f, 0xC0); // Z set, N set, H clear (no bit-3 borrow)
    }

    #[test]
    fn c01_12_dec_hl_wrap_zero_to_ff_sets_n_and_h() {
        let mut bus = testutil::new_bus();
        bus.write(0xC100, 0x00);
        let mut cpu = Cpu {
            h: 0xC1,
            l: 0x00,
            f: 0x80, // Z set before the instruction; must be cleared
            ..Cpu::default()
        };

        let ticks = testutil::exec(&mut cpu, &mut bus, &[0x35]); // DEC (HL)

        assert_eq!(ticks, 3);
        assert_eq!(bus.read(0xC100), 0xFF); // wrapped around in memory
        assert_eq!(cpu.f, 0x60); // Z clear, N set, H set (low nibble $00 underflowed)
    }

    // ---- Pure flag function: the four flag combinations at the boundaries ----

    #[test]
    fn c01_12_incdec8_flags_table_at_boundaries() {
        // INC: Z only on wrap, H only when the low nibble overflows.
        assert_eq!(incdec8_flags(0x35, 0x36, false), 0x00);
        assert_eq!(incdec8_flags(0xFF, 0x00, false), 0xA0); // Z and H
        assert_eq!(incdec8_flags(0x0F, 0x10, false), 0x20); // H only
        assert_eq!(incdec8_flags(0x7E, 0x7F, false), 0x00);
        // DEC: N always set, Z when old is $01 (result zero), H when the low nibble underflows.
        assert_eq!(incdec8_flags(0x5A, 0x59, true), 0x40); // N only
        assert_eq!(incdec8_flags(0x00, 0xFF, true), 0x60); // N and H
        assert_eq!(incdec8_flags(0x10, 0x0F, true), 0x60); // N and H: low nibble $00 underflows at bit 3
        assert_eq!(incdec8_flags(0x7E, 0x7D, true), 0x40);
    }
}
