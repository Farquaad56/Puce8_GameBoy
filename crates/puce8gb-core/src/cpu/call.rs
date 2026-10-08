//! Subroutines (task C01_11): CALL a16, CALL cc,a16, RET, RET cc and RST n. Decoded by opcode
//! bit fields per decision C_00; no table.

use super::Cpu;
use crate::bus::Bus;

impl Cpu {
    /// Subroutines (task C01_11). Returns true when the current opcode is in this group, false
    /// otherwise. Timings and flag effects from note 02c (seed Opcodes.json):
    /// - CALL a16 ($CD; block 3, y == 1, z == 5): 3 bytes, 24 T = 6 M-cycles, no flags. The low
    ///   byte of a16 is read on step 2 and the high byte on step 3 (little-endian order); SP is
    ///   decremented by two on step 4 (an internal M-cycle with no bus access, decision C_00),
    ///   the high byte of the return address is stored at (SP) on step 5 and the low byte at
    ///   (SP)+1 on step 6, where PC is also set to a16. The return address is the instruction
    ///   after the three-byte CALL (PC + 3).
    /// - CALL cc,a16 ($C4/$CC/$D4/$DC; block 3, z == 4, bit5 clear so y < 4): 3 bytes. Taken:
    ///   24 T = 6 M-cycles; not taken: 12 T = 3 M-cycles. No flags. The low byte of a16 is read
    ///   on step 2 and the high byte on step 3 regardless of the condition; when the condition
    ///   (selected by y) is met, SP is decremented on step 4 and the return address pushed on
    ///   steps 5-6 with PC set to a16. When not met the instruction ends after three M-cycles
    ///   with PC advanced past the three bytes.
    /// - RET ($C9; block 3, y == 1, z == 1): 1 byte, 16 T = 4 M-cycles, no flags. The high byte
    ///   of the return address is read from (SP) on step 2 and the low byte from (SP)+1 on step
    ///   3; PC is set to the popped value and SP incremented by two on step 4.
    /// - RET cc ($C0/$C8/$D0/$D8; block 3, z == 0, bit5 clear so y < 4): 1 byte. Taken:
    ///   20 T = 5 M-cycles; not taken: 8 T = 2 M-cycles. No flags. The condition (selected by y)
    ///   is checked on step 2 with no bus access; when met, the return address is popped on steps
    ///   3-4 and PC set to it with SP incremented by two on step 5. When not met the instruction
    ///   ends after two M-cycles with PC advanced past the opcode.
    /// - RST n ($C7/$CF/$D7/$DF/$E7/$EF/$F7/$FF; block 3, z == 7): 1 byte, 16 T = 4 M-cycles,
    ///   no flags. SP is decremented by two on step 2 (an internal M-cycle with no bus access),
    ///   the high byte of the return address is stored at (SP) on step 3 and the low byte at
    ///   (SP)+1 on step 4, where PC is also set to the vector address n * 8 (n = y). The return
    ///   address is the instruction after the one-byte RST (PC + 1).
    pub(super) fn exec_call(&mut self, bus: &mut Bus) -> bool {
        let op = self.opcode;

        // CALL a16 ($CD): block 3, y == 1, z == 5. Six M-cycles total.
        if x_is(op, 3) && y_is(op, 1) && z_is(op, 5) {
            match self.step {
                // Step 1 is the fetch M-cycle (decision C_00): no bus access yet.
                1 => return true,
                // Step 2: read the low byte of a16 at PC; latch it for step 6.
                2 => {
                    self.lo = bus.read(self.pc);
                    self.pc = self.pc.wrapping_add(1);
                    return true;
                }
                // Step 3: read the high byte of a16 (little-endian order, note 02c). PC now
                // points past all three bytes, i.e. at the return address.
                3 => {
                    self.hi = bus.read(self.pc);
                    self.pc = self.pc.wrapping_add(1);
                    return true;
                }
                // Step 4: SP = SP - 2 (an internal M-cycle with no bus access, decision C_00).
                4 => {
                    self.sp = self.sp.wrapping_sub(2);
                    return true;
                }
                // Step 5: store the high byte of the return address at (SP) (one bus access).
                5 => {
                    let ret_addr = self.pc;
                    bus.write(self.sp, (ret_addr >> 8) as u8);
                    return true;
                }
                // Step 6: store the low byte of the return address at (SP)+1 (one bus access),
                // set PC to a16 (a register write, so no bus access), then end.
                _ => {
                    let ret_addr = self.pc;
                    let target = ((self.hi as u16) << 8) | self.lo as u16;
                    bus.write(self.sp.wrapping_add(1), (ret_addr & 0xFF) as u8);
                    self.pc = target; // CALL a16: PC = a16 (register write, no bus access)
                    self.done();
                    return true;
                }
            }
        }

        // CALL cc,a16 ($C4/$CC/$D4/$DC): block 3, z == 4, bit5 clear (y < 4). Taken: six M-
        // cycles; not taken: three. The condition index is y itself (0=NZ, 1=Z, 2=NC, 3=C).
        if x_is(op, 3) && z_is(op, 4) && !bit5_set(op) {
            match self.step {
                // Step 1 is the fetch M-cycle (decision C_00): no bus access yet.
                1 => return true,
                // Step 2: read the low byte of a16 at PC; latch it for step 6.
                2 => {
                    self.lo = bus.read(self.pc);
                    self.pc = self.pc.wrapping_add(1);
                    return true;
                }
                // Step 3: read the high byte of a16 (little-endian order, note 02c). PC now
                // points past all three bytes. When the condition is not met this is the last
                // M-cycle (not-taken = 12 T = 3 M); PC stays advanced past the instruction.
                3 => {
                    self.hi = bus.read(self.pc);
                    self.pc = self.pc.wrapping_add(1);
                    if !self.cond_met(y_of(op)) {
                        self.done(); // not taken: end after three M-cycles
                        return true;
                    }
                    return true;
                }
                // Step 4 (taken only): SP = SP - 2 (an internal M-cycle with no bus access).
                4 => {
                    self.sp = self.sp.wrapping_sub(2);
                    return true;
                }
                // Step 5 (taken only): store the high byte of the return address at (SP).
                5 => {
                    let ret_addr = self.pc;
                    bus.write(self.sp, (ret_addr >> 8) as u8);
                    return true;
                }
                // Step 6 (taken only): store the low byte at (SP)+1, set PC to a16, then end.
                _ => {
                    let ret_addr = self.pc;
                    let target = ((self.hi as u16) << 8) | self.lo as u16;
                    bus.write(self.sp.wrapping_add(1), (ret_addr & 0xFF) as u8);
                    self.pc = target; // taken: PC = a16 (register write, no bus access)
                    self.done();
                    return true;
                }
            }
        }

        // RET ($C9): block 3, y == 1, z == 1. Four M-cycles total.
        if x_is(op, 3) && y_is(op, 1) && z_is(op, 1) {
            match self.step {
                // Step 1 is the fetch M-cycle (decision C_00): no bus access yet.
                1 => return true,
                // Step 2: read the high byte of the return address from (SP); SP not advanced yet.
                2 => {
                    self.hi = bus.read(self.sp);
                    return true;
                }
                // Step 3: read the low byte of the return address from (SP)+1.
                3 => {
                    self.lo = bus.read(self.sp.wrapping_add(1));
                    return true;
                }
                // Step 4: PC = popped value, SP += 2 (register writes only), then end.
                _ => {
                    let value = ((self.hi as u16) << 8) | self.lo as u16;
                    self.pc = value; // RET: PC = (SP) (register write, no bus access)
                    self.sp = self.sp.wrapping_add(2);
                    self.done();
                    return true;
                }
            }
        }

        // RET cc ($C0/$C8/$D0/$D8): block 3, z == 0, bit5 clear (y < 4). Taken: five M-cycles;
        // not taken: two. The condition index is y itself (0=NZ, 1=Z, 2=NC, 3=C).
        if x_is(op, 3) && z_is(op, 0) && !bit5_set(op) {
            match self.step {
                // Step 1 is the fetch M-cycle (decision C_00): no bus access yet.
                1 => return true,
                // Step 2: check the condition with no bus access. When not met this is the last
                // M-cycle (not-taken = 8 T = 2 M); PC stays advanced past the opcode.
                2 => {
                    if !self.cond_met(y_of(op)) {
                        self.done(); // not taken: end after two M-cycles
                        return true;
                    }
                    return true;
                }
                // Step 3 (taken only): read the high byte of the return address from (SP).
                3 => {
                    self.hi = bus.read(self.sp);
                    return true;
                }
                // Step 4 (taken only): read the low byte of the return address from (SP)+1.
                4 => {
                    self.lo = bus.read(self.sp.wrapping_add(1));
                    return true;
                }
                // Step 5 (taken only): PC = popped value, SP += 2 (register writes), then end.
                _ => {
                    let value = ((self.hi as u16) << 8) | self.lo as u16;
                    self.pc = value; // taken: PC = (SP) (register write, no bus access)
                    self.sp = self.sp.wrapping_add(2);
                    self.done();
                    return true;
                }
            }
        }

        // RST n ($C7/$CF/$D7/$DF/$E7/$EF/$F7/$FF): block 3, z == 7. Four M-cycles total. The
        // vector address is n * 8 where n = y (0..7), giving $00/$08/.../$38 (note 02c).
        if x_is(op, 3) && z_is(op, 7) {
            match self.step {
                // Step 1 is the fetch M-cycle (decision C_00): no bus access yet. PC now points
                // past the one-byte opcode, i.e. at the return address.
                1 => return true,
                // Step 2: SP = SP - 2 (an internal M-cycle with no bus access, decision C_00).
                2 => {
                    self.sp = self.sp.wrapping_sub(2);
                    return true;
                }
                // Step 3: store the high byte of the return address at (SP) (one bus access).
                3 => {
                    let ret_addr = self.pc;
                    bus.write(self.sp, (ret_addr >> 8) as u8);
                    return true;
                }
                // Step 4: store the low byte of the return address at (SP)+1 (one bus access),
                // set PC to the vector address (a register write, so no bus access), then end.
                _ => {
                    let ret_addr = self.pc;
                    let vector = y_of(op) as u16 * 8; // RST n: PC = n * 8 (register write)
                    bus.write(self.sp.wrapping_add(1), (ret_addr & 0xFF) as u8);
                    self.pc = vector;
                    self.done();
                    return true;
                }
            }
        }

        false
    }

    /// Evaluate the condition selected by index 0=NZ, 1=Z, 2=NC, 3=C (note 02a: Z is set iff a
    /// result was zero; C is set on carry/borrow).
    fn cond_met(&self, idx: u8) -> bool {
        match idx {
            0 => self.f & 0x80 == 0, // NZ: Z flag clear
            1 => self.f & 0x80 != 0, // Z: Z flag set
            2 => self.f & 0x10 == 0, // NC: C flag clear
            _ => self.f & 0x10 != 0, // C: C flag set
        }
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

    // ---- Family 1: CALL a16 ($CD; note 02c seed Opcodes.json: [24 T = 6 M], no flags) ----

    #[test]
    fn c01_11_call_a16_costs_six_ticks_and_pushes_return_address() {
        let mut bus = testutil::new_bus();
        let mut cpu = Cpu {
            sp: 0xC100,
            f: 0x80, // Z set
            ..Cpu::default()
        };

        let ticks = testutil::exec(&mut cpu, &mut bus, &[0xCD, 0x56, 0x78]); // CALL $7856

        assert_eq!(ticks, 6); // note 02c: 24 T = 6 M-cycles
        assert_eq!(cpu.pc, 0x7856); // PC set to the target
        assert_eq!(cpu.sp, 0xC0FE); // SP decremented by two (0xC100 - 2)
        assert_eq!(bus.peek(0xC0FE), 0xC0); // high byte of return address ($C003) at (SP)
        assert_eq!(bus.peek(0xC0FF), 0x03); // low byte of the return address at (SP)+1
        assert_eq!(cpu.f, 0x80); // flags unchanged (Z preserved)
    }

    #[test]
    fn c01_11_call_a16_sp_wraps_at_bottom_of_memory() {
        // Edge case: SP = $0001; CALL decrements it past $0000 and wraps to $FFFF. The high
        // byte of the return address is stored at (SP) = $FFFF; the low-byte write to $0000
        // lands in ROM, which is read-only (decision A_06), so it is dropped by the bus.
        let mut bus = testutil::new_bus();
        let mut cpu = Cpu {
            sp: 0x0001,
            ..Cpu::default()
        };

        let ticks = testutil::exec(&mut cpu, &mut bus, &[0xCD, 0x56, 0x78]); // CALL $7856

        assert_eq!(ticks, 6); // note 02c: 24 T = 6 M-cycles
        assert_eq!(cpu.sp, 0xFFFF); // SP wrapped past $0000 (0x0001 - 2)
        assert_eq!(bus.peek(0xFFFF), 0xC0); // high byte of return address ($C003) at (SP) = $FFFF
        assert_eq!(bus.peek(0x0000), 0x00); // low-byte write to $0000 dropped (ROM read-only)
        assert_eq!(cpu.pc, 0x7856); // PC set to the target
    }

    #[test]
    fn c01_11_call_a16_masks_f_low_nibble() {
        // Flag boundary: a dirty low nibble of F is re-masked at instruction end; no flag is
        // affected by CALL.
        let mut bus = testutil::new_bus();
        let mut cpu = Cpu {
            sp: 0xC100,
            f: 0xFF, // all four flags set plus a dirty low nibble
            ..Cpu::default()
        };

        let ticks = testutil::exec(&mut cpu, &mut bus, &[0xCD, 0x56, 0x78]); // CALL $7856

        assert_eq!(ticks, 6);
        assert_eq!(cpu.f, 0xF0); // high nibble preserved, low nibble masked to zero
    }

    // ---- Family 2: CALL cc,a16 (note 02c seed Opcodes.json: taken [24 T = 6 M], not-taken [12 T = 3 M]) ----

    #[test]
    fn c01_11_call_nz_a16_taken_costs_six_ticks() {
        // $C4 CALL NZ,a16 with Z clear: the call is taken.
        let mut bus = testutil::new_bus();
        let mut cpu = Cpu {
            sp: 0xC100,
            f: 0x00, // Z clear (NZ true)
            ..Cpu::default()
        };

        let ticks = testutil::exec(&mut cpu, &mut bus, &[0xC4, 0x56, 0x78]); // CALL NZ,$7856

        assert_eq!(ticks, 6); // note 02c: taken = 24 T = 6 M-cycles
        assert_eq!(cpu.pc, 0x7856); // PC set to the target
        assert_eq!(cpu.sp, 0xC0FE); // SP decremented by two
        assert_eq!(bus.peek(0xC0FE), 0xC0); // high byte of return address ($C003) at (SP)
        assert_eq!(bus.peek(0xC0FF), 0x03); // low byte at (SP)+1
    }

    #[test]
    fn c01_11_call_nz_a16_not_taken_costs_three_ticks() {
        // $C4 CALL NZ,a16 with Z set: the call is not taken.
        let mut bus = testutil::new_bus();
        let mut cpu = Cpu {
            sp: 0xC100,
            f: 0x80, // Z set (NZ false)
            ..Cpu::default()
        };

        let ticks = testutil::exec(&mut cpu, &mut bus, &[0xC4, 0x56, 0x78]); // CALL NZ,$7856

        assert_eq!(ticks, 3); // note 02c: not-taken = 12 T = 3 M-cycles
        assert_eq!(cpu.pc, 0xC003); // PC advanced past the three bytes (no jump)
        assert_eq!(cpu.sp, 0xC100); // SP unchanged (nothing pushed)
    }

    #[test]
    fn c01_11_call_z_a16_taken_costs_six_ticks() {
        // $CC CALL Z,a16 with Z set: the call is taken.
        let mut bus = testutil::new_bus();
        let mut cpu = Cpu {
            sp: 0xC100,
            f: 0x80, // Z set (Z true)
            ..Cpu::default()
        };

        let ticks = testutil::exec(&mut cpu, &mut bus, &[0xCC, 0xAB, 0xCD]); // CALL Z,$CDAB

        assert_eq!(ticks, 6);
        assert_eq!(cpu.pc, 0xCDAB);
    }

    #[test]
    fn c01_11_call_nc_a16_not_taken_costs_three_ticks() {
        // $D4 CALL NC,a16 with C set: the call is not taken.
        let mut bus = testutil::new_bus();
        let mut cpu = Cpu {
            sp: 0xC100,
            f: 0x90, // Z and C set (NC false)
            ..Cpu::default()
        };

        let ticks = testutil::exec(&mut cpu, &mut bus, &[0xD4, 0x12, 0x34]); // CALL NC,$3412

        assert_eq!(ticks, 3);
        assert_eq!(cpu.pc, 0xC003);
        assert_eq!(cpu.sp, 0xC100);
    }

    #[test]
    fn c01_11_call_c_a16_taken_costs_six_ticks() {
        // $DC CALL C,a16 with C set: the call is taken.
        let mut bus = testutil::new_bus();
        let mut cpu = Cpu {
            sp: 0xC100,
            f: 0x10, // C set (C true)
            ..Cpu::default()
        };

        let ticks = testutil::exec(&mut cpu, &mut bus, &[0xDC, 0xFF, 0x00]); // CALL C,$00FF

        assert_eq!(ticks, 6);
        assert_eq!(cpu.pc, 0x00FF);
    }

    #[test]
    fn c01_11_call_cc_a16_not_taken_preserves_flags() {
        let mut bus = testutil::new_bus();
        let mut cpu = Cpu {
            sp: 0xC100,
            f: 0x80, // Z set
            ..Cpu::default()
        };

        let ticks = testutil::exec(&mut cpu, &mut bus, &[0xC4, 0x56, 0x78]); // CALL NZ,$7856 (not taken)

        assert_eq!(ticks, 3);
        assert_eq!(cpu.f, 0x80); // flags unchanged
    }

    #[test]
    fn c01_11_call_cc_a16_taken_masks_f_low_nibble() {
        // Flag boundary: a dirty low nibble of F is re-masked at instruction end. Z clear so NZ
        // is true (taken); the other flags and the dirty low nibble are present before the mask.
        let mut bus = testutil::new_bus();
        let mut cpu = Cpu {
            sp: 0xC100,
            f: 0x7F, // Z clear (NZ true) plus N/H/C set and a dirty low nibble
            ..Cpu::default()
        };

        let ticks = testutil::exec(&mut cpu, &mut bus, &[0xC4, 0x12, 0x34]); // CALL NZ,$3412 (taken)

        assert_eq!(ticks, 6);
        assert_eq!(cpu.pc, 0x3412);
        assert_eq!(cpu.f, 0x70); // high nibble preserved (N/H/C), low nibble masked to zero
    }

    // ---- Family 3: RET ($C9; note 02c seed Opcodes.json: [16 T = 4 M], no flags) ----

    #[test]
    fn c01_11_ret_costs_four_ticks_and_pops_return_address() {
        let mut bus = testutil::new_bus();
        // Seed the stack with a return address (high byte at (SP), low byte at (SP)+1).
        let sp = 0xC100u16;
        bus.write(sp, 0x78); // high byte of the return address ($7856) at (SP)
        bus.write(sp.wrapping_add(1), 0x56); // low byte at (SP)+1
        let mut cpu = Cpu {
            sp,
            f: 0x80, // Z set
            ..Cpu::default()
        };

        let ticks = testutil::exec(&mut cpu, &mut bus, &[0xC9]); // RET

        assert_eq!(ticks, 4); // note 02c: 16 T = 4 M-cycles
        assert_eq!(cpu.pc, 0x7856); // PC set to the popped return address
        assert_eq!(cpu.sp, 0xC102); // SP incremented by two (0xC100 + 2)
        assert_eq!(cpu.f, 0x80); // flags unchanged
    }

    #[test]
    fn c01_11_ret_masks_f_low_nibble() {
        // Flag boundary: a dirty low nibble of F is re-masked at instruction end.
        let mut bus = testutil::new_bus();
        let sp = 0xC100u16;
        bus.write(sp, 0x78);
        bus.write(sp.wrapping_add(1), 0x56);
        let mut cpu = Cpu {
            sp,
            f: 0xFF, // all flags set plus dirty low nibble
            ..Cpu::default()
        };

        let ticks = testutil::exec(&mut cpu, &mut bus, &[0xC9]); // RET

        assert_eq!(ticks, 4);
        assert_eq!(cpu.f, 0xF0); // high nibble preserved, low nibble masked to zero
    }

    // ---- Family 4: RET cc (note 02c seed Opcodes.json: taken [20 T = 5 M], not-taken [8 T = 2 M]) ----

    #[test]
    fn c01_11_ret_nz_taken_costs_five_ticks() {
        // $C0 RET NZ with Z clear: the return is taken.
        let mut bus = testutil::new_bus();
        let sp = 0xC100u16;
        bus.write(sp, 0x78);
        bus.write(sp.wrapping_add(1), 0x56);
        let mut cpu = Cpu {
            sp,
            f: 0x00, // Z clear (NZ true)
            ..Cpu::default()
        };

        let ticks = testutil::exec(&mut cpu, &mut bus, &[0xC0]); // RET NZ

        assert_eq!(ticks, 5); // note 02c: taken = 20 T = 5 M-cycles
        assert_eq!(cpu.pc, 0x7856); // PC set to the popped return address
        assert_eq!(cpu.sp, 0xC102); // SP incremented by two
    }

    #[test]
    fn c01_11_ret_nz_not_taken_costs_two_ticks() {
        // $C0 RET NZ with Z set: the return is not taken.
        let mut bus = testutil::new_bus();
        let sp = 0xC100u16;
        bus.write(sp, 0x78);
        bus.write(sp.wrapping_add(1), 0x56);
        let mut cpu = Cpu {
            sp,
            f: 0x80, // Z set (NZ false)
            ..Cpu::default()
        };

        let ticks = testutil::exec(&mut cpu, &mut bus, &[0xC0]); // RET NZ

        assert_eq!(ticks, 2); // note 02c: not-taken = 8 T = 2 M-cycles
        assert_eq!(cpu.pc, 0xC001); // PC advanced past the one-byte opcode (no return)
        assert_eq!(cpu.sp, 0xC100); // SP unchanged (nothing popped)
    }

    #[test]
    fn c01_11_ret_z_taken_costs_five_ticks() {
        // $C8 RET Z with Z set: the return is taken.
        let mut bus = testutil::new_bus();
        let sp = 0xC100u16;
        bus.write(sp, 0xAB); // high byte of the return address ($ABCD) at (SP)
        bus.write(sp.wrapping_add(1), 0xCD); // low byte at (SP)+1
        let mut cpu = Cpu {
            sp,
            f: 0x80, // Z set (Z true)
            ..Cpu::default()
        };

        let ticks = testutil::exec(&mut cpu, &mut bus, &[0xC8]); // RET Z

        assert_eq!(ticks, 5);
        assert_eq!(cpu.pc, 0xABCD);
    }

    #[test]
    fn c01_11_ret_nc_not_taken_costs_two_ticks() {
        // $D0 RET NC with C set: the return is not taken.
        let mut bus = testutil::new_bus();
        let sp = 0xC100u16;
        bus.write(sp, 0x78);
        bus.write(sp.wrapping_add(1), 0x56);
        let mut cpu = Cpu {
            sp,
            f: 0x90, // Z and C set (NC false)
            ..Cpu::default()
        };

        let ticks = testutil::exec(&mut cpu, &mut bus, &[0xD0]); // RET NC

        assert_eq!(ticks, 2);
        assert_eq!(cpu.pc, 0xC001);
        assert_eq!(cpu.sp, 0xC100);
    }

    #[test]
    fn c01_11_ret_c_taken_costs_five_ticks() {
        // $D8 RET C with C set: the return is taken.
        let mut bus = testutil::new_bus();
        let sp = 0xC100u16;
        bus.write(sp, 0x00); // high byte of the return address ($00FF) at (SP)
        bus.write(sp.wrapping_add(1), 0xFF); // low byte at (SP)+1
        let mut cpu = Cpu {
            sp,
            f: 0x10, // C set (C true)
            ..Cpu::default()
        };

        let ticks = testutil::exec(&mut cpu, &mut bus, &[0xD8]); // RET C

        assert_eq!(ticks, 5);
        assert_eq!(cpu.pc, 0x00FF);
    }

    #[test]
    fn c01_11_ret_cc_not_taken_preserves_flags() {
        let mut bus = testutil::new_bus();
        let sp = 0xC100u16;
        bus.write(sp, 0x78);
        bus.write(sp.wrapping_add(1), 0x56);
        let mut cpu = Cpu {
            sp,
            f: 0x80, // Z set
            ..Cpu::default()
        };

        let ticks = testutil::exec(&mut cpu, &mut bus, &[0xC0]); // RET NZ (not taken)

        assert_eq!(ticks, 2);
        assert_eq!(cpu.f, 0x80); // flags unchanged
    }

    #[test]
    fn c01_11_ret_cc_taken_masks_f_low_nibble() {
        // Flag boundary: a dirty low nibble of F is re-masked at instruction end. Z clear so NZ
        // is true (taken); the other flags and the dirty low nibble are present before the mask.
        let mut bus = testutil::new_bus();
        let sp = 0xC100u16;
        bus.write(sp, 0x78);
        bus.write(sp.wrapping_add(1), 0x56);
        let mut cpu = Cpu {
            sp,
            f: 0x7F, // Z clear (NZ true) plus N/H/C set and a dirty low nibble
            ..Cpu::default()
        };

        let ticks = testutil::exec(&mut cpu, &mut bus, &[0xC0]); // RET NZ (taken)

        assert_eq!(ticks, 5);
        assert_eq!(cpu.pc, 0x7856);
        assert_eq!(cpu.f, 0x70); // high nibble preserved (N/H/C), low nibble masked to zero
    }

    // ---- Family 5: RST n (note 02c seed Opcodes.json: [16 T = 4 M], no flags) ----

    #[test]
    fn c01_11_rst_0x38_costs_four_ticks_and_jumps_to_vector() {
        let mut bus = testutil::new_bus();
        let mut cpu = Cpu {
            sp: 0xC100,
            f: 0x80, // Z set
            ..Cpu::default()
        };

        let ticks = testutil::exec(&mut cpu, &mut bus, &[0xFF]); // RST $38

        assert_eq!(ticks, 4); // note 02c: 16 T = 4 M-cycles
        assert_eq!(cpu.pc, 0x38); // PC set to the vector address ($38)
        assert_eq!(cpu.sp, 0xC0FE); // SP decremented by two (0xC100 - 2)
        assert_eq!(bus.peek(0xC0FE), 0xC0); // high byte of return address ($C001) at (SP)
        assert_eq!(bus.peek(0xC0FF), 0x01); // low byte of the return address at (SP)+1
        assert_eq!(cpu.f, 0x80); // flags unchanged
    }

    #[test]
    fn c01_11_rst_0x00_jumps_to_vector_zero() {
        let mut bus = testutil::new_bus();
        let mut cpu = Cpu {
            sp: 0xC100,
            ..Cpu::default()
        };

        let ticks = testutil::exec(&mut cpu, &mut bus, &[0xC7]); // RST $00

        assert_eq!(ticks, 4); // note 02c: 16 T = 4 M-cycles
        assert_eq!(cpu.pc, 0x00); // PC set to the vector address ($00)
    }

    #[test]
    fn c01_11_rst_sp_wraps_at_bottom_of_memory() {
        // Edge case: SP = $0001; RST decrements it past $0000 and wraps to $FFFF. The high
        // byte of the return address is stored at (SP) = $FFFF; the low-byte write to $0000
        // lands in ROM, which is read-only (decision A_06), so it is dropped by the bus.
        let mut bus = testutil::new_bus();
        let mut cpu = Cpu {
            sp: 0x0001,
            ..Cpu::default()
        };

        let ticks = testutil::exec(&mut cpu, &mut bus, &[0xFF]); // RST $38

        assert_eq!(ticks, 4);
        assert_eq!(cpu.sp, 0xFFFF); // SP wrapped past $0000 (0x0001 - 2)
        assert_eq!(bus.peek(0xFFFF), 0xC0); // high byte of return address ($C001) at (SP) = $FFFF
        assert_eq!(cpu.pc, 0x38); // PC set to the vector address
    }

    #[test]
    fn c01_11_rst_masks_f_low_nibble() {
        // Flag boundary: a dirty low nibble of F is re-masked at instruction end.
        let mut bus = testutil::new_bus();
        let mut cpu = Cpu {
            sp: 0xC100,
            f: 0xFF, // all flags set plus dirty low nibble
            ..Cpu::default()
        };

        let ticks = testutil::exec(&mut cpu, &mut bus, &[0xFF]); // RST $38

        assert_eq!(ticks, 4);
        assert_eq!(cpu.f, 0xF0); // high nibble preserved, low nibble masked to zero
    }
}
