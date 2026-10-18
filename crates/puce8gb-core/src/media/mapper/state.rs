//! Save state of the cartridge mapper (E09_02, decision A_05): a tag byte (0 RomOnly, 1 Mbc1,
//! 2 Mbc2, 3 Mbc3, 4 Mbc5) then the state of that mapper. The ROM image is not saved.

use super::{Mapper, RomOnly};
use crate::media::{read_ram_block, write_ram_block};
use crate::savestate::{StateError, StateReader, StateWriter};

impl RomOnly {
    /// Writes the RAM block (length 0 when the cartridge has no RAM).
    pub fn write_state(&self, w: &mut StateWriter<'_>) -> Result<(), StateError> {
        write_ram_block(w, self.ram.as_deref().unwrap_or(&[]))
    }

    /// Reads what `write_state` wrote; the RAM is left unchanged on error.
    pub fn read_state(&mut self, r: &mut StateReader<'_>) -> Result<(), StateError> {
        let expected = self.ram.as_ref().map_or(0, Vec::len);
        let data = read_ram_block(r, expected)?;
        if let Some(ram) = self.ram.as_mut() {
            ram.copy_from_slice(data);
        }
        Ok(())
    }
}

impl Mapper {
    fn tag(&self) -> u8 {
        match self {
            Mapper::RomOnly(_) => 0,
            Mapper::Mbc1(_) => 1,
            Mapper::Mbc2(_) => 2,
            Mapper::Mbc3(_) => 3,
            Mapper::Mbc5(_) => 4,
        }
    }

    /// Writes the tag byte then the state of the mapper variant.
    pub fn write_state(&self, w: &mut StateWriter<'_>) -> Result<(), StateError> {
        w.put_u8(self.tag())?;
        match self {
            Mapper::RomOnly(m) => m.write_state(w),
            Mapper::Mbc1(m) => m.write_state(w),
            Mapper::Mbc2(m) => m.write_state(w),
            Mapper::Mbc3(m) => m.write_state(w),
            Mapper::Mbc5(m) => m.write_state(w),
        }
    }

    /// Reads what `write_state` wrote. A tag above 4 is `InvalidState`; a tag that differs
    /// from the loaded mapper variant is `RomMismatch`.
    pub fn read_state(&mut self, r: &mut StateReader<'_>) -> Result<(), StateError> {
        let tag = r.get_u8()?;
        if tag > 4 {
            return Err(StateError::InvalidState);
        }
        if tag != self.tag() {
            return Err(StateError::RomMismatch);
        }
        match self {
            Mapper::RomOnly(m) => m.read_state(r),
            Mapper::Mbc1(m) => m.read_state(r),
            Mapper::Mbc2(m) => m.read_state(r),
            Mapper::Mbc3(m) => m.read_state(r),
            Mapper::Mbc5(m) => m.read_state(r),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::media::header::Header;
    use crate::media::mbc2::Mbc2;

    fn rom_only(ram_code: u8) -> RomOnly {
        RomOnly::new(vec![0u8; 0x8000], ram_code).expect("valid")
    }

    fn sample() -> RomOnly {
        let mut m = rom_only(0x02);
        let ram = m.ram.as_mut().expect("ram");
        ram[0] = 0xAB;
        ram[8191] = 0xCD;
        m
    }

    fn saved(m: &RomOnly) -> Vec<u8> {
        let mut buf = vec![0u8; 16384];
        let mut w = StateWriter::new(&mut buf);
        m.write_state(&mut w).expect("room");
        let len = w.position();
        buf.truncate(len);
        buf
    }

    fn saved_mapper(m: &Mapper) -> Vec<u8> {
        let mut buf = vec![0u8; 32768];
        let mut w = StateWriter::new(&mut buf);
        m.write_state(&mut w).expect("room");
        let len = w.position();
        buf.truncate(len);
        buf
    }

    fn header(cartridge_type: u8) -> Header {
        Header {
            title: [0; 16],
            cartridge_type,
            rom_size_code: 0,
            ram_size_code: 2,
            header_checksum: 0,
            checksum_ok: true,
        }
    }

    #[test]
    fn e09_02_rom_only_layout_is_fixed() {
        let bytes = saved(&sample());
        assert_eq!(bytes.len(), 8196);
        assert_eq!(&bytes[..4], &[0x00, 0x20, 0x00, 0x00]);
        assert_eq!(bytes[4], 0xAB);
        assert_eq!(bytes[8195], 0xCD);
    }

    #[test]
    fn e09_02_rom_only_without_ram_writes_only_a_zero_length() {
        let mut m = rom_only(0x00);
        let bytes = saved(&m);
        assert_eq!(bytes, [0, 0, 0, 0]);
        m.read_state(&mut StateReader::new(&bytes)).expect("valid");
        assert_eq!(saved(&m), bytes);
    }

    #[test]
    fn e09_02_rom_only_round_trip_is_byte_identical() {
        let bytes = saved(&sample());
        let mut loaded = rom_only(0x02);
        let mut r = StateReader::new(&bytes);
        loaded.read_state(&mut r).expect("valid");
        assert_eq!(r.remaining(), 0);
        assert_eq!(loaded.ram.as_ref().expect("ram")[0], 0xAB);
        assert_eq!(saved(&loaded), bytes);
    }

    #[test]
    fn e09_02_rom_only_ram_size_mismatch_is_rom_mismatch() {
        let with_ram = saved(&sample());
        let mut without = rom_only(0x00);
        assert_eq!(
            without.read_state(&mut StateReader::new(&with_ram)),
            Err(StateError::RomMismatch)
        );
        assert_eq!(saved(&without), [0, 0, 0, 0]);
        let mut m = sample();
        assert_eq!(
            m.read_state(&mut StateReader::new(&[0, 0, 0, 0])),
            Err(StateError::RomMismatch)
        );
        assert_eq!(saved(&m), with_ram);
    }

    #[test]
    fn e09_02_rom_only_truncated_data_is_bad_size_and_leaves_ram_unchanged() {
        let bytes = saved(&sample());
        for len in [0usize, 1, 3, 4, 100, 8195] {
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
    fn e09_02_mapper_round_trip_for_every_variant() {
        for (cartridge_type, tag) in [(0x00u8, 0u8), (0x01, 1), (0x05, 2), (0x13, 3), (0x19, 4)] {
            let mut source =
                Mapper::new(vec![0u8; 0x8000], &header(cartridge_type)).expect("valid");
            source.write(0x0000, 0x0A);
            source.write(0xA000, 0x5A);
            let bytes = saved_mapper(&source);
            assert_eq!(bytes[0], tag, "type {cartridge_type:#04x}");
            let mut target =
                Mapper::new(vec![0u8; 0x8000], &header(cartridge_type)).expect("valid");
            let mut r = StateReader::new(&bytes);
            target.read_state(&mut r).expect("valid");
            assert_eq!(r.remaining(), 0);
            assert_eq!(saved_mapper(&target), bytes);
            assert_eq!(target.read(0xA000), source.read(0xA000));
        }
    }

    #[test]
    fn e09_02_mapper_tag_mismatch_and_invalid_tag_are_rejected() {
        let bytes = saved_mapper(&Mapper::RomOnly(sample()));
        assert_eq!(bytes[0], 0);
        let mut other = Mapper::Mbc2(Mbc2::new(vec![0u8; 0x8000]));
        let before = saved_mapper(&other);
        assert_eq!(
            other.read_state(&mut StateReader::new(&bytes)),
            Err(StateError::RomMismatch)
        );
        assert_eq!(saved_mapper(&other), before);
        let mut bad = bytes.clone();
        bad[0] = 9;
        let mut m = Mapper::RomOnly(sample());
        assert_eq!(
            m.read_state(&mut StateReader::new(&bad)),
            Err(StateError::InvalidState)
        );
        assert_eq!(saved_mapper(&m), bytes);
    }

    #[test]
    fn e09_02_mapper_write_to_small_buffer_fails() {
        let mut buf = vec![0u8; 8196];
        let mut w = StateWriter::new(&mut buf);
        assert_eq!(
            Mapper::RomOnly(sample()).write_state(&mut w),
            Err(StateError::BufferTooSmall)
        );
    }
}
