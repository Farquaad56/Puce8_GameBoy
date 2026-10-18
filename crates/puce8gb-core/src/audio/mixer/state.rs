//! Save state of one high-pass filter (E09_02, decision A_05): the capacitor, 8 bytes.

use super::Hpf;
use crate::savestate::{StateError, StateReader, StateWriter};

impl Hpf {
    /// Writes the capacitor as a little-endian 64-bit two's complement value.
    pub fn write_state(&self, w: &mut StateWriter<'_>) -> Result<(), StateError> {
        w.put_u64(u64::from_le_bytes(self.cap.to_le_bytes()))
    }

    /// Reads what `write_state` wrote; the filter is left unchanged on error.
    pub fn read_state(&mut self, r: &mut StateReader<'_>) -> Result<(), StateError> {
        self.cap = i64::from_le_bytes(r.get_u64()?.to_le_bytes());
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn saved(hpf: &Hpf) -> [u8; 8] {
        let mut buf = [0u8; 8];
        let mut w = StateWriter::new(&mut buf);
        hpf.write_state(&mut w).expect("room");
        assert_eq!(w.position(), 8);
        buf
    }

    #[test]
    fn e09_02_hpf_layout_is_fixed() {
        assert_eq!(
            saved(&Hpf {
                cap: 0x0102_0304_0506_0708
            }),
            [8, 7, 6, 5, 4, 3, 2, 1]
        );
        assert_eq!(
            saved(&Hpf { cap: -2 }),
            [0xFE, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF]
        );
    }

    #[test]
    fn e09_02_hpf_round_trip_keeps_negative_values() {
        let bytes = [0xFE, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF];
        let mut hpf = Hpf::default();
        let mut r = StateReader::new(&bytes);
        hpf.read_state(&mut r).expect("valid");
        assert_eq!(r.remaining(), 0);
        assert_eq!(hpf.cap, -2);
        assert_eq!(saved(&hpf), bytes);
    }

    #[test]
    fn e09_02_hpf_truncated_data_is_bad_size_and_leaves_filter_unchanged() {
        let bytes = [8u8, 7, 6, 5, 4, 3, 2, 1];
        for len in 0..8 {
            let mut hpf = Hpf { cap: 77 };
            assert_eq!(
                hpf.read_state(&mut StateReader::new(&bytes[..len])),
                Err(StateError::BadSize),
                "len {len}"
            );
            assert_eq!(hpf.cap, 77);
        }
    }

    #[test]
    fn e09_02_hpf_write_to_small_buffer_fails() {
        let mut buf = [0u8; 7];
        let mut w = StateWriter::new(&mut buf);
        assert_eq!(
            Hpf::default().write_state(&mut w),
            Err(StateError::BufferTooSmall)
        );
    }
}
