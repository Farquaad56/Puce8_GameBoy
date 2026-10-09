//! 8-bit logic and compare on A (task C01_14): AND/XOR/OR/CP with a register, (HL) or an
//! immediate. Decoded by opcode bit fields per decision C_00; no table.

use super::alu_add::sub8;
use super::Cpu;
use crate::bus::Bus;

impl Cpu {
    /// 8-bit logic and compare on A (task C01_14). Returns true when the current opcode is
    /// in this group, false otherwise. Timings and flag effects from note 02c (seed
    /// Opcodes.json) and pandocs historical L2172-2183:
    /// - AND/XOR/OR/CP A,r (block 2, y == 4..7): 4 T = 1 M-cycle; the fetch is the whole
    ///   instruction. Flags z010 for AND, z000 for XOR/OR, z1hc for CP.
    /// - AND/XOR/OR/CP A,(HL) (block 2, y == 4..7, z == 6): 8 T = 2 M-cycles; step 2 reads
    ///   (HL), at most one bus access per decision C_00.
    /// - AND/XOR/OR/CP A,n (block 3, z == 6, y == 4..7: $E6/$EE/$F6/$FE): 8 T = 2 M-cycles;
    ///   step 2 reads the immediate at PC and advances it. CP does not store the result.
    pub(super) fn exec_alu_logic(&mut self, bus: &mut Bus) -> bool {
        let op = self.opcode;
        let x = op >> 6;
        let y = (op >> 3) & 7;
        let z = op & 7;

        // Register and (HL) forms: block 2, y in {4 AND, 5 XOR, 6 OR, 7 CP}.
        if x == 2 && y >= 4 {
            if z != 6 {
                // Register form: the fetch M-cycle is the whole instruction.
                let operand = self.read_r8(z);
                let (result, flags) = logic_op(self.a, operand, y);
                self.store_result(y, result, flags);
                self.done();
                return true;
            }

            // (HL) form: two M-cycles total.
            match self.step {
                // Step 1 is the fetch M-cycle (decision C_00): no bus access yet.
                1 => return true,
                _ => {
                    let operand = bus.read(self.hl());
                    let (result, flags) = logic_op(self.a, operand, y);
                    self.store_result(y, result, flags);
                    self.done();
                    return true;
                }
            }
        }

        // Immediate forms: block 3, z == 6, y in {4 AND ($E6), 5 XOR ($EE), 6 OR ($F6),
        // 7 CP ($FE)}. Two M-cycles total.
        if x == 3 && z == 6 && y >= 4 {
            match self.step {
                // Step 1 is the fetch M-cycle (decision C_00): no bus access yet.
                1 => return true,
                _ => {
                    let operand = bus.read(self.pc);
                    self.pc = self.pc.wrapping_add(1);
                    let (result, flags) = logic_op(self.a, operand, y);
                    self.store_result(y, result, flags);
                    self.done();
                    return true;
                }
            }
        }

        false
    }

    /// Apply the group's flag rule: CP computes flags only (SUB without storing), the logic
    /// ops store the result in A.
    fn store_result(&mut self, y: u8, result: u8, flags: u8) {
        if y != 7 {
            self.a = result;
        }
        self.f = flags;
    }
}

/// AND/XOR/OR/CP on A with an operand. Returns the new value of A and the Z N H C flags
/// (low nibble zero). Pure so it can be tested without a bus:
/// - AND: z010, result is always stored;
/// - XOR / OR: z000, result is always stored;
/// - CP: z1hc like SUB but the result is not stored (task C01_14).
pub(super) fn logic_op(a: u8, b: u8, y: u8) -> (u8, u8) {
    match y {
        4 => and8(a, b),
        5 => xor8(a, b),
        6 => or8(a, b),
        _ => sub8(a, b, false), // CP: SUB without storing the result
    }
}

/// AND A,b. Z set iff the result is zero, N cleared, H always set, C cleared (pandocs
/// historical L2172-2174: z010).
pub(super) fn and8(a: u8, b: u8) -> (u8, u8) {
    let result = a & b;
    let mut f = 0x30u8; // H set, C clear
    if result == 0x00 {
        f |= 0x80; // Z
    }
    (result, f)
}

/// XOR A,b. Z set iff the result is zero, N/H/C all cleared (pandocs historical L2175-2177:
/// z000).
pub(super) fn xor8(a: u8, b: u8) -> (u8, u8) {
    let result = a ^ b;
    let mut f = 0u8;
    if result == 0x00 {
        f |= 0x80; // Z
    }
    (result, f)
}

/// OR A,b. Z set iff the result is zero, N/H/C all cleared (pandocs historical L2178-2180:
/// z000).
pub(super) fn or8(a: u8, b: u8) -> (u8, u8) {
    let result = a | b;
    let mut f = 0u8;
    if result == 0x00 {
        f |= 0x80; // Z
    }
    (result, f)
}

#[cfg(test)]
mod tests {
    use super::{and8, or8, xor8};
    use crate::cpu::{testutil, Cpu};

    // ---- Family 1: AND A,r ($A0-$A7; note 02c seed Opcodes.json: 4 T = 1 M-cycle, z010;
    // pandocs historical L2172) ----

    #[test]
    fn c01_14_and_ar_costs_one_tick_and_sets_h_only() {
        let mut bus = testutil::new_bus();
        let mut cpu = Cpu {
            a: 0x5A,
            b: 0x3C,
            f: 0xF0, // all flags set before the instruction; Z/N/C must be recomputed
            ..Cpu::default()
        };

        let ticks = testutil::exec(&mut cpu, &mut bus, &[0xA0]); // AND A,B

        assert_eq!(ticks, 1);
        assert_eq!(cpu.a, 0x18); // $5A & $3C
        assert_eq!(cpu.pc, 0xC001); // PC advanced past the opcode
        assert_eq!(cpu.f, 0x30); // H set only (result nonzero), N and C recomputed clear
    }

    #[test]
    fn c01_14_and_ar_zero_result_sets_z_and_h() {
        let mut bus = testutil::new_bus();
        let mut cpu = Cpu {
            a: 0x7B,
            b: 0x84, // $7B & $84 == $00 (disjoint bit patterns)
            ..Cpu::default()
        };

        let ticks = testutil::exec(&mut cpu, &mut bus, &[0xA0]); // AND A,B

        assert_eq!(ticks, 1);
        assert_eq!(cpu.a, 0x00);
        assert_eq!(cpu.f, 0xB0); // Z set (result zero) and H always set; N/C clear
    }

    #[test]
    fn c01_14_and_a_hl_costs_two_ticks_and_reads_memory() {
        let mut bus = testutil::new_bus();
        bus.write(0xC100, 0x3C);
        let mut cpu = Cpu {
            a: 0x5A,
            h: 0xC1,
            l: 0x00,
            f: 0xF0, // all flags set before the instruction; must be recomputed
            ..Cpu::default()
        };

        let ticks = testutil::exec(&mut cpu, &mut bus, &[0xA6]); // AND A,(HL)

        assert_eq!(ticks, 2);
        assert_eq!(cpu.a, 0x18); // $5A & (HL)
        assert_eq!(bus.read(0xC100), 0x3C); // memory untouched (read only)
        assert_eq!(cpu.pc, 0xC001); // PC advanced past the opcode only
        assert_eq!(cpu.f, 0x30); // H set only
    }

    #[test]
    fn c01_14_and_an_costs_two_ticks_and_advances_pc_by_two() {
        let mut bus = testutil::new_bus();
        let mut cpu = Cpu {
            a: 0xFF,
            f: 0xF0, // all flags set before the instruction; must be recomputed
            ..Cpu::default()
        };

        let ticks = testutil::exec(&mut cpu, &mut bus, &[0xE6, 0x24]); // AND A,$24

        assert_eq!(ticks, 2);
        assert_eq!(cpu.a, 0x24); // $FF & $24
        assert_eq!(cpu.pc, 0xC002); // PC advanced past opcode and immediate
        assert_eq!(cpu.f, 0x30); // H set only (result nonzero)
    }

    // ---- Family 2: XOR A,r ($A8-$AF; note 02c seed Opcodes.json: 4 T = 1 M-cycle, z000;
    // pandocs historical L2175) ----

    #[test]
    fn c01_14_xor_ar_costs_one_tick_and_clears_all_flags() {
        let mut bus = testutil::new_bus();
        let mut cpu = Cpu {
            a: 0x5A,
            b: 0x3C,
            f: 0xF0, // all flags set before the instruction; Z/N/H must be recomputed
            ..Cpu::default()
        };

        let ticks = testutil::exec(&mut cpu, &mut bus, &[0xA8]); // XOR A,B

        assert_eq!(ticks, 1);
        assert_eq!(cpu.a, 0x66); // $5A ^ $3C
        assert_eq!(cpu.pc, 0xC001);
        assert_eq!(cpu.f, 0x00); // Z/N/H/C all clear (result nonzero)
    }

    #[test]
    fn c01_14_xor_a_a_sets_z_only() {
        let mut bus = testutil::new_bus();
        let mut cpu = Cpu {
            a: 0x4D,
            f: 0xF0, // all flags set before the instruction; H must be recomputed clear
            ..Cpu::default()
        };

        let ticks = testutil::exec(&mut cpu, &mut bus, &[0xAF]); // XOR A,A

        assert_eq!(ticks, 1);
        assert_eq!(cpu.a, 0x00); // a value xor itself is zero
        assert_eq!(cpu.f, 0x80); // Z set only (H stays clear, unlike AND)
    }

    #[test]
    fn c01_14_xor_a_hl_costs_two_ticks_and_reads_memory() {
        let mut bus = testutil::new_bus();
        bus.write(0xC100, 0x3C);
        let mut cpu = Cpu {
            a: 0x5A,
            h: 0xC1,
            l: 0x00,
            ..Cpu::default()
        };

        let ticks = testutil::exec(&mut cpu, &mut bus, &[0xAE]); // XOR A,(HL)

        assert_eq!(ticks, 2);
        assert_eq!(cpu.a, 0x66); // $5A ^ (HL)
        assert_eq!(bus.read(0xC100), 0x3C); // memory untouched (read only)
        assert_eq!(cpu.pc, 0xC001);
        assert_eq!(cpu.f, 0x00); // all flags clear
    }

    #[test]
    fn c01_14_xor_an_costs_two_ticks_and_advances_pc_by_two() {
        let mut bus = testutil::new_bus();
        let mut cpu = Cpu {
            a: 0x5A,
            ..Cpu::default()
        };

        let ticks = testutil::exec(&mut cpu, &mut bus, &[0xEE, 0x3C]); // XOR A,$3C

        assert_eq!(ticks, 2);
        assert_eq!(cpu.a, 0x66); // $5A ^ $3C
        assert_eq!(cpu.pc, 0xC002); // PC advanced past opcode and immediate
        assert_eq!(cpu.f, 0x00); // all flags clear
    }

    // ---- Family 3: OR A,r ($B0-$B7; note 02c seed Opcodes.json: 4 T = 1 M-cycle, z000;
    // pandocs historical L2178) ----

    #[test]
    fn c01_14_or_ar_costs_one_tick_and_clears_all_flags() {
        let mut bus = testutil::new_bus();
        let mut cpu = Cpu {
            a: 0x5A,
            b: 0x3C,
            f: 0xF0, // all flags set before the instruction; Z/N/H must be recomputed
            ..Cpu::default()
        };

        let ticks = testutil::exec(&mut cpu, &mut bus, &[0xB0]); // OR A,B

        assert_eq!(ticks, 1);
        assert_eq!(cpu.a, 0x7E); // $5A | $3C
        assert_eq!(cpu.pc, 0xC001);
        assert_eq!(cpu.f, 0x00); // Z/N/H/C all clear (result nonzero)
    }

    #[test]
    fn c01_14_or_ar_zero_result_sets_z_only() {
        let mut bus = testutil::new_bus();
        let mut cpu = Cpu {
            a: 0x00,
            b: 0x00,
            f: 0xF0, // all flags set before the instruction; H must be recomputed clear
            ..Cpu::default()
        };

        let ticks = testutil::exec(&mut cpu, &mut bus, &[0xB0]); // OR A,B

        assert_eq!(ticks, 1);
        assert_eq!(cpu.a, 0x00); // $00 | $00 stays zero
        assert_eq!(cpu.f, 0x80); // Z set only (H stays clear, unlike AND)
    }

    #[test]
    fn c01_14_or_a_hl_costs_two_ticks_and_reads_memory() {
        let mut bus = testutil::new_bus();
        bus.write(0xC100, 0x3C);
        let mut cpu = Cpu {
            a: 0x5A,
            h: 0xC1,
            l: 0x00,
            ..Cpu::default()
        };

        let ticks = testutil::exec(&mut cpu, &mut bus, &[0xB6]); // OR A,(HL)

        assert_eq!(ticks, 2);
        assert_eq!(cpu.a, 0x7E); // $5A | (HL)
        assert_eq!(bus.read(0xC100), 0x3C); // memory untouched (read only)
        assert_eq!(cpu.pc, 0xC001);
        assert_eq!(cpu.f, 0x00); // all flags clear
    }

    #[test]
    fn c01_14_or_an_costs_two_ticks_and_advances_pc_by_two() {
        let mut bus = testutil::new_bus();
        let mut cpu = Cpu {
            a: 0x5A,
            ..Cpu::default()
        };

        let ticks = testutil::exec(&mut cpu, &mut bus, &[0xF6, 0x3C]); // OR A,$3C

        assert_eq!(ticks, 2);
        assert_eq!(cpu.a, 0x7E); // $5A | $3C
        assert_eq!(cpu.pc, 0xC002); // PC advanced past opcode and immediate
        assert_eq!(cpu.f, 0x00); // all flags clear
    }

    // ---- Family 4: CP A,r ($B8-$BF; note 02c seed Opcodes.json: 4 T = 1 M-cycle, z1hc;
    // pandocs historical L2181) ----

    #[test]
    fn c01_14_cp_ar_costs_one_tick_and_keeps_a() {
        let mut bus = testutil::new_bus();
        let mut cpu = Cpu {
            a: 0x5A,
            b: 0x19, // low nibble $A - $B does not underflow: N only
            f: 0xF0, // all flags set before the instruction; Z/H/C must be recomputed
            ..Cpu::default()
        };

        let ticks = testutil::exec(&mut cpu, &mut bus, &[0xB8]); // CP A,B

        assert_eq!(ticks, 1);
        assert_eq!(cpu.a, 0x5A); // A is not modified by the compare
        assert_eq!(cpu.pc, 0xC001);
        assert_eq!(cpu.f, 0x40); // N set (subtraction), Z/H/C clear (no underflow)
    }

    #[test]
    fn c01_14_cp_ar_borrow_sets_n_h_and_c() {
        let mut bus = testutil::new_bus();
        let mut cpu = Cpu {
            a: 0x2A,
            b: 0x5B, // $2A - $5B underflows the byte
            ..Cpu::default()
        };

        let ticks = testutil::exec(&mut cpu, &mut bus, &[0xB8]); // CP A,B

        assert_eq!(ticks, 1);
        assert_eq!(cpu.a, 0x2A); // A is not modified by the compare
        assert_eq!(cpu.f, 0x70); // N set, H and C both set (low nibble and full-byte underflow)
    }

    #[test]
    fn c01_14_cp_ar_equal_sets_z_and_n() {
        let mut bus = testutil::new_bus();
        let mut cpu = Cpu {
            a: 0x4D,
            b: 0x4D,
            f: 0x30, // H and C set before the instruction; they must be recomputed clear
            ..Cpu::default()
        };

        let ticks = testutil::exec(&mut cpu, &mut bus, &[0xB8]); // CP A,B

        assert_eq!(ticks, 1);
        assert_eq!(cpu.a, 0x4D); // A is not modified by the compare
        assert_eq!(cpu.f, 0xC0); // Z set (result zero) and N set; H/C clear (no underflow)
    }

    #[test]
    fn c01_14_cp_a_hl_costs_two_ticks_and_keeps_a() {
        let mut bus = testutil::new_bus();
        bus.write(0xC100, 0x5B); // $2A - (HL) underflows the byte
        let mut cpu = Cpu {
            a: 0x2A,
            h: 0xC1,
            l: 0x00,
            ..Cpu::default()
        };

        let ticks = testutil::exec(&mut cpu, &mut bus, &[0xBE]); // CP A,(HL)

        assert_eq!(ticks, 2);
        assert_eq!(cpu.a, 0x2A); // A is not modified by the compare
        assert_eq!(bus.read(0xC100), 0x5B); // memory untouched (read only)
        assert_eq!(cpu.pc, 0xC001);
        assert_eq!(cpu.f, 0x70); // N set, H and C both set
    }

    #[test]
    fn c01_14_cp_an_costs_two_ticks_and_advances_pc_by_two() {
        let mut bus = testutil::new_bus();
        let mut cpu = Cpu {
            a: 0x51,
            ..Cpu::default()
        };

        let ticks = testutil::exec(&mut cpu, &mut bus, &[0xFE, 0x2F]); // CP A,$2F

        assert_eq!(ticks, 2);
        assert_eq!(cpu.a, 0x51); // A is not modified by the compare
        assert_eq!(cpu.pc, 0xC002); // PC advanced past opcode and immediate
        assert_eq!(cpu.f, 0x60); // N set and H set (low nibble underflows), C clear
    }

    // ---- Pure flag functions: boundaries of each logic op ----

    #[test]
    fn c01_14_and8_boundaries() {
        assert_eq!(and8(0x5A, 0x3C), (0x18, 0x30)); // H only: result nonzero
        assert_eq!(and8(0xFF, 0x00), (0x00, 0xB0)); // Z and H: result zero
        assert_eq!(and8(0x7B, 0x84), (0x00, 0xB0)); // disjoint bit patterns give zero
    }

    #[test]
    fn c01_14_xor8_boundaries() {
        assert_eq!(xor8(0x5A, 0x3C), (0x66, 0x00)); // no flags at all
        assert_eq!(xor8(0x4D, 0x4D), (0x00, 0x80)); // Z only: a value xor itself is zero
    }

    #[test]
    fn c01_14_or8_boundaries() {
        assert_eq!(or8(0x5A, 0x3C), (0x7E, 0x00)); // no flags at all
        assert_eq!(or8(0x00, 0x00), (0x00, 0x80)); // Z only: zero or zero is zero
    }
}
