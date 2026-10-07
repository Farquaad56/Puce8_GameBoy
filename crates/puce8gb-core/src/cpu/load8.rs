//! Register-only 8-bit loads (task C01_05): NOP, LD r,r' and LD r,n for
//! r, r' in B C D E H L A. Decoded by opcode bit fields per decision C_00; no table.

use super::Cpu;
use crate::bus::Bus;

impl Cpu {
    /// Register-only 8-bit loads (task C01_05). Returns true when the current opcode is in
    /// this group, false otherwise. Timings and flag effects from note 02c:
    /// - NOP ($00): 4 T = 1 M-cycle, no flags affected.
    /// - LD r,r' (block 1, destination and source both plain registers, not (HL)): 4 T =
    ///   1 M-cycle, no flags affected.
    /// - LD r,n (block 0, z == 6, source not (HL)): 8 T = 2 M-cycles, no flags affected; the
    ///   immediate is read on step 2 (at most one bus access per decision C_00).
    pub(super) fn exec_load8_reg(&mut self, bus: &mut Bus) -> bool {
        let op = self.opcode;

        // NOP ($00): the fetch M-cycle is the whole instruction.
        if op == 0x00 {
            self.done();
            return true;
        }

        let x = op >> 6;
        let y = (op >> 3) & 7;
        let z = op & 7;

        // LD r,n: block 0, immediate source, destination is a plain register.
        if x == 0 && z == 6 && y != 6 {
            match self.step {
                // Step 1 is the fetch M-cycle (decision C_00): no bus access yet.
                1 => return true,
                _ => {
                    let value = bus.read(self.pc);
                    self.pc = self.pc.wrapping_add(1);
                    self.store_r8(y, value);
                    self.done();
                    return true;
                }
            }
        }

        // LD r,r': block 1, both destination and source are plain registers (not (HL)).
        if x == 1 && y != 6 && z != 6 {
            let value = self.read_r8(z);
            self.store_r8(y, value);
            self.done();
            return true;
        }

        false
    }

    /// Read a plain 8-bit register by its bit-field index (0=B .. 5=L, 7=A). Index 6 is the
    /// (HL) form and is never passed here.
    fn read_r8(&self, idx: u8) -> u8 {
        match idx {
            0 => self.b,
            1 => self.c,
            2 => self.d,
            3 => self.e,
            4 => self.h,
            5 => self.l,
            _ => self.a,
        }
    }

    /// Write a plain 8-bit register by its bit-field index (0=B .. 5=L, 7=A).
    fn store_r8(&mut self, idx: u8, value: u8) {
        match idx {
            0 => self.b = value,
            1 => self.c = value,
            2 => self.d = value,
            3 => self.e = value,
            4 => self.h = value,
            5 => self.l = value,
            _ => self.a = value,
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::cpu::{testutil, Cpu};

    // ---- Family 1: NOP (note 02c spot-checks: $00 NOP [4 T = 1 M], no flags) ----

    #[test]
    fn c01_05_nop_costs_one_tick_and_preserves_flags() {
        let mut bus = testutil::new_bus();
        let mut cpu = Cpu {
            f: 0xF0,
            ..Cpu::default()
        }; // all four flags set

        let ticks = testutil::exec(&mut cpu, &mut bus, &[0x00]);

        assert_eq!(ticks, 1);
        assert_eq!(cpu.pc, 0xC001); // PC advanced past the opcode
        assert_eq!(cpu.f, 0xF0); // flags unchanged (low nibble already zero)
    }

    #[test]
    fn c01_05_nop_masks_f_low_nibble() {
        // Flag boundary: a dirty low nibble of F is re-masked at instruction end.
        let mut bus = testutil::new_bus();
        let mut cpu = Cpu {
            f: 0xFF,
            ..Cpu::default()
        };

        let ticks = testutil::exec(&mut cpu, &mut bus, &[0x00]);

        assert_eq!(ticks, 1);
        assert_eq!(cpu.f, 0xF0); // high nibble preserved, low nibble masked to zero
    }

    // ---- Family 2: LD r,r' (note 02c: block 1 register loads [4 T = 1 M], no flags) ----

    #[test]
    fn c01_05_ld_rr_b_c_costs_one_tick() {
        let mut bus = testutil::new_bus();
        let mut cpu = Cpu {
            b: 0x00,
            c: 0x35,
            f: 0x80,
            ..Cpu::default()
        };

        let ticks = testutil::exec(&mut cpu, &mut bus, &[0x41]); // LD B,C

        assert_eq!(ticks, 1);
        assert_eq!(cpu.b, 0x35); // destination now holds the source value
        assert_eq!(cpu.c, 0x35); // source unchanged
        assert_eq!(cpu.pc, 0xC001);
        assert_eq!(cpu.f, 0x80); // flags unchanged (Z preserved)
    }

    #[test]
    fn c01_05_ld_rr_self_copy_is_a_valid_instruction() {
        // Edge case: r == r' ($40 LD B,B) is in the list; 1 M-cycle, no flags.
        let mut bus = testutil::new_bus();
        let mut cpu = Cpu {
            b: 0x7E,
            f: 0x30,
            ..Cpu::default()
        };

        let ticks = testutil::exec(&mut cpu, &mut bus, &[0x40]); // LD B,B

        assert_eq!(ticks, 1);
        assert_eq!(cpu.b, 0x7E);
        assert_eq!(cpu.f, 0x30); // H and C preserved
    }

    #[test]
    fn c01_05_ld_rr_a_from_h() {
        let mut bus = testutil::new_bus();
        let mut cpu = Cpu {
            h: 0x2A,
            a: 0x00,
            ..Cpu::default()
        };

        let ticks = testutil::exec(&mut cpu, &mut bus, &[0x7C]); // LD A,H

        assert_eq!(ticks, 1);
        assert_eq!(cpu.a, 0x2A);
    }

    // ---- Family 3: LD r,n (note 02c: block 0 immediate loads [8 T = 2 M], no flags) ----

    #[test]
    fn c01_05_ld_rn_a_n_costs_two_ticks() {
        let mut bus = testutil::new_bus();
        let mut cpu = Cpu {
            a: 0x00,
            f: 0x10,
            ..Cpu::default()
        };

        let ticks = testutil::exec(&mut cpu, &mut bus, &[0x3E, 0xAB]); // LD A,$AB

        assert_eq!(ticks, 2);
        assert_eq!(cpu.a, 0xAB);
        assert_eq!(cpu.pc, 0xC002); // PC advanced past opcode and immediate
        assert_eq!(cpu.f, 0x10); // flags unchanged (C preserved)
    }

    #[test]
    fn c01_05_ld_rn_b_n() {
        let mut bus = testutil::new_bus();
        let mut cpu = Cpu {
            b: 0x00,
            ..Cpu::default()
        };

        let ticks = testutil::exec(&mut cpu, &mut bus, &[0x06, 0x13]); // LD B,$13

        assert_eq!(ticks, 2);
        assert_eq!(cpu.b, 0x13);
    }

    #[test]
    fn c01_05_ld_rn_pc_wraps_at_top_of_memory() {
        // Edge case: the immediate straddles $FFFF -> $0000; the next fetch wraps.
        let mut bus = testutil::new_bus();
        bus.write(0xFFFE, 0x3E); // LD A,n at $FFFE
        bus.write(0xFFFF, 0x5A); // immediate at $FFFF
        let mut cpu = Cpu {
            pc: 0xFFFE,
            ..Cpu::default()
        };

        cpu.tick(&mut bus); // fetch opcode (step 1)
        assert!(!cpu.at_boundary());
        cpu.tick(&mut bus); // read the wrapping immediate (step 2), done
        assert!(cpu.at_boundary());

        assert_eq!(cpu.a, 0x5A);
        assert_eq!(cpu.pc, 0x0000); // wrapped past $FFFF
    }

    #[test]
    fn c01_05_ld_hl_n_still_unimplemented() {
        // Guard: $36 (LD (HL),n) is not in the C01_05 list; it must stay unimplemented.
        let mut bus = testutil::new_bus();
        let mut cpu = Cpu::default();

        let ticks = testutil::exec(&mut cpu, &mut bus, &[0x36]); // LD (HL),n

        assert_eq!(ticks, 1);
        assert_eq!(cpu.unimplemented(), Some((0x36, 0xC000)));
    }
}
