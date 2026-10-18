//! Save state of the MBC3 mapper and its RTC (E09_02, decision A_05): enabled, rom_bank, select,
//! latch_armed, rtc (7 bytes), latched (7 bytes), dot_acc (u64), then the RAM block (u32 length
//! + bytes). An RTC is: seconds, minutes, hours, day (u16), halt (0/1), carry (0/1). The ROM is
//!   not saved. dot_acc must stay below RTC_DOTS_PER_SECOND. The range of rom_bank and select:
//!   UNKNOWN - to confirm, not validated.

use super::{Mbc3, Rtc, RTC_BYTES, RTC_DOTS_PER_SECOND};
use crate::media::{read_ram_block, write_ram_block};
use crate::savestate::{StateError, StateReader, StateWriter};

fn read_rtc(r: &mut StateReader<'_>) -> Result<Rtc, StateError> {
    let mut bytes = [0u8; RTC_BYTES];
    bytes.copy_from_slice(r.get_bytes(RTC_BYTES)?);
    if bytes[5] > 1 || bytes[6] > 1 {
        return Err(StateError::InvalidState);
    }
    Ok(Rtc::from_bytes(&bytes))
}

impl Mbc3 {
    /// Writes the registers, both clocks, the sub-second dots and the RAM block.
    pub fn write_state(&self, w: &mut StateWriter<'_>) -> Result<(), StateError> {
        w.put_bool(self.enabled)?;
        w.put_u8(self.rom_bank)?;
        w.put_u8(self.select)?;
        w.put_bool(self.latch_armed)?;
        w.put_bytes(&self.rtc.to_bytes())?;
        w.put_bytes(&self.latched.to_bytes())?;
        w.put_u64(self.dot_acc)?;
        write_ram_block(w, &self.ram)
    }

    /// Reads what `write_state` wrote; a RAM length other than the current one is
    /// `RomMismatch`. The mapper is left unchanged on error.
    pub fn read_state(&mut self, r: &mut StateReader<'_>) -> Result<(), StateError> {
        let enabled = r.get_bool()?;
        let rom_bank = r.get_u8()?;
        let select = r.get_u8()?;
        let latch_armed = r.get_bool()?;
        let rtc = read_rtc(r)?;
        let latched = read_rtc(r)?;
        let dot_acc = r.get_u64()?;
        if dot_acc >= RTC_DOTS_PER_SECOND {
            return Err(StateError::InvalidState);
        }
        let ram = read_ram_block(r, self.ram.len())?;
        self.ram.copy_from_slice(ram);
        self.enabled = enabled;
        self.rom_bank = rom_bank;
        self.select = select;
        self.latch_armed = latch_armed;
        self.rtc = rtc;
        self.latched = latched;
        self.dot_acc = dot_acc;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mbc3(ram_code: u8) -> Mbc3 {
        Mbc3::new(vec![0u8; 0x8000], ram_code).expect("valid")
    }

    fn sample() -> Mbc3 {
        let mut m = mbc3(0x02);
        m.enabled = true;
        m.rom_bank = 0x21;
        m.select = 9;
        m.latch_armed = true;
        m.rtc = Rtc {
            seconds: 30,
            minutes: 20,
            hours: 10,
            day: 300,
            halt: false,
            carry: true,
        };
        m.latched = Rtc {
            seconds: 59,
            minutes: 59,
            hours: 23,
            day: 1,
            halt: true,
            carry: false,
        };
        m.dot_acc = 0x0012_3456;
        m.ram[0] = 0x33;
        m.ram[8191] = 0x44;
        m
    }

    const HEAD_BYTES: [u8; 30] = [
        0x01, 0x21, 0x09, 0x01, 0x1E, 0x14, 0x0A, 0x2C, 0x01, 0x00, 0x01, 0x3B, 0x3B, 0x17, 0x01,
        0x00, 0x01, 0x00, 0x56, 0x34, 0x12, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x20, 0x00, 0x00,
    ];

    fn saved(m: &Mbc3) -> Vec<u8> {
        let mut buf = vec![0u8; 16384];
        let mut w = StateWriter::new(&mut buf);
        m.write_state(&mut w).expect("room");
        let len = w.position();
        buf.truncate(len);
        buf
    }

    #[test]
    fn e09_02_mbc3_layout_is_fixed() {
        let bytes = saved(&sample());
        assert_eq!(bytes.len(), 8222);
        assert_eq!(&bytes[..30], &HEAD_BYTES);
        assert_eq!(bytes[30], 0x33);
        assert_eq!(bytes[8221], 0x44);
    }

    #[test]
    fn e09_02_mbc3_round_trip_is_byte_identical() {
        let bytes = saved(&sample());
        let mut loaded = mbc3(0x02);
        let mut r = StateReader::new(&bytes);
        loaded.read_state(&mut r).expect("valid");
        assert_eq!(r.remaining(), 0);
        assert_eq!(loaded.rtc, sample().rtc);
        assert_eq!(loaded.latched, sample().latched);
        assert_eq!(loaded.dot_acc, 0x0012_3456);
        assert_eq!(loaded.select, 9);
        assert!(loaded.latch_armed);
        assert_eq!(saved(&loaded), bytes);
    }

    #[test]
    fn e09_02_mbc3_truncated_data_is_bad_size_and_leaves_mapper_unchanged() {
        let bytes = saved(&sample());
        for len in [0usize, 1, 3, 4, 10, 11, 17, 18, 25, 26, 29, 30, 100, 8221] {
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
    fn e09_02_mbc3_corrupted_flags_are_invalid_state() {
        for index in [0usize, 3, 9, 10, 16, 17] {
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
    fn e09_02_mbc3_sub_second_dots_at_the_limit_are_invalid_state() {
        let mut bytes = saved(&sample());
        bytes[18..26].copy_from_slice(&RTC_DOTS_PER_SECOND.to_le_bytes());
        let mut m = sample();
        assert_eq!(
            m.read_state(&mut StateReader::new(&bytes)),
            Err(StateError::InvalidState)
        );
        assert_eq!(saved(&m), saved(&sample()));
        bytes[18..26].copy_from_slice(&(RTC_DOTS_PER_SECOND - 1).to_le_bytes());
        m.read_state(&mut StateReader::new(&bytes)).expect("valid");
        assert_eq!(m.dot_acc, RTC_DOTS_PER_SECOND - 1);
    }

    #[test]
    fn e09_02_mbc3_ram_size_mismatch_is_rom_mismatch() {
        let bytes = saved(&sample());
        let mut no_ram = mbc3(0x00);
        let before = saved(&no_ram);
        assert_eq!(
            no_ram.read_state(&mut StateReader::new(&bytes)),
            Err(StateError::RomMismatch)
        );
        assert_eq!(saved(&no_ram), before);
    }

    #[test]
    fn e09_02_mbc3_write_to_small_buffer_fails() {
        let mut buf = vec![0u8; 8221];
        let mut w = StateWriter::new(&mut buf);
        assert_eq!(
            sample().write_state(&mut w),
            Err(StateError::BufferTooSmall)
        );
    }
}
