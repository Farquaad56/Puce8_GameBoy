//! Frames-per-second counter with one decimal, integer math only (E07_06).

/// Counts frames over windows of at least one second.
#[derive(Debug, Default)]
pub struct FpsCounter {
    count: u32,
    window_start_ms: u64,
    fps_x10: u32,
}

impl FpsCounter {
    /// Registers one frame shown at `now_ms` (milliseconds since an arbitrary start).
    pub fn frame(&mut self, now_ms: u64) {
        self.count += 1;
        let elapsed = now_ms.saturating_sub(self.window_start_ms);
        if elapsed >= 1000 {
            let x10 = u64::from(self.count) * 10_000 / elapsed;
            self.fps_x10 = u32::try_from(x10).unwrap_or(u32::MAX);
            self.count = 0;
            self.window_start_ms = now_ms;
        }
    }

    /// Frames per second times ten, from the last full window.
    #[cfg(test)]
    pub fn fps_x10(&self) -> u32 {
        self.fps_x10
    }

    /// Text such as "59.7 FPS".
    pub fn label(&self) -> String {
        format!("{}.{} FPS", self.fps_x10 / 10, self.fps_x10 % 10)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn e07_06_fps_sixty_frames_in_one_second() {
        let mut counter = FpsCounter::default();
        for k in 1..=60u64 {
            counter.frame(k * 1000 / 60);
        }
        assert_eq!(counter.fps_x10(), 600);
        assert_eq!(counter.label(), "60.0 FPS");
    }

    #[test]
    fn e07_06_fps_has_one_decimal() {
        let mut counter = FpsCounter::default();
        for k in 1..=60u64 {
            counter.frame(k * 1005 / 60);
        }
        assert_eq!(counter.fps_x10(), 597);
        assert_eq!(counter.label(), "59.7 FPS");
    }

    #[test]
    fn e07_06_fps_is_zero_before_the_first_full_window() {
        let mut counter = FpsCounter::default();
        for k in 1..=30u64 {
            counter.frame(k * 16);
        }
        assert_eq!(counter.label(), "0.0 FPS");
    }
}
