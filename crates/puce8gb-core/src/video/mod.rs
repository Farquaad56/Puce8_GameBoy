//! PPU / video stub (C01_15): dot counter and LY ($FF44) so ROMs that poll the screen
//! line do not hang. Mode tracking, rendering and the full PPU arrive with epic E03;
//! nothing is drawn yet.

use crate::machine::FRAMEBUFFER_SIZE;

/// Dots per scanline: 456 (note 01_timing.md "Lignes par frame").
pub const LINE_DOTS: u32 = 456;

/// Scanlines per frame: 154; LY holds 0..=153, values 144-153 are the VBlank period
/// (note 01_timing.md "Lignes par frame", note 04a "LY").
pub const LINES_PER_FRAME: u8 = 154;

/// First scanline of the VBlank period: LY = 144..=153 (note 01_timing.md).
pub const VBLANK_FIRST_LINE: u8 = 144;

/// Video state: framebuffer of 2-bit pixel indices, 160 x 144 (decision A_04), plus the
/// dot counter and LY position (C01_15).
#[derive(Debug)]
pub struct Video {
    pub frame: [u8; FRAMEBUFFER_SIZE],
    /// Dots elapsed in the current scanline, 0..=LINE_DOTS-1 (C01_15).
    dot_in_line: u32,
    /// Current scanline index = LY ($FF44), 0..=153 (note 04a "LY").
    ly: u8,
}

impl Default for Video {
    fn default() -> Self {
        Video {
            frame: [0x00; FRAMEBUFFER_SIZE],
            dot_in_line: 0,
            ly: 0,
        }
    }
}

impl Video {
    /// Post-boot state (note 08): LY = 0 at the start of line 0.
    pub fn reset(&mut self) {
        self.frame.fill(0x00);
        self.dot_in_line = 0;
        self.ly = 0;
    }

    /// Advance one dot (decision A_01: Machine::tick() = one dot). Returns true when the
    /// scanline index changes, i.e. on the last dot of a line.
    pub fn tick(&mut self) -> bool {
        self.dot_in_line = self.dot_in_line.wrapping_add(1);
        if self.dot_in_line == LINE_DOTS {
            self.dot_in_line = 0;
            // LY holds 0..=153 (note 04a "LY"); after the last line of the frame it wraps.
            let next = self.ly.wrapping_add(1);
            self.ly = if next >= LINES_PER_FRAME { 0 } else { next };
            true
        } else {
            false
        }
    }

    /// LY ($FF44): current scanline, stable during the whole line (note 04a "LY").
    pub fn ly(&self) -> u8 {
        self.ly
    }

    /// True while in VBlank: LY = 144..=153 (note 01_timing.md).
    pub fn vblank(&self) -> bool {
        self.ly >= VBLANK_FIRST_LINE
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::machine::FRAME_DOTS;

    #[test]
    fn c01_15_line_is_456_dots() {
        let mut v = Video::default();
        for _ in 0..(LINE_DOTS - 1) {
            assert!(!v.tick()); // the line has not ended yet
        }
        assert_eq!(v.ly(), 0);
        assert!(v.tick()); // the 456th dot ends line 0 (note 01_timing.md)
        assert_eq!(v.ly(), 1);
    }

    #[test]
    fn c01_15_ly_wraps_at_documented_last_line() {
        let mut v = Video::default();
        // Line 153, the documented last line (note 04a "LY"), starts after 153 lines.
        for _ in 0..(LINE_DOTS * u32::from(LINES_PER_FRAME - 1)) {
            v.tick();
        }
        assert_eq!(v.ly(), LINES_PER_FRAME - 1);
        // The last line lasts exactly 456 dots, then LY wraps to 0.
        for _ in 0..LINE_DOTS {
            v.tick();
        }
        assert_eq!(v.ly(), 0);
    }

    #[test]
    fn c01_15_frame_of_70224_dots_returns_to_line_zero() {
        let mut v = Video::default();
        for _ in 0..FRAME_DOTS {
            v.tick();
        }
        assert_eq!(v.ly(), 0); // one frame = 154 lines x 456 dots (note 01_timing.md)
        assert!(!v.vblank());
    }

    #[test]
    fn c01_15_vblank_spans_lines_144_through_153() {
        let mut v = Video::default();
        for line in 0..u32::from(LINES_PER_FRAME) {
            assert_eq!(v.ly(), (line % u32::from(LINES_PER_FRAME)) as u8);
            assert_eq!(v.vblank(), line >= u32::from(VBLANK_FIRST_LINE));
            for _ in 0..LINE_DOTS {
                v.tick();
            }
        }
    }

    #[test]
    fn c01_15_reset_restarts_ly_at_zero() {
        let mut v = Video::default();
        for _ in 0..(LINE_DOTS * 3) {
            v.tick();
        }
        assert_eq!(v.ly(), 3);
        v.reset();
        assert_eq!(v.ly(), 0);
    }
}
