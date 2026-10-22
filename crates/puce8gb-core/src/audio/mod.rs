//! APU / audio (decisions A_04/A_05): channel state, mixer, sample ring and frame sequencer.

pub mod apu;
pub mod mixer;
pub mod noise;
pub mod pulse;
mod state;
pub mod wave;

use crate::audio::mixer::{analog15, mix_side, Hpf};
use crate::machine::AUDIO_RING_SAMPLES;

/// M-cycles between two output samples: 128 dots = 32768 Hz (decision A_04).
pub const SAMPLE_PERIOD_MCYCLES: u8 = 32;

/// Audio output: interleaved stereo sample ring at 32768 Hz, ~0.125 s of latency
/// (decision A_04).
#[derive(Debug)]
pub struct Audio {
    /// Interleaved L/R samples; `head` is the next write index, `tail` the next read index.
    pub ring: [i16; AUDIO_RING_SAMPLES],
    pub head: u32,
    pub tail: u32,
    /// APU channels and frame sequencer state (E04_01 to E04_04).
    pub apu: crate::audio::apu::Apu,
    /// M-cycles elapsed since the last output sample (0..SAMPLE_PERIOD_MCYCLES).
    pub sample_phase: u8,
    hpf_left: Hpf,
    hpf_right: Hpf,
}

impl Default for Audio {
    fn default() -> Self {
        Audio {
            ring: [0x00; AUDIO_RING_SAMPLES],
            head: 0,
            tail: 0,
            apu: crate::audio::apu::Apu::default(),
            sample_phase: 0,
            hpf_left: Hpf::default(),
            hpf_right: Hpf::default(),
        }
    }
}

impl Audio {
    /// Restart the sampling phase and clear the high-pass filters (machine reset).
    pub fn reset_mixer(&mut self) {
        self.sample_phase = 0;
        self.hpf_left.reset();
        self.hpf_right.reset();
    }

    /// Store one stereo frame in the ring. A full ring drops the frame (decision A_04);
    /// one frame slot always stays free so that head == tail means empty.
    fn push_frame(&mut self, left: i16, right: i16) {
        let cap = self.ring.len() as u32;
        let next = (self.head + 2) % cap;
        if next == self.tail {
            return;
        }
        self.ring[self.head as usize] = left;
        self.ring[(self.head + 1) as usize] = right;
        self.head = next;
    }

    /// Called once per M-cycle after the APU ticked. `nr50` and `nr51` are the master volume and
    /// panning registers. Every SAMPLE_PERIOD_MCYCLES calls, one stereo frame is mixed and stored.
    /// When all four DACs are off the output is 0 and the filters restart from 0
    /// (detail of the filter state in that case: UNKNOWN - to confirm).
    pub fn mix_tick(&mut self, nr50: u8, nr51: u8) {
        self.sample_phase += 1;
        if self.sample_phase < SAMPLE_PERIOD_MCYCLES {
            return;
        }
        self.sample_phase = 0;
        let apu = &self.apu;
        let any_dac = apu.pulse1.dac_on || apu.pulse2.dac_on || apu.wave.dac_on || apu.noise.dac_on;
        if !any_dac {
            self.hpf_left.reset();
            self.hpf_right.reset();
            self.push_frame(0, 0);
            return;
        }
        let analog = [
            analog15(apu.pulse1.output(), apu.pulse1.dac_on),
            analog15(apu.pulse2.output(), apu.pulse2.dac_on),
            analog15(apu.wave.output(), apu.wave.dac_on),
            analog15(apu.noise.output(), apu.noise.dac_on),
        ];
        let left = mix_side(analog, nr51 >> 4, nr50 >> 4);
        let right = mix_side(analog, nr51 & 0x0F, nr50);
        let left = self.hpf_left.apply(left).clamp(-32768, 32767) as i16;
        let right = self.hpf_right.apply(right).clamp(-32768, 32767) as i16;
        self.push_frame(left, right);
    }

    /// PCM12 and PCM34 values (CGB registers $FF76/$FF77): low nibble = lower channel number.
    /// A channel that is disabled or whose DAC is off reads 0.
    pub fn pcm(&self) -> (u8, u8) {
        let apu = &self.apu;
        let level =
            |out: u8, dac_on: bool, enabled: bool| if dac_on && enabled { out & 0x0F } else { 0 };
        let c1 = level(apu.pulse1.output(), apu.pulse1.dac_on, apu.pulse1.enabled);
        let c2 = level(apu.pulse2.output(), apu.pulse2.dac_on, apu.pulse2.enabled);
        let c3 = level(apu.wave.output(), apu.wave.dac_on, apu.wave.enabled);
        let c4 = level(apu.noise.output(), apu.noise.dac_on, apu.noise.enabled);
        (c1 | (c2 << 4), c3 | (c4 << 4))
    }

    /// Drain up to `into.len()` interleaved samples from the ring (decision A_04).
    /// Only whole L/R frames are returned (the count is always even).
    pub fn drain(&mut self, into: &mut [i16]) -> usize {
        let cap = self.ring.len() as u32;
        if self.head == self.tail || into.is_empty() {
            return 0;
        }
        let available = ((self.head + cap - self.tail) % cap) as usize;
        let n = std::cmp::min(available, into.len()) & !1;
        // Read the ring in at most two contiguous chunks (wrap point).
        let start = self.tail as usize;
        let first = std::cmp::min(n, self.ring.len() - start);
        into[..first].copy_from_slice(&self.ring[start..start + first]);
        if n > first {
            into[first..n].copy_from_slice(&self.ring[..n - first]);
        }
        self.tail = (self.tail + n as u32) % cap;
        n
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn e04_05_one_frame_every_32_m_cycles() {
        let mut a = Audio::default();
        let mut buf = [0i16; 64];
        for _ in 0..31 {
            a.mix_tick(0x00, 0x00);
        }
        assert_eq!(a.drain(&mut buf), 0);
        a.mix_tick(0x00, 0x00);
        assert_eq!(a.drain(&mut buf), 2);
        for _ in 0..(32 * 10) {
            a.mix_tick(0x00, 0x00);
        }
        assert_eq!(a.drain(&mut buf), 20);
    }

    #[test]
    fn e04_05_all_dacs_off_is_silent() {
        let mut a = Audio::default();
        let mut buf = [0i16; 8];
        for _ in 0..32 {
            a.mix_tick(0x77, 0xFF);
        }
        assert_eq!(a.drain(&mut buf), 2);
        assert_eq!(buf[0], 0);
        assert_eq!(buf[1], 0);
    }

    #[test]
    fn e04_05_channel_1_routed_to_the_left_only() {
        let mut a = Audio::default();
        a.apu.pulse1.write_nrx2(0xF0);
        let mut buf = [0i16; 8];
        for _ in 0..32 {
            a.mix_tick(0x77, 0x10);
        }
        assert_eq!(a.drain(&mut buf), 2);
        assert_eq!(buf[0], 8160);
        assert_eq!(buf[1], 0);
    }

    #[test]
    fn e04_05_channel_1_routed_to_the_right_only_with_volume_2() {
        let mut a = Audio::default();
        a.apu.pulse1.write_nrx2(0xF0);
        let mut buf = [0i16; 8];
        for _ in 0..32 {
            a.mix_tick(0x02, 0x01);
        }
        assert_eq!(a.drain(&mut buf), 2);
        assert_eq!(buf[0], 0);
        assert_eq!(buf[1], 3060);
    }

    #[test]
    fn e04_05_ring_wraps_and_drain_keeps_order() {
        let mut a = Audio::default();
        let cap = AUDIO_RING_SAMPLES;
        let first = (cap / 2 - 100) as i16;
        for i in 0..first {
            a.push_frame(i, -i);
        }
        let mut sink = vec![0i16; cap];
        assert_eq!(a.drain(&mut sink), 2 * first as usize);
        for i in 0..200i16 {
            a.push_frame(i, -i);
        }
        let mut out = vec![0i16; cap];
        assert_eq!(a.drain(&mut out), 400);
        assert_eq!(out[0], 0);
        assert_eq!(out[1], 0);
        assert_eq!(out[2], 1);
        assert_eq!(out[3], -1);
        assert_eq!(out[398], 199);
        assert_eq!(out[399], -199);
    }

    #[test]
    fn battery_pcm_default_audio_reads_zero() {
        let a = Audio::default();
        assert_eq!(a.pcm(), (0, 0));
    }
}
