//! SM83 CPU (decision A_03): registers and post-boot state for now; instruction execution,
//! interrupts and HALT/STOP arrive with later tasks.

/// Post-boot CPU register state at PC=$0100 on DMG (note 08 "Registres CPU apres boot").
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Cpu {
    pub a: u8,
    /// Flags byte: bit 7 Z, bit 6 N, bit 5 H, bit 4 C.
    pub f: u8,
    pub b: u8,
    pub c: u8,
    pub d: u8,
    pub e: u8,
    pub h: u8,
    pub l: u8,
    pub sp: u16,
    pub pc: u16,
}

impl Cpu {
    /// Post-boot values (note 08). Z set and N clear; H and C are both set iff the header
    /// checksum byte $014D is not $00 (Power_Up_Sequence.md#CPU-registers L237).
    /// CONFLIT: specs 2001 give fixed H=1 C=1; to be settled by mooneye boot_regs-dmgABC
    /// (note 08).
    pub fn reset(checksum: u8) -> Self {
        let mut f = 0x80; // Z set, N clear
        if checksum != 0x00 {
            f |= 0x30; // H and C both set
        }
        Cpu {
            a: 0x01,
            f,
            b: 0x00,
            c: 0x13,
            d: 0x00,
            e: 0xD8,
            h: 0x01,
            l: 0x4D,
            sp: 0xFFFE,
            pc: 0x0100,
        }
    }
}
