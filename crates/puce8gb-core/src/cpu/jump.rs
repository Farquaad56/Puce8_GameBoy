//! Jumps (task C01_10): JP a16, JP HL, JP cc,a16 and JR e8 / JR cc,e8. Decoded by opcode
//! bit fields per decision C_00; no table.

use super::Cpu;
use crate::bus::Bus;

impl Cpu {
    /// Jumps (task C01_10). Returns true when the current opcode is in this group, false
    /// otherwise. Timings and flag effects from note 02c (seed Opcodes.json):
    /// - JP a16 ($C3; block 3, y == 0, z == 3): 3 bytes, 16 T = 4 M-cycles, no flags. The
    ///   low byte of a16 is read on step 2 and the high byte on step 3 (little-endian order);
    ///   PC is set to a16 on step 4 (a register write, so no bus access).
    /// - JP HL ($E9; block 3, y == 5, z == 1): 1 byte, 4 T = 1 M-cycle, no flags. PC is set
    ///   to HL on the fetch step itself (register reads only, no bus access).
    /// - JP cc,a16 ($C2/$CA/$D2/$DA; block 3, z == 2, bit5 clear so y < 4): 3 bytes. Taken:
    ///   16 T = 4 M-cycles; not taken: 12 T = 3 M-cycles. No flags. The low byte of a16 is
    ///   read on step 2 and the high byte on step 3 regardless of the condition; PC is set to
    ///   a16 only when the condition (selected by y) is met, on step 4.
    /// - JR e8 ($18; block 0, y == 3, z == 0): 2 bytes, 12 T = 3 M-cycles, no flags. The
    ///   signed immediate e8 is read on step 2; PC = PC + sign_extend(e8) on step 3 (register
    ///   arithmetic only, wrapping).
    /// - JR cc,e8 ($20/$28/$30/$38; block 0, z == 0, bit5 set so y >= 4): 2 bytes. Taken:
    ///   12 T = 3 M-cycles; not taken: 8 T = 2 M-cycles. No flags. The signed immediate e8 is
    ///   read on step 2 regardless of the condition; PC = PC + sign_extend(e8) only when the
    ///   condition (selected by y - 4) is met, on step 3.
    pub(super) fn exec_jump(&mut self, bus: &mut Bus) -> bool {
        let op = self.opcode;

        // JP a16 ($C3): block 3, y == 0, z == 3. Four M-cycles total.
        if x_is(op, 3) && y_is(op, 0) && z_is(op, 3) {
            match self.step {
                // Step 1 is the fetch M-cycle (decision C_00): no bus access yet.
                1 => return true,
                // Step 2: read the low byte of a16 at PC; latch it for step 4.
                2 => {
                    self.lo = bus.read(self.pc);
                    self.pc = self.pc.wrapping_add(1);
                    return true;
                }
                // Step 3: read the high byte of a16 (little-endian order, note 02c).
                3 => {
                    self.hi = bus.read(self.pc);
                    self.pc = self.pc.wrapping_add(1);
                    return true;
                }
                // Step 4: PC = a16 (a register write, so no bus access), then end.
                _ => {
                    let addr = ((self.hi as u16) << 8) | self.lo as u16;
                    self.pc = addr; // JP a16: PC = a16 (register write, no bus access)
                    self.done();
                    return true;
                }
            }
        }

        // JP HL ($E9): block 3, y == 5, z == 1. One M-cycle total (the fetch).
        if x_is(op, 3) && y_is(op, 5) && z_is(op, 1) {
            let addr = ((self.h as u16) << 8) | self.l as u16;
            self.pc = addr; // JP HL: PC = HL (register reads only, no bus access)
            self.done();
            return true;
        }

        // JP cc,a16 ($C2/$CA/$D2/$DA): block 3, z == 2, bit5 clear (y < 4). Taken: four
        // M-cycles; not taken: three. The condition index is y itself (0=NZ, 1=Z, 2=NC, 3=C).
        if x_is(op, 3) && z_is(op, 2) && !bit5_set(op) {
            match self.step {
                // Step 1 is the fetch M-cycle (decision C_00): no bus access yet.
                1 => return true,
                // Step 2: read the low byte of a16 at PC; latch it for step 4.
                2 => {
                    self.lo = bus.read(self.pc);
                    self.pc = self.pc.wrapping_add(1);
                    return true;
                }
                // Step 3: read the high byte of a16 (little-endian order, note 02c), whether
                // or not the condition is met. When the condition is not met this is the last
                // M-cycle (not-taken = 12 T = 3 M).
                3 => {
                    self.hi = bus.read(self.pc);
                    self.pc = self.pc.wrapping_add(1);
                    if !self.condition_met(y_of(op)) {
                        self.done(); // not taken: end after three M-cycles
                        return true;
                    }
                    return true;
                }
                // Step 4 (taken only): PC = a16 (a register write, so no bus access), then end.
                _ => {
                    let addr = ((self.hi as u16) << 8) | self.lo as u16;
                    self.pc = addr; // taken: PC = a16 (register write, no bus access)
                    self.done();
                    return true;
                }
            }
        }

        // JR e8 ($18): block 0, y == 3, z == 0. Three M-cycles total.
        if x_is(op, 0) && y_is(op, 3) && z_is(op, 0) {
            match self.step {
                // Step 1 is the fetch M-cycle (decision C_00): no bus access yet.
                1 => return true,
                // Step 2: read the signed immediate e8 at PC; latch it for step 3.
                2 => {
                    self.lo = bus.read(self.pc);
                    self.pc = self.pc.wrapping_add(1);
                    return true;
                }
                // Step 3: PC += sign_extend(e8) (register arithmetic only, wrapping), then end.
                _ => {
                    self.pc = self.pc.wrapping_add(sign_extend(self.lo));
                    self.done();
                    return true;
                }
            }
        }

        // JR cc,e8 ($20/$28/$30/$38): block 0, z == 0, bit5 set (y >= 4). Taken: three
        // M-cycles; not taken: two. The condition index is y - 4 (0=NZ, 1=Z, 2=NC, 3=C).
        if x_is(op, 0) && z_is(op, 0) && bit5_set(op) {
            match self.step {
                // Step 1 is the fetch M-cycle (decision C_00): no bus access yet.
                1 => return true,
                // Step 2: read the signed immediate e8 at PC; latch it for step 3. When the
                // condition is not met this is the last M-cycle (not-taken = 8 T = 2 M).
                2 => {
                    self.lo = bus.read(self.pc);
                    self.pc = self.pc.wrapping_add(1);
                    if !self.condition_met(y_of(op) - 4) {
                        self.done(); // not taken: end after two M-cycles
                        return true;
                    }
                    return true;
                }
                // Step 3 (taken only): PC += sign_extend(e8) (register arithmetic, wrapping).
                _ => {
                    self.pc = self.pc.wrapping_add(sign_extend(self.lo));
                    self.done();
                    return true;
                }
            }
        }

        false
    }

    /// Evaluate the jump condition selected by index 0=NZ, 1=Z, 2=NC, 3=C (note 02a: Z is set
    /// iff a result was zero; C is set on carry/borrow).
    fn condition_met(&self, idx: u8) -> bool {
        match idx {
            0 => self.f & 0x80 == 0, // NZ: Z flag clear
            1 => self.f & 0x80 != 0, // Z: Z flag set
            2 => self.f & 0x10 == 0, // NC: C flag clear
            _ => self.f & 0x10 != 0, // C: C flag set
        }
    }
}

/// Sign-extend a signed 8-bit immediate to 16 bits for wrapping addition to PC.
fn sign_extend(e8: u8) -> u16 {
    if e8 & 0x80 != 0 {
        (e8 as u16) | 0xFF00 // negative value: fill the high byte with ones
    } else {
        e8 as u16
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
fn bit5_set(op: u8) -> bool {
    (op >> 5) & 1 != 0
}
fn y_of(op: u8) -> u8 {
    (op >> 3) & 7
}

#[cfg(test)]
mod tests {
    use crate::cpu::{testutil, Cpu};

    // ---- Family 1: JP a16 ($C3; note 02c seed Opcodes.json: [16 T = 4 M], no flags) ----

    #[test]
    fn c01_10_jp_a16_costs_four_ticks_and_sets_pc() {
        let mut bus = testutil::new_bus();
        let mut cpu = Cpu {
            f: 0x80, // Z set
            ..Cpu::default()
        };

        let ticks = testutil::exec(&mut cpu, &mut bus, &[0xC3, 0x56, 0x78]); // JP $7856

        assert_eq!(ticks, 4); // note 02c: 16 T = 4 M-cycles
        assert_eq!(cpu.pc, 0x7856); // PC set to the absolute target
        assert_eq!(cpu.f, 0x80); // flags unchanged (Z preserved)
    }

    #[test]
    fn c01_10_jp_a16_to_zero() {
        let mut bus = testutil::new_bus();
        let mut cpu = Cpu { ..Cpu::default() };

        let ticks = testutil::exec(&mut cpu, &mut bus, &[0xC3, 0x00, 0x00]); // JP $0000

        assert_eq!(ticks, 4);
        assert_eq!(cpu.pc, 0x0000); // PC set to the bottom of memory
    }

    #[test]
    fn c01_10_jp_a16_to_top_of_memory() {
        let mut bus = testutil::new_bus();
        let mut cpu = Cpu { ..Cpu::default() };

        let ticks = testutil::exec(&mut cpu, &mut bus, &[0xC3, 0xFF, 0xFF]); // JP $FFFF

        assert_eq!(ticks, 4);
        assert_eq!(cpu.pc, 0xFFFF); // PC set to the top of memory
    }

    #[test]
    fn c01_10_jp_a16_masks_f_low_nibble() {
        // Flag boundary: a dirty low nibble of F is re-masked at instruction end; no flag is
        // affected by JP.
        let mut bus = testutil::new_bus();
        let mut cpu = Cpu {
            f: 0xFF, // all four flags set plus a dirty low nibble
            ..Cpu::default()
        };

        let ticks = testutil::exec(&mut cpu, &mut bus, &[0xC3, 0x12, 0x34]); // JP $3412 (little-endian)

        assert_eq!(ticks, 4);
        assert_eq!(cpu.pc, 0x3412); // low byte first: $12 then $34 -> $3412
        assert_eq!(cpu.f, 0xF0); // high nibble preserved, low nibble masked to zero
    }

    // ---- Family 2: JP HL ($E9; note 02c seed Opcodes.json: [4 T = 1 M], no flags) ----

    #[test]
    fn c01_10_jp_hl_costs_one_tick_and_sets_pc() {
        let mut bus = testutil::new_bus();
        let mut cpu = Cpu {
            h: 0xAB,
            l: 0xCD,
            f: 0x30, // H and C set
            ..Cpu::default()
        };

        let ticks = testutil::exec(&mut cpu, &mut bus, &[0xE9]); // JP HL

        assert_eq!(ticks, 1); // note 02c: 4 T = 1 M-cycle (the fetch is the whole instruction)
        assert_eq!(cpu.pc, 0xABCD); // PC set to the value of HL
        assert_eq!(cpu.f, 0x30); // flags unchanged
    }

    #[test]
    fn c01_10_jp_hl_to_zero() {
        let mut bus = testutil::new_bus();
        let mut cpu = Cpu {
            h: 0x00,
            l: 0x00,
            ..Cpu::default()
        };

        let ticks = testutil::exec(&mut cpu, &mut bus, &[0xE9]); // JP HL with HL = $0000

        assert_eq!(ticks, 1);
        assert_eq!(cpu.pc, 0x0000);
    }

    #[test]
    fn c01_10_jp_hl_to_top_of_memory() {
        let mut bus = testutil::new_bus();
        let mut cpu = Cpu {
            h: 0xFF,
            l: 0xFF,
            ..Cpu::default()
        };

        let ticks = testutil::exec(&mut cpu, &mut bus, &[0xE9]); // JP HL with HL = $FFFF

        assert_eq!(ticks, 1);
        assert_eq!(cpu.pc, 0xFFFF);
    }

    // ---- Family 3: JP cc,a16 (note 02c seed Opcodes.json: taken [16 T = 4 M], not-taken [12 T = 3 M]) ----

    #[test]
    fn c01_10_jp_nz_a16_taken_costs_four_ticks() {
        // $C2 JP NZ,a16 with Z clear: the jump is taken.
        let mut bus = testutil::new_bus();
        let mut cpu = Cpu {
            f: 0x00, // Z clear (NZ true)
            ..Cpu::default()
        };

        let ticks = testutil::exec(&mut cpu, &mut bus, &[0xC2, 0x56, 0x78]); // JP NZ,$7856

        assert_eq!(ticks, 4); // note 02c: taken = 16 T = 4 M-cycles
        assert_eq!(cpu.pc, 0x7856); // PC set to the target
    }

    #[test]
    fn c01_10_jp_nz_a16_not_taken_costs_three_ticks() {
        // $C2 JP NZ,a16 with Z set: the jump is not taken.
        let mut bus = testutil::new_bus();
        let mut cpu = Cpu {
            f: 0x80, // Z set (NZ false)
            ..Cpu::default()
        };

        let ticks = testutil::exec(&mut cpu, &mut bus, &[0xC2, 0x56, 0x78]); // JP NZ,$7856

        assert_eq!(ticks, 3); // note 02c: not-taken = 12 T = 3 M-cycles
        assert_eq!(cpu.pc, 0xC003); // PC advanced past the three bytes (opcode + a16)
    }

    #[test]
    fn c01_10_jp_z_a16_taken_costs_four_ticks() {
        // $CA JP Z,a16 with Z set: the jump is taken.
        let mut bus = testutil::new_bus();
        let mut cpu = Cpu {
            f: 0x80, // Z set (Z true)
            ..Cpu::default()
        };

        let ticks = testutil::exec(&mut cpu, &mut bus, &[0xCA, 0xAB, 0xCD]); // JP Z,$CDAB

        assert_eq!(ticks, 4);
        assert_eq!(cpu.pc, 0xCDAB);
    }

    #[test]
    fn c01_10_jp_z_a16_not_taken_costs_three_ticks() {
        // $CA JP Z,a16 with Z clear: the jump is not taken.
        let mut bus = testutil::new_bus();
        let mut cpu = Cpu {
            f: 0x00, // Z clear (Z false)
            ..Cpu::default()
        };

        let ticks = testutil::exec(&mut cpu, &mut bus, &[0xCA, 0xAB, 0xCD]); // JP Z,$CDAB

        assert_eq!(ticks, 3);
        assert_eq!(cpu.pc, 0xC003);
    }

    #[test]
    fn c01_10_jp_nc_a16_taken_costs_four_ticks() {
        // $D2 JP NC,a16 with C clear: the jump is taken.
        let mut bus = testutil::new_bus();
        let mut cpu = Cpu {
            f: 0x80, // Z set but C clear (NC true)
            ..Cpu::default()
        };

        let ticks = testutil::exec(&mut cpu, &mut bus, &[0xD2, 0x12, 0x34]); // JP NC,$3412

        assert_eq!(ticks, 4);
        assert_eq!(cpu.pc, 0x3412);
    }

    #[test]
    fn c01_10_jp_nc_a16_not_taken_costs_three_ticks() {
        // $D2 JP NC,a16 with C set: the jump is not taken.
        let mut bus = testutil::new_bus();
        let mut cpu = Cpu {
            f: 0x90, // Z and C set (NC false)
            ..Cpu::default()
        };

        let ticks = testutil::exec(&mut cpu, &mut bus, &[0xD2, 0x12, 0x34]); // JP NC,$3412

        assert_eq!(ticks, 3);
        assert_eq!(cpu.pc, 0xC003);
    }

    #[test]
    fn c01_10_jp_c_a16_taken_costs_four_ticks() {
        // $DA JP C,a16 with C set: the jump is taken.
        let mut bus = testutil::new_bus();
        let mut cpu = Cpu {
            f: 0x10, // C set (C true)
            ..Cpu::default()
        };

        let ticks = testutil::exec(&mut cpu, &mut bus, &[0xDA, 0xFF, 0x00]); // JP C,$00FF

        assert_eq!(ticks, 4);
        assert_eq!(cpu.pc, 0x00FF);
    }

    #[test]
    fn c01_10_jp_c_a16_not_taken_costs_three_ticks() {
        // $DA JP C,a16 with C clear: the jump is not taken.
        let mut bus = testutil::new_bus();
        let mut cpu = Cpu {
            f: 0x80, // Z set but C clear (C false)
            ..Cpu::default()
        };

        let ticks = testutil::exec(&mut cpu, &mut bus, &[0xDA, 0xFF, 0x00]); // JP C,$00FF

        assert_eq!(ticks, 3);
        assert_eq!(cpu.pc, 0xC003);
    }

    #[test]
    fn c01_10_jp_cc_a16_not_taken_preserves_flags() {
        let mut bus = testutil::new_bus();
        let mut cpu = Cpu {
            f: 0x80, // Z set
            ..Cpu::default()
        };

        let ticks = testutil::exec(&mut cpu, &mut bus, &[0xC2, 0x56, 0x78]); // JP NZ,$7856 (not taken)

        assert_eq!(ticks, 3);
        assert_eq!(cpu.f, 0x80); // flags unchanged
    }

    #[test]
    fn c01_10_jp_cc_a16_taken_masks_f_low_nibble() {
        // Flag boundary: a dirty low nibble of F is re-masked at instruction end.
        let mut bus = testutil::new_bus();
        let mut cpu = Cpu {
            f: 0xFF, // all flags set plus dirty low nibble; Z set so the condition is met
            ..Cpu::default()
        };

        let ticks = testutil::exec(&mut cpu, &mut bus, &[0xCA, 0x12, 0x34]); // JP Z,$3412 (taken)

        assert_eq!(ticks, 4);
        assert_eq!(cpu.pc, 0x3412);
        assert_eq!(cpu.f, 0xF0); // high nibble preserved, low nibble masked to zero
    }

    // ---- Family 4: JR e8 ($18; note 02c seed Opcodes.json: [12 T = 3 M], no flags) ----

    #[test]
    fn c01_10_jr_e8_positive_offset_costs_three_ticks() {
        let mut bus = testutil::new_bus();
        let mut cpu = Cpu {
            f: 0x80, // Z set
            ..Cpu::default()
        };

        let ticks = testutil::exec(&mut cpu, &mut bus, &[0x18, 0x0A]); // JR +$0A

        assert_eq!(ticks, 3); // note 02c: 12 T = 3 M-cycles
                              // PC after the two-byte instruction is $C002; adding +$0A gives $C00C.
        assert_eq!(cpu.pc, 0xC00C);
        assert_eq!(cpu.f, 0x80); // flags unchanged
    }

    #[test]
    fn c01_10_jr_e8_negative_offset_costs_three_ticks() {
        let mut bus = testutil::new_bus();
        let mut cpu = Cpu { ..Cpu::default() };

        // e8 = $F6 is -10 as a signed byte. PC after the instruction is $C002; adding -10 gives $BFF8.
        let ticks = testutil::exec(&mut cpu, &mut bus, &[0x18, 0xF6]); // JR -$0A

        assert_eq!(ticks, 3);
        assert_eq!(cpu.pc, 0xBFF8);
    }

    #[test]
    fn c01_10_jr_e8_zero_offset() {
        let mut bus = testutil::new_bus();
        let mut cpu = Cpu { ..Cpu::default() };

        let ticks = testutil::exec(&mut cpu, &mut bus, &[0x18, 0x00]); // JR +$00 (stay in place)

        assert_eq!(ticks, 3);
        assert_eq!(cpu.pc, 0xC002); // PC is just past the two-byte instruction
    }

    #[test]
    fn c01_10_jr_e8_max_positive_offset() {
        let mut bus = testutil::new_bus();
        let mut cpu = Cpu { ..Cpu::default() };

        // e8 = $7F is +127. PC after the instruction is $C002; adding 127 gives $C081.
        let ticks = testutil::exec(&mut cpu, &mut bus, &[0x18, 0x7F]); // JR +$7F

        assert_eq!(ticks, 3);
        assert_eq!(cpu.pc, 0xC081);
    }

    #[test]
    fn c01_10_jr_e8_min_negative_offset() {
        let mut bus = testutil::new_bus();
        let mut cpu = Cpu { ..Cpu::default() };

        // e8 = $80 is -128. PC after the instruction is $C002; subtracting 128 gives $BF82.
        let ticks = testutil::exec(&mut cpu, &mut bus, &[0x18, 0x80]); // JR -$80

        assert_eq!(ticks, 3);
        assert_eq!(cpu.pc, 0xBF82);
    }

    #[test]
    fn c01_10_jr_e8_wraps_past_top_of_memory() {
        // Edge case: the target wraps past $FFFF. Place the instruction at $FFFE so that
        // PC after the two bytes is $0000, and e8 = $7F gives $007F.
        let mut bus = testutil::new_bus();
        bus.write(0xFFFE, 0x18); // JR e8 at $FFFE
        bus.write(0xFFFF, 0x7F); // e8 = +$7F at $FFFF (wraps)
        let mut cpu = Cpu {
            pc: 0xFFFE,
            ..Cpu::default()
        };

        cpu.tick(&mut bus); // fetch opcode (step 1)
        assert!(!cpu.at_boundary());
        cpu.tick(&mut bus); // read e8 at $FFFF, PC wraps to $0000 (step 2)
        assert!(!cpu.at_boundary());
        cpu.tick(&mut bus); // PC = $0000 + $7F = $007F (step 3), done
        assert!(cpu.at_boundary());

        assert_eq!(cpu.pc, 0x007F);
    }

    #[test]
    fn c01_10_jr_e8_wraps_past_bottom_of_memory() {
        // Edge case: the target wraps below $0000. Place the instruction at $0002 so that
        // PC after the two bytes is $0004, and e8 = $F8 (-8) gives $FFFF... wait: $0004 - 8 = $FFFC.
        let mut bus = testutil::new_bus();
        // $0002/$0003 are ROM (read-only via bus.write), so set the image bytes directly.
        bus.rom[2] = 0x18; // JR e8 at $0002
        bus.rom[3] = 0xF8; // e8 = -$08 at $0003
        let mut cpu = Cpu {
            pc: 0x0002,
            ..Cpu::default()
        };

        cpu.tick(&mut bus); // fetch opcode (step 1)
        assert!(!cpu.at_boundary());
        cpu.tick(&mut bus); // read e8 at $0003, PC = $0004 (step 2)
        assert!(!cpu.at_boundary());
        cpu.tick(&mut bus); // PC = $0004 - 8 = $FFFC (step 3), done
        assert!(cpu.at_boundary());

        assert_eq!(cpu.pc, 0xFFFC);
    }

    #[test]
    fn c01_10_jr_e8_masks_f_low_nibble() {
        // Flag boundary: a dirty low nibble of F is re-masked at instruction end.
        let mut bus = testutil::new_bus();
        let mut cpu = Cpu {
            f: 0xFF,
            ..Cpu::default()
        };

        let ticks = testutil::exec(&mut cpu, &mut bus, &[0x18, 0x02]); // JR +$02

        assert_eq!(ticks, 3);
        assert_eq!(cpu.f, 0xF0); // high nibble preserved, low nibble masked to zero
    }

    // ---- Family 5: JR cc,e8 (note 02c seed Opcodes.json: taken [12 T = 3 M], not-taken [8 T = 2 M]) ----

    #[test]
    fn c01_10_jr_nz_e8_taken_costs_three_ticks() {
        // $20 JR NZ,e8 with Z clear: the jump is taken.
        let mut bus = testutil::new_bus();
        let mut cpu = Cpu {
            f: 0x00, // Z clear (NZ true)
            ..Cpu::default()
        };

        let ticks = testutil::exec(&mut cpu, &mut bus, &[0x20, 0x0A]); // JR NZ,+$0A

        assert_eq!(ticks, 3); // note 02c: taken = 12 T = 3 M-cycles
        assert_eq!(cpu.pc, 0xC00C); // PC after instruction ($C002) + $0A
    }

    #[test]
    fn c01_10_jr_nz_e8_not_taken_costs_two_ticks() {
        // $20 JR NZ,e8 with Z set: the jump is not taken.
        let mut bus = testutil::new_bus();
        let mut cpu = Cpu {
            f: 0x80, // Z set (NZ false)
            ..Cpu::default()
        };

        let ticks = testutil::exec(&mut cpu, &mut bus, &[0x20, 0x0A]); // JR NZ,+$0A

        assert_eq!(ticks, 2); // note 02c: not-taken = 8 T = 2 M-cycles
        assert_eq!(cpu.pc, 0xC002); // PC just past the two-byte instruction (no offset applied)
    }

    #[test]
    fn c01_10_jr_z_e8_taken_costs_three_ticks() {
        // $28 JR Z,e8 with Z set: the jump is taken.
        let mut bus = testutil::new_bus();
        let mut cpu = Cpu {
            f: 0x80, // Z set (Z true)
            ..Cpu::default()
        };

        let ticks = testutil::exec(&mut cpu, &mut bus, &[0x28, 0xF6]); // JR Z,-$0A

        assert_eq!(ticks, 3);
        assert_eq!(cpu.pc, 0xBFF8); // PC after instruction ($C002) - $0A
    }

    #[test]
    fn c01_10_jr_z_e8_not_taken_costs_two_ticks() {
        // $28 JR Z,e8 with Z clear: the jump is not taken.
        let mut bus = testutil::new_bus();
        let mut cpu = Cpu {
            f: 0x00, // Z clear (Z false)
            ..Cpu::default()
        };

        let ticks = testutil::exec(&mut cpu, &mut bus, &[0x28, 0xF6]); // JR Z,-$0A

        assert_eq!(ticks, 2);
        assert_eq!(cpu.pc, 0xC002);
    }

    #[test]
    fn c01_10_jr_nc_e8_taken_costs_three_ticks() {
        // $30 JR NC,e8 with C clear: the jump is taken.
        let mut bus = testutil::new_bus();
        let mut cpu = Cpu {
            f: 0x80, // Z set but C clear (NC true)
            ..Cpu::default()
        };

        let ticks = testutil::exec(&mut cpu, &mut bus, &[0x30, 0x05]); // JR NC,+$05

        assert_eq!(ticks, 3);
        assert_eq!(cpu.pc, 0xC007);
    }

    #[test]
    fn c01_10_jr_nc_e8_not_taken_costs_two_ticks() {
        // $30 JR NC,e8 with C set: the jump is not taken.
        let mut bus = testutil::new_bus();
        let mut cpu = Cpu {
            f: 0x90, // Z and C set (NC false)
            ..Cpu::default()
        };

        let ticks = testutil::exec(&mut cpu, &mut bus, &[0x30, 0x05]); // JR NC,+$05

        assert_eq!(ticks, 2);
        assert_eq!(cpu.pc, 0xC002);
    }

    #[test]
    fn c01_10_jr_c_e8_taken_costs_three_ticks() {
        // $38 JR C,e8 with C set: the jump is taken.
        let mut bus = testutil::new_bus();
        let mut cpu = Cpu {
            f: 0x10, // C set (C true)
            ..Cpu::default()
        };

        let ticks = testutil::exec(&mut cpu, &mut bus, &[0x38, 0x80]); // JR C,-$80

        assert_eq!(ticks, 3);
        assert_eq!(cpu.pc, 0xBF82); // PC after instruction ($C002) - $80
    }

    #[test]
    fn c01_10_jr_c_e8_not_taken_costs_two_ticks() {
        // $38 JR C,e8 with C clear: the jump is not taken.
        let mut bus = testutil::new_bus();
        let mut cpu = Cpu {
            f: 0x80, // Z set but C clear (C false)
            ..Cpu::default()
        };

        let ticks = testutil::exec(&mut cpu, &mut bus, &[0x38, 0x80]); // JR C,-$80

        assert_eq!(ticks, 2);
        assert_eq!(cpu.pc, 0xC002);
    }

    #[test]
    fn c01_10_jr_cc_e8_not_taken_preserves_flags() {
        let mut bus = testutil::new_bus();
        let mut cpu = Cpu {
            f: 0x80, // Z set
            ..Cpu::default()
        };

        let ticks = testutil::exec(&mut cpu, &mut bus, &[0x20, 0x0A]); // JR NZ,+$0A (not taken)

        assert_eq!(ticks, 2);
        assert_eq!(cpu.f, 0x80); // flags unchanged
    }

    #[test]
    fn c01_10_jr_cc_e8_taken_masks_f_low_nibble() {
        // Flag boundary: a dirty low nibble of F is re-masked at instruction end.
        let mut bus = testutil::new_bus();
        let mut cpu = Cpu {
            f: 0xFF, // all flags set plus dirty low nibble; Z set so the condition is met
            ..Cpu::default()
        };

        let ticks = testutil::exec(&mut cpu, &mut bus, &[0x28, 0x02]); // JR Z,+$02 (taken)

        assert_eq!(ticks, 3);
        assert_eq!(cpu.pc, 0xC004);
        assert_eq!(cpu.f, 0xF0); // high nibble preserved, low nibble masked to zero
    }

    #[test]
    fn c01_10_jr_cc_e8_not_taken_masks_f_low_nibble() {
        // Flag boundary: even when not taken, the instruction end re-masks F.
        let mut bus = testutil::new_bus();
        let mut cpu = Cpu {
            f: 0xFF, // all flags set plus dirty low nibble; Z set so NZ is false
            ..Cpu::default()
        };

        let ticks = testutil::exec(&mut cpu, &mut bus, &[0x20, 0x02]); // JR NZ,+$02 (not taken)

        assert_eq!(ticks, 2);
        assert_eq!(cpu.pc, 0xC002);
        assert_eq!(cpu.f, 0xF0); // high nibble preserved, low nibble masked to zero
    }
}
