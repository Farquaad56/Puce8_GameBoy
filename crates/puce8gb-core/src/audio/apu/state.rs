//! Save state of the APU (E09_02, decision A_05): step, powered, prev_div_bit, then the four
//! channels in order pulse1, pulse2, wave, noise. 98 bytes.

use super::Apu;
use crate::savestate::{StateError, StateReader, StateWriter};

impl Apu {
    /// Writes the APU state: step (0..=7), powered, prev_div_bit, then pulse1 (23 bytes),
    /// pulse2 (23), wave (31) and noise (18).
    pub fn write_state(&self, w: &mut StateWriter<'_>) -> Result<(), StateError> {
        w.put_u8(self.step)?;
        w.put_bool(self.powered)?;
        w.put_bool(self.prev_div_bit)?;
        self.pulse1.write_state(w)?;
        self.pulse2.write_state(w)?;
        self.wave.write_state(w)?;
        self.noise.write_state(w)
    }

    /// Reads what `write_state` wrote; a step above 7 is `InvalidState`. The APU is left
    /// unchanged on error.
    pub fn read_state(&mut self, r: &mut StateReader<'_>) -> Result<(), StateError> {
        let step = r.get_u8()?;
        if step > 7 {
            return Err(StateError::InvalidState);
        }
        let powered = r.get_bool()?;
        let prev_div_bit = r.get_bool()?;
        let mut next = Apu::default();
        next.pulse1.read_state(r)?;
        next.pulse2.read_state(r)?;
        next.wave.read_state(r)?;
        next.noise.read_state(r)?;
        next.step = step;
        next.powered = powered;
        next.prev_div_bit = prev_div_bit;
        *self = next;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[allow(clippy::field_reassign_with_default)]
    fn sample() -> Apu {
        let mut apu = Apu::default();
        apu.step = 5;
        apu.powered = true;
        apu.prev_div_bit = true;
        apu.pulse1.freq = 0x0102;
        apu.pulse2.duty = 3;
        apu.wave.pos = 7;
        apu.noise.lfsr = 0x7FFF;
        apu
    }

    fn sample_bytes() -> [u8; 98] {
        let mut e = [0u8; 98];
        e[0] = 5;
        e[1] = 1;
        e[2] = 1;
        e[7] = 0x02;
        e[8] = 0x01;
        e[28] = 0x03;
        e[57] = 0x07;
        e[85] = 0xFF;
        e[86] = 0x7F;
        e
    }

    fn saved(apu: &Apu) -> [u8; 98] {
        let mut buf = [0u8; 98];
        let mut w = StateWriter::new(&mut buf);
        apu.write_state(&mut w).expect("room");
        assert_eq!(w.position(), 98);
        buf
    }

    #[test]
    fn e09_02_apu_layout_is_fixed() {
        assert_eq!(saved(&sample()), sample_bytes());
    }

    #[test]
    fn e09_02_apu_round_trip_is_byte_identical() {
        let bytes = sample_bytes();
        let mut loaded = Apu::default();
        let mut r = StateReader::new(&bytes);
        loaded.read_state(&mut r).expect("valid");
        assert_eq!(r.remaining(), 0);
        assert_eq!(loaded.step, 5);
        assert_eq!(loaded.pulse1.freq, 0x0102);
        assert_eq!(loaded.noise.lfsr, 0x7FFF);
        assert_eq!(saved(&loaded), bytes);
    }

    #[test]
    fn e09_02_apu_truncated_data_is_bad_size_and_leaves_apu_unchanged() {
        let bytes = sample_bytes();
        for len in 0..98 {
            let mut apu = sample();
            assert_eq!(
                apu.read_state(&mut StateReader::new(&bytes[..len])),
                Err(StateError::BadSize),
                "len {len}"
            );
            assert_eq!(saved(&apu), bytes);
        }
    }

    #[test]
    fn e09_02_apu_corrupted_bytes_are_invalid_state_and_leave_apu_unchanged() {
        for (index, value) in [
            (0usize, 8u8),
            (1, 2),
            (2, 2),
            (3, 2),
            (26, 2),
            (49, 2),
            (80, 2),
        ] {
            let mut bytes = sample_bytes();
            bytes[index] = value;
            let mut apu = sample();
            assert_eq!(
                apu.read_state(&mut StateReader::new(&bytes)),
                Err(StateError::InvalidState),
                "index {index}"
            );
            assert_eq!(saved(&apu), sample_bytes());
        }
    }

    #[test]
    fn e09_02_apu_write_to_small_buffer_fails() {
        let mut buf = [0u8; 97];
        let mut w = StateWriter::new(&mut buf);
        assert_eq!(
            sample().write_state(&mut w),
            Err(StateError::BufferTooSmall)
        );
    }
}
