//! Display geometry for the desktop window (E10_01, decision A_08).
//! Pure functions, no egui type. Integer scales 1..=6 only.

/// Game Boy screen width in pixels.
#[allow(dead_code)] // used by E10_03
pub const GB_WIDTH: u32 = 160;
/// Game Boy screen height in pixels.
#[allow(dead_code)] // used by E10_03
pub const GB_HEIGHT: u32 = 144;
/// Smallest window scale.
#[allow(dead_code)] // used by E10_02
pub const MIN_SCALE: u32 = 1;
/// Largest window scale.
#[allow(dead_code)] // used by E10_02
pub const MAX_SCALE: u32 = 6;

/// Clamps a scale to MIN_SCALE..=MAX_SCALE (0 gives 1, 9 gives 6).
#[allow(dead_code)] // used by E10_02
pub fn clamp_scale(n: u32) -> u32 {
    n.clamp(MIN_SCALE, MAX_SCALE)
}

/// Image size in pixels for a scale.
#[allow(dead_code)] // used by E10_03
pub fn image_size(scale: u32) -> (u32, u32) {
    let s = clamp_scale(scale);
    (GB_WIDTH * s, GB_HEIGHT * s)
}

/// Menu label for a scale: "1x1" .. "6x6".
#[allow(dead_code)] // used by E10_03
pub fn scale_label(scale: u32) -> String {
    let s = clamp_scale(scale);
    format!("{s}x{s}")
}

/// Largest scale whose image fits in the available area; 1 when none fits.
#[allow(dead_code)] // used by E10_03
pub fn best_scale(avail_w: f32, avail_h: f32) -> u32 {
    (MIN_SCALE..=MAX_SCALE)
        .rev()
        .find(|&s| (GB_WIDTH * s) as f32 <= avail_w && (GB_HEIGHT * s) as f32 <= avail_h)
        .unwrap_or(MIN_SCALE)
}

/// Window inner size for a scale: image plus the height of the menu and status bars.
#[allow(dead_code)] // used by E10_03
pub fn window_size(scale: u32, chrome_h: f32, min_w: f32) -> (f32, f32) {
    let s = clamp_scale(scale);
    (
        ((GB_WIDTH * s) as f32).max(min_w),
        (GB_HEIGHT * s) as f32 + chrome_h,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn e10_01_clamp_scale_limits() {
        assert_eq!(clamp_scale(0), 1);
        assert_eq!(clamp_scale(3), 3);
        assert_eq!(clamp_scale(9), 6);
    }

    #[test]
    fn e10_01_image_size_scales_160x144() {
        assert_eq!(image_size(4), (640, 576));
        assert_eq!(image_size(1), (160, 144));
        assert_eq!(image_size(99), (960, 864));
    }

    #[test]
    fn e10_01_scale_label_format() {
        assert_eq!(scale_label(2), "2x2");
        assert_eq!(scale_label(6), "6x6");
        assert_eq!(scale_label(0), "1x1");
    }

    #[test]
    fn e10_01_best_scale_picks_largest_fit() {
        assert_eq!(best_scale(800.0, 600.0), 4);
        assert_eq!(best_scale(100.0, 100.0), 1);
        assert_eq!(best_scale(5000.0, 5000.0), 6);
        assert_eq!(best_scale(f32::NAN, 600.0), 1);
    }

    #[test]
    fn e10_01_window_size_adds_chrome_and_min_width() {
        assert_eq!(window_size(3, 60.0, 400.0), (480.0, 492.0));
        assert_eq!(window_size(1, 60.0, 400.0), (400.0, 204.0));
    }
}
