//! Audio output: cpal stream fed by a lock-free ring of stereo pairs (E07_03).
use crate::pacing::DOT_HZ;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use puce8gb_core::machine::FRAME_DOTS;
use rtrb::{Producer, RingBuffer};

/// Silence written when the ring runs dry (underrun policy).
pub const SILENCE: [f32; 2] = [0.0, 0.0];
/// Fill level to keep in the ring, in emulated frames.
pub const TARGET_FRAMES: usize = 3;

/// Pacing decision derived from the ring fill level.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pace {
    RunFrame,
    Wait,
}

/// Stereo pairs produced by one emulated frame at the device rate, rounded up.
pub fn frame_pairs(rate: u32) -> usize {
    let num = u64::from(rate) * u64::from(FRAME_DOTS);
    usize::try_from(num.div_ceil(DOT_HZ)).unwrap_or(usize::MAX)
}

/// Fill level (in pairs) the emulation thread tries to keep.
pub fn target_fill(rate: u32) -> usize {
    frame_pairs(rate) * TARGET_FRAMES
}

/// Ring length in pairs: a quarter of a second.
pub fn ring_capacity(rate: u32) -> usize {
    usize::try_from(rate / 4).unwrap_or(usize::MAX)
}

/// Run another frame while the ring is below the target, otherwise wait.
pub fn pacing_decision(fill: usize, target: usize) -> Pace {
    if fill < target {
        Pace::RunFrame
    } else {
        Pace::Wait
    }
}

/// Overflow policy: only what fits is pushed, the excess is dropped.
pub fn push_budget(free: usize, produced: usize) -> usize {
    free.min(produced)
}

/// Underrun policy: a missing pair becomes silence.
pub fn fill_frame(popped: Option<[f32; 2]>) -> [f32; 2] {
    popped.unwrap_or(SILENCE)
}

/// Open output stream; the producer side of the ring stays with the emulation thread.
pub struct AudioOut {
    pub producer: Producer<[f32; 2]>,
    pub rate: u32,
    capacity: usize,
    _stream: cpal::Stream,
}

impl AudioOut {
    /// Opens the default output device (f32, at least 2 channels); None when unavailable.
    pub fn open() -> Option<AudioOut> {
        let host = cpal::default_host();
        let device = host.default_output_device()?;
        let supported = device.default_output_config().ok()?;
        if supported.sample_format() != cpal::SampleFormat::F32 || supported.channels() < 2 {
            return None;
        }
        let rate = supported.sample_rate().0;
        let channels = usize::from(supported.channels());
        let config = supported.config();
        let capacity = ring_capacity(rate);
        let (producer, mut consumer) = RingBuffer::<[f32; 2]>::new(capacity);
        let stream = device
            .build_output_stream(
                &config,
                move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                    for frame in data.chunks_mut(channels) {
                        let pair = fill_frame(consumer.pop().ok());
                        if let [l, r, rest @ ..] = frame {
                            *l = pair[0];
                            *r = pair[1];
                            rest.fill(0.0);
                        }
                    }
                },
                |_err| {},
                None,
            )
            .ok()?;
        stream.play().ok()?;
        Some(AudioOut {
            producer,
            rate,
            capacity,
            _stream: stream,
        })
    }

    /// Free slots in the ring, in pairs.
    pub fn free(&self) -> usize {
        self.producer.slots()
    }

    /// Pairs waiting in the ring.
    pub fn fill(&self) -> usize {
        self.capacity.saturating_sub(self.producer.slots())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn e07_03_frame_pairs_matches_integer_ratio() {
        assert_eq!(frame_pairs(48_000), 804);
        assert_eq!(frame_pairs(44_100), 739);
        assert_eq!(frame_pairs(32_768), 549);
    }

    #[test]
    fn e07_03_target_fits_in_ring() {
        for rate in [8_000u32, 22_050, 44_100, 48_000, 96_000, 192_000] {
            assert!(target_fill(rate) < ring_capacity(rate));
        }
        assert_eq!(target_fill(48_000), 2412);
        assert_eq!(ring_capacity(48_000), 12_000);
    }

    #[test]
    fn e07_03_pacing_decision_follows_fill_level() {
        assert_eq!(pacing_decision(0, 2412), Pace::RunFrame);
        assert_eq!(pacing_decision(2411, 2412), Pace::RunFrame);
        assert_eq!(pacing_decision(2412, 2412), Pace::Wait);
        assert_eq!(pacing_decision(9000, 2412), Pace::Wait);
    }

    #[test]
    fn e07_03_overflow_drops_the_excess() {
        assert_eq!(push_budget(10, 4), 4);
        assert_eq!(push_budget(3, 8), 3);
        assert_eq!(push_budget(0, 5), 0);
    }

    #[test]
    fn e07_03_underrun_is_silence() {
        assert_eq!(fill_frame(None), [0.0, 0.0]);
        assert_eq!(fill_frame(Some([0.25, -0.5])), [0.25, -0.5]);
    }
}
