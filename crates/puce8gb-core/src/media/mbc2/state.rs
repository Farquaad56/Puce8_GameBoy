//! Save state of the MBC2 mapper (E09_02, decision A_05): ram_enabled, rom_bank, then the
//! built-in RAM block (u32 length + bytes). The ROM is not saved. The range of rom_bank:
//! UNKNOWN - to confirm, not validated.

use super::Mbc2;
use crate::media::{read_ram_block, write_ram_block};
use crate::savestate::{StateError, StateReader, StateWriter};

impl Mbc2 {
    /// Writes the registers and the RAM block.
    pub fn write_state(&self, w: &mut StateWriter<'_>) -> Result<(), StateError> {
        w.put_bool(self.ram_enabled)?;
        w.put_u8(self.rom_bank)?;
        write_ram_block(w, &self.ram)
    }

    /// Reads what `write_state` wrote; a RAM length other than the current one is
    /// `RomMismatch`. The mapper is left unchanged on error.
    pub fn read_state(&mut self, r: &mut StateReader<'_>) -> Result<(), StateError> {
        let ram_enabled = r.get_bool()?;
        let rom_bank = r.get_u8()?;
        let ram = read_ram_block(r, self.ram.len())?;
        self.ram.copy_from_slice(ram);
        self.ram_enabled = ram_enabled;
        self.rom_bank = rom_bank;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Mbc2 {
        let mut m = Mbc2::new(vec![0u8; 0x8000]);
        m.ram_enabled = true;
        m.rom_bank = 0x0A;
        m.ram[0] = 0x0F;
        let last = m.ram.len() - 1;
        m.ram[last] = 0x05;
        m
    }

    fn saved(m: &Mbc2) -> Vec<u8> {
        let mut buf = vec![0u8; 4096];
        let mut w = StateWriter::new(&mut buf);
        m.write_state(&mut w).expect("room");
        let len = w.position();
        buf.truncate(len);
        buf
    }

    #[test]
    fn e09_02_mbc2_layout_is_fixed() {
        let m = sample();
        let n = m.ram.len();
        let bytes = saved(&m);
        assert_eq!(bytes.len(), 6 + n);
        assert_eq!(&bytes[..2], &[0x01, 0x0A]);
        assert_eq!(&bytes[2..6], &(n as u32).to_le_bytes());
        assert_eq!(bytes[6], 0x0F);
        assert_eq!(bytes[5 + n], 0x05);
    }

    #[test]
    fn e09_02_mbc2_round_trip_is_byte_identical() {
        let bytes = saved(&sample());
        let mut loaded = Mbc2::new(vec![0u8; 0x8000]);
        let mut r = StateReader::new(&bytes);
        loaded.read_state(&mut r).expect("valid");
        assert_eq!(r.remaining(), 0);
        assert!(loaded.ram_enabled);
        assert_eq!(loaded.rom_bank, 0x0A);
        assert_eq!(loaded.ram[0], 0x0F);
        assert_eq!(saved(&loaded), bytes);
    }

    #[test]
    fn e09_02_mbc2_truncated_data_is_bad_size_and_leaves_mapper_unchanged() {
        let bytes = saved(&sample());
        for len in [0usize, 1, 2, 5, 6, 100, bytes.len() - 1] {
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
    fn e09_02_mbc2_corrupted_flag_is_invalid_state() {
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
    fn e09_02_mbc2_ram_length_mismatch_is_rom_mismatch() {
        let mut bytes = saved(&sample());
        bytes[2] = bytes[2].wrapping_add(1);
        let mut m = sample();
        assert_eq!(
            m.read_state(&mut StateReader::new(&bytes)),
            Err(StateError::RomMismatch)
        );
        assert_eq!(saved(&m), saved(&sample()));
    }

    #[test]
    fn e09_02_mbc2_write_to_small_buffer_fails() {
        let needed = saved(&sample()).len();
        let mut buf = vec![0u8; needed - 1];
        let mut w = StateWriter::new(&mut buf);
        assert_eq!(
            sample().write_state(&mut w),
            Err(StateError::BufferTooSmall)
        );
    }
}
