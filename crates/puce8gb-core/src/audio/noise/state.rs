//! Save state of the noise channel (E09_02, decision A_05): every field, fixed order, 18 bytes.
//! Ranges of the LFSR, shift, divider and timers: UNKNOWN - to confirm, not validated.

use super::Noise;
use crate::savestate::{StateError, StateReader, StateWriter};

impl Noise {
    /// Writes the channel state, 18 bytes: enabled, dac_on, length, length_enable, volume,
    /// lfsr (u16), width_short, shift, divider, timer (u32), env_initial, env_add, env_period,
    /// env_timer.
    pub fn write_state(&self, w: &mut StateWriter<'_>) -> Result<(), StateError> {
        w.put_bool(self.enabled)?;
        w.put_bool(self.dac_on)?;
        w.put_u8(self.length)?;
        w.put_bool(self.length_enable)?;
        w.put_u8(self.volume)?;
        w.put_u16(self.lfsr)?;
        w.put_bool(self.width_short)?;
        w.put_u8(self.shift)?;
        w.put_u8(self.divider)?;
        w.put_u32(self.timer)?;
        w.put_u8(self.env_initial)?;
        w.put_bool(self.env_add)?;
        w.put_u8(self.env_period)?;
        w.put_u8(self.env_timer)
    }

    /// Reads what `write_state` wrote; the channel is left unchanged on error.
    pub fn read_state(&mut self, r: &mut StateReader<'_>) -> Result<(), StateError> {
        let next = Noise {
            enabled: r.get_bool()?,
            dac_on: r.get_bool()?,
            length: r.get_u8()?,
            length_enable: r.get_bool()?,
            volume: r.get_u8()?,
            lfsr: r.get_u16()?,
            width_short: r.get_bool()?,
            shift: r.get_u8()?,
            divider: r.get_u8()?,
            timer: r.get_u32()?,
            env_initial: r.get_u8()?,
            env_add: r.get_bool()?,
            env_period: r.get_u8()?,
            env_timer: r.get_u8()?,
        };
        *self = next;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_BYTES: [u8; 18] = [
        0x01, 0x01, 0x15, 0x00, 0x0B, 0x5A, 0x5A, 0x01, 0x07, 0x03, 0x03, 0x02, 0x01, 0x00, 0x0A,
        0x00, 0x05, 0x04,
    ];

    fn sample() -> Noise {
        Noise {
            enabled: true,
            dac_on: true,
            length: 0x15,
            length_enable: false,
            volume: 0x0B,
            lfsr: 0x5A5A,
            width_short: true,
            shift: 7,
            divider: 3,
            timer: 0x0001_0203,
            env_initial: 0x0A,
            env_add: false,
            env_period: 5,
            env_timer: 4,
        }
    }

    fn saved(noise: &Noise) -> [u8; 18] {
        let mut buf = [0u8; 18];
        let mut w = StateWriter::new(&mut buf);
        noise.write_state(&mut w).expect("room");
        assert_eq!(w.position(), 18);
        buf
    }

    #[test]
    fn e09_02_noise_layout_is_fixed() {
        assert_eq!(saved(&sample()), SAMPLE_BYTES);
    }

    #[test]
    fn e09_02_noise_round_trip_is_byte_identical() {
        let mut loaded = Noise::default();
        let mut r = StateReader::new(&SAMPLE_BYTES);
        loaded.read_state(&mut r).expect("valid");
        assert_eq!(r.remaining(), 0);
        assert_eq!(loaded.lfsr, 0x5A5A);
        assert_eq!(loaded.timer, 0x0001_0203);
        assert_eq!(saved(&loaded), SAMPLE_BYTES);
    }

    #[test]
    fn e09_02_noise_truncated_data_is_bad_size_and_leaves_channel_unchanged() {
        for len in 0..18 {
            let mut noise = sample();
            assert_eq!(
                noise.read_state(&mut StateReader::new(&SAMPLE_BYTES[..len])),
                Err(StateError::BadSize),
                "len {len}"
            );
            assert_eq!(saved(&noise), SAMPLE_BYTES);
        }
    }

    #[test]
    fn e09_02_noise_corrupted_flag_is_invalid_state() {
        for index in [0usize, 1, 3, 7, 15] {
            let mut bytes = SAMPLE_BYTES;
            bytes[index] = 2;
            let mut noise = sample();
            assert_eq!(
                noise.read_state(&mut StateReader::new(&bytes)),
                Err(StateError::InvalidState),
                "index {index}"
            );
            assert_eq!(saved(&noise), SAMPLE_BYTES);
        }
    }

    #[test]
    fn e09_02_noise_write_to_small_buffer_fails() {
        let mut buf = [0u8; 17];
        let mut w = StateWriter::new(&mut buf);
        assert_eq!(
            sample().write_state(&mut w),
            Err(StateError::BufferTooSmall)
        );
    }
}
