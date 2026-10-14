//! PPU / video (E03_01): dot-exact line and frame timing, LY ($FF44), the PPU modes 0-3
//! sequence and the VBlank interrupt request. Rendering arrives with later E03 tasks;
//! nothing is drawn yet.

use crate::machine::FRAMEBUFFER_SIZE;

/// Dots per scanline: 456 (note 01_timing.md "Lignes par frame").
pub const LINE_DOTS: u32 = 456;

/// Scanlines per frame: 154; LY holds 0..=153, values 144-153 are the VBlank period
/// (note 01_timing.md "Lignes par frame", note 04a "LY").
pub const LINES_PER_FRAME: u8 = 154;

/// First scanline of the VBlank period: LY = 144..=153 (note 01_timing.md).
pub const VBLANK_FIRST_LINE: u8 = 144;

/// Dots of mode 2 (OAM scan) at the start of a visible line: dots 0..79 (note 04a "STAT bits").
pub const MODE2_DOTS: u32 = 80;

/// Length of mode 3 in dots, fixed constant for now. Replaced by the real end condition
/// (SCX fine scroll, window and OBJ penalties) in tasks E03_03, E03_11 and E03_13.
pub const MODE3_LEN: u32 = 172;

// LCDC ($FF40) bit masks (E03_09 spec, note 04a "LCDC bits"); later tasks use them.
/// Bit 7: LCD and PPU enable (0 = off).
pub const LCDC_ENABLE: u8 = 0x80;
/// Bit 6: window tile map area (0 = $9800, 1 = $9C00).
pub const LCDC_WIN_MAP: u8 = 0x40;
/// Bit 5: window enable.
pub const LCDC_WIN_ENABLE: u8 = 0x20;
/// Bit 4: BG and window tile data area (1 = $8000 unsigned, 0 = base $9000 signed).
pub const LCDC_TILE_DATA: u8 = 0x10;
/// Bit 3: BG tile map area (0 = $9800, 1 = $9C00).
pub const LCDC_BG_MAP: u8 = 0x08;
/// Bit 2: sprite size (0 = 8x8, 1 = 8x16).
pub const LCDC_OBJ_SIZE: u8 = 0x04;
/// Bit 1: sprite enable.
pub const LCDC_OBJ_ENABLE: u8 = 0x02;
/// Bit 0: BG and window enable (DMG).
pub const LCDC_BG_ENABLE: u8 = 0x01;

/// PPU state: framebuffer of 2-bit pixel indices, 160 x 144 (decision A_04), plus the dot
/// counter, LY position and current mode (E03_01).
#[derive(Debug)]
pub struct Ppu {
    pub frame: [u8; FRAMEBUFFER_SIZE],
    /// Dot within the current scanline, 0..=LINE_DOTS-1 (decision A_01).
    dot_in_line: u32,
    /// Current scanline index = LY ($FF44), 0..=153 (note 04a "LY").
    ly: u8,
    /// Current PPU mode, 0-3 (note 04a "STAT bits"); reads as 0 while the LCD is off.
    mode: u8,
    /// LCD and PPU enable = LCDC ($FF40) bit 7 (E03_09 spec): false stops the dot counter.
    lcd_on: bool,
}

impl Default for Ppu {
    fn default() -> Self {
        let mut p = Ppu {
            frame: [0x00; FRAMEBUFFER_SIZE],
            dot_in_line: 0,
            ly: 0,
            mode: 0,
            lcd_on: true, // post-boot LCDC ($FF40) is $91: bit 7 set (note 08)
        };
        p.mode = Self::mode_for(p.ly, p.dot_in_line); // start of line 0 is OAM scan (mode 2)
        p
    }
}

impl Ppu {
    /// Post-boot state (note 08): LY = 0 at the start of line 0, LCD enabled.
    pub fn reset(&mut self) {
        self.frame.fill(0x00);
        self.dot_in_line = 0;
        self.ly = 0;
        self.mode = Self::mode_for(self.ly, self.dot_in_line); // mode 2 at start of line 0
        self.lcd_on = true; // post-boot LCDC ($FF40) is $91: bit 7 set (note 08)
    }

    /// LCD and PPU enable state (LCDC ($FF40) bit 7, E03_09 spec).
    pub fn lcd_on(&self) -> bool {
        self.lcd_on
    }

    /// Set the LCD enable from a written LCDC value (E03_09 spec):
    /// - 1 -> 0: dot = 0, LY = 0, mode = 0; no VBlank or STAT request while off.
    /// - 0 -> 1: the PPU restarts at line 0, dot 0 with the normal start-of-line mode (2);
    ///   what is displayed on that first line is UNKNOWN - to confirm (note 04b).
    pub fn set_lcd_enabled(&mut self, lcdc: u8) {
        let on = lcdc & LCDC_ENABLE != 0;
        if on == self.lcd_on {
            return; // no transition: the PPU keeps running or stays stopped
        }
        self.lcd_on = on;
        if on {
            self.dot_in_line = 0;
            self.ly = 0;
            self.mode = Self::mode_for(self.ly, self.dot_in_line); // normal line (E03_09 spec)
        } else {
            self.dot_in_line = 0;
            self.ly = 0;
            self.mode = 0; // STAT mode bits read 0 while the PPU is disabled (note 04a "STAT")
        }
    }

    /// Advance one dot (decision A_01: Machine::tick() = one dot). Returns true when the
    /// scanline index changes, i.e. on the last dot of a line. The mode is recomputed from
    /// the new (LY, dot) position after every tick (E03_01). While the LCD is off the PPU
    /// does not advance at all (E03_09 spec).
    pub fn tick(&mut self) -> bool {
        if !self.lcd_on {
            return false; // LCD off: the dot counter, LY and mode are frozen (E03_09 spec)
        }
        let next_dot = self.dot_in_line.wrapping_add(1);
        if next_dot == LINE_DOTS {
            // End of line: wrap to dot 0 and advance LY; after the last line it wraps (note 04a "LY").
            self.dot_in_line = 0;
            let next_ly = u32::from(self.ly).wrapping_add(1);
            self.ly = if next_ly >= u32::from(LINES_PER_FRAME) {
                0
            } else {
                next_ly as u8
            };
        } else {
            self.dot_in_line = next_dot;
        }
        self.mode = Self::mode_for(self.ly, self.dot_in_line);
        next_dot == LINE_DOTS
    }

    /// PPU mode for a given scanline and dot position (E03_01): lines 144..=153 are VBlank
    /// (mode 1); visible lines give mode 2 on dots 0..79, mode 3 on the next MODE3_LEN dots,
    /// then mode 0 for the rest of the line (note 04a "STAT bits", note 04b).
    fn mode_for(ly: u8, dot: u32) -> u8 {
        if u32::from(ly) >= u32::from(VBLANK_FIRST_LINE) {
            1 // VBlank (note 01_timing.md)
        } else if dot < MODE2_DOTS {
            2 // OAM scan, dots 0..79 (note 04a "STAT bits")
        } else if dot < MODE2_DOTS + MODE3_LEN {
            3 // pixels, exactly MODE3_LEN dots (E03_01)
        } else {
            0 // HBlank, the rest of the line (note 04b)
        }
    }

    /// LY ($FF44): current scanline, stable during the whole line (note 04a "LY").
    pub fn ly(&self) -> u8 {
        self.ly
    }

    /// Current PPU mode, 0-3 (E03_01).
    pub fn mode(&self) -> u8 {
        self.mode
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
        let mut v = Ppu::default();
        for _ in 0..(LINE_DOTS - 1) {
            assert!(!v.tick()); // the line has not ended yet
        }
        assert_eq!(v.ly(), 0);
        assert!(v.tick()); // the 456th dot ends line 0 (note 01_timing.md)
        assert_eq!(v.ly(), 1);
    }

    #[test]
    fn c01_15_ly_wraps_at_documented_last_line() {
        let mut v = Ppu::default();
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
        let mut v = Ppu::default();
        for _ in 0..FRAME_DOTS {
            v.tick();
        }
        assert_eq!(v.ly(), 0); // one frame = 154 lines x 456 dots (note 01_timing.md)
        assert!(!v.vblank());
    }

    #[test]
    fn c01_15_vblank_spans_lines_144_through_153() {
        let mut v = Ppu::default();
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
        let mut v = Ppu::default();
        for _ in 0..(LINE_DOTS * 3) {
            v.tick();
        }
        assert_eq!(v.ly(), 3);
        v.reset();
        assert_eq!(v.ly(), 0);
    }

    #[test]
    fn e03_01_line_is_exactly_456_dots() {
        let mut p = Ppu::default();
        for _ in 0..(LINE_DOTS - 1) {
            assert!(!p.tick()); // the line has not ended yet (note 01_timing.md)
        }
        assert_eq!(p.ly(), 0);
        assert!(p.tick()); // the 456th dot ends the line and advances LY
        assert_eq!(p.ly(), 1);
    }

    #[test]
    fn e03_01_ly_wraps_from_153_to_0_after_one_frame() {
        let mut p = Ppu::default();
        // Advance to the start of line 153, the last line (note 04a "LY").
        for _ in 0..(FRAME_DOTS - LINE_DOTS) {
            p.tick();
        }
        assert_eq!(p.ly(), LINES_PER_FRAME - 1); // LY = 153 at the start of the last line
                                                 // The last line lasts exactly 456 dots; its final dot wraps LY from 153 to 0, so after
                                                 // exactly FRAME_DOTS ticks (one frame) LY is back at 0.
        for _ in 0..LINE_DOTS {
            p.tick();
        }
        assert_eq!(p.ly(), 0);
    }

    #[test]
    fn e03_01_mode_sequence_over_one_visible_line() {
        let mut p = Ppu::default(); // start of line 0, a visible line (note 04a "LY")
        let mut counts = [0u32; 4];
        for dot in 0..LINE_DOTS {
            if dot > 0 {
                p.tick(); // advance to this dot within the line
            }
            let expected = if dot < MODE2_DOTS {
                2 // OAM scan: dots 0..79 (note 04a "STAT bits")
            } else if dot < MODE2_DOTS + MODE3_LEN {
                3 // pixels: exactly MODE3_LEN dots, i.e. 80..=251 (E03_01)
            } else {
                0 // HBlank: the rest of the line, i.e. 252..=455 (note 04b)
            };
            assert_eq!(p.mode(), expected, "mode at dot {dot}");
            counts[p.mode() as usize] += 1;
        }
        // mode 3 lasts exactly MODE3_LEN dots and mode 2 exactly MODE2_DOTS dots.
        assert_eq!(counts[2], MODE2_DOTS); // 80 dots of OAM scan (note 04a "STAT bits")
        assert_eq!(counts[3], MODE3_LEN); // exactly 172 dots of pixels (E03_01)
        assert_eq!(counts[0], LINE_DOTS - MODE2_DOTS - MODE3_LEN); // the rest is HBlank (note 04b)
        assert_eq!(counts[1], 0); // no VBlank on a visible line
    }

    #[test]
    fn e03_01_mode_1_on_every_dot_of_vblank_lines() {
        let mut p = Ppu::default();
        // Advance to the start of line 144, the first VBlank line (note 01_timing.md).
        for _ in 0..(LINE_DOTS * u32::from(VBLANK_FIRST_LINE)) {
            p.tick();
        }
        assert_eq!(p.ly(), VBLANK_FIRST_LINE); // LY = 144
                                               // Every dot of lines 144..=153 is mode 1 (VBlank).
        for line in VBLANK_FIRST_LINE..LINES_PER_FRAME {
            for _ in 0..LINE_DOTS {
                assert_eq!(p.ly(), line);
                assert_eq!(p.mode(), 1, "mode at LY={line}"); // mode 1 throughout VBlank (note 04a)
                p.tick();
            }
        }
    }

    #[test]
    fn e03_09_lcd_off_resets_ly_dot_and_mode() {
        let mut p = Ppu::default();
        // Advance to line 5, dot 100 (inside mode 3 of a visible line).
        for _ in 0..(LINE_DOTS * 5 + 100) {
            p.tick();
        }
        assert_eq!(p.ly(), 5);
        assert!(p.lcd_on()); // post-boot LCDC is $91: the LCD starts on (note 08)
                             // LCDC bit 7 going 1 -> 0: dot = 0, LY = 0, mode = 0 (E03_09 spec).
        p.set_lcd_enabled(0x11);
        assert!(!p.lcd_on());
        assert_eq!(p.ly(), 0); // LY reset to 0
                               // The STAT mode bits read 0 while the PPU is disabled (note 04a "STAT").
        assert_eq!(p.mode(), 0);
    }

    #[test]
    fn e03_09_lcd_off_freezes_the_dot_counter() {
        let mut p = Ppu::default();
        for _ in 0..(LINE_DOTS * 5 + 100) {
            p.tick();
        }
        p.set_lcd_enabled(0x11); // LCD off: the dot counter, LY and mode are frozen (E03_09 spec)
                                 // A whole frame of ticks changes nothing while the LCD is off.
        for _ in 0..FRAME_DOTS {
            assert!(!p.tick()); // no line change: no VBlank request can be raised either
        }
        assert_eq!(p.ly(), 0);
        assert_eq!(p.mode(), 0);
    }

    #[test]
    fn e03_09_lcd_on_restarts_at_line_zero_dot_zero() {
        let mut p = Ppu::default();
        for _ in 0..(LINE_DOTS * 5 + 100) {
            p.tick();
        }
        p.set_lcd_enabled(0x11); // LCD off first (E03_09 spec)
                                 // LCDC bit 7 going 0 -> 1: the PPU restarts at line 0, dot 0 with the
                                 // normal start-of-line mode; what is displayed on that first line is
                                 // UNKNOWN - to confirm (note 04b).
        p.set_lcd_enabled(0x91);
        assert!(p.lcd_on());
        assert_eq!(p.ly(), 0); // restarted at line 0
                               // The normal start-of-line mode is OAM scan, mode 2 (E03_09 spec).
        assert_eq!(p.mode(), 2);
    }

    #[test]
    fn e03_09_lcd_on_resumes_normal_line_progression() {
        let mut p = Ppu::default();
        for _ in 0..(LINE_DOTS * 5 + 100) {
            p.tick();
        }
        p.set_lcd_enabled(0x11); // LCD off (E03_09 spec)
                                 // After re-enabling, the dot counter advances normally from line 0.
        p.set_lcd_enabled(0x91);
        for _ in 0..LINE_DOTS {
            p.tick(); // one full line: LY wraps from 0 to 1
        }
        assert_eq!(p.ly(), 1);
    }

    #[test]
    fn e03_09_lcdc_masks_match_the_spec_bits() {
        // The eight LCDC bit masks (E03_09 spec, note 04a "LCDC bits").
        assert_eq!(LCDC_ENABLE, 0x80);
        assert_eq!(LCDC_WIN_MAP, 0x40);
        assert_eq!(LCDC_WIN_ENABLE, 0x20);
        assert_eq!(LCDC_TILE_DATA, 0x10);
        assert_eq!(LCDC_BG_MAP, 0x08);
        assert_eq!(LCDC_OBJ_SIZE, 0x04);
        assert_eq!(LCDC_OBJ_ENABLE, 0x02);
        assert_eq!(LCDC_BG_ENABLE, 0x01);
    }
}
