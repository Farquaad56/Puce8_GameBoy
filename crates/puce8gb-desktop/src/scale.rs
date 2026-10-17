/// Largest integer scale (>= 1) at which a w x h image fits in an avail_w x avail_h area (E07_01).
pub fn integer_scale(avail_w: u32, avail_h: u32, w: u32, h: u32) -> u32 {
    if w == 0 || h == 0 {
        return 1;
    }
    (avail_w / w).min(avail_h / h).max(1)
}

#[cfg(test)]
mod tests {
    use super::integer_scale;

    #[test]
    fn e07_01_exact_multiples() {
        assert_eq!(integer_scale(160, 144, 160, 144), 1);
        assert_eq!(integer_scale(320, 288, 160, 144), 2);
        assert_eq!(integer_scale(480, 432, 160, 144), 3);
    }

    #[test]
    fn e07_01_rounds_down_when_not_a_multiple() {
        assert_eq!(integer_scale(479, 431, 160, 144), 2);
        assert_eq!(integer_scale(639, 575, 160, 144), 3);
    }

    #[test]
    fn e07_01_limited_by_the_tighter_axis() {
        assert_eq!(integer_scale(1600, 288, 160, 144), 2);
        assert_eq!(integer_scale(320, 1440, 160, 144), 2);
    }

    #[test]
    fn e07_01_never_below_one() {
        assert_eq!(integer_scale(0, 0, 160, 144), 1);
        assert_eq!(integer_scale(100, 100, 160, 144), 1);
        assert_eq!(integer_scale(500, 500, 0, 144), 1);
    }
}
