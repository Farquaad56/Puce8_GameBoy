//! SM83 CPU engine (decision C_00): hand-written M-cycle state machine. One tick is one
//! M-cycle with at most one bus access; opcodes are decoded by bit-field groups, no table.

pub mod load8;
pub mod load_ptr;
pub mod registers;

#[cfg(test)]
pub(crate) mod testutil;

use self::registers::{Registers, F_USED_BITS};
use crate::bus::Bus;

/// Post-boot CPU (decision C_00): public register fields plus the private M-cycle engine
/// state. Fixed size, Copy, no allocation.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Cpu {
    pub a: u8,
    /// Flags byte: bit 7 Z, bit 6 N, bit 5 H, bit 4 C; bits 3-0 always zero (note 02a).
    pub f: u8,
    pub b: u8,
    pub c: u8,
    pub d: u8,
    pub e: u8,
    pub h: u8,
    pub l: u8,
    pub sp: u16,
    pub pc: u16,

    /// Fetched opcode byte (decision C_00).
    opcode: u8,
    /// M-cycle step within the current instruction; 0 means instruction boundary.
    step: u8,
    /// Operand latch, low byte (decision C_00); used by groups from C01_05 on.
    #[allow(dead_code)]
    lo: u8,
    /// Operand latch, high byte (decision C_00); used by groups from C01_05 on.
    #[allow(dead_code)]
    hi: u8,
    /// CB prefix flag (decision C_00); used by the CB group in C01_25/C01_26.
    #[allow(dead_code)]
    cb: bool,
    /// Last opcode no group claimed, with the address of its byte (decision C_00).
    unimplemented: Option<(u8, u16)>,
}

impl Cpu {
    /// Post-boot values (note 02a "Valeurs de reset DMG des registres CPU"). Delegates to
    /// the register file so the reset values live in one place. Z set and N clear; H and C
    /// are both set iff the header checksum byte $014D is not $00 (Power_Up_Sequence.md#CPU-registers).
    pub fn reset(checksum: u8) -> Self {
        let regs = Registers::reset(checksum);
        Cpu {
            a: regs.a,
            f: regs.f,
            b: regs.b,
            c: regs.c,
            d: regs.d,
            e: regs.e,
            h: regs.h,
            l: regs.l,
            sp: regs.sp,
            pc: regs.pc,
            ..Self::default()
        }
    }

    /// View the register file (A F B C D E H L SP PC) with pair and flag accessors.
    pub fn regs(&self) -> Registers {
        Registers {
            a: self.a,
            f: self.f,
            b: self.b,
            c: self.c,
            d: self.d,
            e: self.e,
            h: self.h,
            l: self.l,
            sp: self.sp,
            pc: self.pc,
        }
    }

    /// True when no instruction is in flight (decision C_00).
    pub fn at_boundary(&self) -> bool {
        self.step == 0
    }

    /// Last opcode no group claimed, with the address of its byte (decision C_00).
    pub fn unimplemented(&self) -> Option<(u8, u16)> {
        self.unimplemented
    }

    /// Test seam for other crates (task C01_48): store an unimplemented-opcode record
    /// without executing anything. Not used by the engine itself.
    #[doc(hidden)]
    pub fn debug_set_unimplemented(&mut self, opcode: u8, pc: u16) {
        self.unimplemented = Some((opcode, pc));
    }

    /// Execute one M-cycle (decision C_00): at most one bus access.
    pub fn tick(&mut self, bus: &mut Bus) {
        if self.step == 0 {
            // Instruction boundary: fetch the opcode at PC (one bus read).
            let addr = self.pc;
            self.opcode = bus.read(addr);
            self.pc = self.pc.wrapping_add(1);
            self.step = 1;
        } else {
            self.step += 1;
        }

        // Group dispatch chain (decision C_00): each group returns true when it claims the
        // current opcode; the first claim wins and stops the chain. The groups are disjoint in
        // opcode space, so their relative order does not change which opcodes they cover.
        let claimed = self.exec_load8_reg(bus) || self.exec_load_ptr(bus);
        if !claimed {
            // No group claims this opcode: record it and cost its fetch M-cycle only.
            self.record_unimplemented(self.opcode);
        }
    }

    /// Record an opcode no group claimed (decision C_00): store the byte with the address
    /// of its byte, end the instruction so step is back to 0. No bus access here: the
    /// fetch already happened in `tick()`.
    fn record_unimplemented(&mut self, opcode: u8) {
        self.unimplemented = Some((opcode, self.pc.wrapping_sub(1)));
        self.done();
    }

    /// End of instruction (decision C_00): back to the boundary; F low nibble re-masked.
    fn done(&mut self) {
        // Bits 3-0 of F are "not used (always zero)" (note 02a).
        self.f &= F_USED_BITS;
        self.step = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::testutil;
    use super::Cpu;

    #[test]
    fn c01_01_at_boundary_after_reset() {
        assert!(Cpu::reset(0).at_boundary());
    }

    #[test]
    fn c01_01_default_is_all_zero() {
        let cpu = Cpu::default();
        assert_eq!(
            (cpu.a, cpu.f, cpu.b, cpu.c, cpu.d, cpu.e, cpu.h, cpu.l),
            (0, 0, 0, 0, 0, 0, 0, 0)
        );
        assert_eq!(cpu.sp, 0);
        assert_eq!(cpu.pc, 0);
        assert!(cpu.at_boundary());
        assert_eq!(cpu.unimplemented(), None);
    }

    #[test]
    fn c01_01_reset_values_checksum_zero() {
        let cpu = Cpu::reset(0);
        assert_eq!(cpu.a, 0x01);
        assert_eq!(cpu.f, 0x80); // Z set, N clear; H and C clear for checksum $00
        assert_eq!(cpu.b, 0x00);
        assert_eq!(cpu.c, 0x13);
        assert_eq!(cpu.d, 0x00);
        assert_eq!(cpu.e, 0xD8);
        assert_eq!(cpu.h, 0x01);
        assert_eq!(cpu.l, 0x4D);
        assert_eq!(cpu.sp, 0xFFFE);
        assert_eq!(cpu.pc, 0x0100);
        let r = cpu.regs();
        assert_eq!(
            (r.a, r.f, r.b, r.c, r.d, r.e, r.h, r.l),
            (0x01, 0x80, 0x00, 0x13, 0x00, 0xD8, 0x01, 0x4D)
        );
        assert_eq!(r.sp, 0xFFFE);
        assert_eq!(r.pc, 0x0100);
    }

    #[test]
    fn c01_01_reset_values_checksum_nonzero() {
        let cpu = Cpu::reset(0x4D);
        assert_eq!(cpu.f, 0xB0); // Z set plus H and C both set
    }

    #[test]
    fn c01_01_unknown_opcode_recorded_at_address_and_costs_one_tick() {
        // Dirty F low nibble: it must be re-masked at instruction end.
        let mut cpu = Cpu {
            f: 0xFF,
            ..Cpu::default()
        };
        cpu.pc = 0xC001; // as if the opcode byte was just fetched from $C000

        cpu.record_unimplemented(0x99);

        assert_eq!(cpu.unimplemented(), Some((0x99, 0xC000)));
        assert_eq!(cpu.step, 0); // step back to 0
        assert!(cpu.at_boundary());
        assert_eq!(cpu.f & 0x0F, 0); // F low nibble is 0
    }

    #[test]
    fn c01_01_unknown_opcode_record_overwritten_by_next_fetch() {
        let mut cpu = Cpu {
            pc: 0xC001, // as if the first opcode byte was just fetched from $C000
            ..Cpu::default()
        };

        cpu.record_unimplemented(0x99);
        assert_eq!(cpu.unimplemented(), Some((0x99, 0xC000)));

        // The next fetch records the new opcode at its own address.
        cpu.pc = 0xC002; // as if a second byte was just fetched from $C001
        cpu.record_unimplemented(0xA5);
        assert_eq!(cpu.unimplemented(), Some((0xA5, 0xC001)));
    }

    #[test]
    fn c01_48_debug_set_unimplemented_stores_record() {
        // Test seam (task C01_48): store a record without executing anything, so other
        // crates can test the unimplemented-opcode path with real opcodes.
        let mut cpu = Cpu::default();
        assert_eq!(cpu.unimplemented(), None);

        cpu.debug_set_unimplemented(0x99, 0x0100);

        assert_eq!(cpu.unimplemented(), Some((0x99, 0x0100)));
    }

    #[test]
    fn c01_42_empty_dispatch_fetches_one_byte() {
        // One tick at an instruction boundary is exactly one bus read: the fetch. The
        // assertions below hold whatever the dispatch chain does afterwards, so this
        // test needs no real opcode (decision C_00).
        let mut bus = testutil::new_bus();
        bus.write(0xC000, 0x7E);
        let mut cpu = Cpu {
            pc: 0xC000,
            ..Cpu::default()
        };

        cpu.tick(&mut bus);

        assert_eq!(cpu.opcode, 0x7E); // the byte read from $C000
        assert_eq!(cpu.pc, 0xC001); // PC advanced by exactly one
    }
}
