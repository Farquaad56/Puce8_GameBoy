//! SM83 CPU (decision A_03): registers and post-boot state for now; instruction execution,
//! interrupts and HALT/STOP arrive with later tasks.

pub mod registers;

use self::registers::Registers;
use crate::bus::Bus;

/// Micro-op execution state (decision A_03).
#[derive(Default, Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum InstrState {
    /// No instruction in flight; next tick() fetches opcode from PC.
    #[default]
    Idle,
    /// Currently executing one instruction (decision A_03): `info` is the decoded
    /// entry, `remaining` counts down to 0 over the M-cycle budget, and
    /// `op_periph_read` tracks how many operand bytes have been consumed.
    Active {
        info: OpInfo,
        remaining: u8,
        op_periph_read: u8,
    },
}

/// Opcode metadata for the micro-op engine (decision A_03).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OpInfo {
    /// Total bytes in the instruction (opcode + immediate values).
    pub bytes: u8,
    /// M-cycle count for the "taken" case.
    pub m_taken: u8,
}

impl Default for OpInfo {
    fn default() -> Self {
        // NOP-like default: 1 byte, 1 M-cycle.
        Self {
            bytes: 1,
            m_taken: 1,
        }
    }
}

/// Minimal inline opcode table for E02_01 (full generated table arrives with E02_02).
/// All entries default to NOP-like timing; only index 0x00 (the NOP opcode) is correct.
const OP_NOP: OpInfo = OpInfo {
    bytes: 1,
    m_taken: 1,
};
pub const OPCODES: [OpInfo; 256] = [OP_NOP; 256];

/// Post-boot CPU register state at PC=$0100 on DMG (note 08 "Registres CPU apres boot").
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Cpu {
    pub a: u8,
    /// Flags byte: bit 7 Z, bit 6 N, bit 5 H, bit 4 C.
    /// Bits 3-0 are "not used (always zero)" per note 02a.
    pub f: u8,
    pub b: u8,
    pub c: u8,
    pub d: u8,
    pub e: u8,
    pub h: u8,
    pub l: u8,
    pub sp: u16,
    pub pc: u16,

    /// Micro-op engine state (decision A_03).
    #[cfg_attr(test, allow(dead_code))]
    instr_state: InstrState,
}

impl Cpu {
    /// Post-boot values (note 02a "Valeurs de reset DMG des registres CPU"). Delegates to
    /// the register file so the reset values live in one place. Z set and N clear; H and C
    /// are both set iff the header checksum byte $014D is not $00 (Power_Up_Sequence.md#CPU-registers).
    /// CONFLIT: specs 2001 give fixed H=1 C=1; to be settled by mooneye boot_regs-dmgABC.
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
            instr_state: InstrState::Idle,
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

    /// Execute one M-cycle (decision A_03): exactly one bus access per call.
    pub fn tick(&mut self, bus: &mut Bus) {
        // Fetch new opcode if no instruction in flight (decision A_03).
        if matches!(self.instr_state, InstrState::Idle) {
            let opcode = bus.read(self.pc);
            self.pc = self.pc.wrapping_add(1);

            let info = OPCODES[opcode as usize];
            self.instr_state = InstrState::Active {
                info,
                remaining: 0, // set below after confirming we enter the active arm
                op_periph_read: 0,
            };
        }

        if let InstrState::Active {
            ref mut info,
            ref mut remaining,
            ref mut op_periph_read,
        } = self.instr_state
        {
            *remaining += 1;

            // For multi-byte instructions, read the next operand byte from PC.
            if *op_periph_read < info.bytes.saturating_sub(1) && *remaining <= info.m_taken {
                let _byte = bus.read(self.pc);
                self.pc = self.pc.wrapping_add(1);
                *op_periph_read += 1;
            }

            if *remaining >= info.m_taken {
                // Instruction boundary (note 02a): bits 3-0 of F are "not used (always zero)",
                // so the low nibble is re-masked whenever an instruction completes.
                self.f &= registers::F_USED_BITS;
                self.instr_state = InstrState::Idle;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper to place code bytes in WRAM and run until the first instruction boundary.
    pub fn exec(cpu: &mut Cpu, bus: &mut Bus, code: &[u8]) -> u32 {
        let addr = 0xC000u16;
        for (i, &b) in code.iter().enumerate() {
            bus.write(addr.wrapping_add(i as u16), b);
        }

        cpu.pc = addr;

        let mut mcycle_count = 0u32;
        loop {
            let before_idle = matches!(&cpu.instr_state, InstrState::Idle);
            cpu.tick(bus);
            mcycle_count += 1;

            match &cpu.instr_state {
                InstrState::Active { .. } => {} // Still in instruction, continue
                InstrState::Idle => {
                    if before_idle {
                        // Single-cycle instruction (like NOP) completed this cycle.
                        break;
                    }
                    // Otherwise was active, now idle = just completed an instruction.
                    break;
                }
            }
        }

        mcycle_count
    }

    #[test]
    fn e02_01_nop_takes_table_cycles() {
        let mut bus = Bus::new(vec![0xA5; 32 * 1024]);
        let start_addr = 0xC000u16;

        // Write NOP to WRAM at start_addr
        for (i, &b) in [0x00].iter().enumerate() {
            bus.write(start_addr.wrapping_add(i as u16), b);
        }

        let mut cpu = Cpu {
            pc: start_addr,
            ..Cpu::default()
        };

        let cycles = exec(&mut cpu, &mut bus, &[0x00]);
        assert_eq!(cycles, 1, "NOP should take 1 M-cycle per table");
    }

    #[test]
    fn e02_01_nop_advances_pc() {
        let mut bus = Bus::new(vec![0xA5; 32 * 1024]);
        let start_addr = 0xC000u16;

        // Write NOP to WRAM at start_addr
        for (i, &b) in [0x00].iter().enumerate() {
            bus.write(start_addr.wrapping_add(i as u16), b);
        }

        let mut cpu = Cpu {
            pc: start_addr,
            ..Cpu::default()
        };

        exec(&mut cpu, &mut bus, &[0x00]);

        assert_eq!(cpu.pc, start_addr + 1, "PC should advance by 1 after NOP");
    }

    #[test]
    fn e02_01_nop_preserves_f_low_nibble() {
        let mut bus = Bus::new(vec![0xA5; 32 * 1024]);
        let start_addr = 0xC000u16;

        // Write NOP to WRAM at start_addr
        for (i, &b) in [0x00].iter().enumerate() {
            bus.write(start_addr.wrapping_add(i as u16), b);
        }

        // Set all flag bits (including low nibble) to 1.
        let mut cpu = Cpu {
            pc: start_addr,
            f: 0x0F,
            ..Cpu::default()
        };

        exec(&mut cpu, &mut bus, &[0x00]);

        assert_eq!(cpu.f & 0x0F, 0, "low nibble of F must stay 0 after NOP");
    }

    #[test]
    fn e02_01_stub_other_opcodes_dont_panic() {
        let mut bus = Bus::new(vec![0xA5; 32 * 1024]);
        // Write a non-NOP opcode (LD BC, n16) followed by two operand bytes.
        for (i, &b) in [0x01, 0x12, 0x34].iter().enumerate() {
            bus.write(0xC000 + i as u16, b);
        }

        let mut cpu = Cpu {
            pc: 0xC000,
            ..Cpu::default()
        };

        // Should not panic - all instructions behave as NOP for E02_01.
        for _ in 0..3 {
            cpu.tick(&mut bus);
        }

        // PC should advance past the instruction bytes (stub behavior).
        assert_eq!(
            cpu.pc, 0xC003,
            "PC should advance past stubbed multi-byte opcode"
        );
    }

    #[test]
    fn e02_01_default_f_has_low_nibble_zero() {
        let cpu = Cpu::default();
        // Default CPU should have valid F with low nibble zero.
        assert_eq!(cpu.f & 0x0F, 0, "Default F must have low nibble = 0");
    }
}
