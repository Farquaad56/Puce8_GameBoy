//! Volume curve: percent to linear gain (E07_06).

pub const DEFAULT_VOLUME: u8 = 80;
pub const MAX_VOLUME: u8 = 100;

/// Perceptual curve: gain = (percent / 100)^2, percent clamped to 0..=100.
pub fn gain_for_percent(percent: u8) -> f32 {
    let p = f32::from(percent.min(MAX_VOLUME)) / f32::from(MAX_VOLUME);
    p * p
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn e07_06_volume_curve_ends_and_middle() {
        assert_eq!(gain_for_percent(0), 0.0);
        assert_eq!(gain_for_percent(100), 1.0);
        assert_eq!(gain_for_percent(50), 0.25);
    }

    #[test]
    fn e07_06_volume_above_100_is_clamped() {
        assert_eq!(gain_for_percent(101), 1.0);
        assert_eq!(gain_for_percent(255), 1.0);
    }

    #[test]
    fn e07_06_volume_curve_is_monotonic() {
        for p in 0..100u8 {
            assert!(gain_for_percent(p) < gain_for_percent(p + 1));
        }
    }
}
