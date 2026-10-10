//! Single-byte accumulator operations (task C01_22): RLCA/RRCA/RLA/RRA/CPL/SCF/CCF.
//! Decoded by opcode bit fields per decision C_00; no table. All seven are 4 T = 1 M-cycle,
//! so the fetch is the whole instruction (no bus access).

use super::Cpu;
use crate::bus::Bus;

impl Cpu {
    /// Single-byte accumulator operations (task C01_22). Returns true when the current opcode
    /// is in this group, false otherwise. Timings and flag effects from note 02c (seed
    /// Opcodes.json) and pandocs historical L2189/L2199-2202/L2229-2230: all seven are
    /// 4 T = 1 M-cycle, so the fetch is the whole instruction (no bus access).
    /// - RLCA ($07) / RRCA ($0F): rotate A left/right; C = rotated-out bit; Z/N/H cleared.
    /// - RLA ($17) / RRA ($1F): rotate A left/right THROUGH carry; the old C is shifted in;
    ///   C = rotated-out bit; Z/N/H cleared (unlike the CB forms, which set Z).
    /// - CPL ($2F): A = ~A; Z and C unchanged, N and H set.
    /// - SCF ($37): set C; A unchanged; Z unchanged, N/H cleared.
    /// - CCF ($3F): complement C; A unchanged; Z unchanged, N/H cleared.
    pub(super) fn exec_acc_misc(&mut self, _bus: &mut Bus) -> bool {
        let op = self.opcode;

        // Block 0, z == 7: the single-byte accumulator ops (y selects which one). DAA ($27,
        // y == 4) is NOT in this group and falls through to unimplemented.
        if x_is(op, 0) && z_is(op, 7) {
            match y_of(op) {
                0 => {
                    let (new_a, nf) = rlca(self.a); // RLCA
                    self.a = new_a;
                    self.f = nf;
                    self.done();
                    return true;
                }
                1 => {
                    let (new_a, nf) = rrca(self.a); // RRCA
                    self.a = new_a;
                    self.f = nf;
                    self.done();
                    return true;
                }
                2 => {
                    let (new_a, nf) = rla(self.a, self.f); // RLA
                    self.a = new_a;
                    self.f = nf;
                    self.done();
                    return true;
                }
                3 => {
                    let (new_a, nf) = rra(self.a, self.f); // RRA
                    self.a = new_a;
                    self.f = nf;
                    self.done();
                    return true;
                }
                5 => {
                    let (new_a, nf) = cpl(self.a, self.f); // CPL
                    self.a = new_a;
                    self.f = nf;
                    self.done();
                    return true;
                }
                6 => {
                    self.f = scf(self.f); // SCF: A unchanged
                    self.done();
                    return true;
                }
                7 => {
                    self.f = ccf(self.f); // CCF: A unchanged
                    self.done();
                    return true;
                }
                _ => return false, // y == 4 is DAA ($27), not in this group (task C01_22)
            }
        }

        false
    }
}

// ---- Pure flag/result functions (decision C_00: testable without a bus) ----

/// RLCA ($07): rotate A left; bit 7 wraps to bit 0, C = old bit 7. Z/N/H always cleared
/// (pandocs historical L2199: 000c).
pub(super) fn rlca(a: u8) -> (u8, u8) {
    let new_a = a.rotate_left(1); // rotate left by one (bit 7 wraps to bit 0)
    let nf = if a & 0x80 != 0 { 0x10 } else { 0 }; // C = old bit 7; Z/N/H clear
    (new_a, nf)
}

/// RRCA ($0F): rotate A right; bit 0 wraps to bit 7, C = old bit 0. Z/N/H always cleared
/// (pandocs historical L2201: 000c).
pub(super) fn rrca(a: u8) -> (u8, u8) {
    let new_a = a.rotate_right(1); // rotate right by one (bit 0 wraps to bit 7)
    let nf = if a & 0x01 != 0 { 0x10 } else { 0 }; // C = old bit 0; Z/N/H clear
    (new_a, nf)
}

/// RLA ($17): rotate A left THROUGH carry; the old C is shifted in at bit 0. C = old bit 7;
/// Z/N/H always cleared (pandocs historical L2200: 000c).
pub(super) fn rla(a: u8, f: u8) -> (u8, u8) {
    let carry_in = if f & 0x10 != 0 { 1 } else { 0 }; // old C shifted in at bit 0
    let new_a = (a << 1) | carry_in;
    let nf = if a & 0x80 != 0 { 0x10 } else { 0 }; // C = old bit 7; Z/N/H clear
    (new_a, nf)
}

/// RRA ($1F): rotate A right THROUGH carry; the old C is shifted in at bit 7. C = old bit 0;
/// Z/N/H always cleared (pandocs historical L2202: 000c).
pub(super) fn rra(a: u8, f: u8) -> (u8, u8) {
    let carry_in = if f & 0x10 != 0 { 0x80 } else { 0 }; // old C shifted in at bit 7
    let new_a = (a >> 1) | carry_in;
    let nf = if a & 0x01 != 0 { 0x10 } else { 0 }; // C = old bit 0; Z/N/H clear
    (new_a, nf)
}

/// CPL ($2F): A = ~A. Z and C unchanged, N and H set (pandocs historical L2189: -11-).
pub(super) fn cpl(a: u8, f: u8) -> (u8, u8) {
    let new_a = a ^ 0xFF; // complement all eight bits
    let nf = (f & 0x90) | 0x60; // Z and C preserved, N and H set
    (new_a, nf)
}

/// SCF ($37): set the carry flag. A unchanged; Z unchanged, N/H cleared, C set
/// (pandocs historical L2230: -001).
pub(super) fn scf(f: u8) -> u8 {
    (f & 0x80) | 0x10 // Z preserved, N and H cleared, C set
}

/// CCF ($3F): complement the carry flag. A unchanged; Z unchanged, N/H cleared, C toggled
/// (pandocs historical L2229: -00c).
pub(super) fn ccf(f: u8) -> u8 {
    let old_c = f & 0x10 != 0;
    (f & 0x80) | if old_c { 0 } else { 0x10 } // Z preserved, N and H cleared, C toggled
}

// ---- Opcode bit-field helpers (decision C_00: x = op>>6, y = (op>>3)&7, z = op&7) ----

fn x_is(op: u8, v: u8) -> bool {
    op >> 6 == v
}
fn y_of(op: u8) -> u8 {
    (op >> 3) & 7
}
fn z_is(op: u8, v: u8) -> bool {
    op & 7 == v
}

#[cfg(test)]
mod tests {
    use crate::cpu::{testutil, Cpu};

    // ---- Family RLCA ($07; note 02c seed Opcodes.json: 4 T = 1 M-cycle, flags 000c) ----

    #[test]
    fn c01_22_rlca_costs_one_tick_and_sets_c_from_bit7() {
        let mut bus = testutil::new_bus();
        let mut cpu = Cpu {
            a: 0xA5, // bit 7 set
            f: 0xF0, // all four flags set before the instruction; Z/N/H must be cleared
            ..Cpu::default()
        };

        let ticks = testutil::exec(&mut cpu, &mut bus, &[0x07]); // RLCA

        assert_eq!(ticks, 1); // note 02c: 4 T = 1 M-cycle (the fetch is the whole instruction)
        assert_eq!(cpu.a, 0x4B); // $A5 rotated left: ($A5 << 1) | ($A5 >> 7) = $4A | 1
        assert_eq!(cpu.pc, 0xC001); // PC advanced past the opcode
        assert_eq!(cpu.f, 0x10); // C set (old bit 7 was 1), Z/N/H cleared
    }

    #[test]
    fn c01_22_rlca_wraparound_clears_c_when_bit7_is_zero() {
        let mut bus = testutil::new_bus();
        let mut cpu = Cpu {
            a: 0x80, // bit 7 set, all other bits zero
            ..Cpu::default()
        };

        let ticks = testutil::exec(&mut cpu, &mut bus, &[0x07]); // RLCA

        assert_eq!(ticks, 1);
        assert_eq!(cpu.a, 0x01); // $80 rotated left wraps to $01 (edge: single-bit value)
        assert_eq!(cpu.f, 0x10); // C set from old bit 7; Z/N/H clear
    }

    // ---- Family RRCA ($0F; note 02c seed Opcodes.json: 4 T = 1 M-cycle, flags 000c) ----

    #[test]
    fn c01_22_rrca_costs_one_tick_and_sets_c_from_bit0() {
        let mut bus = testutil::new_bus();
        let mut cpu = Cpu {
            a: 0x55, // bit 0 set
            f: 0xF0, // all four flags set before the instruction; Z/N/H must be cleared
            ..Cpu::default()
        };

        let ticks = testutil::exec(&mut cpu, &mut bus, &[0x0F]); // RRCA

        assert_eq!(ticks, 1); // note 02c: 4 T = 1 M-cycle
        assert_eq!(cpu.a, 0xAA); // $55 rotated right: ($55 >> 1) | (($55 & 1) << 7) = $2A | $80
        assert_eq!(cpu.pc, 0xC001); // PC advanced past the opcode
        assert_eq!(cpu.f, 0x10); // C set (old bit 0 was 1), Z/N/H cleared
    }

    #[test]
    fn c01_22_rrca_wraparound_sets_bit7_from_old_bit0() {
        let mut bus = testutil::new_bus();
        let mut cpu = Cpu {
            a: 0x81, // bit 7 and bit 0 both set; the LSB must wrap into MSB
            ..Cpu::default()
        };

        let ticks = testutil::exec(&mut cpu, &mut bus, &[0x0F]); // RRCA

        assert_eq!(ticks, 1);
        assert_eq!(cpu.a, 0xC0); // $81 rotated right: ($81 >> 1) | (($81 & 1) << 7) = $40 | $80 (edge: wrap-around)
        assert_eq!(cpu.f, 0x10); // C set (old bit 0 was 1), Z/N/H clear
    }

    // ---- Family RLA ($17; note 02c seed Opcodes.json: 4 T = 1 M-cycle, flags 000c) ----

    #[test]
    fn c01_22_rla_costs_one_tick_and_shifts_old_c_in() {
        let mut bus = testutil::new_bus();
        let mut cpu = Cpu {
            a: 0xA5, // bit 7 set
            f: 0xF0, // all four flags set (C set) before the instruction; Z/N/H must be cleared
            ..Cpu::default()
        };

        let ticks = testutil::exec(&mut cpu, &mut bus, &[0x17]); // RLA

        assert_eq!(ticks, 1); // note 02c: 4 T = 1 M-cycle
        assert_eq!(cpu.a, 0x4B); // $A5 rotated left through carry: ($A5 << 1) | old C(=1) = $4B
        assert_eq!(cpu.pc, 0xC001); // PC advanced past the opcode
        assert_eq!(cpu.f, 0x10); // C set (old bit 7 was 1), Z/N/H cleared
    }

    #[test]
    fn c01_22_rla_with_c_clear_does_not_shift_in() {
        let mut bus = testutil::new_bus();
        let mut cpu = Cpu {
            a: 0xFF, // all bits set; old bit 7 is 1
            f: 0x00, // C clear before the instruction
            ..Cpu::default()
        };

        let ticks = testutil::exec(&mut cpu, &mut bus, &[0x17]); // RLA

        assert_eq!(ticks, 1);
        assert_eq!(cpu.a, 0xFE); // $FF rotated left through carry with C clear: ($FF << 1) | 0 = $FE
        assert_eq!(cpu.f, 0x10); // C set (old bit 7 was 1), Z/N/H clear
    }

    // ---- Family RRA ($1F; note 02c seed Opcodes.json: 4 T = 1 M-cycle, flags 000c) ----

    #[test]
    fn c01_22_rra_costs_one_tick_and_shifts_old_c_in() {
        let mut bus = testutil::new_bus();
        let mut cpu = Cpu {
            a: 0x55, // bit 0 set
            f: 0xF0, // all four flags set (C set) before the instruction; Z/N/H must be cleared
            ..Cpu::default()
        };

        let ticks = testutil::exec(&mut cpu, &mut bus, &[0x1F]); // RRA

        assert_eq!(ticks, 1); // note 02c: 4 T = 1 M-cycle
        assert_eq!(cpu.a, 0xAA); // $55 rotated right through carry: ($55 >> 1) | old C(=$80) = $AA
        assert_eq!(cpu.pc, 0xC001); // PC advanced past the opcode
        assert_eq!(cpu.f, 0x10); // C set (old bit 0 was 1), Z/N/H cleared
    }

    #[test]
    fn c01_22_rra_with_c_clear_does_not_shift_in() {
        let mut bus = testutil::new_bus();
        let mut cpu = Cpu {
            a: 0x02, // bit 0 clear; old bit 7 is 0
            f: 0x00, // C clear before the instruction
            ..Cpu::default()
        };

        let ticks = testutil::exec(&mut cpu, &mut bus, &[0x1F]); // RRA

        assert_eq!(ticks, 1);
        assert_eq!(cpu.a, 0x01); // $02 rotated right through carry with C clear: ($02 >> 1) | 0 = $01
        assert_eq!(cpu.f, 0x00); // C clear (old bit 0 was 0), Z/N/H clear
    }

    // ---- Family CPL ($2F; note 02c seed Opcodes.json: 4 T = 1 M-cycle, flags -11-) ----

    #[test]
    fn c01_22_cpl_costs_one_tick_and_complements_a() {
        let mut bus = testutil::new_bus();
        let mut cpu = Cpu {
            a: 0x55,
            f: 0xF0, // all four flags set before the instruction; Z and C must be preserved
            ..Cpu::default()
        };

        let ticks = testutil::exec(&mut cpu, &mut bus, &[0x2F]); // CPL

        assert_eq!(ticks, 1); // note 02c: 4 T = 1 M-cycle
        assert_eq!(cpu.a, 0xAA); // $55 complemented to $AA
        assert_eq!(cpu.pc, 0xC001); // PC advanced past the opcode
        assert_eq!(cpu.f, 0xF0); // Z and C preserved (both were set), N and H set
    }

    #[test]
    fn c01_22_cpl_does_not_recompute_z_even_when_result_is_zero() {
        let mut bus = testutil::new_bus();
        let mut cpu = Cpu {
            a: 0xFF, // complements to $00
            f: 0x10, // Z clear (only C set) before the instruction
            ..Cpu::default()
        };

        let ticks = testutil::exec(&mut cpu, &mut bus, &[0x2F]); // CPL

        assert_eq!(ticks, 1);
        assert_eq!(cpu.a, 0x00); // $FF complemented to $00 (edge: result is zero)
        assert_eq!(cpu.f, 0x70); // Z stays clear (not recomputed), N and H set, C preserved
    }

    // ---- Family SCF ($37; note 02c seed Opcodes.json: 4 T = 1 M-cycle, flags -001) ----

    #[test]
    fn c01_22_scf_costs_one_tick_and_sets_c() {
        let mut bus = testutil::new_bus();
        let mut cpu = Cpu {
            a: 0x5A, // A must be unchanged
            f: 0xF0, // all four flags set before the instruction; Z preserved, N/H cleared
            ..Cpu::default()
        };

        let ticks = testutil::exec(&mut cpu, &mut bus, &[0x37]); // SCF

        assert_eq!(ticks, 1); // note 02c: 4 T = 1 M-cycle
        assert_eq!(cpu.a, 0x5A); // A unchanged
        assert_eq!(cpu.pc, 0xC001); // PC advanced past the opcode
        assert_eq!(cpu.f, 0x90); // Z preserved (was set), N/H cleared, C set
    }

    #[test]
    fn c01_22_scf_with_all_flags_clear_sets_only_c() {
        let mut bus = testutil::new_bus();
        let mut cpu = Cpu {
            f: 0x00, // all flags clear before the instruction
            ..Cpu::default()
        };

        let ticks = testutil::exec(&mut cpu, &mut bus, &[0x37]); // SCF

        assert_eq!(ticks, 1);
        assert_eq!(cpu.f, 0x10); // only C set (edge: flag boundary from all-clear)
    }

    // ---- Family CCF ($3F; note 02c seed Opcodes.json: 4 T = 1 M-cycle, flags -00c) ----

    #[test]
    fn c01_22_ccf_costs_one_tick_and_toggles_c() {
        let mut bus = testutil::new_bus();
        let mut cpu = Cpu {
            a: 0x5A, // A must be unchanged
            f: 0xF0, // all four flags set (C set) before the instruction; Z preserved, N/H cleared
            ..Cpu::default()
        };

        let ticks = testutil::exec(&mut cpu, &mut bus, &[0x3F]); // CCF

        assert_eq!(ticks, 1); // note 02c: 4 T = 1 M-cycle
        assert_eq!(cpu.a, 0x5A); // A unchanged
        assert_eq!(cpu.pc, 0xC001); // PC advanced past the opcode
        assert_eq!(cpu.f, 0x80); // Z preserved (was set), N/H cleared, C toggled to clear
    }

    #[test]
    fn c01_22_ccf_with_c_clear_sets_only_z() {
        let mut bus = testutil::new_bus();
        let mut cpu = Cpu {
            f: 0x90, // Z and C set before the instruction; C must toggle to clear
            ..Cpu::default()
        };

        let ticks = testutil::exec(&mut cpu, &mut bus, &[0x3F]); // CCF

        assert_eq!(ticks, 1);
        assert_eq!(cpu.f, 0x80); // Z preserved (was set), N/H cleared, C toggled to clear
    }
}
