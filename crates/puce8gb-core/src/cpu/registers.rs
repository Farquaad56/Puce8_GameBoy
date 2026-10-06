//! SM83 CPU register file (note 02a): A F B C D E H L SP PC, with 16-bit pair
//! accessors and flag helpers. No execution yet; the micro-op engine arrives in a
//! later task (decision A_03).

/// Mask keeping only the four used flag bits of F (bits 7-4). Bits 3-0 are "not
/// used (always zero)" per note 02a, so every write to F is masked with this.
pub const F_USED_BITS: u8 = 0xF0;

/// The six SM83 registers plus SP and PC (note 02a "Registres : AF BC DE HL SP PC").
/// Only AF, BC, DE, HL are also accessible as pairs of octets; SP and PC are not.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Registers {
    pub a: u8,
    /// Flags byte: bit 7 Z, bit 6 N, bit 5 H, bit 4 C (note 02a). Bits 3-0 always zero.
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

impl Registers {
    /// DMG power-up register values (note 02a "Valeurs de reset DMG des registres CPU").
    /// A=$01 B=$00 C=$13 D=$00 E=$D8 H=$01 L=$4D PC=$0100 SP=$FFFE.
    /// F: Z set, N clear; H and C both set iff the header checksum byte $014D is not $00.
    /// CONFLIT (note 02a): specs 2001 give a fixed AF=$01B0; to be settled by mooneye
    /// boot_regs-dmgABC.gb. All values above are taken from the notes, none invented.
    pub fn reset(checksum: u8) -> Self {
        let mut f = 0x80; // Z set, N clear
        if checksum != 0x00 {
            f |= 0x30; // H and C both set
        }
        Self {
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

    /// Combine a high and low octet into a 16-bit pair.
    const fn pair(hi: u8, lo: u8) -> u16 {
        ((hi as u16) << 8) | (lo as u16)
    }

    // ---- 16-bit pair accessors (note 02a "Paires 16-bit r16") ----

    /// Read the AF pair: A in the high byte, F in the low byte.
    pub fn af(&self) -> u16 {
        Self::pair(self.a, self.f)
    }

    /// Write the AF pair. Setting AF masks the low nibble of F (bits 3-0 stay zero).
    pub fn set_af(&mut self, value: u16) {
        self.a = (value >> 8) as u8;
        self.f = ((value & 0xFF) as u8) & F_USED_BITS;
    }

    /// Read the BC pair.
    pub fn bc(&self) -> u16 {
        Self::pair(self.b, self.c)
    }

    /// Write the BC pair.
    pub fn set_bc(&mut self, value: u16) {
        self.b = (value >> 8) as u8;
        self.c = (value & 0xFF) as u8;
    }

    /// Read the DE pair.
    pub fn de(&self) -> u16 {
        Self::pair(self.d, self.e)
    }

    /// Write the DE pair.
    pub fn set_de(&mut self, value: u16) {
        self.d = (value >> 8) as u8;
        self.e = (value & 0xFF) as u8;
    }

    /// Read the HL pair.
    pub fn hl(&self) -> u16 {
        Self::pair(self.h, self.l)
    }

    /// Write the HL pair.
    pub fn set_hl(&mut self, value: u16) {
        self.h = (value >> 8) as u8;
        self.l = (value & 0xFF) as u8;
    }

    // ---- Flag helpers (note 02a "Bits du registre de flags F") ----
    // Each setter/clearer touches only its own bit and re-masks with F_USED_BITS, so the
    // low nibble of F is always kept at zero.

    /// Z flag (bit 7): set iff a result was zero.
    pub fn z(&self) -> bool {
        self.f & 0x80 != 0
    }

    /// Set the Z flag.
    pub fn set_z(&mut self) {
        self.f = (self.f | 0x80) & F_USED_BITS;
    }

    /// Clear the Z flag.
    pub fn clear_z(&mut self) {
        self.f &= !0x80;
    }

    /// N flag (bit 6): set for a subtraction/comparison.
    pub fn n(&self) -> bool {
        self.f & 0x40 != 0
    }

    /// Set the N flag.
    pub fn set_n(&mut self) {
        self.f = (self.f | 0x40) & F_USED_BITS;
    }

    /// Clear the N flag.
    pub fn clear_n(&mut self) {
        self.f &= !0x40;
    }

    /// H flag (bit 5): half carry of the low four bits, used by DAA.
    pub fn h(&self) -> bool {
        self.f & 0x20 != 0
    }

    /// Set the H flag.
    pub fn set_h(&mut self) {
        self.f = (self.f | 0x20) & F_USED_BITS;
    }

    /// Clear the H flag.
    pub fn clear_h(&mut self) {
        self.f &= !0x20;
    }

    /// C flag (bit 4): carry/borrow, used by DAA and 16-bit ops.
    pub fn c(&self) -> bool {
        self.f & 0x10 != 0
    }

    /// Set the C flag.
    pub fn set_c(&mut self) {
        self.f = (self.f | 0x10) & F_USED_BITS;
    }

    /// Clear the C flag.
    pub fn clear_c(&mut self) {
        self.f &= !0x10;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn e02_01_pair_round_trip() {
        let mut r = Registers::default();
        for value in [0x0000u16, 0xFFFF, 0x1234, 0xA5A5, 0x8000] {
            r.set_af(value);
            r.set_bc(value);
            r.set_de(value);
            r.set_hl(value);
            assert_eq!(r.bc(), value, "BC round trip");
            assert_eq!(r.de(), value, "DE round trip");
            assert_eq!(r.hl(), value, "HL round trip");
        }
    }

    #[test]
    fn e02_01_set_af_masks_f_low_nibble() {
        let mut r = Registers::default();
        // A high byte 0x00, F low byte with all bits set: only the used flag bits survive.
        r.set_af(0x00FF);
        assert_eq!(r.a, 0x00);
        assert_eq!(r.f, 0xF0, "low nibble of F must be masked to zero");
        assert_eq!(r.af(), 0x00F0);

        // A non-zero high byte is preserved while the low nibble stays masked.
        r.set_af(0xABCD);
        assert_eq!(r.a, 0xAB);
        assert_eq!(r.f, 0xC0);
    }

    #[test]
    fn e02_01_f_low_nibble_stays_zero() {
        let mut r = Registers::default();
        // Drive every flag setter and the AF setter with garbage; the low nibble never moves.
        for value in [0x00FFu16, 0xFFFF, 0x7F7F] {
            r.set_af(value);
            assert_eq!(r.f & 0x0F, 0, "set_af keeps F low nibble zero");
        }
        for _ in 0..2 {
            r.set_z();
            r.set_n();
            r.set_h();
            r.set_c();
            assert_eq!(r.f & 0x0F, 0, "flag setters keep F low nibble zero");
            r.clear_z();
            r.clear_n();
            r.clear_h();
            r.clear_c();
            assert_eq!(r.f & 0x0F, 0, "flag clearers keep F low nibble zero");
        }
    }

    #[test]
    fn e02_01_each_flag_independent() {
        // Each flag occupies a single distinct bit and can be set/cleared without touching the rest.
        for (name, bit) in [("z", 0x80u8), ("n", 0x40), ("h", 0x20), ("c", 0x10)] {
            let mut r = Registers::default();
            // Start with every flag set, then clear only this one.
            r.set_z();
            r.set_n();
            r.set_h();
            r.set_c();
            assert_eq!(r.f, 0xF0);
            match bit {
                0x80 => r.clear_z(),
                0x40 => r.clear_n(),
                0x20 => r.clear_h(),
                _ => r.clear_c(),
            }
            assert_eq!(r.f & bit, 0, "{name} bit cleared");
            // The other three flags are untouched.
            assert_eq!(
                r.f & !bit,
                0xF0 ^ bit,
                "other flags preserved when clearing {name}"
            );

            // Now set only this one from a clean register.
            let mut r = Registers::default();
            match bit {
                0x80 => r.set_z(),
                0x40 => r.set_n(),
                0x20 => r.set_h(),
                _ => r.set_c(),
            }
            assert_eq!(r.f, bit, "only the {name} bit is set");
        }
    }

    #[test]
    fn e02_01_reset_values_from_notes() {
        // Checksum $00: H and C clear (note 02a).
        let r = Registers::reset(0x00);
        assert_eq!(r.a, 0x01);
        assert_eq!(r.f, 0x80, "Z set, N clear, H and C clear");
        assert_eq!(r.b, 0x00);
        assert_eq!(r.c, 0x13);
        assert_eq!(r.d, 0x00);
        assert_eq!(r.e, 0xD8);
        assert_eq!(r.h, 0x01);
        assert_eq!(r.l, 0x4D);
        assert_eq!(r.sp, 0xFFFE);
        assert_eq!(r.pc, 0x0100);

        // Checksum not $00: H and C both set (note 02a).
        let r = Registers::reset(0x5A);
        assert_eq!(r.f, 0xB0, "Z set, N clear, H and C set");
    }
}
