//! CB-prefixed single-bit family (task C01_26): BIT n,r|(HL), RES n,r|(HL), SET n,r|(HL).
//! Decoded by opcode bit fields per decision C_00; no table.
//!
//! Timings and flag effects from note 02c (seed Opcodes.json) and pandocs historical
//! L2218-2223: the CB second byte selects the op in its top two bits (`(op >> 6) & 3`,
//! where 1 = BIT, 2 = RES, 3 = SET), the bit number in `(op >> 3) & 7`, and the register
//! in `op & 7` (where 6 = (HL)). All three ops are in this group, so a valid second byte is
//! `>= $40` (anything `< $40` is another CB group: C01_25 rotate/shift). Register form =
//! 8 T = 2 M-cycles total; BIT n,(HL) = 12 T = 3 M-cycles (read only, no write-back);
//! RES/SET n,(HL) = 16 T = 4 M-cycles (read then write back). Flags: BIT sets Z = tested
//! bit is clear, N clear, H set, C unchanged; RES and SET change no flags. At most one bus
//! access per M-cycle (decision C_00).

use super::Cpu;
use crate::bus::Bus;

impl Cpu {
    /// CB-prefixed single-bit family (task C01_26): BIT/RES/SET on a register or on (HL).
    /// Returns true when the current opcode is in this group, false otherwise. See the module
    /// docs for timings and flag effects. A second byte `< $40` belongs to another CB group
    /// (C01_25 rotate/shift) and was already claimed upstream by `exec_cb_shift`. The second
    /// byte is read once by `exec_cb_shift` at step 2 and latched in `self.lo`; this group
    /// reuses that latch so the fetch happens exactly once across the whole CB instruction.
    pub(super) fn exec_cb_bit(&mut self, bus: &mut Bus) -> bool {
        // Only the CB prefix ($CB = x3 y1 z3) reaches here; no other group claims it.
        if self.opcode != 0xCB {
            return false;
        }

        match self.step {
            // Step 1 is the prefix fetch M-cycle (decision C_00): no bus access yet. Normally
            // claimed first by exec_cb_shift; kept so this group stays self-contained if the
            // dispatch order changes.
            1 => {
                self.cb = true;
                true
            }
            // Step 2: the second byte was already fetched and latched by exec_cb_shift (one
            // bus access, PC advanced). A single-bit op has a second byte >= $40; anything <
            // $40 is another CB group handled upstream.
            2 => {
                let cb_op = self.lo;
                if cb_op < 0x40 {
                    return false; // not this group: rotate/shift, already claimed upstream
                }
                if (cb_op & 7) != 6 {
                    // Register form: two M-cycles total. Apply now, no memory access.
                    let value = self.read_r8(cb_op & 7);
                    match (cb_op >> 6) & 3 {
                        1 => {
                            // BIT n,r: test only; set flags from the tested bit, do not write back.
                            self.f = cb_bit_flags(cb_op, value, self.f & 0x10 != 0);
                        }
                        _ => {
                            // RES/SET n,r: modify the register in place; no flag change.
                            self.store_r8(cb_op & 7, cb_bit_op(cb_op, value));
                        }
                    }
                    self.done();
                }
                true // (HL) form continues to step 3
            }
            _ => {
                // Steps 3-4: only reached for the (HL) form of a BIT/RES/SET op.
                if !self.cb || self.lo < 0x40 || (self.lo & 7) != 6 {
                    return false; // not this group's (HL) op
                }
                match self.step {
                    3 => {
                        let value = bus.read(self.hl()); // read (HL); one bus access
                        if (self.lo >> 6) & 3 == 1 {
                            // BIT n,(HL): set flags from the tested bit, no write-back. Total 3 M-cycles.
                            self.f = cb_bit_flags(self.lo, value, self.f & 0x10 != 0);
                            self.done();
                        } else {
                            self.hi = value; // RES/SET n,(HL): hold the read for the write-back step
                        }
                        true
                    }
                    _ => {
                        // RES/SET n,(HL): compute the new value and write back (HL). Total 4 M-cycles.
                        let result = cb_bit_op(self.lo, self.hi);
                        bus.write(self.hl(), result); // one bus access
                        self.done();
                        true
                    }
                }
            }
        }
    }
}

/// Apply a CB SET/RES op to a value (second byte >= $40). BIT is a no-op here; only RES and
/// SET modify the value. Pure so it can be tested without a bus (note 02c seed Opcodes.json:
/// set/res n,r [8 T = 2 M], set/res n,(HL) [16 T = 4 M]).
pub(super) fn cb_bit_op(op: u8, value: u8) -> u8 {
    let family = (op >> 6) & 3; // 1=BIT, 2=RES, 3=SET
    let bit = 1u8 << ((op >> 3) & 7);
    match family {
        2 => value & !bit, // RES n: clear the tested bit
        _ => value | bit,  // SET n (and BIT no-op): set the tested bit
    }
}

/// Flags after a CB BIT op (second byte >= $40, family 1). Z = tested bit is clear, N clear,
/// H set, C unchanged. Pure so it can be tested without a bus (note 02c seed Opcodes.json:
/// bit n,r [8 T = 2 M], bit n,(HL) [12 T = 3 M]; flags z01-).
pub(super) fn cb_bit_flags(op: u8, value: u8, carry_in: bool) -> u8 {
    let z_set = ((value >> ((op >> 3) & 7)) & 1) == 0; // Z set iff the tested bit is clear
    let mut f = 0x20u8; // H set
    if z_set {
        f |= 0x80; // Z
    }
    if carry_in {
        f |= 0x10; // C unchanged (preserved from before the instruction)
    }
    f
}

#[cfg(test)]
mod tests {
    use super::{cb_bit_flags, cb_bit_op};
    use crate::cpu::{testutil, Cpu};

    // ---- Family BIT n,r (CB 0x40-0x7F; note 02c seed Opcodes.json: [8 T = 2 M], z01-) ----

    #[test]
    fn c01_26_bit_a_register_form_costs_two_ticks_and_sets_z_when_bit_clear() {
        let mut bus = testutil::new_bus();
        let mut cpu = Cpu {
            a: 0x40, // bit 0 clear -> Z must be set
            f: 0xF0, // all flags set before the instruction; N/H recomputed, C preserved
            ..Cpu::default()
        };

        let ticks = testutil::exec(&mut cpu, &mut bus, &[0xCB, 0x47]); // BIT 0,A

        assert_eq!(ticks, 2); // register form: prefix fetch + second-byte fetch (note 02c)
        assert_eq!(cpu.a, 0x40); // BIT does not write back; A unchanged
        assert_eq!(cpu.pc, 0xC002); // PC advanced past the CB prefix and the second byte
        assert_eq!(cpu.f, 0xB0); // Z set (bit clear), N clear, H set, C preserved (was set)
    }

    #[test]
    fn c01_26_bit_a_register_form_clears_z_when_bit_set() {
        let mut bus = testutil::new_bus();
        let mut cpu = Cpu {
            a: 0x80, // bit 7 set -> Z must be clear
            f: 0xF0, // all flags set before the instruction; N/H recomputed, C preserved
            ..Cpu::default()
        };

        let ticks = testutil::exec(&mut cpu, &mut bus, &[0xCB, 0x7F]); // BIT 7,A

        assert_eq!(ticks, 2);
        assert_eq!(cpu.a, 0x80); // A unchanged (no write-back)
        assert_eq!(cpu.pc, 0xC002);
        assert_eq!(cpu.f, 0x30); // Z clear (bit set), N clear, H set, C preserved (was set)
    }

    // ---- Family BIT n,(HL) (CB 0x46/0x4E/...; note 02c seed Opcodes.json: [12 T = 3 M], z01-) ----

    #[test]
    fn c01_26_bit_hl_memory_form_costs_three_ticks_and_does_not_write_back() {
        let mut bus = testutil::new_bus();
        bus.write(0xC100, 0x40); // (HL) target cell; bit 0 clear -> Z set
        let mut cpu = Cpu {
            h: 0xC1,
            l: 0x00,
            f: 0xF0, // all flags set before the instruction; N/H recomputed, C preserved
            ..Cpu::default()
        };

        let ticks = testutil::exec(&mut cpu, &mut bus, &[0xCB, 0x46]); // BIT 0,(HL)

        assert_eq!(ticks, 3); // (HL) form: prefix + second byte + read (HL), no write-back
        assert_eq!(bus.peek(0xC100), 0x40); // memory unchanged: BIT does not write back
        assert_eq!(cpu.pc, 0xC002);
        assert_eq!(cpu.f, 0xB0); // Z set (bit clear), N clear, H set, C preserved
    }

    #[test]
    fn c01_26_bit_hl_memory_form_clears_z_when_bit_set() {
        let mut bus = testutil::new_bus();
        bus.write(0xC100, 0x80); // (HL) target cell; bit 7 set -> Z clear
        let mut cpu = Cpu {
            h: 0xC1,
            l: 0x00,
            f: 0xF0,
            ..Cpu::default()
        };

        let ticks = testutil::exec(&mut cpu, &mut bus, &[0xCB, 0x7E]); // BIT 7,(HL)

        assert_eq!(ticks, 3);
        assert_eq!(bus.peek(0xC100), 0x80); // memory unchanged
        assert_eq!(cpu.pc, 0xC002);
        assert_eq!(cpu.f, 0x30); // Z clear (bit set), N clear, H set, C preserved
    }

    // ---- Family RES n,r (CB 0x80-0xBF; note 02c seed Opcodes.json: [8 T = 2 M], no flags) ----

    #[test]
    fn c01_26_res_b_register_form_costs_two_ticks_and_clears_bit() {
        let mut bus = testutil::new_bus();
        let mut cpu = Cpu {
            b: 0xFF, // all bits set; RES 0 clears bit 0
            f: 0xF0, // flags must be left untouched by RES
            ..Cpu::default()
        };

        let ticks = testutil::exec(&mut cpu, &mut bus, &[0xCB, 0x80]); // RES 0,B

        assert_eq!(ticks, 2); // register form: prefix fetch + second-byte fetch (note 02c)
        assert_eq!(cpu.b, 0xFE); // bit 0 cleared: $FF -> $FE
        assert_eq!(cpu.pc, 0xC002);
        assert_eq!(cpu.f, 0xF0); // RES changes no flags; F left as it was
    }

    // ---- Family RES n,(HL) (CB 0x86/...; note 02c seed Opcodes.json: [16 T = 4 M], no flags) ----

    #[test]
    fn c01_26_res_hl_memory_form_costs_four_ticks_and_writes_back() {
        let mut bus = testutil::new_bus();
        bus.write(0xC100, 0xFF); // (HL) target cell; RES 0 clears bit 0
        let mut cpu = Cpu {
            h: 0xC1,
            l: 0x00,
            f: 0xF0,
            ..Cpu::default()
        };

        let ticks = testutil::exec(&mut cpu, &mut bus, &[0xCB, 0x86]); // RES 0,(HL)

        assert_eq!(ticks, 4); // (HL) form: prefix + second byte + read (HL) + write back
        assert_eq!(bus.peek(0xC100), 0xFE); // bit 0 cleared and written back to memory
        assert_eq!(cpu.pc, 0xC002);
        assert_eq!(cpu.f, 0xF0); // RES changes no flags
    }

    // ---- Family SET n,r (CB 0xC0-0xFF; note 02c seed Opcodes.json: [8 T = 2 M], no flags) ----

    #[test]
    fn c01_26_set_a_register_form_costs_two_ticks_and_sets_bit() {
        let mut bus = testutil::new_bus();
        let mut cpu = Cpu {
            a: 0x80, // bit 0 clear; SET 0 sets it
            f: 0xF0, // flags must be left untouched by SET
            ..Cpu::default()
        };

        let ticks = testutil::exec(&mut cpu, &mut bus, &[0xCB, 0xC7]); // SET 0,A

        assert_eq!(ticks, 2);
        assert_eq!(cpu.a, 0x81); // bit 0 set: $80 -> $81
        assert_eq!(cpu.pc, 0xC002);
        assert_eq!(cpu.f, 0xF0); // SET changes no flags; F left as it was
    }

    // ---- Family SET n,(HL) (CB 0xC6/...; note 02c seed Opcodes.json: [16 T = 4 M], no flags) ----

    #[test]
    fn c01_26_set_hl_memory_form_costs_four_ticks_and_writes_back() {
        let mut bus = testutil::new_bus();
        bus.write(0xC100, 0xFE); // (HL) target cell; bit 0 clear; SET 0 sets it -> $FF
        let mut cpu = Cpu {
            h: 0xC1,
            l: 0x00,
            f: 0xF0,
            ..Cpu::default()
        };

        let ticks = testutil::exec(&mut cpu, &mut bus, &[0xCB, 0xC6]); // SET 0,(HL)

        assert_eq!(ticks, 4);
        assert_eq!(bus.peek(0xC100), 0xFF); // bit 0 set and written back to memory
        assert_eq!(cpu.pc, 0xC002);
        assert_eq!(cpu.f, 0xF0); // SET changes no flags
    }

    // ---- Edge cases: wrap-around (top bit) and flag/value boundaries ----

    #[test]
    fn c01_26_set_bit_seven_wraparound_sets_top_bit() {
        let mut bus = testutil::new_bus();
        let mut cpu = Cpu {
            a: 0x7F, // bits 0-6 set; SET 7 wraps the top bit in
            ..Cpu::default()
        };

        let ticks = testutil::exec(&mut cpu, &mut bus, &[0xCB, 0xFF]); // SET 7,A (family 3, n=7, reg A)

        assert_eq!(ticks, 2);
        assert_eq!(cpu.a, 0xFF); // bit 7 set: $7F -> $FF (top bit wraps in)
    }

    #[test]
    fn c01_26_cb_bit_op_and_flags_boundaries() {
        // SET/RES value boundaries (pure function).
        assert_eq!(cb_bit_op(0xC7, 0x7E), 0x7F); // SET 0: $7E -> $7F (bit 0 set)
        assert_eq!(cb_bit_op(0xFF, 0x7F), 0xFF); // SET 7: $7F -> $FF (top bit wraps in)
        assert_eq!(cb_bit_op(0xC7, 0x7F), 0x7F); // SET 0 on a value whose bit 0 is already set: no-op
        assert_eq!(cb_bit_op(0xC0, 0x00), 0x01); // SET 0 on zero -> $01
        assert_eq!(cb_bit_op(0x80, 0x01), 0x00); // RES 0: clears the only set bit -> zero
        assert_eq!(cb_bit_op(0xB8, 0xFF), 0x7F); // RES 7: $FF -> $7F (top bit cleared)

        // BIT flag boundaries (z01-): Z set iff tested bit clear; H always set; C preserved.
        assert_eq!(cb_bit_flags(0x47, 0x40, false), 0xA0); // BIT 0,A: $40 bit0 clear -> Z+H set, C clear
        assert_eq!(cb_bit_flags(0x47, 0x41, true), 0x30); // BIT 0,A: $41 bit0 set -> Z clear; H set; C preserved (in)
        assert_eq!(cb_bit_flags(0x7F, 0x80, false), 0x20); // BIT 7,A: $80 bit7 set -> Z clear; H set only
    }
}
