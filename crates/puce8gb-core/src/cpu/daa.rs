//! DAA ($27): decimal adjust of the accumulator (task C01_23).
//! Decoded by opcode bit fields per decision C_00; no table. 4 T = 1 M-cycle,
//! so the fetch is the whole instruction (no bus access).

use super::Cpu;
use crate::bus::Bus;

impl Cpu {
    /// DAA ($27): decimal adjust of A from flags N/H/C (task C01_23). Returns true when
    /// the current opcode is in this group, false otherwise. 4 T = 1 M-cycle (note 02c
    /// seed Opcodes.json: bytes 1, cycles [4]; pandocs historical L2188), so the fetch is
    /// the whole instruction (no bus access). Flags z-0x (pandocs historical L2188): Z
    /// recomputed, N preserved, H cleared, C = carry out of the BCD correction.
    pub(super) fn exec_daa(&mut self, _bus: &mut Bus) -> bool {
        if self.opcode == 0x27 {
            // DAA (block 0, y == 4, z == 7; note 02a L80). Register work only.
            let n_flag = self.f & 0x40 != 0;
            let h_flag = self.f & 0x20 != 0;
            let c_flag = self.f & 0x10 != 0;

            let mut a = self.a;
            let mut adj: u8 = 0;
            let mut carry = c_flag;
            if !n_flag {
                // Addition: correct the low digit, then the high digit.
                if h_flag || (a & 0x0F) > 0x09 {
                    adj |= 0x06;
                }
                if c_flag || a > 0x99 {
                    adj |= 0x60;
                    carry = true;
                }
                a = a.wrapping_add(adj);
            } else {
                // Subtraction: undo the low-digit borrow, then the high-digit borrow.
                if h_flag {
                    adj |= 0x06;
                }
                if c_flag {
                    adj |= 0x60;
                }
                a = a.wrapping_sub(adj);
            }

            self.a = a;
            // Flags z-0x: Z recomputed, N preserved, H cleared, C = carry.
            let nf = (self.f & 0x40) | if a == 0 { 0x80 } else { 0 } | if carry { 0x10 } else { 0 };
            self.f = nf;
            self.done();
            return true;
        }

        false
    }
}

#[cfg(test)]
mod tests {
    use crate::cpu::{testutil, Cpu};

    // ---- Family DAA ($27; note 02c seed Opcodes.json: bytes 1, cycles [4] = 1 M-cycle, flags z-0x) ----

    #[test]
    fn c01_23_daa_add_no_flags_adjusts_low_digit() {
        let mut bus = testutil::new_bus();
        let mut cpu = Cpu {
            a: 0x0A, // low digit over $09 after an addition; N/H/C clear
            f: 0x00,
            ..Cpu::default()
        };

        let ticks = testutil::exec(&mut cpu, &mut bus, &[0x27]); // DAA

        assert_eq!(ticks, 1); // note 02c: 4 T = 1 M-cycle (the fetch is the whole instruction)
        assert_eq!(cpu.a, 0x10); // $0A + $06 = $10 (low-digit correction only)
        assert_eq!(cpu.pc, 0xC001); // PC advanced past the opcode
        assert_eq!(cpu.f, 0x00); // Z clear, N preserved (clear), H cleared, C clear
    }

    #[test]
    fn c01_23_daa_add_wrap_sets_c_and_z() {
        let mut bus = testutil::new_bus();
        let mut cpu = Cpu {
            a: 0x9A, // above $99 after an addition; N/H/C clear
            f: 0x00,
            ..Cpu::default()
        };

        let ticks = testutil::exec(&mut cpu, &mut bus, &[0x27]); // DAA

        assert_eq!(ticks, 1);
        assert_eq!(cpu.a, 0x00); // $9A + $66 wraps to $00 (edge: wrap-around)
        assert_eq!(cpu.pc, 0xC001);
        assert_eq!(cpu.f, 0x90); // Z set (result zero), N preserved (clear), H cleared, C set
    }

    #[test]
    fn c01_23_daa_add_with_h_adjusts_low_digit() {
        let mut bus = testutil::new_bus();
        let mut cpu = Cpu {
            a: 0x05, // low digit within $09 but H set (half carry of the addition)
            f: 0x20, // N clear, H set, C clear
            ..Cpu::default()
        };

        let ticks = testutil::exec(&mut cpu, &mut bus, &[0x27]); // DAA

        assert_eq!(ticks, 1);
        assert_eq!(cpu.a, 0x0B); // $05 + $06 = $0B (H forces the low-digit correction)
        assert_eq!(cpu.pc, 0xC001);
        assert_eq!(cpu.f, 0x00); // Z clear, N preserved (clear), H cleared, C clear
    }

    #[test]
    fn c01_23_daa_add_with_c_only_adjusts_high_digit() {
        let mut bus = testutil::new_bus();
        let mut cpu = Cpu {
            a: 0x00, // low digit fine; only the carry flag is set
            f: 0x10, // N clear, H clear, C set
            ..Cpu::default()
        };

        let ticks = testutil::exec(&mut cpu, &mut bus, &[0x27]); // DAA

        assert_eq!(ticks, 1);
        assert_eq!(cpu.a, 0x60); // $00 + $60 = $60 (high-digit correction only)
        assert_eq!(cpu.pc, 0xC001);
        assert_eq!(cpu.f, 0x10); // Z clear, N preserved (clear), H cleared, C set
    }

    #[test]
    fn c01_23_daa_sub_with_h_corrects_low_digit() {
        let mut bus = testutil::new_bus();
        let mut cpu = Cpu {
            a: 0x10, // low digit within $09 but H set (half borrow of the subtraction)
            f: 0x60, // N set, H set, C clear
            ..Cpu::default()
        };

        let ticks = testutil::exec(&mut cpu, &mut bus, &[0x27]); // DAA

        assert_eq!(ticks, 1);
        assert_eq!(cpu.a, 0x0A); // $10 - $06 = $0A (H forces the low-digit correction)
        assert_eq!(cpu.pc, 0xC001);
        assert_eq!(cpu.f, 0x40); // Z clear, N preserved (set), H cleared, C clear
    }

    #[test]
    fn c01_23_daa_sub_with_c_only_corrects_high_digit() {
        let mut bus = testutil::new_bus();
        let mut cpu = Cpu {
            a: 0x00, // low digit fine; only the borrow flag is set
            f: 0x50, // N set, H clear, C set
            ..Cpu::default()
        };

        let ticks = testutil::exec(&mut cpu, &mut bus, &[0x27]); // DAA

        assert_eq!(ticks, 1);
        assert_eq!(cpu.a, 0xA0); // $00 - $60 wraps to $A0 (edge: borrow wrap-around)
        assert_eq!(cpu.pc, 0xC001);
        assert_eq!(cpu.f, 0x50); // Z clear, N preserved (set), H cleared, C set
    }
}
