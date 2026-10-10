//! Signed-offset SP operations (task C01_18): ADD SP,e8 ($E8) and LD HL,SP+e8 ($F8).
//! Decoded by opcode bit fields per decision C_00; no table.

use super::Cpu;
use crate::bus::Bus;

impl Cpu {
    /// Signed-offset SP operations (task C01_18). Returns true when the current opcode is in
    /// this group, false otherwise. Timings and flag effects from note 02c / seed Opcodes.json.
    /// ADD SP,e8 ($E8; block 3, bit5 set, z == 0, bit3 set, bit4 clear) costs 16 T = 4 M-cycles:
    /// the e8 immediate is read on step 2 (one bus access), step 3 is an internal M-cycle with no
    /// bus access, and the SP write happens on step 4 (a register copy, so no bus access).
    /// LD HL,SP+e8 ($F8; block 3, bit5 set, z == 0, bit3 set, bit4 set) costs 12 T = 3 M-cycles:
    /// the e8 immediate is read on step 2 (one bus access), and the HL write plus flags happen on
    /// step 3 (register copies, so no bus access). In both cases e8 is a signed 8-bit offset
    /// (note 02a) and the flags are "00hc": Z and N cleared, H and C from the low-byte addition
    /// sp_low + e8. At most one bus access per M-cycle (decision C_00).
    pub(super) fn exec_sp_offset(&mut self, bus: &mut Bus) -> bool {
        let op = self.opcode;

        // ADD SP,e8 ($E8): four M-cycles total. The e8 immediate is latched on step 2; the SP
        // write and flags happen on step 4 (register copy, no bus access). Step 3 is an internal
        // M-cycle with no bus access.
        if x_is(op, 3) && bit5_set(op) && z_is(op, 0) && bit3_set(op) && !bit4_set(op) {
            match self.step {
                // Step 1 is the fetch M-cycle (decision C_00): no bus access yet.
                1 => return true,
                // Step 2: read the e8 immediate at PC; latch it for step 4.
                2 => {
                    self.lo = bus.read(self.pc);
                    self.pc = self.pc.wrapping_add(1);
                    return true;
                }
                // Step 3: internal M-cycle, no bus access (the extra cycle of the 4-M form).
                3 => return true,
                _ => {
                    let e8 = self.lo;
                    let sp_low = (self.sp & 0xFF) as u8; // original low byte, before the update
                    let offset = (e8 as i8) as i16 as u16; // sign-extend the signed 8-bit offset
                    self.sp = self.sp.wrapping_add(offset);
                    self.f = sp_offset_flags(sp_low, e8);
                    self.done();
                    return true;
                }
            }
        }

        // LD HL,SP+e8 ($F8): three M-cycles total. The e8 immediate is latched on step 2; the
        // HL write and flags happen on step 3 (register copies, no bus access).
        if x_is(op, 3) && bit5_set(op) && z_is(op, 0) && bit3_set(op) && bit4_set(op) {
            match self.step {
                // Step 1 is the fetch M-cycle (decision C_00): no bus access yet.
                1 => return true,
                // Step 2: read the e8 immediate at PC; latch it for step 3.
                2 => {
                    self.lo = bus.read(self.pc);
                    self.pc = self.pc.wrapping_add(1);
                    return true;
                }
                _ => {
                    let e8 = self.lo;
                    let sp_low = (self.sp & 0xFF) as u8; // original low byte, before the update
                    let offset = (e8 as i8) as i16 as u16; // sign-extend the signed 8-bit offset
                    let result = self.sp.wrapping_add(offset);
                    self.h = (result >> 8) as u8;
                    self.l = result as u8;
                    self.f = sp_offset_flags(sp_low, e8);
                    self.done();
                    return true;
                }
            }
        }

        false
    }
}

/// Flags for ADD SP,e8 / LD HL,SP+e8 (note 02a "00hc"): Z and N cleared; H is the carry out of
/// bit 3 and C the carry out of bit 7 of the low-byte addition sp_low + e8. Pure function so it
/// can be tested without a bus (decision C_00).
fn sp_offset_flags(sp_low: u8, e8: u8) -> u8 {
    let low_sum = (sp_low as u16) + (e8 as u16); // unsigned low-byte addition, 0..=0x1FE
    let mut f = 0u8; // Z and N cleared (note 02a "00hc")
    if ((sp_low & 0x0F) as u16 + (e8 & 0x0F) as u16) > 0x0F {
        f |= 0x20; // H: carry out of bit 3 of the low-byte addition
    }
    if low_sum > 0xFF {
        f |= 0x10; // C: carry out of bit 7 of the low-byte addition
    }
    f
}

// ---- Opcode bit-field helpers (decision C_00: x = op>>6, y = (op>>3)&7, z = op&7) ----

fn x_is(op: u8, v: u8) -> bool {
    op >> 6 == v
}
fn z_is(op: u8, v: u8) -> bool {
    op & 7 == v
}
/// Bit 3 of the opcode (the low bit of y).
fn bit3_set(op: u8) -> bool {
    (op >> 3) & 1 != 0
}
/// Bit 4 of the opcode: clear for ADD SP,e8 ($E8), set for LD HL,SP+e8 ($F8).
fn bit4_set(op: u8) -> bool {
    (op >> 4) & 1 != 0
}
/// Bit 5 of the opcode; both signed-offset forms have it set.
fn bit5_set(op: u8) -> bool {
    (op >> 5) & 1 != 0
}

#[cfg(test)]
mod tests {
    use crate::cpu::{testutil, Cpu};

    // ---- Flag rule for the low-byte addition (note 02a "00hc": Z,N cleared; H,C calculated) ----

    #[test]
    fn c01_18_sp_offset_flags_clears_z_and_n() {
        // No carry: both H and C clear, so F is zero regardless of the prior flag state.
        assert_eq!(super::sp_offset_flags(0x00, 0x00), 0x00);
        assert_eq!(super::sp_offset_flags(0x7E, 0x01), 0x00); // low_sum $7F; nibble $0E+$01=$0F, no bit-3 carry
    }

    #[test]
    fn c01_18_sp_offset_flags_sets_carry_on_wrap() {
        // Carry boundary: sp_low + e8 exceeds $FF (carry out of bit 7).
        assert_eq!(super::sp_offset_flags(0xFF, 0x01), 0x30); // C and H both set
        assert_eq!(super::sp_offset_flags(0xFE, 0x02), 0x30); // $FE + $02 = $100
    }

    #[test]
    fn c01_18_sp_offset_flags_sets_half_carry_at_boundary() {
        // Half-carry boundary: carry out of bit 3 but not bit 7.
        assert_eq!(super::sp_offset_flags(0x0F, 0x01), 0x20); // $0F + $01 = $10 (bit-3 carry)
        assert_eq!(super::sp_offset_flags(0x6F, 0x01), 0x20); // low nibble $0F + $01 overflows
    }

    #[test]
    fn c01_18_sp_offset_flags_keeps_c_clear_below_wrap() {
        // Carry boundary: sp_low + e8 = $FF, exactly at the wrap but not past it. No bit-7
        // carry (C clear) and no bit-3 carry either (nibble $0E + $01 = $0F).
        assert_eq!(super::sp_offset_flags(0xFE, 0x01), 0x00);
    }

    // ---- Family 1: ADD SP,e8 (note 02c / seed Opcodes.json: [16 T = 4 M], flags "00hc") ----

    #[test]
    fn c01_18_add_sp_e8_costs_four_ticks() {
        let mut bus = testutil::new_bus();
        let mut cpu = Cpu {
            sp: 0x1234,
            f: 0x80, // Z set before the instruction; must be cleared
            ..Cpu::default()
        };

        let ticks = testutil::exec(&mut cpu, &mut bus, &[0xE8, 0x05]); // ADD SP,$05

        assert_eq!(ticks, 4); // note 02c / seed Opcodes.json: 16 T = 4 M-cycles
        assert_eq!(cpu.sp, 0x1239); // SP += $05 (positive offset)
        assert_eq!(cpu.pc, 0xC002); // PC advanced past opcode and the e8 immediate
        assert_eq!(cpu.f & 0x80, 0); // Z cleared (note 02a "00hc")
        assert_eq!(cpu.unimplemented(), None); // claimed by this group, not recorded unimplemented
    }

    #[test]
    fn c01_18_add_sp_e8_negative_offset_subtracts() {
        let mut bus = testutil::new_bus();
        let mut cpu = Cpu {
            sp: 0x1234,
            ..Cpu::default()
        };

        let ticks = testutil::exec(&mut cpu, &mut bus, &[0xE8, 0xFF]); // ADD SP,$FF (=-1)

        assert_eq!(ticks, 4);
        assert_eq!(cpu.sp, 0x1233); // SP -= 1 (e8 = $FF is the signed offset -1)
    }

    #[test]
    fn c01_18_add_sp_e8_wraps_at_top_of_memory() {
        let mut bus = testutil::new_bus();
        let mut cpu = Cpu {
            sp: 0xFFFF,
            ..Cpu::default()
        };

        let ticks = testutil::exec(&mut cpu, &mut bus, &[0xE8, 0x01]); // ADD SP,$01

        assert_eq!(ticks, 4);
        assert_eq!(cpu.sp, 0x0000); // $FFFF + $01 wraps to $0000
    }

    #[test]
    fn c01_18_add_sp_e8_sets_carry_on_wrap() {
        let mut bus = testutil::new_bus();
        let mut cpu = Cpu {
            sp: 0x07FF, // low byte $FF; +$01 overflows the low byte
            ..Cpu::default()
        };

        let ticks = testutil::exec(&mut cpu, &mut bus, &[0xE8, 0x01]); // ADD SP,$01

        assert_eq!(ticks, 4);
        assert_eq!(cpu.sp, 0x0800);
        assert_eq!(cpu.f & 0x10, 0x10); // C set: carry out of bit 7 of the low-byte addition
        assert_eq!(cpu.f & 0x20, 0x20); // H set too (bit-3 carry)
    }

    #[test]
    fn c01_18_add_sp_e8_sets_half_carry_at_boundary() {
        let mut bus = testutil::new_bus();
        let mut cpu = Cpu {
            sp: 0x120F, // low byte $0F; +$01 overflows the low nibble but not the byte
            ..Cpu::default()
        };

        let ticks = testutil::exec(&mut cpu, &mut bus, &[0xE8, 0x01]); // ADD SP,$01

        assert_eq!(ticks, 4);
        assert_eq!(cpu.sp, 0x1210);
        assert_eq!(cpu.f & 0x20, 0x20); // H set: carry out of bit 3 of the low-byte addition
        assert_eq!(cpu.f & 0x10, 0); // C clear: no carry out of bit 7 yet
    }

    #[test]
    fn c01_18_add_sp_e8_clears_z_and_n() {
        let mut bus = testutil::new_bus();
        let mut cpu = Cpu {
            sp: 0x1234,
            f: 0xC0, // Z and N set before the instruction; both must be cleared
            ..Cpu::default()
        };

        let ticks = testutil::exec(&mut cpu, &mut bus, &[0xE8, 0x05]); // ADD SP,$05 (no carry)

        assert_eq!(ticks, 4);
        assert_eq!(cpu.f, 0x00); // Z and N cleared; H and C clear (no low-byte carry)
    }

    #[test]
    fn c01_18_add_sp_e8_masks_f_low_nibble() {
        let mut bus = testutil::new_bus();
        let mut cpu = Cpu {
            sp: 0x1234,
            f: 0xFF, // all four flags set plus a dirty low nibble
            ..Cpu::default()
        };

        let ticks = testutil::exec(&mut cpu, &mut bus, &[0xE8, 0x05]); // ADD SP,$05 (no carry)

        assert_eq!(ticks, 4);
        assert_eq!(cpu.f, 0x00); // Z,N cleared and H,C clear; low nibble masked to zero
    }

    // ---- Family 2: LD HL,SP+e8 (note 02c / seed Opcodes.json: [12 T = 3 M], flags "00hc") ----

    #[test]
    fn c01_18_ld_hl_sp_e8_costs_three_ticks() {
        let mut bus = testutil::new_bus();
        let mut cpu = Cpu {
            sp: 0x1234,
            h: 0x00,
            l: 0x00,
            f: 0x80, // Z set before the instruction; must be cleared
            ..Cpu::default()
        };

        let ticks = testutil::exec(&mut cpu, &mut bus, &[0xF8, 0x05]); // LD HL,SP+$05

        assert_eq!(ticks, 3); // note 02c / seed Opcodes.json: 12 T = 3 M-cycles
        assert_eq!((cpu.h as u16) << 8 | cpu.l as u16, 0x1239); // HL = SP + $05 (positive offset)
        assert_eq!(cpu.pc, 0xC002); // PC advanced past opcode and the e8 immediate
        assert_eq!(cpu.f & 0x80, 0); // Z cleared (note 02a "00hc")
        assert_eq!(cpu.unimplemented(), None); // claimed by this group, not recorded unimplemented
    }

    #[test]
    fn c01_18_ld_hl_sp_e8_negative_offset_subtracts() {
        let mut bus = testutil::new_bus();
        let mut cpu = Cpu {
            sp: 0x1234,
            ..Cpu::default()
        };

        let ticks = testutil::exec(&mut cpu, &mut bus, &[0xF8, 0xFF]); // LD HL,SP+$FF (=-1)

        assert_eq!(ticks, 3);
        assert_eq!((cpu.h as u16) << 8 | cpu.l as u16, 0x1233); // HL = SP - 1
    }

    #[test]
    fn c01_18_ld_hl_sp_e8_wraps_at_top_of_memory() {
        let mut bus = testutil::new_bus();
        let mut cpu = Cpu {
            sp: 0xFFFF,
            ..Cpu::default()
        };

        let ticks = testutil::exec(&mut cpu, &mut bus, &[0xF8, 0x01]); // LD HL,SP+$01

        assert_eq!(ticks, 3);
        assert_eq!((cpu.h as u16) << 8 | cpu.l as u16, 0x0000); // $FFFF + $01 wraps to $0000
    }

    #[test]
    fn c01_18_ld_hl_sp_e8_sets_carry_on_wrap() {
        let mut bus = testutil::new_bus();
        let mut cpu = Cpu {
            sp: 0x07FF, // low byte $FF; +$01 overflows the low byte
            ..Cpu::default()
        };

        let ticks = testutil::exec(&mut cpu, &mut bus, &[0xF8, 0x01]); // LD HL,SP+$01

        assert_eq!(ticks, 3);
        assert_eq!((cpu.h as u16) << 8 | cpu.l as u16, 0x0800);
        assert_eq!(cpu.f & 0x10, 0x10); // C set: carry out of bit 7 of the low-byte addition
        assert_eq!(cpu.f & 0x20, 0x20); // H set too (bit-3 carry)
    }

    #[test]
    fn c01_18_ld_hl_sp_e8_sets_half_carry_at_boundary() {
        let mut bus = testutil::new_bus();
        let mut cpu = Cpu {
            sp: 0x120F, // low byte $0F; +$01 overflows the low nibble but not the byte
            ..Cpu::default()
        };

        let ticks = testutil::exec(&mut cpu, &mut bus, &[0xF8, 0x01]); // LD HL,SP+$01

        assert_eq!(ticks, 3);
        assert_eq!((cpu.h as u16) << 8 | cpu.l as u16, 0x1210);
        assert_eq!(cpu.f & 0x20, 0x20); // H set: carry out of bit 3 of the low-byte addition
        assert_eq!(cpu.f & 0x10, 0); // C clear: no carry out of bit 7 yet
    }

    #[test]
    fn c01_18_ld_hl_sp_e8_clears_z_and_n() {
        let mut bus = testutil::new_bus();
        let mut cpu = Cpu {
            sp: 0x1234,
            f: 0xC0, // Z and N set before the instruction; both must be cleared
            ..Cpu::default()
        };

        let ticks = testutil::exec(&mut cpu, &mut bus, &[0xF8, 0x05]); // LD HL,SP+$05 (no carry)

        assert_eq!(ticks, 3);
        assert_eq!(cpu.f, 0x00); // Z and N cleared; H and C clear (no low-byte carry)
    }

    #[test]
    fn c01_18_ld_hl_sp_e8_masks_f_low_nibble() {
        let mut bus = testutil::new_bus();
        let mut cpu = Cpu {
            sp: 0x1234,
            f: 0xFF, // all four flags set plus a dirty low nibble
            ..Cpu::default()
        };

        let ticks = testutil::exec(&mut cpu, &mut bus, &[0xF8, 0x05]); // LD HL,SP+$05 (no carry)

        assert_eq!(ticks, 3);
        assert_eq!(cpu.f, 0x00); // Z,N cleared and H,C clear; low nibble masked to zero
    }
}
