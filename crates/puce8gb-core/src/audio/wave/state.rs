//! Save state of the wave channel (E09_02, decision A_05): every field, fixed order, 31 bytes.
//! Ranges of pos, sample, volume_code and timer: UNKNOWN - to confirm, not validated.

use super::Wave;
use crate::savestate::{StateError, StateReader, StateWriter};

impl Wave {
    /// Writes the channel state, 31 bytes: enabled, dac_on, length (u16), length_enable,
    /// volume_code, freq (u16), pos, sample, since_read, pending_corruption (flag + index),
    /// ram (16 bytes), timer (u16).
    pub fn write_state(&self, w: &mut StateWriter<'_>) -> Result<(), StateError> {
        let (flag, index) = match self.pending_corruption {
            Some(index) => (true, index),
            None => (false, 0),
        };
        w.put_bool(self.enabled)?;
        w.put_bool(self.dac_on)?;
        w.put_u16(self.length)?;
        w.put_bool(self.length_enable)?;
        w.put_u8(self.volume_code)?;
        w.put_u16(self.freq)?;
        w.put_u8(self.pos)?;
        w.put_u8(self.sample)?;
        w.put_u8(self.since_read)?;
        w.put_bool(flag)?;
        w.put_u8(index)?;
        w.put_bytes(&self.ram)?;
        w.put_u16(self.timer)
    }

    /// Reads what `write_state` wrote; the channel is left unchanged on error.
    pub fn read_state(&mut self, r: &mut StateReader<'_>) -> Result<(), StateError> {
        let enabled = r.get_bool()?;
        let dac_on = r.get_bool()?;
        let length = r.get_u16()?;
        let length_enable = r.get_bool()?;
        let volume_code = r.get_u8()?;
        let freq = r.get_u16()?;
        let pos = r.get_u8()?;
        let sample = r.get_u8()?;
        let since_read = r.get_u8()?;
        let flag = r.get_bool()?;
        let index = r.get_u8()?;
        let mut ram = [0u8; 16];
        ram.copy_from_slice(r.get_bytes(16)?);
        let timer = r.get_u16()?;
        self.enabled = enabled;
        self.dac_on = dac_on;
        self.length = length;
        self.length_enable = length_enable;
        self.volume_code = volume_code;
        self.freq = freq;
        self.pos = pos;
        self.sample = sample;
        self.since_read = since_read;
        self.pending_corruption = if flag { Some(index) } else { None };
        self.ram = ram;
        self.timer = timer;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_BYTES: [u8; 31] = [
        0x01, 0x01, 0x00, 0x01, 0x01, 0x02, 0x45, 0x03, 0x11, 0x0A, 0xFF, 0x01, 0x09, 0x00, 0x11,
        0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0xFF, 0x78,
        0x06,
    ];

    fn sample() -> Wave {
        Wave {
            enabled: true,
            dac_on: true,
            length: 0x0100,
            length_enable: true,
            volume_code: 2,
            freq: 0x0345,
            pos: 17,
            sample: 0x0A,
            since_read: 255,
            pending_corruption: Some(9),
            ram: [
                0x00, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xAA, 0xBB, 0xCC, 0xDD,
                0xEE, 0xFF,
            ],
            timer: 0x0678,
        }
    }

    fn saved(wave: &Wave) -> [u8; 31] {
        let mut buf = [0u8; 31];
        let mut w = StateWriter::new(&mut buf);
        wave.write_state(&mut w).expect("room");
        assert_eq!(w.position(), 31);
        buf
    }

    #[test]
    fn e09_02_wave_layout_is_fixed() {
        assert_eq!(saved(&sample()), SAMPLE_BYTES);
    }

    #[test]
    fn e09_02_wave_round_trip_is_byte_identical() {
        let mut loaded = Wave::default();
        let mut r = StateReader::new(&SAMPLE_BYTES);
        loaded.read_state(&mut r).expect("valid");
        assert_eq!(r.remaining(), 0);
        assert_eq!(loaded.pending_corruption, Some(9));
        assert_eq!(loaded.ram[15], 0xFF);
        assert_eq!(loaded.length, 0x0100);
        assert_eq!(saved(&loaded), SAMPLE_BYTES);
    }

    #[test]
    fn e09_02_wave_absent_pending_corruption_clears_previous_value() {
        let mut bytes = SAMPLE_BYTES;
        bytes[11] = 0;
        bytes[12] = 0;
        let mut wave = sample();
        wave.read_state(&mut StateReader::new(&bytes))
            .expect("valid");
        assert_eq!(wave.pending_corruption, None);
        assert_eq!(saved(&wave), bytes);
    }

    #[test]
    fn e09_02_wave_truncated_data_is_bad_size_and_leaves_channel_unchanged() {
        for len in 0..31 {
            let mut wave = sample();
            assert_eq!(
                wave.read_state(&mut StateReader::new(&SAMPLE_BYTES[..len])),
                Err(StateError::BadSize),
                "len {len}"
            );
            assert_eq!(saved(&wave), SAMPLE_BYTES);
        }
    }

    #[test]
    fn e09_02_wave_corrupted_flag_is_invalid_state() {
        for index in [0usize, 1, 4, 11] {
            let mut bytes = SAMPLE_BYTES;
            bytes[index] = 2;
            let mut wave = sample();
            assert_eq!(
                wave.read_state(&mut StateReader::new(&bytes)),
                Err(StateError::InvalidState),
                "index {index}"
            );
            assert_eq!(saved(&wave), SAMPLE_BYTES);
        }
    }

    #[test]
    fn e09_02_wave_write_to_small_buffer_fails() {
        let mut buf = [0u8; 30];
        let mut w = StateWriter::new(&mut buf);
        assert_eq!(
            sample().write_state(&mut w),
            Err(StateError::BufferTooSmall)
        );
    }
}
