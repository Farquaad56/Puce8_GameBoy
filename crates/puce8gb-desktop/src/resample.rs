//! Windowed-sinc resampler: 32768 Hz core stream to the device rate, low-pass before decimation (E07_04).
use std::collections::VecDeque;
use std::f64::consts::PI;

/// Native sample rate of the core output (decision A_04): one stereo pair every 128 dots.
pub const SRC_RATE: u32 = 32_768;
/// Zero crossings of the sinc kernel on each side of the centre (kernel width in cutoff periods).
const ZERO_CROSSINGS: f64 = 12.0;
/// Cutoff as a fraction of the lower of the two Nyquist frequencies.
const CUTOFF: f64 = 0.9;
/// Lowest output rate supported; lower values are clamped.
const MIN_DST_RATE: u32 = 8_000;

fn to_f32(sample: i16) -> f32 {
    f32::from(sample) / 32768.0
}

/// Blackman-windowed sinc, `x` in input samples from the output instant.
fn kernel(x: f64, scale: f64, half: f64) -> f64 {
    let u = x / half;
    if u.abs() >= 1.0 {
        return 0.0;
    }
    let z = scale * x;
    let sinc = if z.abs() < 1e-12 {
        1.0
    } else {
        (PI * z).sin() / (PI * z)
    };
    let window = 0.42 + 0.5 * (PI * u).cos() + 0.08 * (2.0 * PI * u).cos();
    scale * sinc * window
}

/// Converts the 32768 Hz core stream to the device rate: integer phase accumulator for the
/// timing, windowed-sinc low-pass for the values.
#[derive(Debug)]
pub struct Resampler {
    dst_rate: u32,
    phase: u32,
    scale: f64,
    half_f: f64,
    window: VecDeque<[f32; 2]>,
    weights: Vec<f64>,
}

impl Resampler {
    pub fn new(dst_rate: u32) -> Self {
        let dst_rate = dst_rate.max(MIN_DST_RATE);
        let ratio = (f64::from(dst_rate) / f64::from(SRC_RATE)).min(1.0);
        let scale = CUTOFF * ratio;
        let half = (ZERO_CROSSINGS / scale).ceil() as usize;
        let taps = 2 * half;
        Resampler {
            dst_rate,
            phase: 0,
            scale,
            half_f: half as f64,
            window: VecDeque::from(vec![[0.0f32; 2]; taps]),
            weights: vec![0.0; taps],
        }
    }

    /// Feeds one input pair and appends the output pairs it produces.
    pub fn push(&mut self, cur: [i16; 2], out: &mut Vec<[f32; 2]>) {
        self.window.pop_front();
        self.window.push_back([to_f32(cur[0]), to_f32(cur[1])]);
        while self.phase < self.dst_rate {
            let t = f64::from(self.phase) / f64::from(self.dst_rate);
            out.push(self.interpolate(t));
            self.phase += SRC_RATE;
        }
        self.phase -= self.dst_rate;
    }

    /// Output pair at fraction `t` (0..1) after the centre input sample, weights normalised to unity.
    fn interpolate(&mut self, t: f64) -> [f32; 2] {
        let mut sum = 0.0;
        for (j, w) in self.weights.iter_mut().enumerate() {
            let x = (j as f64) - (self.half_f - 1.0) - t;
            *w = kernel(x, self.scale, self.half_f);
            sum += *w;
        }
        let mut acc = [0.0f64; 2];
        for (w, frame) in self.weights.iter().zip(self.window.iter()) {
            acc[0] += w * f64::from(frame[0]);
            acc[1] += w * f64::from(frame[1]);
        }
        if sum.abs() < 1e-12 {
            return [0.0, 0.0];
        }
        [(acc[0] / sum) as f32, (acc[1] / sum) as f32]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run_tone(dst_rate: u32, freq_hz: f64, amplitude: f64) -> Vec<[f32; 2]> {
        let mut resampler = Resampler::new(dst_rate);
        let mut out = Vec::new();
        for n in 0..SRC_RATE {
            let phase = 2.0 * PI * freq_hz * f64::from(n) / f64::from(SRC_RATE);
            let s = (amplitude * phase.sin()).round() as i16;
            resampler.push([s, s], &mut out);
        }
        out
    }

    fn rms(frames: &[[f32; 2]]) -> f64 {
        let sum: f64 = frames.iter().map(|f| f64::from(f[0]).powi(2)).sum();
        (sum / frames.len() as f64).sqrt()
    }

    #[test]
    fn e07_04_dc_input_keeps_unity_gain() {
        for rate in [48_000u32, 44_100, 22_050] {
            let mut resampler = Resampler::new(rate);
            let mut out = Vec::new();
            for _ in 0..3000 {
                resampler.push([10_000, -10_000], &mut out);
            }
            let want = 10_000.0 / 32768.0;
            for frame in &out[300..] {
                assert!((f64::from(frame[0]) - want).abs() < 1e-4, "rate {rate}");
                assert!((f64::from(frame[1]) + want).abs() < 1e-4, "rate {rate}");
            }
        }
    }

    #[test]
    fn e07_04_tone_above_output_nyquist_is_attenuated_by_30_db() {
        let out = run_tone(22_050, 14_000.0, 20_000.0);
        let input_rms = 20_000.0 / 32768.0 / 2.0f64.sqrt();
        let limit = input_rms * 10f64.powf(-30.0 / 20.0);
        assert!(rms(&out[500..]) < limit);
    }

    #[test]
    fn e07_04_passband_tone_keeps_its_level() {
        for rate in [48_000u32, 22_050] {
            let out = run_tone(rate, 1_000.0, 20_000.0);
            let input_rms = 20_000.0 / 32768.0 / 2.0f64.sqrt();
            let ratio = rms(&out[500..]) / input_rms;
            assert!(ratio > 0.95 && ratio < 1.05, "rate {rate} ratio {ratio}");
        }
    }

    #[test]
    fn e07_04_output_length_matches_ratio() {
        for rate in [48_000u32, 44_100, 22_050, 96_000] {
            let mut resampler = Resampler::new(rate);
            let mut out = Vec::new();
            for _ in 0..SRC_RATE {
                resampler.push([i16::MAX, i16::MIN], &mut out);
            }
            assert_eq!(out.len(), usize::try_from(rate).unwrap());
        }
        let mut resampler = Resampler::new(48_000);
        let mut out = Vec::new();
        for _ in 0..100 {
            resampler.push([1, 1], &mut out);
        }
        assert_eq!(out.len(), 147);
    }

    #[test]
    fn e07_04_same_rate_is_one_to_one() {
        let mut resampler = Resampler::new(32_768);
        let mut out = Vec::new();
        for _ in 0..100 {
            resampler.push([5, -5], &mut out);
        }
        assert_eq!(out.len(), 100);
    }

    #[test]
    fn e07_04_silence_stays_silent() {
        let mut resampler = Resampler::new(44_100);
        let mut out = Vec::new();
        for _ in 0..500 {
            resampler.push([0, 0], &mut out);
        }
        assert!(out.iter().all(|f| f[0] == 0.0 && f[1] == 0.0));
    }
}
