//! CB-prefixed rotate/shift family (task C01_25): RLC/RRC/RL/RR/SLA/SRA/SWAP/SRL on a
//! register or on (HL). Decoded by opcode bit fields per decision C_00; no table.
//!
//! Timings and flag effects from note 02c (seed Opcodes.json) and pandocs historical
//! L2203-2216: the CB second byte selects the op in its high bits (`(op >> 3) & 7`) and the
//! register in its low bits (`op & 7`, where 6 = (HL)). All eight ops are in this group, so a
//! valid shift/rotate second byte is `< $40` (anything `>= $40` is another CB group: C01_26).
//! Register form = 8 T = 2 M-cycles total (prefix fetch + second-byte fetch); (HL) form =
//! 16 T = 4 M-cycles total (prefix fetch + second-byte fetch + read (HL) + write (HL)).
//! Flags: Z = result zero, N/H clear, C = rotated-out bit; SWAP clears C instead. At most one
//! bus access per M-cycle (decision C_00).

use super::Cpu;
use crate::bus::Bus;

impl Cpu {
    /// CB-prefixed rotate/shift family (task C01_25). Returns true when the current opcode is
    /// a shift/rotate op in this group, false otherwise. See the module docs for timings and
    /// flag effects. A second byte `>= $40` belongs to another CB group (C01_26 BIT/RES/SET):
    /// after its fetch it is handed off by returning false so a later CB group can claim it.
    pub(super) fn exec_cb_shift(&mut self, bus: &mut Bus) -> bool {
        // Only the CB prefix ($CB = x3 y1 z3) reaches here; no other group claims it.
        if self.opcode != 0xCB {
            return false;
        }

        match self.step {
            // Step 1 is the prefix fetch M-cycle (decision C_00): no bus access yet. Mark that
            // we are inside a CB instruction so later steps can tell it apart from other ops.
            1 => {
                self.cb = true;
                true
            }
            // Step 2: fetch the second byte at PC (one bus access). A shift/rotate op has a
            // second byte < $40; anything >= $40 is another CB group and is handed off after
            // this fetch (the fetch already happened, so we must not re-read it downstream).
            2 => {
                let cb_op = bus.read(self.pc);
                self.pc = self.pc.wrapping_add(1);
                if cb_op >= 0x40 {
                    return false; // not this group: hand off to a later CB group / unimplemented
                }
                self.lo = cb_op; // latch the op for the (HL) read/write steps
                if (cb_op & 7) != 6 {
                    // Register form: two M-cycles total. Apply the op now, no memory access.
                    let value = self.read_r8(cb_op & 7);
                    let carry_in = self.f & 0x10 != 0;
                    let (result, flags) = cb_shift_op(cb_op, value, carry_in);
                    self.store_r8(cb_op & 7, result);
                    self.f = flags;
                    self.done();
                }
                true // (HL) form continues to step 3; register form already done()
            }
            _ => {
                // Steps 3-4: only reached for the (HL) form of a shift/rotate op.
                if !self.cb || self.lo >= 0x40 || (self.lo & 7) != 6 {
                    return false; // not this group's (HL) op
                }
                match self.step {
                    3 => {
                        self.hi = bus.read(self.hl()); // read (HL); one bus access
                        true
                    }
                    _ => {
                        let carry_in = self.f & 0x10 != 0;
                        let (result, flags) = cb_shift_op(self.lo, self.hi, carry_in);
                        bus.write(self.hl(), result); // write back (HL); one bus access
                        self.f = flags;
                        self.done();
                        true
                    }
                }
            }
        }
    }
}

/// Apply a CB shift/rotate op (second byte < $40) to a value. Returns the new value and the
/// Z N H C flags (low nibble zero). Pure so it can be tested without a bus:
/// - RLC/RRC/RL/RR rotate; SLA/SRA/SRL shift; SWAP exchanges the two nibbles.
/// - Flags: Z = result zero, N/H clear, C = rotated-out bit; SWAP clears C instead (note 02c).
pub(super) fn cb_shift_op(op: u8, value: u8, carry_in: bool) -> (u8, u8) {
    let group = (op >> 3) & 7;
    let c_bit = if carry_in { 1 } else { 0 };
    let result = match group {
        0 => value.rotate_left(1),          // RLC: rotate left
        1 => value.rotate_right(1),         // RRC: rotate right
        2 => (value << 1) | c_bit,          // RL: rotate left through carry
        3 => (value >> 1) | (c_bit << 7),   // RR: rotate right through carry
        4 => value << 1,                    // SLA: shift left arithmetic (b0 = 0)
        5 => (value & 0x80) | (value >> 1), // SRA: shift right arithmetic (sign preserved)
        6 => value.rotate_left(4),          // SWAP: exchange low/hi nibble
        _ => value >> 1,                    // SRL: shift right logical (b7 = 0)
    };
    let c_out = match group {
        6 => false,                       // SWAP clears the carry flag
        0 | 2 | 4 => (value & 0x80) != 0, // RLC/RL/SLA: rotated-out bit is old b7
        _ => (value & 1) != 0,            // RRC/RR/SRA/SRL: rotated-out bit is old b0
    };
    let mut f = 0u8;
    if result == 0x00 {
        f |= 0x80; // Z
    }
    if c_out {
        f |= 0x10; // C
    }
    (result, f)
}

#[cfg(test)]
mod tests {
    use super::cb_shift_op;
    use crate::cpu::{testutil, Cpu};

    // ---- Family RLC r (CB 0x00-0x07; note 02c seed Opcodes.json: [8 T = 2 M], z00c) ----

    #[test]
    fn c01_25_rlc_b_register_form_costs_two_ticks() {
        let mut bus = testutil::new_bus();
        let mut cpu = Cpu {
            b: 0x80,
            f: 0xF0, // all flags set before the instruction; Z/N/H must be recomputed
            ..Cpu::default()
        };

        let ticks = testutil::exec(&mut cpu, &mut bus, &[0xCB, 0x00]); // RLC B

        assert_eq!(ticks, 2); // register form: prefix fetch + second-byte fetch (note 02c)
        assert_eq!(cpu.b, 0x01); // $80 rotated left -> $01
        assert_eq!(cpu.pc, 0xC002); // PC advanced past the CB prefix and the second byte
        assert_eq!(cpu.f, 0x10); // C set (rotated-out bit was old b7), Z/N/H clear
    }

    #[test]
    fn c01_25_rlc_c0_wraparound_sets_carry() {
        let mut bus = testutil::new_bus();
        let mut cpu = Cpu {
            a: 0xC0,
            ..Cpu::default()
        };

        let ticks = testutil::exec(&mut cpu, &mut bus, &[0xCB, 0x07]); // RLC A

        assert_eq!(ticks, 2);
        assert_eq!(cpu.a, 0x81); // $C0 rotated left: top bit wraps to the bottom
        assert_eq!(cpu.f, 0x10); // C set (old b7 was 1), Z clear
    }

    // ---- Family RRC r (CB 0x08-0x0F; note 02c seed Opcodes.json: [8 T = 2 M], z00c) ----

    #[test]
    fn c01_25_rrc_a_register_form_costs_two_ticks() {
        let mut bus = testutil::new_bus();
        let mut cpu = Cpu {
            a: 0x01,
            f: 0xF0, // all flags set before the instruction; Z/N/H must be recomputed
            ..Cpu::default()
        };

        let ticks = testutil::exec(&mut cpu, &mut bus, &[0xCB, 0x0F]); // RRC A (group 1, register index 7)

        assert_eq!(ticks, 2);
        assert_eq!(cpu.a, 0x80); // $01 rotated right -> $80
        assert_eq!(cpu.pc, 0xC002);
        assert_eq!(cpu.f, 0x10); // C set (rotated-out bit was old b0), Z/N/H clear
    }

    // ---- Family RL r (CB 0x10-0x17; note 02c seed Opcodes.json: [8 T = 2 M], z00c) ----

    #[test]
    fn c01_25_rl_e_through_carry_shifts_in_old_c() {
        let mut bus = testutil::new_bus();
        let mut cpu = Cpu {
            e: 0x80,
            f: 0x10, // carry set before the instruction; it is shifted in at bit 0
            ..Cpu::default()
        };

        let ticks = testutil::exec(&mut cpu, &mut bus, &[0xCB, 0x13]); // RL E (register index 3)

        assert_eq!(ticks, 2);
        assert_eq!(cpu.e, 0x01); // ($80 << 1) | old C = $00 | $01
        assert_eq!(cpu.f, 0x10); // C set (rotated-out bit was old b7), Z/N/H clear
    }

    // ---- Family RR r (CB 0x18-0x1F; note 02c seed Opcodes.json: [8 T = 2 M], z00c) ----

    #[test]
    fn c01_25_rr_d_through_carry_shifts_in_old_c() {
        let mut bus = testutil::new_bus();
        let mut cpu = Cpu {
            d: 0x01,
            f: 0x10, // carry set before the instruction; it is shifted in at bit 7
            ..Cpu::default()
        };

        let ticks = testutil::exec(&mut cpu, &mut bus, &[0xCB, 0x1A]); // RR D (register index 2)

        assert_eq!(ticks, 2);
        assert_eq!(cpu.d, 0x80); // ($01 >> 1) | (old C << 7) = $00 | $80
        assert_eq!(cpu.f, 0x10); // C set (rotated-out bit was old b0), Z/N/H clear
    }

    // ---- Family SLA r (CB 0x20-0x27; note 02c seed Opcodes.json: [8 T = 2 M], z00c) ----

    #[test]
    fn c01_25_sla_h_register_form_sets_z_and_c() {
        let mut bus = testutil::new_bus();
        let mut cpu = Cpu {
            h: 0x80,
            f: 0xF0, // all flags set before the instruction; Z/N/H must be recomputed
            ..Cpu::default()
        };

        let ticks = testutil::exec(&mut cpu, &mut bus, &[0xCB, 0x24]); // SLA H (register index 4)

        assert_eq!(ticks, 2);
        assert_eq!(cpu.h, 0x00); // $80 shifted left -> $00 (b0 forced to 0)
        assert_eq!(cpu.f, 0x90); // Z set (result zero) and C set (rotated-out bit was old b7)
    }

    #[test]
    fn c01_25_sla_zero_result_sets_z_only() {
        let mut bus = testutil::new_bus();
        let mut cpu = Cpu {
            b: 0x00,
            f: 0xF0, // all flags set before the instruction; N/H/C must be recomputed clear
            ..Cpu::default()
        };

        let ticks = testutil::exec(&mut cpu, &mut bus, &[0xCB, 0x20]); // SLA B (register index 0)

        assert_eq!(ticks, 2);
        assert_eq!(cpu.b, 0x00); // $00 shifted left stays zero
        assert_eq!(cpu.f, 0x80); // Z set only: result zero and rotated-out bit (old b7) clear
    }

    // ---- Family SRA r (CB 0x28-0x2F; note 02c seed Opcodes.json: [8 T = 2 M], z00c) ----

    #[test]
    fn c01_25_sra_l_preserves_sign_bit() {
        let mut bus = testutil::new_bus();
        let mut cpu = Cpu {
            l: 0x80,
            f: 0xF0, // all flags set before the instruction; Z/N/H must be recomputed
            ..Cpu::default()
        };

        let ticks = testutil::exec(&mut cpu, &mut bus, &[0xCB, 0x2D]); // SRA L (register index 5)

        assert_eq!(ticks, 2);
        assert_eq!(cpu.l, 0xC0); // sign preserved: ($80 & $80) | ($80 >> 1) = $C0
        assert_eq!(cpu.f, 0x00); // Z/N/H/C all clear (result nonzero, rotated-out bit old b0 = 0)
    }

    // ---- Family SWAP r (CB 0x30-0x37; note 02c seed Opcodes.json: [8 T = 2 M], z000) ----

    #[test]
    fn c01_25_swap_c_clears_carry_flag() {
        let mut bus = testutil::new_bus();
        let mut cpu = Cpu {
            c: 0xAB,
            f: 0x10, // carry set before the instruction; SWAP must clear it
            ..Cpu::default()
        };

        let ticks = testutil::exec(&mut cpu, &mut bus, &[0xCB, 0x31]); // SWAP C (register index 1)

        assert_eq!(ticks, 2);
        assert_eq!(cpu.c, 0xBA); // nibbles exchanged: $AB -> $BA
        assert_eq!(cpu.f, 0x00); // Z/N/H clear and C cleared by SWAP (unlike the other ops)
    }

    #[test]
    fn c01_25_swap_zero_result_sets_z_and_clears_c() {
        let mut bus = testutil::new_bus();
        let mut cpu = Cpu {
            b: 0x00,
            f: 0xF0, // all flags set before the instruction; N/H/C must be recomputed clear
            ..Cpu::default()
        };

        let ticks = testutil::exec(&mut cpu, &mut bus, &[0xCB, 0x30]); // SWAP B (register index 0)

        assert_eq!(ticks, 2);
        assert_eq!(cpu.b, 0x00); // $00 swapped stays zero
        assert_eq!(cpu.f, 0x80); // Z set only: result zero and C cleared by SWAP
    }

    // ---- Family SRL r (CB 0x38-0x3F; note 02c seed Opcodes.json: [8 T = 2 M], z00c) ----

    #[test]
    fn c01_25_srl_a_register_form_costs_two_ticks() {
        let mut bus = testutil::new_bus();
        let mut cpu = Cpu {
            a: 0x81,
            f: 0xF0, // all flags set before the instruction; Z/N/H must be recomputed
            ..Cpu::default()
        };

        let ticks = testutil::exec(&mut cpu, &mut bus, &[0xCB, 0x3F]); // SRL A (register index 7)

        assert_eq!(ticks, 2);
        assert_eq!(cpu.a, 0x40); // $81 shifted right -> $40 (b7 forced to 0)
        assert_eq!(cpu.f, 0x10); // C set (rotated-out bit was old b0), Z/N/H clear
    }

    // ---- (HL) forms: note 02c seed Opcodes.json [16 T = 4 M]; read then write back ----

    #[test]
    fn c01_25_rlc_hl_memory_form_costs_four_ticks() {
        let mut bus = testutil::new_bus();
        bus.write(0xC100, 0x80); // (HL) target cell
        let mut cpu = Cpu {
            h: 0xC1,
            l: 0x00,
            f: 0xF0, // all flags set before the instruction; Z/N/H must be recomputed
            ..Cpu::default()
        };

        let ticks = testutil::exec(&mut cpu, &mut bus, &[0xCB, 0x06]); // RLC (HL)

        assert_eq!(ticks, 4); // (HL) form: prefix + second byte + read (HL) + write (HL)
        assert_eq!(bus.peek(0xC100), 0x01); // $80 rotated left -> $01 written back to memory
        assert_eq!(cpu.pc, 0xC002); // PC advanced past the CB prefix and the second byte only
        assert_eq!(cpu.f, 0x10); // C set (rotated-out bit was old b7), Z/N/H clear
    }

    #[test]
    fn c01_25_swap_hl_memory_form_clears_carry() {
        let mut bus = testutil::new_bus();
        bus.write(0xC100, 0xAB); // (HL) target cell
        let mut cpu = Cpu {
            h: 0xC1,
            l: 0x00,
            f: 0x10, // carry set before the instruction; SWAP must clear it
            ..Cpu::default()
        };

        let ticks = testutil::exec(&mut cpu, &mut bus, &[0xCB, 0x36]); // SWAP (HL)

        assert_eq!(ticks, 4);
        assert_eq!(bus.peek(0xC100), 0xBA); // nibbles exchanged and written back to memory
        assert_eq!(cpu.pc, 0xC002);
        assert_eq!(cpu.f, 0x00); // Z/N/H clear and C cleared by SWAP
    }

    #[test]
    fn c01_25_srl_hl_memory_form_costs_four_ticks() {
        let mut bus = testutil::new_bus();
        bus.write(0xC100, 0x81); // (HL) target cell
        let mut cpu = Cpu {
            h: 0xC1,
            l: 0x00,
            ..Cpu::default()
        };

        let ticks = testutil::exec(&mut cpu, &mut bus, &[0xCB, 0x3E]); // SRL (HL)

        assert_eq!(ticks, 4);
        assert_eq!(bus.peek(0xC100), 0x40); // $81 shifted right -> $40 written back to memory
        assert_eq!(cpu.pc, 0xC002);
        assert_eq!(cpu.f, 0x10); // C set (rotated-out bit was old b0), Z/N/H clear
    }

    // ---- Pure flag/value function: boundaries of each op ----

    #[test]
    fn c01_25_cb_shift_op_boundaries() {
        assert_eq!(cb_shift_op(0x00, 0x80, false), (0x01, 0x10)); // RLC $80 -> $01, C set
        assert_eq!(cb_shift_op(0x0F, 0x80, false), (0x40, 0x00)); // RRC $80 -> $40, C clear (old b0 was 0)
        assert_eq!(cb_shift_op(0x13, 0x80, true), (0x01, 0x10)); // RL $80 with C in -> $01, C out
        assert_eq!(cb_shift_op(0x1A, 0x01, true), (0x80, 0x10)); // RR $01 with C in -> $80, C out
        assert_eq!(cb_shift_op(0x24, 0x80, false), (0x00, 0x90)); // SLA $80 -> $00, Z and C set
        assert_eq!(cb_shift_op(0x2D, 0x80, false), (0xC0, 0x00)); // SRA $80 -> $C0, sign kept
        assert_eq!(cb_shift_op(0x31, 0xAB, true), (0xBA, 0x00)); // SWAP $AB -> $BA, C cleared
        assert_eq!(cb_shift_op(0x3F, 0x81, false), (0x40, 0x10)); // SRL $81 -> $40, C set
    }
}
