//! Save state of the MBC5 mapper (E09_02, decision A_05): enabled, rom_bank (u16), ram_bank,
//! then the RAM block (u32 length + bytes). The ROM is not saved. The ranges of rom_bank and
//! ram_bank: UNKNOWN - to confirm, not validated.

use super::Mbc5;
use crate::media::{read_ram_block, write_ram_block};
use crate::savestate::{StateError, StateReader, StateWriter};

impl Mbc5 {
    /// Writes the registers and the RAM block.
    pub fn write_state(&self, w: &mut StateWriter<'_>) -> Result<(), StateError> {
        w.put_bool(self.enabled)?;
        w.put_u16(self.rom_bank)?;
        w.put_u8(self.ram_bank)?;
        write_ram_block(w, &self.ram)
    }

    /// Reads what `write_state` wrote; a RAM length other than the current one is
    /// `RomMismatch`. The mapper is left unchanged on error.
    pub fn read_state(&mut self, r: &mut StateReader<'_>) -> Result<(), StateError> {
        let enabled = r.get_bool()?;
        let rom_bank = r.get_u16()?;
        let ram_bank = r.get_u8()?;
        let ram = read_ram_block(r, self.ram.len())?;
        self.ram.copy_from_slice(ram);
        self.enabled = enabled;
        self.rom_bank = rom_bank;
        self.ram_bank = ram_bank;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mbc5(ram_code: u8) -> Mbc5 {
        Mbc5::new(vec![0u8; 0x8000], ram_code).expect("valid")
    }

    fn sample() -> Mbc5 {
        let mut m = mbc5(0x02);
        m.enabled = true;
        m.rom_bank = 0x01FF;
        m.ram_bank = 0x0A;
        m.ram[0] = 0x66;
        m.ram[8191] = 0x77;
        m
    }

    fn saved(m: &Mbc5) -> Vec<u8> {
        let mut buf = vec![0u8; 16384];
        let mut w = StateWriter::new(&mut buf);
        m.write_state(&mut w).expect("room");
        let len = w.position();
        buf.truncate(len);
        buf
    }

    #[test]
    fn e09_02_mbc5_layout_is_fixed() {
        let bytes = saved(&sample());
        assert_eq!(bytes.len(), 8200);
        assert_eq!(
            &bytes[..8],
            &[0x01, 0xFF, 0x01, 0x0A, 0x00, 0x20, 0x00, 0x00]
        );
        assert_eq!(bytes[8], 0x66);
        assert_eq!(bytes[8199], 0x77);
    }

    #[test]
    fn e09_02_mbc5_round_trip_is_byte_identical() {
        let bytes = saved(&sample());
        let mut loaded = mbc5(0x02);
        let mut r = StateReader::new(&bytes);
        loaded.read_state(&mut r).expect("valid");
        assert_eq!(r.remaining(), 0);
        assert!(loaded.enabled);
        assert_eq!(loaded.rom_bank, 0x01FF);
        assert_eq!(loaded.ram_bank, 0x0A);
        assert_eq!(loaded.ram[8191], 0x77);
        assert_eq!(saved(&loaded), bytes);
    }

    #[test]
    fn e09_02_mbc5_truncated_data_is_bad_size_and_leaves_mapper_unchanged() {
        let bytes = saved(&sample());
        for len in [0usize, 1, 2, 3, 4, 7, 8, 100, 8199] {
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
    fn e09_02_mbc5_corrupted_flag_is_invalid_state() {
        let mut bytes = saved(&sample());
        bytes[0] = 2;
        let mut m = sample();
        assert_eq!(
            m.read_state(&mut StateReader::new(&bytes)),
            Err(StateError::InvalidState)
        );
        assert_eq!(saved(&m), saved(&sample()));
    }

    #[test]
    fn e09_02_mbc5_ram_size_mismatch_is_rom_mismatch() {
        let bytes = saved(&sample());
        let mut no_ram = mbc5(0x00);
        let before = saved(&no_ram);
        assert_eq!(
            no_ram.read_state(&mut StateReader::new(&bytes)),
            Err(StateError::RomMismatch)
        );
        assert_eq!(saved(&no_ram), before);
    }

    #[test]
    fn e09_02_mbc5_write_to_small_buffer_fails() {
        let mut buf = vec![0u8; 8199];
        let mut w = StateWriter::new(&mut buf);
        assert_eq!(
            sample().write_state(&mut w),
            Err(StateError::BufferTooSmall)
        );
    }
}
