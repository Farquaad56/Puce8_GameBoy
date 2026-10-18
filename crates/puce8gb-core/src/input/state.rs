//! Save state of the button input (E09_02, decision A_05): one byte, bit 0 up, bit 1 down,
//! bit 2 left, bit 3 right, bit 4 a, bit 5 b, bit 6 start, bit 7 select.

use super::Input;
use crate::savestate::{StateError, StateReader, StateWriter};

impl Input {
    /// Writes the button state as one bit mask byte.
    pub fn write_state(&self, w: &mut StateWriter<'_>) -> Result<(), StateError> {
        let bits = u8::from(self.up)
            | (u8::from(self.down) << 1)
            | (u8::from(self.left) << 2)
            | (u8::from(self.right) << 3)
            | (u8::from(self.a) << 4)
            | (u8::from(self.b) << 5)
            | (u8::from(self.start) << 6)
            | (u8::from(self.select) << 7);
        w.put_u8(bits)
    }

    /// Reads what `write_state` wrote; the input is left unchanged on error.
    pub fn read_state(&mut self, r: &mut StateReader<'_>) -> Result<(), StateError> {
        let bits = r.get_u8()?;
        *self = Input {
            up: bits & 0x01 != 0,
            down: bits & 0x02 != 0,
            left: bits & 0x04 != 0,
            right: bits & 0x08 != 0,
            a: bits & 0x10 != 0,
            b: bits & 0x20 != 0,
            start: bits & 0x40 != 0,
            select: bits & 0x80 != 0,
        };
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Input {
        Input {
            up: true,
            left: true,
            a: true,
            start: true,
            ..Input::default()
        }
    }

    #[test]
    fn e09_02_input_layout_is_a_bit_mask() {
        let mut buf = [0u8; 1];
        let mut w = StateWriter::new(&mut buf);
        sample().write_state(&mut w).expect("room");
        assert_eq!(w.position(), 1);
        assert_eq!(buf, [0x55]);
        let mut buf = [0u8; 1];
        Input::default()
            .write_state(&mut StateWriter::new(&mut buf))
            .expect("room");
        assert_eq!(buf, [0x00]);
    }

    #[test]
    fn e09_02_input_round_trip_for_every_bit_mask() {
        for bits in 0..=255u8 {
            let data = [bits];
            let mut input = Input::default();
            input
                .read_state(&mut StateReader::new(&data))
                .expect("valid");
            let mut buf = [0u8; 1];
            input
                .write_state(&mut StateWriter::new(&mut buf))
                .expect("room");
            assert_eq!(buf, data);
        }
    }

    #[test]
    fn e09_02_input_empty_data_is_bad_size_and_leaves_input_unchanged() {
        let mut input = sample();
        assert_eq!(
            input.read_state(&mut StateReader::new(&[])),
            Err(StateError::BadSize)
        );
        assert_eq!(input, sample());
        let mut buf = [0u8; 0];
        assert_eq!(
            input.write_state(&mut StateWriter::new(&mut buf)),
            Err(StateError::BufferTooSmall)
        );
    }
}
