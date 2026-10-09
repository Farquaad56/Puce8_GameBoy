//! Pointer-register 8-bit loads (task C01_06): LD r,(HL), LD (HL),r, LD (HL),n and the
//! pair/HL+/- family LD A,(BC)/(DE)/(HL+)/(HL-) and LD (BC)/(DE)/(HL+)/(HL-),A. Decoded by
//! opcode bit fields per decision C_00; no table.

use super::Cpu;
use crate::bus::Bus;

impl Cpu {
    /// Pointer-register 8-bit loads (task C01_06). Returns true when the current opcode is in
    /// this group, false otherwise. Timings and flag effects from note 02c (seed Opcodes.json):
    /// - LD r,(HL) ($46/$4E/$56/$5E/$66/$6E): 8 T = 2 M-cycles, no flags; the memory byte is
    ///   read on step 2 (at most one bus access per decision C_00).
    /// - LD (HL),r ($70..$75/$77): 8 T = 2 M-cycles, no flags; the register value is written to
    ///   memory on step 2.
    /// - LD (HL),n ($36): 12 T = 3 M-cycles, no flags; the immediate is read on step 2 and stored
    ///   in memory on step 3.
    /// - LD A,(rr) / LD (rr),A (block 0, z == 2, rr = BC/DE/HL+/HL- by y>>1): 8 T = 2 M-cycles,
    ///   no flags; the single memory access happens on step 2. The HL+ and HL- forms advance or
    ///   decrement HL after the access (wrapping).
    pub(super) fn exec_load_ptr(&mut self, bus: &mut Bus) -> bool {
        let op = self.opcode;

        // LD r,(HL): block 1, destination a plain register, source (HL).
        if x_is(op, 1) && y_is_not(op, 6) && z_is(op, 6) {
            match self.step {
                // Step 1 is the fetch M-cycle (decision C_00): no bus access yet.
                1 => return true,
                _ => {
                    let value = bus.read(self.hl());
                    self.store_r8(y_of(op), value);
                    self.done();
                    return true;
                }
            }
        }

        // LD (HL),r: block 1, destination (HL), source a plain register.
        if x_is(op, 1) && y_is(op, 6) && z_is_not(op, 6) {
            match self.step {
                1 => return true,
                _ => {
                    let value = self.read_r8(z_of(op));
                    bus.write(self.hl(), value);
                    self.done();
                    return true;
                }
            }
        }

        // LD (HL),n: block 0, destination (HL), immediate source. Three M-cycles.
        if x_is(op, 0) && y_is(op, 6) && z_is(op, 6) {
            match self.step {
                1 => return true, // fetch M-cycle: no bus access yet
                2 => {
                    let value = bus.read(self.pc);
                    self.pc = self.pc.wrapping_add(1);
                    self.lo = value; // latch the immediate for the write M-cycle
                    return true;
                }
                _ => {
                    bus.write(self.hl(), self.lo);
                    self.done();
                    return true;
                }
            }
        }

        // LD A,(rr) / LD (rr),A: block 0, z == 2. rr = BC/DE/HL+/HL- selected by y>>1; the low
        // bit of y picks the direction (even = store to memory, odd = load into A).
        if x_is(op, 0) && z_is(op, 2) {
            match self.step {
                1 => return true, // fetch M-cycle: no bus access yet
                _ => {
                    let addr = self.ptr_reg(y_of(op));
                    if y_of(op) & 1 == 0 {
                        bus.write(addr, self.a); // LD (rr),A
                    } else {
                        self.a = bus.read(addr); // LD A,(rr)
                    }
                    match y_of(op) >> 1 {
                        2 => self.hl_inc(), // (HL+) form: advance HL after the access
                        3 => self.hl_dec(), // (HL-) form: decrement HL after the access
                        _ => {}
                    }
                    self.done();
                    return true;
                }
            }
        }

        false
    }

    /// The 16-bit value of HL, computed from the public register fields.
    pub(super) fn hl(&self) -> u16 {
        ((self.h as u16) << 8) | self.l as u16
    }

    /// 16-bit value of the pointer register selected by bit-field index y>>1 for the z==2 family:
    /// BC / DE / HL. The (HL+) and (HL-) forms are handled by the caller, which advances or
    /// decrements HL after the memory access.
    fn ptr_reg(&self, y: u8) -> u16 {
        match y >> 1 {
            0 => ((self.b as u16) << 8) | self.c as u16,
            1 => ((self.d as u16) << 8) | self.e as u16,
            _ => ((self.h as u16) << 8) | self.l as u16,
        }
    }

    /// Advance HL by one (wrapping), for the (HL+) forms.
    fn hl_inc(&mut self) {
        let next = self.hl().wrapping_add(1);
        self.h = (next >> 8) as u8;
        self.l = next as u8;
    }

    /// Decrement HL by one (wrapping), for the (HL-) forms.
    fn hl_dec(&mut self) {
        let next = self.hl().wrapping_sub(1);
        self.h = (next >> 8) as u8;
        self.l = next as u8;
    }
}

// ---- Opcode bit-field helpers (decision C_00: x = op>>6, y = (op>>3)&7, z = op&7) ----

fn x_is(op: u8, v: u8) -> bool {
    op >> 6 == v
}
fn y_is(op: u8, v: u8) -> bool {
    (op >> 3) & 7 == v
}
fn y_is_not(op: u8, v: u8) -> bool {
    (op >> 3) & 7 != v
}
fn z_is(op: u8, v: u8) -> bool {
    op & 7 == v
}
fn z_is_not(op: u8, v: u8) -> bool {
    op & 7 != v
}
fn y_of(op: u8) -> u8 {
    (op >> 3) & 7
}
fn z_of(op: u8) -> u8 {
    op & 7
}

#[cfg(test)]
mod tests {
    use crate::cpu::{testutil, Cpu};

    // A WRAM data cell kept clear of the $C000 code area that testutil::exec uses.
    const DATA: u16 = 0xC100;

    /// Seed a known byte at DATA and return it for assertions.
    fn seed_data(bus: &mut crate::bus::Bus, value: u8) -> u8 {
        bus.write(DATA, value);
        value
    }

    // ---- Family 1: LD r,(HL) (note 02c: [8 T = 2 M], no flags) ----

    #[test]
    fn c01_06_ld_b_hl_costs_two_ticks() {
        let mut bus = testutil::new_bus();
        let value = seed_data(&mut bus, 0xAB);
        let mut cpu = Cpu {
            h: (DATA >> 8) as u8,
            l: DATA as u8,
            b: 0x00,
            f: 0x80, // Z set
            ..Cpu::default()
        };

        let ticks = testutil::exec(&mut cpu, &mut bus, &[0x46]); // LD B,(HL)

        assert_eq!(ticks, 2);
        assert_eq!(cpu.b, value); // destination now holds the memory byte
        assert_eq!(cpu.pc, 0xC001); // PC advanced past the opcode
        assert_eq!(cpu.f, 0x80); // flags unchanged (Z preserved)
    }

    #[test]
    fn c01_06_ld_a_hl_reads_memory() {
        let mut bus = testutil::new_bus();
        let value = seed_data(&mut bus, 0x5C);
        let mut cpu = Cpu {
            h: (DATA >> 8) as u8,
            l: DATA as u8,
            a: 0x00,
            ..Cpu::default()
        };

        let ticks = testutil::exec(&mut cpu, &mut bus, &[0x7E]); // LD A,(HL)

        assert_eq!(ticks, 2);
        assert_eq!(cpu.a, value);
    }

    // ---- Family 2: LD (HL),r (note 02c: [8 T = 2 M], no flags) ----

    #[test]
    fn c01_06_ld_hl_a_writes_memory() {
        let mut bus = testutil::new_bus();
        seed_data(&mut bus, 0x00); // start with a known zero at DATA
        let mut cpu = Cpu {
            h: (DATA >> 8) as u8,
            l: DATA as u8,
            a: 0x7E,
            f: 0x30, // H and C set
            ..Cpu::default()
        };

        let ticks = testutil::exec(&mut cpu, &mut bus, &[0x77]); // LD (HL),A

        assert_eq!(ticks, 2);
        assert_eq!(bus.peek(DATA), 0x7E); // memory now holds A
        assert_eq!(cpu.a, 0x7E); // source unchanged
        assert_eq!(cpu.f, 0x30); // flags preserved (H and C)
    }

    #[test]
    fn c01_06_ld_hl_b_writes_memory() {
        let mut bus = testutil::new_bus();
        seed_data(&mut bus, 0x00);
        let mut cpu = Cpu {
            h: (DATA >> 8) as u8,
            l: DATA as u8,
            b: 0x13,
            ..Cpu::default()
        };

        let ticks = testutil::exec(&mut cpu, &mut bus, &[0x70]); // LD (HL),B

        assert_eq!(ticks, 2);
        assert_eq!(bus.peek(DATA), 0x13);
    }

    // ---- Family 3: LD (HL),n (note 02c: [12 T = 3 M], no flags) ----

    #[test]
    fn c01_06_ld_hl_n_costs_three_ticks() {
        let mut bus = testutil::new_bus();
        seed_data(&mut bus, 0x00);
        let mut cpu = Cpu {
            h: (DATA >> 8) as u8,
            l: DATA as u8,
            ..Cpu::default()
        };

        let ticks = testutil::exec(&mut cpu, &mut bus, &[0x36, 0xAB]); // LD (HL),$AB

        assert_eq!(ticks, 3);
        assert_eq!(bus.peek(DATA), 0xAB); // memory now holds the immediate
        assert_eq!(cpu.pc, 0xC002); // PC advanced past opcode and immediate
    }

    #[test]
    fn c01_06_ld_hl_n_masks_f_low_nibble() {
        // Flag boundary: a dirty low nibble of F is re-masked at instruction end; the high
        // nibble (the four real flags) is preserved because no flag is affected.
        let mut bus = testutil::new_bus();
        seed_data(&mut bus, 0x00);
        let mut cpu = Cpu {
            h: (DATA >> 8) as u8,
            l: DATA as u8,
            f: 0xFF, // all four flags set plus a dirty low nibble
            ..Cpu::default()
        };

        let ticks = testutil::exec(&mut cpu, &mut bus, &[0x36, 0x5A]); // LD (HL),$5A

        assert_eq!(ticks, 3);
        assert_eq!(bus.peek(DATA), 0x5A);
        assert_eq!(cpu.f, 0xF0); // high nibble preserved, low nibble masked to zero
    }

    // ---- Family 4: LD A,(BC) / LD A,(DE) (note 02c: [8 T = 2 M], no flags) ----

    #[test]
    fn c01_06_ld_a_bc_costs_two_ticks() {
        let mut bus = testutil::new_bus();
        let value = seed_data(&mut bus, 0x9D);
        let mut cpu = Cpu {
            b: (DATA >> 8) as u8,
            c: DATA as u8,
            a: 0x00,
            ..Cpu::default()
        };

        let ticks = testutil::exec(&mut cpu, &mut bus, &[0x0A]); // LD A,(BC)

        assert_eq!(ticks, 2);
        assert_eq!(cpu.a, value);
    }

    #[test]
    fn c01_06_ld_a_de_costs_two_ticks() {
        let mut bus = testutil::new_bus();
        let value = seed_data(&mut bus, 0x42);
        let mut cpu = Cpu {
            d: (DATA >> 8) as u8,
            e: DATA as u8,
            a: 0x00,
            ..Cpu::default()
        };

        let ticks = testutil::exec(&mut cpu, &mut bus, &[0x1A]); // LD A,(DE)

        assert_eq!(ticks, 2);
        assert_eq!(cpu.a, value);
    }

    // ---- Family 5: LD (BC),A / LD (DE),A (note 02c: [8 T = 2 M], no flags) ----

    #[test]
    fn c01_06_ld_bc_a_writes_memory() {
        let mut bus = testutil::new_bus();
        seed_data(&mut bus, 0x00);
        let mut cpu = Cpu {
            b: (DATA >> 8) as u8,
            c: DATA as u8,
            a: 0xE1,
            ..Cpu::default()
        };

        let ticks = testutil::exec(&mut cpu, &mut bus, &[0x02]); // LD (BC),A

        assert_eq!(ticks, 2);
        assert_eq!(bus.peek(DATA), 0xE1);
    }

    #[test]
    fn c01_06_ld_de_a_writes_memory() {
        let mut bus = testutil::new_bus();
        seed_data(&mut bus, 0x00);
        let mut cpu = Cpu {
            d: (DATA >> 8) as u8,
            e: DATA as u8,
            a: 0x37,
            ..Cpu::default()
        };

        let ticks = testutil::exec(&mut cpu, &mut bus, &[0x12]); // LD (DE),A

        assert_eq!(ticks, 2);
        assert_eq!(bus.peek(DATA), 0x37);
    }

    // ---- Family 6: LD A,(HL+) / LD A,(HL-) (note 02c: [8 T = 2 M], no flags) ----

    #[test]
    fn c01_06_ld_a_hl_inc_reads_then_advances() {
        let mut bus = testutil::new_bus();
        seed_data(&mut bus, 0x77); // value at DATA (the pre-increment address)
        let mut cpu = Cpu {
            h: (DATA >> 8) as u8,
            l: DATA as u8,
            a: 0x00,
            ..Cpu::default()
        };

        let ticks = testutil::exec(&mut cpu, &mut bus, &[0x2A]); // LD A,(HL+)

        assert_eq!(ticks, 2);
        assert_eq!(cpu.a, 0x77); // read the byte at the old HL
        assert_eq!((cpu.h as u16) << 8 | cpu.l as u16, DATA.wrapping_add(1)); // HL advanced
    }

    #[test]
    fn c01_06_ld_a_hl_dec_reads_then_decrements() {
        let mut bus = testutil::new_bus();
        seed_data(&mut bus, 0x88);
        let mut cpu = Cpu {
            h: (DATA >> 8) as u8,
            l: DATA as u8,
            a: 0x00,
            ..Cpu::default()
        };

        let ticks = testutil::exec(&mut cpu, &mut bus, &[0x3A]); // LD A,(HL-)

        assert_eq!(ticks, 2);
        assert_eq!(cpu.a, 0x88);
        assert_eq!((cpu.h as u16) << 8 | cpu.l as u16, DATA.wrapping_sub(1)); // HL decremented
    }

    #[test]
    fn c01_06_ld_a_hl_inc_wraps_at_top_of_memory() {
        // Edge case: HL = $FFFF; LD A,(HL+) reads the byte at $FFFF and wraps HL to $0000.
        let mut bus = testutil::new_bus();
        bus.write(0xFFFF, 0x63);
        let mut cpu = Cpu {
            h: 0xFF,
            l: 0xFF,
            a: 0x00,
            ..Cpu::default()
        };

        let ticks = testutil::exec(&mut cpu, &mut bus, &[0x2A]); // LD A,(HL+)

        assert_eq!(ticks, 2);
        assert_eq!(cpu.a, 0x63);
        assert_eq!((cpu.h as u16) << 8 | cpu.l as u16, 0x0000); // wrapped to $0000
    }

    #[test]
    fn c01_06_ld_a_hl_dec_wraps_at_zero() {
        // Edge case: HL = $0000; LD A,(HL-) reads the byte at $0000 and wraps HL to $FFFF.
        // $0000 is in the ROM region, which is read-only (decision A_06), so the zero-ROM value
        // 0x00 is read back; the point of this test is the HL wrap-around, not the byte value.
        let mut bus = testutil::new_bus();
        let mut cpu = Cpu {
            h: 0x00,
            l: 0x00,
            a: 0xFF, // start non-zero so a read of 0x00 is observable
            ..Cpu::default()
        };

        let ticks = testutil::exec(&mut cpu, &mut bus, &[0x3A]); // LD A,(HL-)

        assert_eq!(ticks, 2);
        assert_eq!(cpu.a, 0x00); // zero-ROM byte read from $0000 (ROM is read-only)
        assert_eq!((cpu.h as u16) << 8 | cpu.l as u16, 0xFFFF); // wrapped to $FFFF
    }

    // ---- Family 7: LD (HL+),A / LD (HL-),A (note 02c: [8 T = 2 M], no flags) ----

    #[test]
    fn c01_06_ld_hl_inc_a_writes_then_advances() {
        let mut bus = testutil::new_bus();
        seed_data(&mut bus, 0x00);
        let mut cpu = Cpu {
            h: (DATA >> 8) as u8,
            l: DATA as u8,
            a: 0xC5,
            ..Cpu::default()
        };

        let ticks = testutil::exec(&mut cpu, &mut bus, &[0x22]); // LD (HL+),A

        assert_eq!(ticks, 2);
        assert_eq!(bus.peek(DATA), 0xC5); // wrote A at the old HL
        assert_eq!((cpu.h as u16) << 8 | cpu.l as u16, DATA.wrapping_add(1)); // HL advanced
    }

    #[test]
    fn c01_06_ld_hl_dec_a_writes_then_decrements() {
        let mut bus = testutil::new_bus();
        seed_data(&mut bus, 0x00);
        let mut cpu = Cpu {
            h: (DATA >> 8) as u8,
            l: DATA as u8,
            a: 0xD9,
            ..Cpu::default()
        };

        let ticks = testutil::exec(&mut cpu, &mut bus, &[0x32]); // LD (HL-),A

        assert_eq!(ticks, 2);
        assert_eq!(bus.peek(DATA), 0xD9);
        assert_eq!((cpu.h as u16) << 8 | cpu.l as u16, DATA.wrapping_sub(1)); // HL decremented
    }
}
