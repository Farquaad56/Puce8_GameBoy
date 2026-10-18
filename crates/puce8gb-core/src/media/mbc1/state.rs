//! Save state of the MBC1 mapper (E09_02, decision A_05): ram_enabled, bank_lo, bank_hi,
//! advanced_mode, then the RAM block (u32 length + bytes). The ROM is not saved. Ranges of the
//! bank registers: UNKNOWN - to confirm, not validated (the mapper reduces them to the real size).

use super::Mbc1;
use crate::media::{read_ram_block, write_ram_block};
use crate::savestate::{StateError, StateReader, StateWriter};

impl Mbc1 {
    /// Writes the registers and the RAM block.
    pub fn write_state(&self, w: &mut StateWriter<'_>) -> Result<(), StateError> {
        w.put_bool(self.ram_enabled)?;
        w.put_u8(self.bank_lo)?;
        w.put_u8(self.bank_hi)?;
        w.put_bool(self.advanced_mode)?;
        write_ram_block(w, &self.ram)
    }

    /// Reads what `write_state` wrote; a RAM length other than the current one is
    /// `RomMismatch`. The mapper is left unchanged on error.
    pub fn read_state(&mut self, r: &mut StateReader<'_>) -> Result<(), StateError> {
        let ram_enabled = r.get_bool()?;
        let bank_lo = r.get_u8()?;
        let bank_hi = r.get_u8()?;
        let advanced_mode = r.get_bool()?;
        let ram = read_ram_block(r, self.ram.len())?;
        self.ram.copy_from_slice(ram);
        self.ram_enabled = ram_enabled;
        self.bank_lo = bank_lo;
        self.bank_hi = bank_hi;
        self.advanced_mode = advanced_mode;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mbc1(ram_code: u8) -> Mbc1 {
        Mbc1::new(vec![0u8; 0x8000], ram_code).expect("valid")
    }

    fn sample() -> Mbc1 {
        let mut m = mbc1(0x02);
        m.ram_enabled = true;
        m.bank_lo = 0x15;
        m.bank_hi = 2;
        m.advanced_mode = true;
        m.ram[0] = 0x11;
        m.ram[8191] = 0x22;
        m
    }

    fn saved(m: &Mbc1) -> Vec<u8> {
        let mut buf = vec![0u8; 16384];
        let mut w = StateWriter::new(&mut buf);
        m.write_state(&mut w).expect("room");
        let len = w.position();
        buf.truncate(len);
        buf
    }

    #[test]
    fn e09_02_mbc1_layout_is_fixed() {
        let bytes = saved(&sample());
        assert_eq!(bytes.len(), 8200);
        assert_eq!(
            &bytes[..8],
            &[0x01, 0x15, 0x02, 0x01, 0x00, 0x20, 0x00, 0x00]
        );
        assert_eq!(bytes[8], 0x11);
        assert_eq!(bytes[8199], 0x22);
    }

    #[test]
    fn e09_02_mbc1_round_trip_is_byte_identical() {
        let bytes = saved(&sample());
        let mut loaded = mbc1(0x02);
        let mut r = StateReader::new(&bytes);
        loaded.read_state(&mut r).expect("valid");
        assert_eq!(r.remaining(), 0);
        assert!(loaded.ram_enabled);
        assert_eq!(loaded.bank_lo, 0x15);
        assert_eq!(loaded.bank_hi, 2);
        assert!(loaded.advanced_mode);
        assert_eq!(loaded.ram[8191], 0x22);
        assert_eq!(saved(&loaded), bytes);
    }

    #[test]
    fn e09_02_mbc1_truncated_data_is_bad_size_and_leaves_mapper_unchanged() {
        let bytes = saved(&sample());
        for len in [0usize, 1, 3, 4, 7, 8, 100, 8199] {
            let mut m = sample();
            assert_eq!(
                m.read_state(&mut StateReader::new(&bytes[..len])),
                Err(StateError::BadSize),
                "len {len}"
            );
            assert_eq!(saved(&m), bytes);
        }
    }

    #[test]
    fn e09_02_mbc1_corrupted_flag_is_invalid_state() {
        for index in [0usize, 3] {
            let mut bytes = saved(&sample());
            bytes[index] = 2;
            let mut m = sample();
            assert_eq!(
                m.read_state(&mut StateReader::new(&bytes)),
                Err(StateError::InvalidState),
                "index {index}"
            );
            assert_eq!(saved(&m), saved(&sample()));
        }
    }

    #[test]
    fn e09_02_mbc1_ram_size_mismatch_is_rom_mismatch() {
        let bytes = saved(&sample());
        let mut no_ram = mbc1(0x00);
        let before = saved(&no_ram);
        assert_eq!(
            no_ram.read_state(&mut StateReader::new(&bytes)),
            Err(StateError::RomMismatch)
        );
        assert_eq!(saved(&no_ram), before);
    }

    #[test]
    fn e09_02_mbc1_write_to_small_buffer_fails() {
        let mut buf = vec![0u8; 8199];
        let mut w = StateWriter::new(&mut buf);
        assert_eq!(
            sample().write_state(&mut w),
            Err(StateError::BufferTooSmall)
        );
    }
}
