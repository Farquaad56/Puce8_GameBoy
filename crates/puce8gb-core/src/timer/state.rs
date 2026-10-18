//! Save state of the timer (E09_02, decision A_05): every field, fixed order, no padding.

use super::Timer;
use crate::savestate::{StateError, StateReader, StateWriter};

impl Timer {
    /// Writes the timer state, 8 bytes: sysclk (u16), prev_signal, tima, tma, tac,
    /// reload_pending, reloading.
    pub fn write_state(&self, w: &mut StateWriter<'_>) -> Result<(), StateError> {
        w.put_u16(self.sysclk)?;
        w.put_bool(self.prev_signal)?;
        w.put_u8(self.tima)?;
        w.put_u8(self.tma)?;
        w.put_u8(self.tac)?;
        w.put_bool(self.reload_pending)?;
        w.put_bool(self.reloading)
    }

    /// Reads what `write_state` wrote; the timer is left unchanged on error.
    pub fn read_state(&mut self, r: &mut StateReader<'_>) -> Result<(), StateError> {
        let sysclk = r.get_u16()?;
        let prev_signal = r.get_bool()?;
        let tima = r.get_u8()?;
        let tma = r.get_u8()?;
        let tac = r.get_u8()?;
        let reload_pending = r.get_bool()?;
        let reloading = r.get_bool()?;
        self.sysclk = sysclk;
        self.prev_signal = prev_signal;
        self.tima = tima;
        self.tma = tma;
        self.tac = tac;
        self.reload_pending = reload_pending;
        self.reloading = reloading;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_BYTES: [u8; 8] = [0x34, 0xAB, 0x01, 0x12, 0x34, 0xFD, 0x01, 0x00];

    fn sample() -> Timer {
        Timer {
            sysclk: 0xAB34,
            prev_signal: true,
            tima: 0x12,
            tma: 0x34,
            tac: 0xFD,
            reload_pending: true,
            reloading: false,
        }
    }

    fn saved(timer: &Timer) -> [u8; 8] {
        let mut buf = [0u8; 8];
        let mut w = StateWriter::new(&mut buf);
        timer.write_state(&mut w).expect("room");
        assert_eq!(w.position(), 8);
        buf
    }

    #[test]
    fn e09_02_timer_layout_is_fixed() {
        assert_eq!(saved(&sample()), SAMPLE_BYTES);
    }

    #[test]
    fn e09_02_timer_round_trip_is_byte_identical() {
        let mut loaded = Timer::default();
        let mut r = StateReader::new(&SAMPLE_BYTES);
        loaded.read_state(&mut r).expect("valid");
        assert_eq!(r.remaining(), 0);
        assert_eq!(loaded.div(), 0xAB);
        assert_eq!(saved(&loaded), SAMPLE_BYTES);
    }

    #[test]
    fn e09_02_timer_truncated_data_is_bad_size_and_leaves_timer_unchanged() {
        for len in 0..8 {
            let mut timer = sample();
            assert_eq!(
                timer.read_state(&mut StateReader::new(&SAMPLE_BYTES[..len])),
                Err(StateError::BadSize),
                "len {len}"
            );
            assert_eq!(saved(&timer), SAMPLE_BYTES);
        }
    }

    #[test]
    fn e09_02_timer_corrupted_flag_is_invalid_state() {
        for index in [2usize, 6, 7] {
            let mut bytes = SAMPLE_BYTES;
            bytes[index] = 2;
            let mut timer = sample();
            assert_eq!(
                timer.read_state(&mut StateReader::new(&bytes)),
                Err(StateError::InvalidState),
                "index {index}"
            );
            assert_eq!(saved(&timer), SAMPLE_BYTES);
        }
    }

    #[test]
    fn e09_02_timer_write_to_small_buffer_fails() {
        let mut buf = [0u8; 7];
        let mut w = StateWriter::new(&mut buf);
        assert_eq!(
            sample().write_state(&mut w),
            Err(StateError::BufferTooSmall)
        );
    }
}
