//! Save state of a pulse channel (E09_02, decision A_05): every field, fixed order, 23 bytes.
//! Ranges of duty_pos, timers and sweep values: UNKNOWN - to confirm, not validated.

use super::Pulse;
use crate::savestate::{StateError, StateReader, StateWriter};

impl Pulse {
    /// Writes the channel state, 23 bytes: enabled, dac_on, duty, duty_pos, freq (u16), length,
    /// length_enable, volume, timer (u16), env_initial, env_add, env_period, env_timer,
    /// sweep_pace, sweep_negate, sweep_shift, sweep_timer, sweep_enabled, shadow (u16),
    /// sweep_negate_used.
    pub fn write_state(&self, w: &mut StateWriter<'_>) -> Result<(), StateError> {
        w.put_bool(self.enabled)?;
        w.put_bool(self.dac_on)?;
        w.put_u8(self.duty)?;
        w.put_u8(self.duty_pos)?;
        w.put_u16(self.freq)?;
        w.put_u8(self.length)?;
        w.put_bool(self.length_enable)?;
        w.put_u8(self.volume)?;
        w.put_u16(self.timer)?;
        w.put_u8(self.env_initial)?;
        w.put_bool(self.env_add)?;
        w.put_u8(self.env_period)?;
        w.put_u8(self.env_timer)?;
        w.put_u8(self.sweep_pace)?;
        w.put_bool(self.sweep_negate)?;
        w.put_u8(self.sweep_shift)?;
        w.put_u8(self.sweep_timer)?;
        w.put_bool(self.sweep_enabled)?;
        w.put_u16(self.shadow)?;
        w.put_bool(self.sweep_negate_used)
    }

    /// Reads what `write_state` wrote; the channel is left unchanged on error.
    pub fn read_state(&mut self, r: &mut StateReader<'_>) -> Result<(), StateError> {
        let next = Pulse {
            enabled: r.get_bool()?,
            dac_on: r.get_bool()?,
            duty: r.get_u8()?,
            duty_pos: r.get_u8()?,
            freq: r.get_u16()?,
            length: r.get_u8()?,
            length_enable: r.get_bool()?,
            volume: r.get_u8()?,
            timer: r.get_u16()?,
            env_initial: r.get_u8()?,
            env_add: r.get_bool()?,
            env_period: r.get_u8()?,
            env_timer: r.get_u8()?,
            sweep_pace: r.get_u8()?,
            sweep_negate: r.get_bool()?,
            sweep_shift: r.get_u8()?,
            sweep_timer: r.get_u8()?,
            sweep_enabled: r.get_bool()?,
            shadow: r.get_u16()?,
            sweep_negate_used: r.get_bool()?,
        };
        *self = next;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_BYTES: [u8; 23] = [
        0x01, 0x01, 0x02, 0x05, 0x56, 0x04, 0x2A, 0x01, 0x09, 0x23, 0x01, 0x0F, 0x01, 0x03, 0x02,
        0x04, 0x00, 0x05, 0x06, 0x01, 0x89, 0x07, 0x01,
    ];

    fn sample() -> Pulse {
        Pulse {
            enabled: true,
            dac_on: true,
            duty: 2,
            duty_pos: 5,
            freq: 0x0456,
            length: 0x2A,
            length_enable: true,
            volume: 9,
            timer: 0x0123,
            env_initial: 0x0F,
            env_add: true,
            env_period: 3,
            env_timer: 2,
            sweep_pace: 4,
            sweep_negate: false,
            sweep_shift: 5,
            sweep_timer: 6,
            sweep_enabled: true,
            shadow: 0x0789,
            sweep_negate_used: true,
        }
    }

    fn saved(pulse: &Pulse) -> [u8; 23] {
        let mut buf = [0u8; 23];
        let mut w = StateWriter::new(&mut buf);
        pulse.write_state(&mut w).expect("room");
        assert_eq!(w.position(), 23);
        buf
    }

    #[test]
    fn e09_02_pulse_layout_is_fixed() {
        assert_eq!(saved(&sample()), SAMPLE_BYTES);
    }

    #[test]
    fn e09_02_pulse_round_trip_is_byte_identical() {
        let mut loaded = Pulse::default();
        let mut r = StateReader::new(&SAMPLE_BYTES);
        loaded.read_state(&mut r).expect("valid");
        assert_eq!(r.remaining(), 0);
        assert_eq!(loaded.freq, 0x0456);
        assert_eq!(loaded.shadow, 0x0789);
        assert_eq!(saved(&loaded), SAMPLE_BYTES);
    }

    #[test]
    fn e09_02_pulse_truncated_data_is_bad_size_and_leaves_channel_unchanged() {
        for len in 0..23 {
            let mut pulse = sample();
            assert_eq!(
                pulse.read_state(&mut StateReader::new(&SAMPLE_BYTES[..len])),
                Err(StateError::BadSize),
                "len {len}"
            );
            assert_eq!(saved(&pulse), SAMPLE_BYTES);
        }
    }

    #[test]
    fn e09_02_pulse_corrupted_flag_is_invalid_state() {
        for index in [0usize, 1, 7, 12, 16, 19, 22] {
            let mut bytes = SAMPLE_BYTES;
            bytes[index] = 2;
            let mut pulse = sample();
            assert_eq!(
                pulse.read_state(&mut StateReader::new(&bytes)),
                Err(StateError::InvalidState),
                "index {index}"
            );
            assert_eq!(saved(&pulse), SAMPLE_BYTES);
        }
    }

    #[test]
    fn e09_02_pulse_write_to_small_buffer_fails() {
        let mut buf = [0u8; 22];
        let mut w = StateWriter::new(&mut buf);
        assert_eq!(
            sample().write_state(&mut w),
            Err(StateError::BufferTooSmall)
        );
    }
}
