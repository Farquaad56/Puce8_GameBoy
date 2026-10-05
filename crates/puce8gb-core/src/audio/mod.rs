//! APU / audio (decisions A_04/A_05): sample ring for now; channel state and mixing arrive
//! with later tasks.

use crate::machine::AUDIO_RING_SAMPLES;

/// Audio output: interleaved stereo sample ring at 32768 Hz, ~0.125 s of latency
/// (decision A_04).
#[derive(Debug)]
pub struct Audio {
    /// Interleaved L/R samples; `head` is the next write index, `tail` the next read index.
    pub ring: [i16; AUDIO_RING_SAMPLES],
    pub head: u32,
    pub tail: u32,
}

impl Default for Audio {
    fn default() -> Self {
        Audio {
            ring: [0x00; AUDIO_RING_SAMPLES],
            head: 0,
            tail: 0,
        }
    }
}

impl Audio {
    /// Drain up to `into.len()` interleaved samples from the ring (decision A_04).
    pub fn drain(&mut self, into: &mut [i16]) -> usize {
        let cap = self.ring.len() as u32;
        if self.head == self.tail || into.is_empty() {
            return 0;
        }
        let available = ((self.head - self.tail) % cap) as usize;
        let n = std::cmp::min(available, into.len());
        // Read the ring in at most two contiguous chunks (wrap point).
        let start = self.tail as usize;
        let first = std::cmp::min(n, self.ring.len() - start);
        into[..first].copy_from_slice(&self.ring[start..start + first]);
        if n > first {
            into[first..].copy_from_slice(&self.ring[..n - first]);
        }
        self.tail = (self.tail + n as u32) % cap;
        n
    }
}
