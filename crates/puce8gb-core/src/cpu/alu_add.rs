//! 8-bit ALU on A (task C01_13): ADD/ADC/SUB/SBC with a register, (HL) or an immediate.
//! Decoded by opcode bit fields per decision C_00; no table.

use super::Cpu;
use crate::bus::Bus;

impl Cpu {
    /// 8-bit ALU on A (task C01_13). Returns true when the current opcode is in this group,
    /// false otherwise. Timings and flag effects from note 02c (seed Opcodes.json) and
    /// pandocs historical L2160-2171:
    /// - ADD/ADC/SUB/SBC A,r (block 2, y == 0..3, z != 6): 4 T = 1 M-cycle; the fetch is the
    ///   whole instruction. Flags z0hc for ADD/ADC, z1hc for SUB/SBC.
    /// - ADD/ADC/SUB/SBC A,(HL) (block 2, y == 0..3, z == 6): 8 T = 2 M-cycles; step 2 reads
    ///   (HL), at most one bus access per decision C_00.
    /// - ADD/ADC/SUB/SBC A,n (block 3, z == 6, y == 0..3: $C6/$CE/$D6/$DE): 8 T = 2 M-cycles;
    ///   step 2 reads the immediate at PC and advances it.
    pub(super) fn exec_alu_add(&mut self, bus: &mut Bus) -> bool {
        let op = self.opcode;
        let x = op >> 6;
        let y = (op >> 3) & 7;
        let z = op & 7;

        // Register and (HL) forms: block 2, y in {0 ADD, 1 ADC, 2 SUB, 3 SBC}.
        if x == 2 && y <= 3 {
            let is_add = y < 2;
            // Only ADC (y == 1) and SBC (y == 3) take the C flag as carry/borrow in; ADD and
            // SUB ignore it (task: "ADC/SBC with carry in").
            let carry_in = (y == 1 || y == 3) && self.f & 0x10 != 0;

            if z != 6 {
                // Register form: the fetch M-cycle is the whole instruction.
                let operand = self.read_r8(z);
                let (result, flags) = alu8(self.a, operand, carry_in, is_add);
                self.a = result;
                self.f = flags;
                self.done();
                return true;
            }

            // (HL) form: two M-cycles total.
            match self.step {
                // Step 1 is the fetch M-cycle (decision C_00): no bus access yet.
                1 => return true,
                _ => {
                    let operand = bus.read(self.hl());
                    let (result, flags) = alu8(self.a, operand, carry_in, is_add);
                    self.a = result;
                    self.f = flags;
                    self.done();
                    return true;
                }
            }
        }

        // Immediate forms: block 3, z == 6, y in {0 ADD ($C6), 1 ADC ($CE), 2 SUB ($D6),
        // 3 SBC ($DE)}. Two M-cycles total.
        if x == 3 && z == 6 && y <= 3 {
            let is_add = y < 2;
            // Only ADC ($CE, y == 1) and SBC ($DE, y == 3) take the C flag as carry/borrow in.
            let carry_in = (y == 1 || y == 3) && self.f & 0x10 != 0;

            match self.step {
                // Step 1 is the fetch M-cycle (decision C_00): no bus access yet.
                1 => return true,
                _ => {
                    let operand = bus.read(self.pc);
                    self.pc = self.pc.wrapping_add(1);
                    let (result, flags) = alu8(self.a, operand, carry_in, is_add);
                    self.a = result;
                    self.f = flags;
                    self.done();
                    return true;
                }
            }
        }

        false
    }
}

/// 8-bit ALU on A: addition (ADD/ADC) or subtraction (SUB/SBC) with an optional carry in.
/// Returns the new value of A and the Z N H C flags (low nibble zero). Pure so it can be
/// tested without a bus; hardware arithmetic only (decision C_00).
pub(super) fn alu8(a: u8, b: u8, carry_in: bool, is_add: bool) -> (u8, u8) {
    if is_add {
        add8(a, b, carry_in)
    } else {
        sub8(a, b, carry_in)
    }
}

/// ADD/ADC A,b with an optional carry in. Z set iff the result is zero, N cleared, H on a
/// low-nibble overflow, C on a full-byte overflow (pandocs historical L2160-2165: z0hc).
pub(super) fn add8(a: u8, b: u8, carry_in: bool) -> (u8, u8) {
    let sum = (a as u16)
        .wrapping_add(b as u16)
        .wrapping_add(carry_in as u16);
    let result = sum as u8; // wraps mod 256
    let mut f = 0u8;
    if result == 0x00 {
        f |= 0x80; // Z
    }
    if (a & 0x0F) as u16 + (b & 0x0F) as u16 + carry_in as u16 > 0x0F {
        f |= 0x20; // H: low-nibble overflow
    }
    if sum > 0xFF {
        f |= 0x10; // C: full-byte overflow
    }
    (result, f)
}

/// SUB/SBC A,b with an optional borrow in. Z set iff the result is zero, N set, H on a
/// low-nibble underflow, C on a full-byte underflow (pandocs historical L2166-2171: z1hc).
pub(super) fn sub8(a: u8, b: u8, borrow_in: bool) -> (u8, u8) {
    let b_total = (b as u16).wrapping_add(borrow_in as u16); // at most 0x100
    let result = (a as u16).wrapping_sub(b_total) as u8; // wraps mod 256
    let mut f = 0x40u8; // N set for the subtraction
    if result == 0x00 {
        f |= 0x80; // Z
    }
    if ((a & 0x0F) as u16) < (b & 0x0F) as u16 + borrow_in as u16 {
        f |= 0x20; // H: low-nibble underflow
    }
    if b_total > a as u16 {
        f |= 0x10; // C: full-byte underflow
    }
    (result, f)
}

#[cfg(test)]
mod tests {
    use super::{add8, sub8};
    use crate::cpu::{testutil, Cpu};

    // ---- Family 1: ADD A,r ($80-$87; note 02c seed Opcodes.json: 4 T = 1 M-cycle, z0hc;
    // pandocs historical L2160) ----

    #[test]
    fn c01_13_add_ar_costs_one_tick_and_clears_all_flags() {
        let mut bus = testutil::new_bus();
        let mut cpu = Cpu {
            a: 0x53,
            b: 0x2B,
            f: 0xF0, // all flags set before the instruction; Z/N/H must be recomputed
            ..Cpu::default()
        };

        let ticks = testutil::exec(&mut cpu, &mut bus, &[0x80]); // ADD A,B

        assert_eq!(ticks, 1);
        assert_eq!(cpu.a, 0x7E); // $53 + $2B
        assert_eq!(cpu.pc, 0xC001); // PC advanced past the opcode
        assert_eq!(cpu.f, 0x00); // Z/N/H clear (no low-nibble overflow), C recomputed clear
    }

    #[test]
    fn c01_13_add_ar_wrap_sets_c_only() {
        let mut bus = testutil::new_bus();
        let mut cpu = Cpu {
            a: 0xF0,
            b: 0x20,
            ..Cpu::default()
        };

        let ticks = testutil::exec(&mut cpu, &mut bus, &[0x80]); // ADD A,B

        assert_eq!(ticks, 1);
        assert_eq!(cpu.a, 0x10); // $F0 + $20 wraps around the byte
        assert_eq!(cpu.f, 0x10); // C set (full-byte overflow), Z/N/H clear
    }

    #[test]
    fn c01_13_add_ar_half_carry_boundary_sets_h_only() {
        let mut bus = testutil::new_bus();
        let mut cpu = Cpu {
            a: 0x0F,
            b: 0x01,
            ..Cpu::default()
        };

        let ticks = testutil::exec(&mut cpu, &mut bus, &[0x80]); // ADD A,B

        assert_eq!(ticks, 1);
        assert_eq!(cpu.a, 0x10); // low nibble $0F overflows into bit 4
        assert_eq!(cpu.f, 0x20); // H set only
    }

    #[test]
    fn c01_13_add_ar_zero_result_sets_z_h_and_c() {
        let mut bus = testutil::new_bus();
        let mut cpu = Cpu {
            a: 0x7B,
            b: 0x85,
            ..Cpu::default()
        };

        let ticks = testutil::exec(&mut cpu, &mut bus, &[0x80]); // ADD A,B

        assert_eq!(ticks, 1);
        assert_eq!(cpu.a, 0x00); // $7B + $85 wraps to zero
        assert_eq!(cpu.f, 0xB0); // Z set (result zero), H and C both set
    }

    // ---- Family 2: ADD A,(HL) ($86; note 02c seed Opcodes.json: 8 T = 2 M-cycles, z0hc;
    // pandocs historical L2162) ----

    #[test]
    fn c01_13_add_a_hl_costs_two_ticks_and_reads_memory() {
        let mut bus = testutil::new_bus();
        bus.write(0xC100, 0x45);
        let mut cpu = Cpu {
            a: 0x2A,
            h: 0xC1,
            l: 0x00,
            f: 0xF0, // all flags set before the instruction; must be recomputed
            ..Cpu::default()
        };

        let ticks = testutil::exec(&mut cpu, &mut bus, &[0x86]); // ADD A,(HL)

        assert_eq!(ticks, 2);
        assert_eq!(cpu.a, 0x6F); // $2A + (HL)
        assert_eq!(bus.read(0xC100), 0x45); // memory untouched (read only)
        assert_eq!(cpu.pc, 0xC001); // PC advanced past the opcode only
        assert_eq!(cpu.f, 0x00); // Z/N/H clear (low nibble $A + $5 = $F), C recomputed clear
    }

    #[test]
    fn c01_13_add_a_hl_wrap_sets_z_h_and_c() {
        let mut bus = testutil::new_bus();
        bus.write(0xC100, 0x7D);
        let mut cpu = Cpu {
            a: 0x83,
            h: 0xC1,
            l: 0x00,
            ..Cpu::default()
        };

        let ticks = testutil::exec(&mut cpu, &mut bus, &[0x86]); // ADD A,(HL)

        assert_eq!(ticks, 2);
        assert_eq!(cpu.a, 0x00); // $83 + $7D wraps to zero
        assert_eq!(cpu.f, 0xB0); // Z set (result zero), H and C both set
    }

    // ---- Family 3: ADD A,n ($C6; note 02c seed Opcodes.json: 8 T = 2 M-cycles, z0hc;
    // pandocs historical L2161) ----

    #[test]
    fn c01_13_add_an_costs_two_ticks_and_advances_pc_by_two() {
        let mut bus = testutil::new_bus();
        let mut cpu = Cpu {
            a: 0x53,
            f: 0xF0, // all flags set before the instruction; must be recomputed
            ..Cpu::default()
        };

        let ticks = testutil::exec(&mut cpu, &mut bus, &[0xC6, 0x2B]); // ADD A,$2B

        assert_eq!(ticks, 2);
        assert_eq!(cpu.a, 0x7E); // $53 + $2B
        assert_eq!(cpu.pc, 0xC002); // PC advanced past opcode and immediate
        assert_eq!(cpu.f, 0x00); // Z/N/H clear (low nibble $3 + $B = $E), C recomputed clear
    }

    #[test]
    fn c01_13_add_an_wrap_sets_z_h_and_c() {
        let mut bus = testutil::new_bus();
        let mut cpu = Cpu {
            a: 0xF5,
            ..Cpu::default()
        };

        let ticks = testutil::exec(&mut cpu, &mut bus, &[0xC6, 0x0B]); // ADD A,$0B

        assert_eq!(ticks, 2);
        assert_eq!(cpu.a, 0x00); // $F5 + $0B wraps to zero
        assert_eq!(cpu.f, 0xB0); // Z set (result zero), H and C both set
    }

    // ---- Family 4: ADC A,r ($88-$8F; note 02c seed Opcodes.json: 4 T = 1 M-cycle, z0hc;
    // pandocs historical L2163) ----

    #[test]
    fn c01_13_adc_ar_with_carry_in_adds_one() {
        let mut bus = testutil::new_bus();
        let mut cpu = Cpu {
            a: 0x2A,
            b: 0x5B,
            f: 0x10, // C set before the instruction: it is added in
            ..Cpu::default()
        };

        let ticks = testutil::exec(&mut cpu, &mut bus, &[0x88]); // ADC A,B

        assert_eq!(ticks, 1);
        assert_eq!(cpu.a, 0x86); // $2A + $5B + carry in
        assert_eq!(cpu.f, 0x20); // H set (low nibble $A + $B + 1 overflows), C clear ($86 stays under $FF)
    }

    #[test]
    fn c01_13_adc_ar_without_carry_in_behaves_like_add() {
        let mut bus = testutil::new_bus();
        let mut cpu = Cpu {
            a: 0xFE,
            b: 0x01,
            ..Cpu::default() // C clear before the instruction
        };

        let ticks = testutil::exec(&mut cpu, &mut bus, &[0x88]); // ADC A,B

        assert_eq!(ticks, 1);
        assert_eq!(cpu.a, 0xFF); // $FE + $01 with no carry in (a wrong +1 would wrap to zero)
        assert_eq!(cpu.f, 0x00); // Z/N/H/C all clear
    }

    #[test]
    fn c01_13_adc_ar_wrap_with_carry_in_sets_z_h_and_c() {
        let mut bus = testutil::new_bus();
        let mut cpu = Cpu {
            a: 0xFE,
            b: 0x01,
            f: 0x10, // C set before the instruction
            ..Cpu::default()
        };

        let ticks = testutil::exec(&mut cpu, &mut bus, &[0x88]); // ADC A,B

        assert_eq!(ticks, 1);
        assert_eq!(cpu.a, 0x00); // $FE + $01 + carry in wraps to zero
        assert_eq!(cpu.f, 0xB0); // Z set (result zero), H and C both set
    }

    // ---- Family 5: ADC A,(HL) ($8E; note 02c seed Opcodes.json: 8 T = 2 M-cycles, z0hc;
    // pandocs historical L2165) ----

    #[test]
    fn c01_13_adc_a_hl_with_carry_in() {
        let mut bus = testutil::new_bus();
        bus.write(0xC100, 0x5B);
        let mut cpu = Cpu {
            a: 0x2A,
            h: 0xC1,
            l: 0x00,
            f: 0x10, // C set before the instruction: it is added in
            ..Cpu::default()
        };

        let ticks = testutil::exec(&mut cpu, &mut bus, &[0x8E]); // ADC A,(HL)

        assert_eq!(ticks, 2);
        assert_eq!(cpu.a, 0x86); // $2A + (HL) + carry in
        assert_eq!(bus.read(0xC100), 0x5B); // memory untouched (read only)
        assert_eq!(cpu.pc, 0xC001);
        assert_eq!(cpu.f, 0x20); // H set (low nibble $A + $B + 1 overflows), C clear ($86 stays under $FF)
    }

    // ---- Family 6: ADC A,n ($CE; note 02c seed Opcodes.json: 8 T = 2 M-cycles, z0hc;
    // pandocs historical L2164) ----

    #[test]
    fn c01_13_adc_an_with_carry_in() {
        let mut bus = testutil::new_bus();
        let mut cpu = Cpu {
            a: 0xF5,
            f: 0x10, // C set before the instruction: it is added in
            ..Cpu::default()
        };

        let ticks = testutil::exec(&mut cpu, &mut bus, &[0xCE, 0x0A]); // ADC A,$0A

        assert_eq!(ticks, 2);
        assert_eq!(cpu.a, 0x00); // $F5 + $0A + carry in wraps to zero
        assert_eq!(cpu.pc, 0xC002); // PC advanced past opcode and immediate
        assert_eq!(cpu.f, 0xB0); // Z set (result zero), H and C both set
    }

    // ---- Family 7: SUB A,r ($90-$97; note 02c seed Opcodes.json: 4 T = 1 M-cycle, z1hc;
    // pandocs historical L2166) ----

    #[test]
    fn c01_13_sub_ar_costs_one_tick_and_sets_n_only() {
        let mut bus = testutil::new_bus();
        let mut cpu = Cpu {
            a: 0x5A,
            b: 0x19, // low nibble $A - $B does not underflow: N only
            f: 0xF0, // all flags set before the instruction; Z/H/C must be recomputed
            ..Cpu::default()
        };

        let ticks = testutil::exec(&mut cpu, &mut bus, &[0x90]); // SUB A,B

        assert_eq!(ticks, 1);
        assert_eq!(cpu.a, 0x41); // $5A - $19
        assert_eq!(cpu.pc, 0xC001);
        assert_eq!(cpu.f, 0x40); // N set (subtraction), Z/H/C clear (no underflow)
    }

    #[test]
    fn c01_13_sub_ar_borrow_sets_n_h_and_c() {
        let mut bus = testutil::new_bus();
        let mut cpu = Cpu {
            a: 0x2A,
            b: 0x5B,
            ..Cpu::default()
        };

        let ticks = testutil::exec(&mut cpu, &mut bus, &[0x90]); // SUB A,B

        assert_eq!(ticks, 1);
        assert_eq!(cpu.a, 0xCF); // $2A - $5B underflows the byte
        assert_eq!(cpu.f, 0x70); // N set, H and C both set (low nibble and full-byte underflow)
    }

    #[test]
    fn c01_13_sub_ar_half_borrow_sets_n_and_h_only() {
        let mut bus = testutil::new_bus();
        let mut cpu = Cpu {
            a: 0x51,
            b: 0x2F,
            ..Cpu::default()
        };

        let ticks = testutil::exec(&mut cpu, &mut bus, &[0x90]); // SUB A,B

        assert_eq!(ticks, 1);
        assert_eq!(cpu.a, 0x22); // $51 - $2F: the low nibble underflows but not the byte
        assert_eq!(cpu.f, 0x60); // N set and H set (low-nibble underflow), C clear
    }

    #[test]
    fn c01_13_sub_ar_zero_result_sets_z_and_n() {
        let mut bus = testutil::new_bus();
        let mut cpu = Cpu {
            a: 0x4D,
            b: 0x4D,
            f: 0x10, // C set before the instruction; it must be recomputed clear
            ..Cpu::default()
        };

        let ticks = testutil::exec(&mut cpu, &mut bus, &[0x90]); // SUB A,B

        assert_eq!(ticks, 1);
        assert_eq!(cpu.a, 0x00); // $4D - $4D reaches zero
        assert_eq!(cpu.f, 0xC0); // Z set (result zero) and N set, H/C clear
    }

    // ---- Family 8: SUB A,(HL) ($96; note 02c seed Opcodes.json: 8 T = 2 M-cycles, z1hc;
    // pandocs historical L2168) ----

    #[test]
    fn c01_13_sub_a_hl_costs_two_ticks_and_sets_n() {
        let mut bus = testutil::new_bus();
        bus.write(0xC100, 0x19); // low nibble $A - $B does not underflow: N only
        let mut cpu = Cpu {
            a: 0x5A,
            h: 0xC1,
            l: 0x00,
            f: 0xF0, // all flags set before the instruction; Z/H/C must be recomputed
            ..Cpu::default()
        };

        let ticks = testutil::exec(&mut cpu, &mut bus, &[0x96]); // SUB A,(HL)

        assert_eq!(ticks, 2);
        assert_eq!(cpu.a, 0x41); // $5A - (HL)
        assert_eq!(bus.read(0xC100), 0x19); // memory untouched (read only)
        assert_eq!(cpu.pc, 0xC001);
        assert_eq!(cpu.f, 0x40); // N set (subtraction), Z/H/C clear (no underflow)
    }

    // ---- Family 9: SUB A,n ($D6; note 02c seed Opcodes.json: 8 T = 2 M-cycles, z1hc;
    // pandocs historical L2167) ----

    #[test]
    fn c01_13_sub_an_borrow_sets_n_h_and_c() {
        let mut bus = testutil::new_bus();
        let mut cpu = Cpu {
            a: 0x2A,
            ..Cpu::default()
        };

        let ticks = testutil::exec(&mut cpu, &mut bus, &[0xD6, 0x5B]); // SUB A,$5B

        assert_eq!(ticks, 2);
        assert_eq!(cpu.a, 0xCF); // $2A - $5B underflows the byte
        assert_eq!(cpu.pc, 0xC002); // PC advanced past opcode and immediate
        assert_eq!(cpu.f, 0x70); // N set, H and C both set (low nibble and full-byte underflow)
    }

    // ---- Family 10: SBC A,r ($98-$9F; note 02c seed Opcodes.json: 4 T = 1 M-cycle, z1hc;
    // pandocs historical L2169) ----

    #[test]
    fn c01_13_sbc_ar_with_borrow_in_subtracts_one_more() {
        let mut bus = testutil::new_bus();
        let mut cpu = Cpu {
            a: 0x5A,
            b: 0x19, // low nibble $A - ($B + borrow) does not underflow: N only
            f: 0x10, // C set before the instruction: it is subtracted in
            ..Cpu::default()
        };

        let ticks = testutil::exec(&mut cpu, &mut bus, &[0x98]); // SBC A,B

        assert_eq!(ticks, 1);
        assert_eq!(cpu.a, 0x40); // $5A - $19 - borrow in (a missing -1 would give $41)
        assert_eq!(cpu.f, 0x40); // N set only (no underflow at bit 3 or the byte)
    }

    #[test]
    fn c01_13_sbc_ar_without_borrow_in_behaves_like_sub() {
        let mut bus = testutil::new_bus();
        let mut cpu = Cpu {
            a: 0x5A,
            b: 0x19,          // low nibble does not underflow: N only
            ..Cpu::default()  // C clear before the instruction
        };

        let ticks = testutil::exec(&mut cpu, &mut bus, &[0x98]); // SBC A,B

        assert_eq!(ticks, 1);
        assert_eq!(cpu.a, 0x41); // $5A - $19 with no borrow in (a wrong -1 would give $40)
        assert_eq!(cpu.f, 0x40); // N set only
    }

    #[test]
    fn c01_13_sbc_ar_wrap_with_borrow_in_sets_n_h_and_c() {
        let mut bus = testutil::new_bus();
        let mut cpu = Cpu {
            a: 0x00,
            b: 0x01,
            f: 0x10, // C set before the instruction
            ..Cpu::default()
        };

        let ticks = testutil::exec(&mut cpu, &mut bus, &[0x98]); // SBC A,B

        assert_eq!(ticks, 1);
        assert_eq!(cpu.a, 0xFE); // $00 - $01 - borrow in wraps around the byte
        assert_eq!(cpu.f, 0x70); // N set, H and C both set (low nibble and full-byte underflow)
    }

    #[test]
    fn c01_13_sbc_ar_zero_result_with_borrow_in_sets_z_and_n() {
        let mut bus = testutil::new_bus();
        let mut cpu = Cpu {
            a: 0x02,
            b: 0x01,
            f: 0x10, // C set before the instruction
            ..Cpu::default()
        };

        let ticks = testutil::exec(&mut cpu, &mut bus, &[0x98]); // SBC A,B

        assert_eq!(ticks, 1);
        assert_eq!(cpu.a, 0x00); // $02 - $01 - borrow in reaches zero exactly
        assert_eq!(cpu.f, 0xC0); // Z set (result zero) and N set; H/C clear (no underflow)
    }

    // ---- Family 11: SBC A,(HL) ($9E; note 02c seed Opcodes.json: 8 T = 2 M-cycles, z1hc;
    // pandocs historical L2171) ----

    #[test]
    fn c01_13_sbc_a_hl_with_borrow_in() {
        let mut bus = testutil::new_bus();
        bus.write(0xC100, 0x19); // low nibble $A - ($B + borrow) does not underflow: N only
        let mut cpu = Cpu {
            a: 0x5A,
            h: 0xC1,
            l: 0x00,
            f: 0x10, // C set before the instruction: it is subtracted in
            ..Cpu::default()
        };

        let ticks = testutil::exec(&mut cpu, &mut bus, &[0x9E]); // SBC A,(HL)

        assert_eq!(ticks, 2);
        assert_eq!(cpu.a, 0x40); // $5A - (HL) - borrow in
        assert_eq!(bus.read(0xC100), 0x19); // memory untouched (read only)
        assert_eq!(cpu.pc, 0xC001);
        assert_eq!(cpu.f, 0x40); // N set only (no underflow at bit 3 or the byte)
    }

    // ---- Family 12: SBC A,n ($DE; note 02c seed Opcodes.json: 8 T = 2 M-cycles, z1hc;
    // pandocs historical L2170) ----

    #[test]
    fn c01_13_sbc_an_with_borrow_in() {
        let mut bus = testutil::new_bus();
        let mut cpu = Cpu {
            a: 0xF5,
            f: 0x10, // C set before the instruction: it is subtracted in
            ..Cpu::default()
        };

        let ticks = testutil::exec(&mut cpu, &mut bus, &[0xDE, 0x0A]); // SBC A,$0A

        assert_eq!(ticks, 2);
        assert_eq!(cpu.a, 0xEA); // $F5 - $0A - borrow in
        assert_eq!(cpu.pc, 0xC002); // PC advanced past opcode and immediate
        assert_eq!(cpu.f, 0x60); // N set and H set (low nibble underflows), C clear
    }

    // ---- Pure flag functions: half-carry/carry boundaries with and without carry in ----

    #[test]
    fn c01_13_add8_boundaries() {
        assert_eq!(add8(0x35, 0x12, false), (0x47, 0x00)); // no flags at all
        assert_eq!(add8(0x0F, 0x01, false), (0x10, 0x20)); // H only: low nibble overflows
        assert_eq!(add8(0xF0, 0x10, false), (0x00, 0x90)); // Z and C: wraps without a half carry
        assert_eq!(add8(0xFF, 0x01, false), (0x00, 0xB0)); // Z, H and C all set at once
        assert_eq!(add8(0xFE, 0x01, true), (0x00, 0xB0)); // carry in pushes the sum over $FF
        assert_eq!(add8(0x7D, 0x82, false), (0xFF, 0x00)); // result $FF sets no flag
    }

    #[test]
    fn c01_13_sub8_boundaries() {
        assert_eq!(sub8(0x5A, 0x19, false), (0x41, 0x40)); // N only: plain subtraction
        assert_eq!(sub8(0x51, 0x2F, false), (0x22, 0x60)); // N and H: low nibble underflows only
        assert_eq!(sub8(0x2A, 0x5B, false), (0xCF, 0x70)); // N, H and C: full-byte underflow
        assert_eq!(sub8(0x4D, 0x4D, false), (0x00, 0xC0)); // Z and N: result zero, no borrow
        assert_eq!(sub8(0x02, 0x01, true), (0x00, 0xC0)); // borrow in consumed exactly to zero
        assert_eq!(sub8(0x00, 0x01, true), (0xFE, 0x70)); // N, H and C: $00 - $01 - borrow in
    }
}
