//! Absolute-address 8-bit loads (task C01_07): LD A,(a16), LD (a16),A, LDH A,(a8),
//! LDH (a8),A and the high-memory register-C forms LDH A,(C) / LDH (C),A. Decoded by opcode
//! bit fields per decision C_00; no table.

use super::Cpu;
use crate::bus::Bus;

impl Cpu {
    /// Absolute-address 8-bit loads (task C01_07). Returns true when the current opcode is in
    /// this group, false otherwise. Timings and flag effects from note 02c (seed Opcodes.json):
    /// - LD A,(a16) ($FA) / LD (a16),A ($EA): 3 bytes, 16 T = 4 M-cycles, no flags; the low
    ///   byte of a16 is read on step 2 and the high byte on step 3 (little-endian order, note
    ///   02c "La table suit le layout SM83 de pandocs"), the single memory access happens on
    ///   step 4. At most one bus access per M-cycle (decision C_00).
    /// - LDH A,(a8) ($F0) / LDH (a8),A ($E0): 2 bytes, 12 T = 3 M-cycles, no flags; the a8
    ///   immediate is read on step 2 and the memory access at $FF00+a8 happens on step 3. The
    ///   address wraps inside the high page (a8 = $FF gives $FFFF).
    /// - LDH A,(C) ($F2) / LDH (C),A ($E2): 1 byte, 8 T = 2 M-cycles, no flags; the memory
    ///   access at $FF00+C happens on step 2. The address wraps inside the high page
    ///   (C = $FF gives $FFFF). Note 02a: "les ports passent par ... $E2/$F2 ($FF00+C)".
    pub(super) fn exec_load_abs(&mut self, bus: &mut Bus) -> bool {
        let op = self.opcode;

        // All six forms live in block 3 with bit 5 set (decision C_00 bit fields). Bit 4 picks
        // the direction: F-block ($F?) loads into A, E-block ($E?) stores to memory. The z and
        // b3 fields pick the family; these guards exclude every non-list opcode in block 3
        // (e.g. $E8/$F8 LD HL,e16, $EC/$FC INI/IND, $E4/$F4 LDI/LDD).
        if x_is(op, 3) && bit5_set(op) {
            let load_into_a = bit4_set(op);

            // Family a16: z == 2 and b3 set. Four M-cycles total; the low byte of a16 is read
            // on step 2 and the high byte on step 3 (little-endian order, note 02c).
            if z_is(op, 2) && bit3_set(op) {
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
                    _ => {
                        let addr = ((self.hi as u16) << 8) | self.lo as u16;
                        if load_into_a {
                            self.a = bus.read(addr); // LD A,(a16)
                        } else {
                            bus.write(addr, self.a); // LD (a16),A
                        }
                        self.done();
                        return true;
                    }
                }
            }

            // Family a8: z == 0 and b3 clear. Three M-cycles total; the address is $FF00+a8
            // and wraps inside the high page (note 02a).
            if z_is(op, 0) && !bit3_set(op) {
                match self.step {
                    1 => return true, // fetch M-cycle: no bus access yet
                    2 => {
                        let a8 = bus.read(self.pc);
                        self.pc = self.pc.wrapping_add(1);
                        self.lo = a8; // latch the immediate for the memory-access M-cycle
                        return true;
                    }
                    _ => {
                        let addr = 0xFF00u16.wrapping_add(self.lo as u16);
                        if load_into_a {
                            self.a = bus.read(addr); // LDH A,(a8)
                        } else {
                            bus.write(addr, self.a); // LDH (a8),A
                        }
                        self.done();
                        return true;
                    }
                }
            }

            // Family C: z == 2 and b3 clear. Two M-cycles total; the address is $FF00+C and
            // wraps inside the high page (note 02a).
            if z_is(op, 2) && !bit3_set(op) {
                match self.step {
                    1 => return true, // fetch M-cycle: no bus access yet
                    _ => {
                        let addr = 0xFF00u16.wrapping_add(self.c as u16);
                        if load_into_a {
                            self.a = bus.read(addr); // LDH A,(C)
                        } else {
                            bus.write(addr, self.a); // LDH (C),A
                        }
                        self.done();
                        return true;
                    }
                }
            }
        }

        false
    }
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
/// Bit 4 of the opcode: set for the F-block ($F?) load-into-A forms, clear for the E-block.
fn bit4_set(op: u8) -> bool {
    (op >> 4) & 1 != 0
}
/// Bit 5 of the opcode; all six absolute-address forms have it set.
fn bit5_set(op: u8) -> bool {
    (op >> 5) & 1 != 0
}

#[cfg(test)]
mod tests {
    use crate::cpu::{testutil, Cpu};

    // ---- Family 1: LD A,(a16) / LD (a16),A (note 02c: [16 T = 4 M], no flags) ----

    #[test]
    fn c01_07_ld_a_a16_costs_four_ticks() {
        let mut bus = testutil::new_bus();
        // a16 = $C100 (low byte first: $00 then $C1). Seed the target cell.
        bus.write(0xC100, 0xAB);
        let mut cpu = Cpu {
            a: 0x00,
            f: 0x80, // Z set
            ..Cpu::default()
        };

        let ticks = testutil::exec(&mut cpu, &mut bus, &[0xFA, 0x00, 0xC1]); // LD A,(a16)

        assert_eq!(ticks, 4);
        assert_eq!(cpu.a, 0xAB); // destination now holds the memory byte
        assert_eq!(cpu.pc, 0xC003); // PC advanced past opcode and both address bytes
        assert_eq!(cpu.f, 0x80); // flags unchanged (Z preserved)
    }

    #[test]
    fn c01_07_ld_a16_a_writes_memory() {
        let mut bus = testutil::new_bus();
        bus.write(0xC200, 0x00); // start with a known zero at the target cell
        let mut cpu = Cpu {
            a: 0x7E,
            f: 0x30, // H and C set
            ..Cpu::default()
        };

        let ticks = testutil::exec(&mut cpu, &mut bus, &[0xEA, 0x00, 0xC2]); // LD (a16),A

        assert_eq!(ticks, 4);
        assert_eq!(bus.peek(0xC200), 0x7E); // memory now holds A
        assert_eq!(cpu.a, 0x7E); // source unchanged
        assert_eq!(cpu.pc, 0xC003);
        assert_eq!(cpu.f, 0x30); // flags preserved (H and C)
    }

    #[test]
    fn c01_07_ld_a_a16_reads_high_page_wrapping() {
        // Edge case: a16 = $FFFF; the low byte is read first ($FF), then the high byte ($FF).
        let mut bus = testutil::new_bus();
        bus.write(0xFFFF, 0x5C); // IE cell (last byte of the IO file)
        let mut cpu = Cpu {
            a: 0x00,
            ..Cpu::default()
        };

        let ticks = testutil::exec(&mut cpu, &mut bus, &[0xFA, 0xFF, 0xFF]); // LD A,(a16)

        assert_eq!(ticks, 4);
        assert_eq!(cpu.a, 0x5C); // read the byte at $FFFF (the high-page wrap target)
    }

    #[test]
    fn c01_07_ld_a_a16_masks_f_low_nibble() {
        // Flag boundary: a dirty low nibble of F is re-masked at instruction end; the high
        // nibble (the four real flags) is preserved because no flag is affected.
        let mut bus = testutil::new_bus();
        bus.write(0xC300, 0x12);
        let mut cpu = Cpu {
            a: 0x00,
            f: 0xFF, // all four flags set plus a dirty low nibble
            ..Cpu::default()
        };

        let ticks = testutil::exec(&mut cpu, &mut bus, &[0xFA, 0x00, 0xC3]); // LD A,(a16)

        assert_eq!(ticks, 4);
        assert_eq!(cpu.a, 0x12);
        assert_eq!(cpu.f, 0xF0); // high nibble preserved, low nibble masked to zero
    }

    // ---- Family 2: LDH A,(a8) / LDH (a8),A (note 02c: [12 T = 3 M], no flags) ----

    #[test]
    fn c01_07_ldh_a_a8_costs_three_ticks() {
        let mut bus = testutil::new_bus();
        // a8 = $46; the target is $FF00+$46 = $FF46 (LCDC). Seed it.
        bus.write(0xFF46, 0x91);
        let mut cpu = Cpu {
            a: 0x00,
            f: 0x80, // Z set
            ..Cpu::default()
        };

        let ticks = testutil::exec(&mut cpu, &mut bus, &[0xF0, 0x46]); // LDH A,(a8)

        assert_eq!(ticks, 3);
        assert_eq!(cpu.a, 0x91); // destination now holds the IO byte at $FF46
        assert_eq!(cpu.pc, 0xC002); // PC advanced past opcode and immediate
        assert_eq!(cpu.f, 0x80); // flags unchanged (Z preserved)
    }

    #[test]
    fn c01_07_ldh_a8_a_writes_memory() {
        let mut bus = testutil::new_bus();
        bus.write(0xFF46, 0x00); // start with a known zero at $FF46
        let mut cpu = Cpu {
            a: 0xC5,
            f: 0x30, // H and C set
            ..Cpu::default()
        };

        let ticks = testutil::exec(&mut cpu, &mut bus, &[0xE0, 0x46]); // LDH (a8),A

        assert_eq!(ticks, 3);
        assert_eq!(bus.peek(0xFF46), 0xC5); // memory now holds A
        assert_eq!(cpu.a, 0xC5); // source unchanged
        assert_eq!(cpu.f, 0x30); // flags preserved (H and C)
    }

    #[test]
    fn c01_07_ldh_a8_wraps_inside_high_page() {
        // Edge case: a8 = $FF; the address is $FF00+$FF = $FFFF, which wraps inside the high page.
        let mut bus = testutil::new_bus();
        bus.write(0xFFFF, 0x63); // IE cell at $FFFF
        let mut cpu = Cpu {
            a: 0x00,
            ..Cpu::default()
        };

        let ticks = testutil::exec(&mut cpu, &mut bus, &[0xF0, 0xFF]); // LDH A,(a8)

        assert_eq!(ticks, 3);
        assert_eq!(cpu.a, 0x63); // read the byte at $FFFF (the high-page wrap target)
    }

    #[test]
    fn c01_07_ldh_a8_masks_f_low_nibble() {
        // Flag boundary: a dirty low nibble of F is re-masked at instruction end.
        let mut bus = testutil::new_bus();
        bus.write(0xFF46, 0x5A);
        let mut cpu = Cpu {
            a: 0x00,
            f: 0xFF, // all four flags set plus a dirty low nibble
            ..Cpu::default()
        };

        let ticks = testutil::exec(&mut cpu, &mut bus, &[0xF0, 0x46]); // LDH A,(a8)

        assert_eq!(ticks, 3);
        assert_eq!(cpu.a, 0x5A);
        assert_eq!(cpu.f, 0xF0); // high nibble preserved, low nibble masked to zero
    }

    // ---- Family 3: LDH A,(C) / LDH (C),A (note 02c: [8 T = 2 M], no flags) ----

    #[test]
    fn c01_07_ldh_a_c_costs_two_ticks() {
        let mut bus = testutil::new_bus();
        // C = $46; the target is $FF00+$46 = $FF46 (LCDC). Seed it.
        bus.write(0xFF46, 0x9D);
        let mut cpu = Cpu {
            c: 0x46,
            a: 0x00,
            f: 0x80, // Z set
            ..Cpu::default()
        };

        let ticks = testutil::exec(&mut cpu, &mut bus, &[0xF2]); // LDH A,(C)

        assert_eq!(ticks, 2);
        assert_eq!(cpu.a, 0x9D); // destination now holds the byte at $FF46 (the address in C)
        assert_eq!(cpu.pc, 0xC001); // PC advanced past the opcode
        assert_eq!(cpu.f, 0x80); // flags unchanged (Z preserved)
    }

    #[test]
    fn c01_07_ldh_c_a_writes_memory() {
        let mut bus = testutil::new_bus();
        bus.write(0xFF46, 0x00); // start with a known zero at $FF46
        let mut cpu = Cpu {
            c: 0x46,
            a: 0xE1,
            f: 0x30, // H and C set
            ..Cpu::default()
        };

        let ticks = testutil::exec(&mut cpu, &mut bus, &[0xE2]); // LDH (C),A

        assert_eq!(ticks, 2);
        assert_eq!(bus.peek(0xFF46), 0xE1); // memory now holds A at $FF46
        assert_eq!(cpu.a, 0xE1); // source unchanged
        assert_eq!(cpu.f, 0x30); // flags preserved (H and C)
    }

    #[test]
    fn c01_07_ldh_a_c_wraps_inside_high_page() {
        // Edge case: C = $FF; the address is $FF00+$FF = $FFFF, which wraps inside the high page.
        let mut bus = testutil::new_bus();
        bus.write(0xFFFF, 0x88); // IE cell at $FFFF
        let mut cpu = Cpu {
            c: 0xFF,
            a: 0x00,
            ..Cpu::default()
        };

        let ticks = testutil::exec(&mut cpu, &mut bus, &[0xF2]); // LDH A,(C)

        assert_eq!(ticks, 2);
        assert_eq!(cpu.a, 0x88); // read the byte at $FFFF (the high-page wrap target)
    }

    #[test]
    fn c01_07_ldh_c_a_masks_f_low_nibble() {
        // Flag boundary: a dirty low nibble of F is re-masked at instruction end.
        let mut bus = testutil::new_bus();
        bus.write(0xFF46, 0x37);
        let mut cpu = Cpu {
            c: 0x46,
            a: 0xD9,
            f: 0xFF, // all four flags set plus a dirty low nibble
            ..Cpu::default()
        };

        let ticks = testutil::exec(&mut cpu, &mut bus, &[0xE2]); // LDH (C),A

        assert_eq!(ticks, 2);
        assert_eq!(bus.peek(0xFF46), 0xD9);
        assert_eq!(cpu.f, 0xF0); // high nibble preserved, low nibble masked to zero
    }
}
