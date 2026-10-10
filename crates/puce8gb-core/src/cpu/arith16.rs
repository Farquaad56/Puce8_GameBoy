//! 16-bit register-pair arithmetic (tasks C01_16/C01_17): INC rr and DEC rr for
//! rr in BC DE HL SP, plus ADD HL,rr. Decoded by opcode bit fields per decision C_00; no table.

use super::Cpu;
use crate::bus::Bus;

impl Cpu {
    /// 16-bit register-pair arithmetic (tasks C01_16/C01_17). Returns true when the current
    /// opcode is in this group, false otherwise. Timings and flag effects from note 02a / seed
    /// Opcodes.json:
    /// - INC rr ($03/$13/$23/$33) / DEC rr ($0B/$1B/$2B/$3B): block 0, z == 3; the pair is
    ///   selected by (op >> 4) & 3 and bit 5 picks INC vs DEC. 8 T = 2 M-cycles, no flags
    ///   affected. The fetch is step 1 (no bus access); the register write happens on step 2
    ///   (a register copy, so no bus access). At most one bus access per M-cycle (decision C_00).
    /// - ADD HL,rr ($09/$19/$29/$39): block 0, z == 1, y odd; the pair is selected by
    ///   y>>1 (BC/DE/HL/SP). 8 T = 2 M-cycles; flags "-0hc" (note 02a: Z untouched, N cleared, H from bit 11 of
    ///   the sum, C from bit 15). The fetch is step 1 (no bus access); the register write
    ///   happens on step 2 (a register copy, so no bus access).
    pub(super) fn exec_arith16(&mut self, _bus: &mut Bus) -> bool {
        let op = self.opcode;

        // INC rr / DEC rr: block 0, z == 3. Two M-cycles total.
        if x_is(op, 0) && z_is(op, 3) {
            match self.step {
                // Step 1 is the fetch M-cycle (decision C_00): no bus access yet.
                1 => return true,
                _ => {
                    let dec = op & 8 != 0;
                    let pair = (op >> 4) & 3;
                    let value = match pair {
                        0 => ((self.b as u16) << 8) | self.c as u16, // BC
                        1 => ((self.d as u16) << 8) | self.e as u16, // DE
                        2 => self.hl(),                              // HL
                        _ => self.sp,                                // SP
                    };
                    let next = if dec {
                        value.wrapping_sub(1)
                    } else {
                        value.wrapping_add(1)
                    };
                    match pair {
                        0 => {
                            self.b = (next >> 8) as u8;
                            self.c = next as u8; // BC
                        }
                        1 => {
                            self.d = (next >> 8) as u8;
                            self.e = next as u8; // DE
                        }
                        2 => {
                            self.h = (next >> 8) as u8;
                            self.l = next as u8; // HL
                        }
                        _ => self.sp = next, // SP
                    }
                    self.done();
                    return true;
                }
            }
        }

        // ADD HL,rr: block 0, z == 1, y odd (y>>1 selects BC/DE/HL/SP). Two M-cycles total.
        if x_is(op, 0) && z_is(op, 1) && (y_of(op) & 1) != 0 {
            match self.step {
                // Step 1 is the fetch M-cycle (decision C_00): no bus access yet.
                1 => return true,
                _ => {
                    let pair = y_of(op) >> 1;
                    let operand = match pair {
                        0 => ((self.b as u16) << 8) | self.c as u16, // BC
                        1 => ((self.d as u16) << 8) | self.e as u16, // DE
                        2 => self.hl(), // HL (read before the write below)
                        _ => self.sp,   // SP
                    };
                    let sum = (self.hl() as u32) + operand as u32; // at most 0x1FFFF
                    let mut f = self.f & 0x80; // Z untouched (note 02a: "-0hc")
                    if (sum >> 11) & 1 != 0 {
                        f |= 0x20; // H from bit 11 of the sum
                    }
                    if sum > 0xFFFF {
                        f |= 0x10; // C from bit 15 of the sum (carry out)
                    }
                    let result = sum as u16; // wraps mod $10000
                    self.h = (result >> 8) as u8;
                    self.l = result as u8;
                    self.f = f;
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
fn z_is(op: u8, v: u8) -> bool {
    op & 7 == v
}
fn y_of(op: u8) -> u8 {
    (op >> 3) & 7
}

#[cfg(test)]
mod tests {
    use crate::cpu::{testutil, Cpu};

    // ---- Family 1: INC rr (note 02a / seed Opcodes.json: [8 T = 2 M], no flags) ----

    #[test]
    fn c01_16_inc_hl_costs_two_ticks() {
        let mut bus = testutil::new_bus();
        let mut cpu = Cpu {
            h: 0x00,
            l: 0xFF, // HL = $00FF
            f: 0x80, // Z set
            ..Cpu::default()
        };

        let ticks = testutil::exec(&mut cpu, &mut bus, &[0x23]); // INC HL

        assert_eq!(ticks, 2); // note 02a / seed Opcodes.json: 8 T = 2 M-cycles
        assert_eq!((cpu.h as u16) << 8 | cpu.l as u16, 0x0100); // HL incremented (wraps low byte)
        assert_eq!(cpu.pc, 0xC001); // PC advanced past the opcode
        assert_eq!(cpu.f, 0x80); // flags unchanged (no flag affected)
    }

    #[test]
    fn c01_16_inc_bc_loads_pair() {
        let mut bus = testutil::new_bus();
        let mut cpu = Cpu {
            b: 0x12,
            c: 0x34, // BC = $1234
            ..Cpu::default()
        };

        let ticks = testutil::exec(&mut cpu, &mut bus, &[0x03]); // INC BC

        assert_eq!(ticks, 2);
        assert_eq!((cpu.b as u16) << 8 | cpu.c as u16, 0x1235); // BC incremented
    }

    #[test]
    fn c01_16_inc_de_loads_pair() {
        let mut bus = testutil::new_bus();
        let mut cpu = Cpu {
            d: 0xCD,
            e: 0xAB, // DE = $CDAB
            ..Cpu::default()
        };

        let ticks = testutil::exec(&mut cpu, &mut bus, &[0x13]); // INC DE

        assert_eq!(ticks, 2);
        assert_eq!((cpu.d as u16) << 8 | cpu.e as u16, 0xCDAC); // DE incremented (low byte $AB -> $AC)
    }

    #[test]
    fn c01_16_inc_sp_loads_pair() {
        let mut bus = testutil::new_bus();
        let mut cpu = Cpu {
            sp: 0xFFFE,
            ..Cpu::default()
        };

        let ticks = testutil::exec(&mut cpu, &mut bus, &[0x33]); // INC SP

        assert_eq!(ticks, 2);
        assert_eq!(cpu.sp, 0xFFFF); // SP incremented
    }

    #[test]
    fn c01_16_inc_hl_wraps_at_top_of_memory() {
        let mut bus = testutil::new_bus();
        let mut cpu = Cpu {
            h: 0xFF,
            l: 0xFF, // HL = $FFFF
            ..Cpu::default()
        };

        let ticks = testutil::exec(&mut cpu, &mut bus, &[0x23]); // INC HL

        assert_eq!(ticks, 2);
        assert_eq!((cpu.h as u16) << 8 | cpu.l as u16, 0x0000); // HL wraps to $0000
    }

    #[test]
    fn c01_16_inc_rr_masks_f_low_nibble() {
        let mut bus = testutil::new_bus();
        let mut cpu = Cpu {
            h: 0x00,
            l: 0x00,
            f: 0xFF, // all four flags set plus a dirty low nibble
            ..Cpu::default()
        };

        testutil::exec(&mut cpu, &mut bus, &[0x23]); // INC HL

        assert_eq!(cpu.f, 0xF0); // high nibble preserved, low nibble masked to zero
    }

    // ---- Family 2: DEC rr (note 02a / seed Opcodes.json: [8 T = 2 M], no flags) ----

    #[test]
    fn c01_16_dec_hl_costs_two_ticks() {
        let mut bus = testutil::new_bus();
        let mut cpu = Cpu {
            h: 0x00,
            l: 0x00, // HL = $0000
            f: 0x10, // C set
            ..Cpu::default()
        };

        let ticks = testutil::exec(&mut cpu, &mut bus, &[0x2B]); // DEC HL

        assert_eq!(ticks, 2); // note 02a / seed Opcodes.json: 8 T = 2 M-cycles
        assert_eq!((cpu.h as u16) << 8 | cpu.l as u16, 0xFFFF); // HL decrements (wraps down)
        assert_eq!(cpu.pc, 0xC001);
        assert_eq!(cpu.f, 0x10); // flags unchanged (no flag affected)
    }

    #[test]
    fn c01_16_dec_bc_loads_pair() {
        let mut bus = testutil::new_bus();
        let mut cpu = Cpu {
            b: 0x12,
            c: 0x34, // BC = $1234
            ..Cpu::default()
        };

        let ticks = testutil::exec(&mut cpu, &mut bus, &[0x0B]); // DEC BC

        assert_eq!(ticks, 2);
        assert_eq!((cpu.b as u16) << 8 | cpu.c as u16, 0x1233); // BC decremented
    }

    #[test]
    fn c01_16_dec_de_loads_pair() {
        let mut bus = testutil::new_bus();
        let mut cpu = Cpu {
            d: 0xCD,
            e: 0xAB, // DE = $CDAB
            ..Cpu::default()
        };

        let ticks = testutil::exec(&mut cpu, &mut bus, &[0x1B]); // DEC DE

        assert_eq!(ticks, 2);
        assert_eq!((cpu.d as u16) << 8 | cpu.e as u16, 0xCDAA); // DE decremented (low byte $AB -> $AA)
    }

    #[test]
    fn c01_16_dec_sp_loads_pair() {
        let mut bus = testutil::new_bus();
        let mut cpu = Cpu {
            sp: 0x0000,
            ..Cpu::default()
        };

        let ticks = testutil::exec(&mut cpu, &mut bus, &[0x3B]); // DEC SP

        assert_eq!(ticks, 2);
        assert_eq!(cpu.sp, 0xFFFF); // SP wraps down to $FFFF
    }

    #[test]
    fn c01_16_dec_rr_is_not_recorded_unimplemented() {
        let mut bus = testutil::new_bus();
        let mut cpu = Cpu { ..Cpu::default() };

        testutil::exec(&mut cpu, &mut bus, &[0x2B]); // DEC HL

        assert_eq!(cpu.unimplemented(), None); // $2B is claimed by the arith16 group
    }

    // ---- Family 3: ADD HL,rr (note 02a / seed Opcodes.json: [8 T = 2 M], flags "-0hc":
    // Z untouched, N cleared, H from bit 11 of the sum, C from bit 15) ----

    #[test]
    fn c01_17_add_hl_bc_costs_two_ticks() {
        let mut bus = testutil::new_bus();
        let mut cpu = Cpu {
            b: 0x12,
            c: 0x34, // BC = $1234
            h: 0x56,
            l: 0x78, // HL = $5678
            f: 0x40, // N set before the instruction; must be cleared
            ..Cpu::default()
        };

        let ticks = testutil::exec(&mut cpu, &mut bus, &[0x09]); // ADD HL,BC

        assert_eq!(ticks, 2); // note 02a / seed Opcodes.json: 8 T = 2 M-cycles
        assert_eq!((cpu.h as u16) << 8 | cpu.l as u16, 0x68AC); // HL += BC
        assert_eq!(cpu.pc, 0xC001); // PC advanced past the opcode
        assert_eq!(cpu.f & 0x40, 0); // N cleared (note 02a: "-0hc")
    }

    #[test]
    fn c01_17_add_hl_de_loads_pair() {
        let mut bus = testutil::new_bus();
        let mut cpu = Cpu {
            d: 0xCD,
            e: 0xAB, // DE = $CDAB
            h: 0x00,
            l: 0x01, // HL = $0001
            ..Cpu::default()
        };

        let ticks = testutil::exec(&mut cpu, &mut bus, &[0x19]); // ADD HL,DE

        assert_eq!(ticks, 2);
        assert_eq!((cpu.h as u16) << 8 | cpu.l as u16, 0xCDAC); // HL += DE ($CDAB + $0001)
    }

    #[test]
    fn c01_17_add_hl_sp_loads_pair() {
        let mut bus = testutil::new_bus();
        let mut cpu = Cpu {
            sp: 0xFFFE,
            h: 0x00,
            l: 0x02, // HL = $0002
            ..Cpu::default()
        };

        let ticks = testutil::exec(&mut cpu, &mut bus, &[0x39]); // ADD HL,SP

        assert_eq!(ticks, 2);
        assert_eq!((cpu.h as u16) << 8 | cpu.l as u16, 0x0000); // $FFFE + $0002 wraps to $0000
        assert_eq!(cpu.f & 0x10, 0x10); // C set: carry out of bit 15 (note 02a)
    }

    #[test]
    fn c01_17_add_hl_hl_doubles_and_wraps() {
        let mut bus = testutil::new_bus();
        let mut cpu = Cpu {
            h: 0x80,
            l: 0x00, // HL = $8000
            ..Cpu::default()
        };

        let ticks = testutil::exec(&mut cpu, &mut bus, &[0x29]); // ADD HL,HL

        assert_eq!(ticks, 2);
        assert_eq!((cpu.h as u16) << 8 | cpu.l as u16, 0x0000); // $8000 + $8000 wraps to $0000
    }

    #[test]
    fn c01_17_add_hl_sets_carry_on_wrap() {
        let mut bus = testutil::new_bus();
        let mut cpu = Cpu {
            b: 0x00,
            c: 0x01, // BC = $0001
            h: 0xFF,
            l: 0xFF, // HL = $FFFF
            f: 0x80, // Z set; must stay untouched
            ..Cpu::default()
        };

        testutil::exec(&mut cpu, &mut bus, &[0x09]); // ADD HL,BC

        assert_eq!((cpu.h as u16) << 8 | cpu.l as u16, 0x0000); // wraps to $0000
        assert_eq!(cpu.f & 0x10, 0x10); // C set: bit 15 of the sum ($10000) is 1 (note 02a)
        assert_eq!(cpu.f & 0x80, 0x80); // Z untouched
    }

    #[test]
    fn c01_17_add_hl_sets_half_carry_at_bit_11() {
        let mut bus = testutil::new_bus();
        let mut cpu = Cpu {
            b: 0x00,
            c: 0x01, // BC = $0001
            h: 0x07,
            l: 0xFF, // HL = $07FF
            ..Cpu::default()
        };

        testutil::exec(&mut cpu, &mut bus, &[0x09]); // ADD HL,BC

        assert_eq!((cpu.h as u16) << 8 | cpu.l as u16, 0x0800); // $07FF + $0001
        assert_eq!(cpu.f & 0x20, 0x20); // H set: bit 11 of the sum is 1 (note 02a)
        assert_eq!(cpu.f & 0x10, 0); // C clear: no carry out
    }

    #[test]
    fn c01_17_add_hl_carry_boundary_keeps_c_clear() {
        let mut bus = testutil::new_bus();
        let mut cpu = Cpu {
            b: 0x00,
            c: 0x01, // BC = $0001
            h: 0xFF,
            l: 0xFE, // HL = $FFFE
            ..Cpu::default()
        };

        testutil::exec(&mut cpu, &mut bus, &[0x09]); // ADD HL,BC

        assert_eq!((cpu.h as u16) << 8 | cpu.l as u16, 0xFFFF); // just below the wrap
        assert_eq!(cpu.f & 0x20, 0x20); // H set: bit 11 of $FFFF is 1
        assert_eq!(cpu.f & 0x10, 0); // C clear: no carry out yet
    }

    #[test]
    fn c01_17_add_hl_keeps_z_and_clears_n() {
        let mut bus = testutil::new_bus();
        let mut cpu = Cpu {
            b: 0x00,
            c: 0x02, // BC = $0002
            h: 0x00,
            l: 0x01, // HL = $0001
            f: 0xF0, // all four flags set before the instruction
            ..Cpu::default()
        };

        testutil::exec(&mut cpu, &mut bus, &[0x09]); // ADD HL,BC

        assert_eq!((cpu.h as u16) << 8 | cpu.l as u16, 0x0003);
        assert_eq!(cpu.f, 0x80); // Z kept (untouched), N/H/C cleared (note 02a: "-0hc")
    }

    #[test]
    fn c01_17_add_hl_z_clear_stays_clear() {
        let mut bus = testutil::new_bus();
        let mut cpu = Cpu {
            b: 0x00,
            c: 0x02, // BC = $0002
            h: 0x00,
            l: 0x05, // HL = $0005
            f: 0x30, // H and C set, Z clear; must stay clear
            ..Cpu::default()
        };

        testutil::exec(&mut cpu, &mut bus, &[0x09]); // ADD HL,BC

        assert_eq!((cpu.h as u16) << 8 | cpu.l as u16, 0x0007);
        assert_eq!(cpu.f, 0x00); // Z untouched (clear), N/H/C cleared
    }

    #[test]
    fn c01_17_add_hl_bc_is_not_recorded_unimplemented() {
        let mut bus = testutil::new_bus();
        let mut cpu = Cpu { ..Cpu::default() };

        testutil::exec(&mut cpu, &mut bus, &[0x09]); // ADD HL,BC

        assert_eq!(cpu.unimplemented(), None); // $09 is claimed by the arith16 group
    }
}
