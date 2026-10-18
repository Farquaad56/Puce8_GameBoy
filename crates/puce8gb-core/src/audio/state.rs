//! Save state of the audio output (E09_02, decision A_05): head, tail, sample_phase, the APU,
//! the two high-pass filters, then only the pending ring samples (from tail up to head,
//! wrapping), each as a little-endian 16-bit value. 123 bytes + 2 per pending sample.
//! The already drained slots of the ring do not influence behaviour and are not saved.

use super::{Audio, SAMPLE_PERIOD_MCYCLES};
use crate::audio::apu::Apu;
use crate::audio::mixer::Hpf;
use crate::machine::AUDIO_RING_SAMPLES;
use crate::savestate::{StateError, StateReader, StateWriter};

const RING_LEN: u32 = AUDIO_RING_SAMPLES as u32;

/// Number of samples waiting between `tail` and `head` in the ring.
fn pending(head: u32, tail: u32) -> usize {
    ((head % RING_LEN + RING_LEN - tail % RING_LEN) % RING_LEN) as usize
}

impl Audio {
    /// Writes the audio state in the order described in the module comment.
    pub fn write_state(&self, w: &mut StateWriter<'_>) -> Result<(), StateError> {
        w.put_u32(self.head)?;
        w.put_u32(self.tail)?;
        w.put_u8(self.sample_phase)?;
        self.apu.write_state(w)?;
        self.hpf_left.write_state(w)?;
        self.hpf_right.write_state(w)?;
        let mut index = (self.tail % RING_LEN) as usize;
        for _ in 0..pending(self.head, self.tail) {
            let sample = self.ring.get(index).copied().unwrap_or(0);
            w.put_u16(u16::from_le_bytes(sample.to_le_bytes()))?;
            index = (index + 1) % AUDIO_RING_SAMPLES;
        }
        Ok(())
    }

    /// Reads what `write_state` wrote. `head` and `tail` must be below the ring size and
    /// `sample_phase` below SAMPLE_PERIOD_MCYCLES, else `InvalidState`. The audio state is
    /// left unchanged on error.
    pub fn read_state(&mut self, r: &mut StateReader<'_>) -> Result<(), StateError> {
        let head = r.get_u32()?;
        let tail = r.get_u32()?;
        let sample_phase = r.get_u8()?;
        if head >= RING_LEN || tail >= RING_LEN || sample_phase >= SAMPLE_PERIOD_MCYCLES {
            return Err(StateError::InvalidState);
        }
        let mut apu = Apu::default();
        apu.read_state(r)?;
        let mut hpf_left = Hpf::default();
        hpf_left.read_state(r)?;
        let mut hpf_right = Hpf::default();
        hpf_right.read_state(r)?;
        let mut ring = [0i16; AUDIO_RING_SAMPLES];
        let mut index = tail as usize;
        for _ in 0..pending(head, tail) {
            let raw = r.get_u16()?;
            ring[index] = i16::from_le_bytes(raw.to_le_bytes());
            index = (index + 1) % AUDIO_RING_SAMPLES;
        }
        self.ring = ring;
        self.head = head;
        self.tail = tail;
        self.apu = apu;
        self.sample_phase = sample_phase;
        self.hpf_left = hpf_left;
        self.hpf_right = hpf_right;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[allow(clippy::field_reassign_with_default)]
    fn sample_audio() -> Audio {
        let mut audio = Audio::default();
        audio.head = 6;
        audio.tail = 2;
        audio.sample_phase = 17;
        audio.ring[2] = -2;
        audio.ring[3] = 3;
        audio.ring[4] = 0x1234;
        audio.ring[5] = i16::MIN;
        audio.apu.step = 5;
        audio.apu.pulse1.freq = 0x0102;
        audio
    }

    fn expected_bytes() -> Vec<u8> {
        let mut e = vec![0x06, 0, 0, 0, 0x02, 0, 0, 0, 0x11];
        let mut apu = [0u8; 98];
        apu[0] = 5;
        apu[7] = 0x02;
        apu[8] = 0x01;
        e.extend_from_slice(&apu);
        e.extend_from_slice(&[0u8; 16]);
        e.extend_from_slice(&[0xFE, 0xFF, 0x03, 0x00, 0x34, 0x12, 0x00, 0x80]);
        e
    }

    fn saved(audio: &Audio) -> Vec<u8> {
        let mut buf = vec![0u8; 4096];
        let mut w = StateWriter::new(&mut buf);
        audio.write_state(&mut w).expect("room");
        let len = w.position();
        buf.truncate(len);
        buf
    }

    #[test]
    fn e09_02_audio_layout_is_fixed() {
        let bytes = saved(&sample_audio());
        assert_eq!(bytes.len(), 131);
        assert_eq!(bytes, expected_bytes());
    }

    #[test]
    fn e09_02_audio_round_trip_is_byte_identical() {
        let mut audio = sample_audio();
        audio.hpf_left.apply(1000);
        audio.hpf_right.apply(-300);
        let bytes = saved(&audio);
        let mut loaded = Audio::default();
        let mut r = StateReader::new(&bytes);
        loaded.read_state(&mut r).expect("valid");
        assert_eq!(r.remaining(), 0);
        assert_eq!(loaded.head, 6);
        assert_eq!(loaded.tail, 2);
        assert_eq!(loaded.sample_phase, 17);
        assert_eq!(loaded.ring[4], 0x1234);
        assert_eq!(loaded.ring[0], 0);
        assert_eq!(loaded.apu.step, 5);
        assert_eq!(saved(&loaded), bytes);
    }

    #[test]
    #[allow(clippy::field_reassign_with_default)]
    fn e09_02_audio_ring_wraps_around() {
        assert_eq!(AUDIO_RING_SAMPLES, 8192);
        let mut audio = Audio::default();
        audio.head = 1;
        audio.tail = 8190;
        audio.ring[8190] = 7;
        audio.ring[8191] = -8;
        audio.ring[0] = 9;
        let bytes = saved(&audio);
        assert_eq!(bytes.len(), 123 + 6);
        assert_eq!(&bytes[123..], &[0x07, 0x00, 0xF8, 0xFF, 0x09, 0x00]);
        let mut loaded = Audio::default();
        loaded
            .read_state(&mut StateReader::new(&bytes))
            .expect("valid");
        assert_eq!(loaded.ring[8190], 7);
        assert_eq!(loaded.ring[8191], -8);
        assert_eq!(loaded.ring[0], 9);
        assert_eq!(loaded.head, 1);
        assert_eq!(loaded.tail, 8190);
        assert_eq!(saved(&loaded), bytes);
    }

    #[test]
    fn e09_02_audio_truncated_data_is_bad_size_and_leaves_audio_unchanged() {
        let bytes = expected_bytes();
        for len in 0..bytes.len() {
            let mut audio = sample_audio();
            assert_eq!(
                audio.read_state(&mut StateReader::new(&bytes[..len])),
                Err(StateError::BadSize),
                "len {len}"
            );
            assert_eq!(saved(&audio), bytes);
        }
    }

    #[test]
    fn e09_02_audio_invalid_values_are_rejected_and_leave_audio_unchanged() {
        for (index, value) in [
            (1usize, 0x20u8),
            (5, 0x20),
            (8, 32),
            (9, 8),
            (10, 2),
            (11, 2),
            (12, 2),
        ] {
            let mut bytes = expected_bytes();
            bytes[index] = value;
            let mut audio = sample_audio();
            assert_eq!(
                audio.read_state(&mut StateReader::new(&bytes)),
                Err(StateError::InvalidState),
                "index {index}"
            );
            assert_eq!(saved(&audio), expected_bytes());
        }
    }

    #[test]
    fn e09_02_audio_write_to_small_buffer_fails() {
        let mut buf = [0u8; 130];
        let mut w = StateWriter::new(&mut buf);
        assert_eq!(
            sample_audio().write_state(&mut w),
            Err(StateError::BufferTooSmall)
        );
    }
}
