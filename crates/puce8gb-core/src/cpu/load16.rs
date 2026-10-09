//! 16-bit immediate loads (task C01_08): LD BC,nn / DE,nn / HL,nn / SP,nn, LD SP,HL and
//! LD (a16),SP. Decoded by opcode bit fields per decision C_00; no table.

use super::Cpu;
use crate::bus::Bus;

impl Cpu {
    /// 16-bit immediate loads (task C01_08). Returns true when the current opcode is in this
    /// group, false otherwise. Timings and flag effects from note 02c (seed Opcodes.json):
    /// - LD rr,nn ($01/$11/$21/$31; block 0, z == 1, y even so y>>1 selects BC/DE/HL/SP):
    ///   3 bytes, 12 T = 3 M-cycles, no flags. The low byte of nn is read on step 2 and the
    ///   high byte on step 3 (little-endian order, note 02c); the pair is stored to rr on
    ///   step 3 (a register write, so no bus access). At most one bus access per M-cycle
    ///   (decision C_00).
    /// - LD SP,HL ($F9; block 3, y == 7): 1 byte, 8 T = 2 M-cycles, no flags. sp = hl() on
    ///   step 2 (a register copy, so no bus access).
    /// - LD (a16),SP ($08; block 0, y == 1): 3 bytes, 20 T = 5 M-cycles, no flags. The low
    ///   byte of a16 is read on step 2 and the high byte on step 3 (little-endian order, note
    ///   02c); both bytes of SP are stored to memory on steps 4 and 5 (low byte at a16, high
    ///   byte at a16+1). The two-byte store is deduced from the 20 T timing: it is one M-cycle
    ///   more than the single-byte $EA form ($EA = 16 T = 4 M), leaving exactly two data-write
    ///   M-cycles. The little-endian order follows note 02c's a16 convention.
    pub(super) fn exec_load16(&mut self, bus: &mut Bus) -> bool {
        let op = self.opcode;

        // LD rr,nn: block 0, z == 1, y even (y>>1 selects BC/DE/HL/SP). Three M-cycles.
        if x_is(op, 0) && z_is(op, 1) && (y_of(op) & 1) == 0 {
            match self.step {
                // Step 1 is the fetch M-cycle (decision C_00): no bus access yet.
                1 => return true,
                // Step 2: read the low byte of nn at PC; latch it for step 3.
                2 => {
                    self.lo = bus.read(self.pc);
                    self.pc = self.pc.wrapping_add(1);
                    return true;
                }
                _ => {
                    // Step 3: read the high byte of nn (little-endian order, note 02c) and store
                    // the pair to rr. The register write is internal (no bus access).
                    self.hi = bus.read(self.pc);
                    self.pc = self.pc.wrapping_add(1);
                    let value = ((self.hi as u16) << 8) | self.lo as u16;
                    match y_of(op) >> 1 {
                        0 => {
                            self.b = (value >> 8) as u8;
                            self.c = value as u8; // BC
                        }
                        1 => {
                            self.d = (value >> 8) as u8;
                            self.e = value as u8; // DE
                        }
                        2 => {
                            self.h = (value >> 8) as u8;
                            self.l = value as u8; // HL
                        }
                        _ => self.sp = value, // SP
                    }
                    self.done();
                    return true;
                }
            }
        }

        // LD SP,HL: block 3, y == 7, z == 1 ($F9 only). Two M-cycles; sp = hl() on step 2
        // (register copy). The z guard excludes the other block-3 y == 7 opcodes ($F8 ADD
        // SP,e8, $FA LD A,(a16), $FB/$FC/$FD, $FF RST $38): without it this group would
        // swallow the immediate of any of them, such as CP A,n ($FE).
        if x_is(op, 3) && y_is(op, 7) && z_is(op, 1) {
            match self.step {
                1 => return true, // fetch M-cycle: no bus access yet
                _ => {
                    let value = ((self.h as u16) << 8) | self.l as u16;
                    self.sp = value;
                    self.done();
                    return true;
                }
            }
        }

        // LD (a16),SP: block 0, y == 1. Five M-cycles; a16 read little-endian on steps 2-3 and
        // both bytes of SP stored to memory on steps 4 and 5 (low byte at a16, high byte at
        // a16+1). At most one bus access per M-cycle (decision C_00).
        if x_is(op, 0) && y_is(op, 1) {
            match self.step {
                1 => return true, // fetch M-cycle: no bus access yet
                2 => {
                    self.lo = bus.read(self.pc);
                    self.pc = self.pc.wrapping_add(1);
                    return true;
                }
                3 => {
                    self.hi = bus.read(self.pc);
                    self.pc = self.pc.wrapping_add(1);
                    return true;
                }
                // Step 4: store the low byte of SP at a16 (one bus access).
                4 => {
                    let addr = ((self.hi as u16) << 8) | self.lo as u16;
                    bus.write(addr, self.sp as u8);
                    return true;
                }
                // Step 5: store the high byte of SP at a16+1 (wrapping), then end.
                _ => {
                    let addr = ((self.hi as u16) << 8) | self.lo as u16;
                    bus.write(addr.wrapping_add(1), (self.sp >> 8) as u8);
                    self.done();
                    return true;
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
fn y_is(op: u8, v: u8) -> bool {
    (op >> 3) & 7 == v
}
fn z_is(op: u8, v: u8) -> bool {
    op & 7 == v
}
fn y_of(op: u8) -> u8 {
    (op >> 3) & 7
}

#[cfg(test)]
mod tests {
    use crate::cpu::{testutil, Cpu};

    // A WRAM data cell kept clear of the $C000 code area that testutil::exec uses.
    const DATA: u16 = 0xC100;

    // ---- Family 1: LD rr,nn (note 02c seed Opcodes.json: [12 T = 3 M], no flags) ----

    #[test]
    fn c01_08_ld_bc_nn_costs_three_ticks() {
        let mut bus = testutil::new_bus();
        let mut cpu = Cpu {
            b: 0x00,
            c: 0x00,
            f: 0x80, // Z set
            ..Cpu::default()
        };

        let ticks = testutil::exec(&mut cpu, &mut bus, &[0x01, 0x34, 0x12]); // LD BC,$1234

        assert_eq!(ticks, 3); // note 02c: 12 T = 3 M-cycles
        assert_eq!((cpu.b as u16) << 8 | cpu.c as u16, 0x1234); // BC now holds nn (little-endian)
        assert_eq!(cpu.pc, 0xC003); // PC advanced past opcode and both immediate bytes
        assert_eq!(cpu.f, 0x80); // flags unchanged (Z preserved)
    }

    #[test]
    fn c01_08_ld_de_nn_loads_pair() {
        let mut bus = testutil::new_bus();
        let mut cpu = Cpu {
            d: 0x00,
            e: 0x00,
            ..Cpu::default()
        };

        let ticks = testutil::exec(&mut cpu, &mut bus, &[0x11, 0xAB, 0xCD]); // LD DE,$CDAB

        assert_eq!(ticks, 3); // note 02c: 12 T = 3 M-cycles
        assert_eq!((cpu.d as u16) << 8 | cpu.e as u16, 0xCDAB); // DE now holds nn (little-endian)
    }

    #[test]
    fn c01_08_ld_hl_nn_loads_pair() {
        let mut bus = testutil::new_bus();
        let mut cpu = Cpu {
            h: 0x00,
            l: 0x00,
            ..Cpu::default()
        };

        let ticks = testutil::exec(&mut cpu, &mut bus, &[0x21, 0x00, 0xC1]); // LD HL,$C100

        assert_eq!(ticks, 3); // note 02c: 12 T = 3 M-cycles
        assert_eq!((cpu.h as u16) << 8 | cpu.l as u16, DATA); // HL now holds nn (little-endian)
    }

    #[test]
    fn c01_08_ld_sp_nn_loads_pair() {
        let mut bus = testutil::new_bus();
        let mut cpu = Cpu {
            sp: 0x0000,
            ..Cpu::default()
        };

        let ticks = testutil::exec(&mut cpu, &mut bus, &[0x31, 0xFE, 0xFF]); // LD SP,$FFFE

        assert_eq!(ticks, 3); // note 02c: 12 T = 3 M-cycles
        assert_eq!(cpu.sp, 0xFFFE); // SP now holds nn (little-endian)
    }

    #[test]
    fn c01_08_ld_rr_nn_masks_f_low_nibble() {
        // Flag boundary: a dirty low nibble of F is re-masked at instruction end; the high
        // nibble (the four real flags) is preserved because no flag is affected.
        let mut bus = testutil::new_bus();
        let mut cpu = Cpu {
            b: 0x00,
            c: 0x00,
            f: 0xFF, // all four flags set plus a dirty low nibble
            ..Cpu::default()
        };

        let ticks = testutil::exec(&mut cpu, &mut bus, &[0x01, 0x34, 0x12]); // LD BC,$1234

        assert_eq!(ticks, 3);
        assert_eq!((cpu.b as u16) << 8 | cpu.c as u16, 0x1234);
        assert_eq!(cpu.f, 0xF0); // high nibble preserved, low nibble masked to zero
    }

    // ---- Family 2: LD SP,HL (note 02c seed Opcodes.json: [8 T = 2 M], no flags) ----

    #[test]
    fn c01_08_ld_sp_hl_costs_two_ticks() {
        let mut bus = testutil::new_bus();
        let mut cpu = Cpu {
            h: (DATA >> 8) as u8,
            l: DATA as u8,
            sp: 0x0000,
            f: 0x30, // H and C set
            ..Cpu::default()
        };

        let ticks = testutil::exec(&mut cpu, &mut bus, &[0xF9]); // LD SP,HL

        assert_eq!(ticks, 2); // note 02c: 8 T = 2 M-cycles
        assert_eq!(cpu.sp, DATA); // SP now holds HL (register copy)
        assert_eq!(cpu.pc, 0xC001); // PC advanced past the opcode
        assert_eq!(cpu.f, 0x30); // flags preserved (H and C)
    }

    #[test]
    fn c01_08_ld_sp_hl_masks_f_low_nibble() {
        // Flag boundary: a dirty low nibble of F is re-masked at instruction end.
        let mut bus = testutil::new_bus();
        let mut cpu = Cpu {
            h: (DATA >> 8) as u8,
            l: DATA as u8,
            sp: 0x0000,
            f: 0xFF, // all four flags set plus a dirty low nibble
            ..Cpu::default()
        };

        let ticks = testutil::exec(&mut cpu, &mut bus, &[0xF9]); // LD SP,HL

        assert_eq!(ticks, 2);
        assert_eq!(cpu.sp, DATA);
        assert_eq!(cpu.f, 0xF0); // high nibble preserved, low nibble masked to zero
    }

    // ---- Family 3: LD (a16),SP (note 02c seed Opcodes.json: [20 T = 5 M], no flags) ----

    #[test]
    fn c01_08_ld_a16_sp_costs_five_ticks() {
        let mut bus = testutil::new_bus();
        // a16 = $C100 (low byte first: $00 then $C1). Seed both target cells to zero.
        bus.write(DATA, 0x00);
        bus.write(DATA.wrapping_add(1), 0x00);
        let mut cpu = Cpu {
            sp: 0x1234,
            f: 0x80, // Z set
            ..Cpu::default()
        };

        let ticks = testutil::exec(&mut cpu, &mut bus, &[0x08, 0x00, 0xC1]); // LD ($C100),SP

        assert_eq!(ticks, 5); // note 02c: 20 T = 5 M-cycles
        assert_eq!(bus.peek(DATA), 0x34); // low byte of SP stored at a16 (little-endian)
        assert_eq!(bus.peek(DATA.wrapping_add(1)), 0x12); // high byte of SP stored at a16+1
        assert_eq!(cpu.sp, 0x1234); // source unchanged
        assert_eq!(cpu.pc, 0xC003); // PC advanced past opcode and both address bytes
        assert_eq!(cpu.f, 0x80); // flags unchanged (Z preserved)
    }

    #[test]
    fn c01_08_ld_a16_sp_wraps_at_top_of_memory() {
        // Edge case: a16 = $FFFF; the low byte of SP is stored at $FFFF and the high byte wraps
        // to $0000. $0000 is in the ROM region, which is read-only (decision A_06), so that
        // write is dropped by the bus; only the $FFFF store is observable.
        let mut bus = testutil::new_bus();
        let mut cpu = Cpu {
            sp: 0x1234,
            ..Cpu::default()
        };

        let ticks = testutil::exec(&mut cpu, &mut bus, &[0x08, 0xFF, 0xFF]); // LD ($FFFF),SP

        assert_eq!(ticks, 5); // note 02c: 20 T = 5 M-cycles
        assert_eq!(bus.peek(0xFFFF), 0x34); // low byte of SP stored at $FFFF (the IE cell)
        assert_eq!(cpu.sp, 0x1234); // source unchanged
    }

    #[test]
    fn c01_08_ld_a16_sp_masks_f_low_nibble() {
        // Flag boundary: a dirty low nibble of F is re-masked at instruction end.
        let mut bus = testutil::new_bus();
        bus.write(DATA, 0x00);
        bus.write(DATA.wrapping_add(1), 0x00);
        let mut cpu = Cpu {
            sp: 0x5678,
            f: 0xFF, // all four flags set plus a dirty low nibble
            ..Cpu::default()
        };

        let ticks = testutil::exec(&mut cpu, &mut bus, &[0x08, 0x00, 0xC1]); // LD ($C100),SP

        assert_eq!(ticks, 5);
        assert_eq!(bus.peek(DATA), 0x78);
        assert_eq!(bus.peek(DATA.wrapping_add(1)), 0x56);
        assert_eq!(cpu.f, 0xF0); // high nibble preserved, low nibble masked to zero
    }
}
