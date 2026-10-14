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

/// Minimum length of mode 3 (pixels) in dots: the initial fetch of 12 dots plus one dot per
/// output pixel (note 04b "FIFO de pixels"). With SCX mod 8 == 0 and no window/sprites, mode 3
/// lasts exactly this many dots; from E03_03 on the real end condition is dynamic.
pub const MODE3_LEN: u32 = 172;

/// Number of pixels per visible line (note 04a "STAT bits").
const LINE_PIXELS: u8 = 160;

/// Dots the pixel fetcher needs before the first output pixel leaves the FIFO (E03_03 spec).
const FETCH_INITIAL_DOTS: u32 = 12;

/// Maximum number of pixels held in a pixel FIFO at once (note 04b "FIFO de pixels").
const FIFO_CAP: usize = 16;

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
/// counter, LY position and current mode (E03_01), and the per-line pixel fetcher with its
/// FIFO and line buffer (E03_03).
#[derive(Debug)]
pub struct Ppu {
    pub frame: [u8; FRAMEBUFFER_SIZE],
    /// Color indices (0..3) of the current visible line, before any palette lookup
    /// (E03_03 spec); filled by the pixel fetcher during mode 3.
    pub line_buf: [u8; LINE_PIXELS as usize],
    /// Dot within the current scanline, 0..=LINE_DOTS-1 (decision A_01).
    dot_in_line: u32,
    /// Current scanline index = LY ($FF44), 0..=153 (note 04a "LY").
    ly: u8,
    /// Current PPU mode, 0-3 (note 04a "STAT bits"); reads as 0 while the LCD is off.
    mode: u8,
    /// LCD and PPU enable = LCDC ($FF40) bit 7 (E03_09 spec): false stops the dot counter.
    lcd_on: bool,
    /// Mirror of LCDC ($FF40) as written by the CPU (E03_09 spec); bit 2 selects the sprite
    /// size for the OAM scan (E03_02).
    lcdc: u8,
    /// OAM indexes of the sprites selected for the current line by the mode 2 scan, in OAM
    /// order, at most 10 (E03_02 spec).
    line_sprites: [u8; 10],
    /// Number of valid entries in `line_sprites` (E03_02).
    line_sprite_count: u8,
    /// Set while the mode 2 OAM scan for the current line is due and not yet performed
    /// (E03_02): at power-up, on entry to mode 2 of a visible line, and after an LCD
    /// off -> on restart; cleared by `scan_oam`.
    oam_scan_pending: bool,
    /// Pixels output into `line_buf` this line, 0..=160 (E03_03 spec): mode 3 ends when it
    /// reaches 160.
    pixels_output: u8,
    /// Pixels produced into the FIFO this line by the fetcher, 0..=160 (note 04b).
    produce_count: u8,
    /// Pixel FIFO of color indices for the current line, up to 16 entries (note 04b "FIFO de pixels").
    fifo: [u8; FIFO_CAP],
    fifo_head: usize,
    fifo_count: usize,
    /// Tile column (0..31) whose row is cached in `tile_low`/`tile_high`; 0xFF forces a load.
    fetcher_col: u8,
    /// Cached low and high bit-plane bytes of the current tile row (note 04a "Tile data format").
    tile_low: u8,
    tile_high: u8,
}

impl Default for Ppu {
    fn default() -> Self {
        let mut p = Ppu {
            frame: [0x00; FRAMEBUFFER_SIZE],
            line_buf: [0x00; LINE_PIXELS as usize],
            dot_in_line: 0,
            ly: 0,
            mode: 0,
            lcd_on: true, // post-boot LCDC ($FF40) is $91: bit 7 set (note 08)
            lcdc: 0x91,   // post-boot LCDC value (E03_09 spec); the machine keeps it current
            line_sprites: [0x00; 10],
            line_sprite_count: 0,
            oam_scan_pending: true, // line 0 starts in mode 2 at power-up (E03_01): the scan is due
            pixels_output: 0,
            produce_count: 0,
            fifo: [0x00; FIFO_CAP],
            fifo_head: 0,
            fifo_count: 0,
            fetcher_col: 0xFF, // sentinel: force a tile load on the first production (E03_03)
            tile_low: 0,
            tile_high: 0,
        };
        p.mode = Self::mode_for(p.ly, p.dot_in_line, p.pixels_output); // start of line 0 is OAM scan (mode 2)
        p
    }
}

impl Ppu {
    /// Post-boot state (note 08): LY = 0 at the start of line 0, LCD enabled.
    pub fn reset(&mut self) {
        self.frame.fill(0x00);
        self.line_buf.fill(0x00);
        self.dot_in_line = 0;
        self.ly = 0;
        self.mode = Self::mode_for(self.ly, self.dot_in_line, self.pixels_output); // mode 2 at start of line 0
        self.lcd_on = true; // post-boot LCDC ($FF40) is $91: bit 7 set (note 08)
        self.lcdc = 0x91; // post-boot LCDC value (E03_09 spec); the machine keeps it current
        self.line_sprites = [0x00; 10];
        self.line_sprite_count = 0;
        self.oam_scan_pending = true; // line 0 starts in mode 2 at power-up (E03_01): the scan is due
        self.reset_pixel_state();
    }

    /// Reset the pixel fetcher state for a new visible line (E03_03): no pixels produced or
    /// output yet, empty FIFO, and a tile load forced on the first production.
    fn reset_pixel_state(&mut self) {
        self.pixels_output = 0;
        self.produce_count = 0;
        self.fifo_head = 0;
        self.fifo_count = 0;
        self.fetcher_col = 0xFF; // sentinel: force a tile load on the first production (E03_03)
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
        self.lcdc = lcdc; // keep the mirror current for the OAM scan (E03_02 spec)
        if on == self.lcd_on {
            return; // no transition: the PPU keeps running or stays stopped
        }
        self.lcd_on = on;
        if on {
            self.dot_in_line = 0;
            self.ly = 0;
            self.mode = Self::mode_for(self.ly, self.dot_in_line, self.pixels_output); // normal line (E03_09 spec)
            self.oam_scan_pending = true; // the restarted line 0 starts in mode 2: its scan is due (E03_02)
            self.reset_pixel_state(); // the restarted line 0 fetches fresh pixels (E03_03)
        } else {
            self.dot_in_line = 0;
            self.ly = 0;
            self.mode = 0; // STAT mode bits read 0 while the PPU is disabled (note 04a "STAT")
            self.oam_scan_pending = false; // the line whose scan was due no longer exists (E03_02)
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
        let prev_mode = self.mode;
        if self.dot_in_line == 0 && u32::from(self.ly) < u32::from(VBLANK_FIRST_LINE) {
            // Start of a visible line: the pixel fetcher starts fresh (E03_03).
            self.reset_pixel_state();
        }
        self.mode = Self::mode_for(self.ly, self.dot_in_line, self.pixels_output);
        if self.mode == 2 && prev_mode != 2 {
            // Entry to the OAM scan of a visible line (E03_01): its sprite selection is due.
            self.oam_scan_pending = true;
        }
        next_dot == LINE_DOTS
    }

    /// True while the mode 2 OAM scan of the current line is due and not yet performed
    /// (E03_02).
    pub(crate) fn oam_scan_due(&self) -> bool {
        self.oam_scan_pending
    }

    /// Perform the mode 2 OAM scan of the current line (E03_02): read the 40 OAM entries
    /// directly from the bus and keep, in OAM order, at most 10 sprites whose Y coordinate
    /// covers LY. Only the Y coordinate is tested: a sprite off-screen by X still counts
    /// toward the limit (E03_02 spec). The scan reads OAM directly; CPU access blocking
    /// arrives with E03_10.
    pub(crate) fn scan_oam(&mut self, oam: &[u8]) {
        let ly = u32::from(self.ly);
        let height = if self.lcdc & LCDC_OBJ_SIZE != 0 {
            16
        } else {
            8
        }; // LCDC bit 2 (E03_09 spec)
        let mut count = 0u8;
        let (entries, _rest) = oam.as_chunks::<4>(); // OAM is exactly 40 entries of 4 bytes (E03_02 spec)
        for (i, entry) in entries.iter().enumerate() {
            let y = u32::from(entry[0]);
            if ly + 16 >= y && ly + 16 < y + height {
                // E03_02 spec: (LY+16) >= Y and (LY+16) < Y+height
                self.line_sprites[count as usize] = i as u8;
                count += 1;
                if count == 10 {
                    break; // at most 10 sprites per line, in OAM order (E03_02 spec)
                }
            }
        }
        self.line_sprite_count = count;
        self.oam_scan_pending = false;
    }

    /// OAM indexes of the sprites selected for the current line by the mode 2 scan, in OAM
    /// order (E03_02). Consumed by the sprite pixel renderer from E03_04 on.
    #[allow(dead_code)] // used by the sprite renderer from E03_04 on; tests use it now
    pub(crate) fn line_sprites(&self) -> &[u8] {
        &self.line_sprites[..self.line_sprite_count as usize]
    }

    /// One dot of pixel fetching (E03_03): during mode 3 the fetcher produces one color index
    /// per dot into its FIFO and, once the initial FETCH_INITIAL_DOTS dots are done, one pixel
    /// leaves the FIFO per dot into `line_buf`. The tile map and tile data are read directly
    /// from VRAM, bypassing CPU access blocking (E03_10 spec). Windows and sprites arrive with
    /// E03_11/E03_12; OBJ penalties with E03_13.
    pub(crate) fn fetch_pixels(&mut self, vram: &[u8], scx: u8, scy: u8) {
        if !self.lcd_on || self.mode != 3 {
            return; // only the pixel phase of a visible line (E03_03 spec); LCD off forces mode 0
        }
        let dots_in_mode3 = self.dot_in_line - MODE2_DOTS; // 0 at dot 80, FETCH_INITIAL_DOTS at the first output dot
        if u32::from(self.produce_count) < u32::from(LINE_PIXELS) {
            // The fetcher produces one pixel per dot until the line is done (note 04b).
            let color = self.next_color(vram, scx, scy);
            self.fifo_push(color);
            self.produce_count += 1;
        }
        if dots_in_mode3 >= FETCH_INITIAL_DOTS && self.fifo_count > 0 {
            // One pixel leaves the FIFO per dot (E03_03 spec).
            let color = self.fifo_pop();
            self.line_buf[self.pixels_output as usize] = color;
            self.pixels_output += 1;
        }
    }

    /// Color index of the next background pixel produced by the fetcher (E03_03 spec):
    /// 0 when the BG is disabled, otherwise from the tile data read directly from VRAM.
    fn next_color(&mut self, vram: &[u8], scx: u8, scy: u8) -> u8 {
        if self.lcdc & LCDC_BG_ENABLE == 0 {
            return 0; // BG disabled: color index 0 (E03_03 spec)
        }
        let x = u32::from(self.produce_count);
        let col = ((u32::from(scx) >> 3) + (x >> 3)) & 31; // background pixel X, in tiles (E03_03 spec)
        if col != u32::from(self.fetcher_col) {
            // New tile column: read its row from VRAM directly (E03_03 spec).
            self.load_tile_row(vram, scy, col as u8);
            self.fetcher_col = col as u8;
        }
        let pixel_in_tile = (x + (u32::from(scx) & 7)) & 7; // fine scroll offset (0 in this task)
        let low_bit = (self.tile_low >> (7 - pixel_in_tile)) & 1;
        let high_bit = (self.tile_high >> (7 - pixel_in_tile)) & 1;
        (high_bit << 1) | low_bit // bit plane 0 = low, bit plane 1 = high (note 04a "Tile data format")
    }

    /// Read the tile row covering background line LY + SCY and tile column `col` from VRAM
    /// directly (E03_03 spec): one byte for the tile index in the map, then two bytes of tile
    /// data. The CPU access blocking does not apply to these reads (E03_10 spec).
    fn load_tile_row(&mut self, vram: &[u8], scy: u8, col: u8) {
        let y = (u32::from(self.ly) + u32::from(scy)) & 0xFF; // background pixel Y (E03_03 spec)
        let tile_row = y >> 3;
        let line_in_tile = y & 7;
        let map_base = if self.lcdc & LCDC_BG_MAP != 0 {
            0x9C00 - 0x8000 // LCDC bit 3: $9C00 (E03_03 spec)
        } else {
            0x9800 - 0x8000 // default map area $9800 (E03_03 spec)
        };
        let tile_index = vram[(map_base + tile_row * 32 + u32::from(col)) as usize];
        let data_base = if self.lcdc & LCDC_TILE_DATA != 0 {
            // unsigned index from $8000 (E03_03 spec)
            u32::from(tile_index)
        } else {
            // signed index from $9000: values 128-255 wrap below the base (E03_03 spec)
            0x1000u32.wrapping_add((tile_index as i8) as u32)
        };
        let row = data_base + line_in_tile * 2;
        self.tile_low = vram[row as usize]; // first byte: low bit plane (note 04a "Tile data format")
        self.tile_high = vram[row as usize + 1]; // second byte: high bit plane (note 04a)
    }

    /// Push a color index into the pixel FIFO; a full FIFO drops the pixel (note 04b).
    fn fifo_push(&mut self, color: u8) {
        if self.fifo_count == FIFO_CAP {
            return; // FIFO full (note 04b); cannot happen with one pixel per dot in this task
        }
        let tail = (self.fifo_head + self.fifo_count) % FIFO_CAP;
        self.fifo[tail] = color;
        self.fifo_count += 1;
    }

    /// Pop the oldest color index from the pixel FIFO.
    fn fifo_pop(&mut self) -> u8 {
        let color = self.fifo[self.fifo_head];
        self.fifo_head = (self.fifo_head + 1) % FIFO_CAP;
        self.fifo_count -= 1;
        color
    }

    /// PPU mode for a given scanline and dot position (E03_01): lines 144..=153 are VBlank
    /// (mode 1); visible lines give mode 2 on dots 0..79, then mode 3 until all 160 pixels of
    /// the line have been output into `line_buf`, then mode 0 for the rest of the line
    /// (note 04a "STAT bits", note 04b). With SCX mod 8 == 0 and no window/sprites, mode 3
    /// lasts exactly MODE3_LEN dots (E03_03 spec); fine scroll, windows and sprites extend it.
    fn mode_for(ly: u8, dot: u32, pixels_output: u8) -> u8 {
        if u32::from(ly) >= u32::from(VBLANK_FIRST_LINE) {
            1 // VBlank (note 01_timing.md)
        } else if dot < MODE2_DOTS {
            2 // OAM scan, dots 0..79 (note 04a "STAT bits")
        } else if pixels_output < LINE_PIXELS {
            3 // pixels: mode 3 ends when the line's 160 pixels are all output (E03_03 spec)
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

    /// True while the CPU may access VRAM ($8000-$9FFF) (E03_10 spec): every mode except 3;
    /// when the LCD is off the PPU mode reads as 0, so access is free. The PPU's own reads are
    /// never blocked: they go through the raw bus arrays, not Bus::read (decision A_01).
    pub(crate) fn vram_accessible(&self) -> bool {
        self.mode != 3 // LCD off forces mode 0, so this covers it too (E03_10 spec)
    }

    /// True while the CPU may access OAM ($FE00-$FE9F) (E03_10 spec): modes 0 and 1 only;
    /// when the LCD is off the PPU mode reads as 0, so access is free. The PPU's own reads are
    /// never blocked: they go through the raw bus arrays, not Bus::read (decision A_01).
    pub(crate) fn oam_accessible(&self) -> bool {
        self.mode < 2 // LCD off forces mode 0, so this covers it too (E03_10 spec)
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
        let vram = [0x00u8; 0x2000]; // empty VRAM: the fetcher reads it directly (E03_03)
        let mut counts = [0u32; 4];
        for dot in 0..LINE_DOTS {
            if dot > 0 {
                p.tick(); // advance to this dot within the line
            }
            p.fetch_pixels(&vram, 0, 0); // one dot of pixel fetching per dot (E03_03)
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

    /// OAM image of 40 entries (E03_02 spec): every entry gets the given Y and X coordinates.
    fn oam_with(y: u8, x: u8) -> [u8; 160] {
        let mut oam = [0x00u8; 160];
        for i in 0..40 {
            oam[i * 4] = y; // byte 0: Y (E03_02 spec)
            oam[i * 4 + 1] = x; // byte 1: X (E03_02 spec)
        }
        oam
    }

    #[test]
    fn e03_02_eleven_sprites_on_one_line_give_the_first_ten_in_oam_order() {
        let mut p = Ppu {
            ly: 4,
            ..Ppu::default()
        }; // LY + 16 = 20 falls in [Y, Y+8) for Y = 20 (E03_02 spec)
        let mut oam = [0x00u8; 160];
        for i in 0..11 {
            // eleven sprites on the line at OAM entries 0-10
            oam[i * 4] = 20; // Y = 20: shown on LY = 20 (E03_02 spec)
        }
        p.scan_oam(&oam);
        let sprites = p.line_sprites();
        assert_eq!(sprites.len(), 10); // at most 10 per line, in OAM order (E03_02 spec)
        for (i, &s) in sprites.iter().enumerate() {
            assert_eq!(s, i as u8); // the first ten entries win; entry 10 is dropped
        }
    }

    #[test]
    fn e03_02_y16_shows_on_lines_0_through_7_for_8x8() {
        let mut p = Ppu::default(); // 8x8 sprites: LCDC bit 2 clear (E03_09 spec)
        let oam = oam_with(16, 40); // one sprite at Y = 16 in every entry (E03_02 spec)
        for ly in 0..LINES_PER_FRAME {
            p.ly = ly;
            p.scan_oam(&oam);
            let shown = !p.line_sprites().is_empty();
            assert_eq!(shown, ly < 8, "Y=16 (8x8) on line {ly}"); // lines 0..7 only (E03_02 spec)
        }
    }

    #[test]
    fn e03_02_y_zero_never_shows() {
        let mut p = Ppu::default(); // 8x8 sprites (E03_09 spec)
        let oam = oam_with(0, 40); // Y = 0 masks the object on every line (E03_02 spec)
        for ly in 0..LINES_PER_FRAME {
            p.ly = ly;
            p.scan_oam(&oam);
            assert!(
                p.line_sprites().is_empty(),
                "Y=0 must never show on line {ly}"
            );
        }
    }

    #[test]
    fn e03_02_y16_shows_on_lines_0_through_15_for_8x16() {
        let mut p = Ppu {
            lcdc: 0x95,
            ..Ppu::default()
        }; // LCDC bit 2 set: 8x16 sprites (E03_02 spec)
        let oam = oam_with(16, 40); // one sprite at Y = 16 in every entry (E03_02 spec)
        for ly in 0..LINES_PER_FRAME {
            p.ly = ly;
            p.scan_oam(&oam);
            let shown = !p.line_sprites().is_empty();
            assert_eq!(shown, ly < 16, "Y=16 (8x16) on line {ly}"); // lines 0..15 only (E03_02 spec)
        }
    }

    #[test]
    fn e03_02_a_sprite_with_x_zero_counts_in_the_limit() {
        let mut p = Ppu {
            ly: 4, // LY + 16 = 20 falls in [Y, Y+8) for Y = 20 (E03_02 spec)
            ..Ppu::default()
        };
        let mut oam = [0x00u8; 160];
        oam[0] = 20; // entry 0: on the line but off-screen at X = 0 (E03_02 spec)
        for i in 1..11 {
            // entries 1-10: on the line, visible X
            oam[i * 4] = 20;
            oam[i * 4 + 1] = 80;
        }
        p.scan_oam(&oam);
        let sprites = p.line_sprites();
        assert_eq!(sprites.len(), 10); // the X=0 sprite counts toward the limit (E03_02 spec)
        for (i, &s) in sprites.iter().enumerate() {
            assert_eq!(s, i as u8); // entries 0-9 win; entry 10 is dropped
        }
    }

    #[test]
    fn e03_02_a_sprite_with_x_at_least_168_counts_in_the_limit() {
        let mut p = Ppu {
            ly: 4, // LY + 16 = 20 falls in [Y, Y+8) for Y = 20 (E03_02 spec)
            ..Ppu::default()
        };
        let mut oam = [0x00u8; 160];
        oam[0] = 20; // entry 0: on the line but off-screen at X = 170 (E03_02 spec)
        oam[1] = 170;
        for i in 1..11 {
            // entries 1-10: on the line, visible X
            oam[i * 4] = 20;
            oam[i * 4 + 1] = 80;
        }
        p.scan_oam(&oam);
        let sprites = p.line_sprites();
        assert_eq!(sprites.len(), 10); // the X=170 sprite counts toward the limit (E03_02 spec)
        for (i, &s) in sprites.iter().enumerate() {
            assert_eq!(s, i as u8); // entries 0-9 win; entry 10 is dropped
        }
    }

    /// Drive a PPU to an absolute dot position of the frame: line `ly`, dot `dot` (E03_10).
    fn ppu_at(ly: u8, dot: u32) -> Ppu {
        let mut p = Ppu::default();
        let vram = [0x00u8; 0x2000]; // zero VRAM with SCX = 0 and SCY = 0: mode 3 lasts exactly MODE3_LEN dots (E03_10 spec)
        for _ in 0..(u32::from(ly) * LINE_DOTS + dot) {
            p.tick();
            p.fetch_pixels(&vram, 0, 0); // one dot of pixel fetching per dot, as Machine::tick does (E03_10 spec)
        }
        p
    }

    #[test]
    fn e03_10_vram_accessible_in_modes_0_1_and_2() {
        assert!(ppu_at(5, 400).vram_accessible()); // mode 0: HBlank (E03_10 spec)
        assert!(ppu_at(150, 100).vram_accessible()); // mode 1: VBlank (E03_10 spec)
        assert!(ppu_at(5, 40).vram_accessible()); // mode 2: OAM scan (E03_10 spec)
    }

    #[test]
    fn e03_10_vram_blocked_in_mode_3() {
        assert!(!ppu_at(5, 200).vram_accessible()); // mode 3: pixels (E03_10 spec)
    }

    #[test]
    fn e03_10_oam_accessible_in_modes_0_and_1() {
        assert!(ppu_at(5, 400).oam_accessible()); // mode 0: HBlank (E03_10 spec)
        assert!(ppu_at(150, 100).oam_accessible()); // mode 1: VBlank (E03_10 spec)
    }

    #[test]
    fn e03_10_oam_blocked_in_modes_2_and_3() {
        assert!(!ppu_at(5, 40).oam_accessible()); // mode 2: OAM scan (E03_10 spec)
        assert!(!ppu_at(5, 200).oam_accessible()); // mode 3: pixels (E03_10 spec)
    }

    #[test]
    fn e03_10_vram_and_oam_free_when_lcd_off() {
        let mut p = Ppu::default();
        for _ in 0..(LINE_DOTS * 5 + 200) {
            p.tick(); // mid mode 3 of line 5
        }
        assert!(!p.vram_accessible()); // blocked while the LCD is on and in mode 3
        p.set_lcd_enabled(0x11); // LCDC bit 7 -> 0: LCD off (E03_09 spec)
        assert!(p.vram_accessible()); // free access when the LCD is off (E03_10 spec)
        assert!(p.oam_accessible()); // free access when the LCD is off (E03_10 spec)
    }

    /// VRAM image for pixel-fetching tests (E03_03): every map entry at $9800 points to tile 0,
    /// whose rows hold the given low/high bit-plane bytes at both the unsigned base $8000 and
    /// the signed base $9000, so it is correct regardless of LCDC bit 4.
    fn vram_with_tile(low: u8, high: u8) -> [u8; 0x2000] {
        let mut vram = [0x00u8; 0x2000];
        for i in 0..1024 {
            vram[(0x9800 - 0x8000) + i] = 0; // map at $9800 (LCDC bit 3 clear): tile index 0
        }
        for base in [0usize, 0x1000] {
            // unsigned base $8000 and signed base $9000 (E03_03 spec)
            for row in 0..16 {
                vram[base + row * 2] = low; // tile index 0, both bases (E03_03 spec)
                vram[base + row * 2 + 1] = high;
            }
        }
        vram
    }

    /// Expected color index of pixel `x` from raw low/high bit-plane bytes (note 04a "Tile data format"):
    /// the row pattern repeats every tile column, so only x mod 8 selects the bit position.
    fn expected_color(low: u8, high: u8, x: usize) -> u8 {
        let pixel_in_tile = x % 8; // one tile column is 8 pixels wide (E03_03 spec)
        let low_bit = (low >> (7 - pixel_in_tile)) & 1;
        let high_bit = (high >> (7 - pixel_in_tile)) & 1;
        (high_bit << 1) | low_bit
    }

    #[test]
    fn e03_03_mode3_last_exactly_172_dots_when_scx_mod_8_is_zero() {
        let mut p = Ppu::default(); // SCX = 0, no window or sprites (E03_03 spec)
        let vram = [0x00u8; 0x2000];
        for dot in 0..LINE_DOTS {
            if dot > 0 {
                p.tick();
            }
            p.fetch_pixels(&vram, 0, 0); // one dot of pixel fetching per dot (E03_03)
            let expected = if dot < MODE2_DOTS {
                2 // OAM scan: dots 0..79 (note 04a "STAT bits")
            } else if dot < MODE2_DOTS + MODE3_LEN {
                3 // mode 3 lasts exactly 172 dots, i.e. 80..=251 (E03_03 spec)
            } else {
                0 // HBlank: the rest of the line (note 04b)
            };
            assert_eq!(p.mode(), expected, "mode at dot {dot}");
        }
    }

    #[test]
    fn e03_03_line_buf_holds_the_fifos_pixels_in_order() {
        let mut p = Ppu::default(); // SCX = 0 (E03_03 spec)
        let vram = vram_with_tile(0x55, 0xCC); // one tile row yields all four color indexes (E03_03 spec)
        for dot in 0..LINE_DOTS {
            if dot > 0 {
                p.tick();
            }
            p.fetch_pixels(&vram, 0, 0);
        }
        assert_eq!(p.pixels_output, LINE_PIXELS); // all 160 pixels of the line are output (E03_03 spec)
        for x in 0..LINE_PIXELS as usize {
            assert_eq!(p.line_buf[x], expected_color(0x55, 0xCC, x), "pixel {x}");
        }
        // Spot checks against the raw bytes: one tile row covers all four color indexes (E03_03 spec).
        assert_eq!(p.line_buf[0], 2); // high bit only
        assert_eq!(p.line_buf[1], 3); // both bits set
        assert_eq!(p.line_buf[2], 0); // no bit set
        assert_eq!(p.line_buf[3], 1); // low bit only
        assert_eq!(p.line_buf[8], 2); // next tile column repeats the same row (E03_03 spec)
    }

    #[test]
    fn e03_03_first_pixel_leaves_the_fifo_after_twelve_dots() {
        let mut p = Ppu::default(); // SCX = 0 (E03_03 spec)
        let vram = [0x00u8; 0x2000];
        for _ in 1..=MODE2_DOTS + FETCH_INITIAL_DOTS - 1 {
            p.tick(); // dots 1..91: the fetcher fills its FIFO, nothing is output yet
            p.fetch_pixels(&vram, 0, 0);
        }
        assert_eq!(p.pixels_output, 0); // no pixel has left the FIFO yet (E03_03 spec)
        p.tick(); // dot MODE2_DOTS + FETCH_INITIAL_DOTS = 92
        p.fetch_pixels(&vram, 0, 0);
        assert_eq!(p.pixels_output, 1); // the first pixel leaves the FIFO (E03_03 spec)
    }

    #[test]
    fn e03_03_bg_disabled_outputs_color_index_zero() {
        let mut p = Ppu {
            lcdc: 0x90, // LCDC bit 0 clear: BG disabled (E03_03 spec)
            ..Ppu::default()
        };
        let vram = vram_with_tile(0xFF, 0xFF); // tile data would give index 3 if read
        for dot in 0..LINE_DOTS {
            if dot > 0 {
                p.tick();
            }
            p.fetch_pixels(&vram, 0, 0);
        }
        assert_eq!(p.pixels_output, LINE_PIXELS); // the line is still output (E03_03 spec)
        assert!(p.line_buf.iter().all(|&c| c == 0)); // color index 0 everywhere (E03_03 spec)
    }

    #[test]
    fn e03_03_signed_tile_index_wraps_below_the_base() {
        let mut p = Ppu {
            lcdc: 0x81,
            ..Ppu::default()
        }; // LCDC bit 4 clear: signed tile index from $9000 (E03_03 spec)
        let mut vram = [0x00u8; 0x2000];
        for i in 0..1024 {
            vram[(0x9800 - 0x8000) + i] = 128; // tile index 128: signed, wraps below $9000 (E03_03 spec)
        }
        let row = (0x9000 - 0x8000) - 128; // address $8F80 in VRAM indexing
        vram[row] = 0x55;
        vram[row + 1] = 0xAA;
        for dot in 0..LINE_DOTS {
            if dot > 0 {
                p.tick();
            }
            p.fetch_pixels(&vram, 0, 0);
        }
        assert_eq!(p.pixels_output, LINE_PIXELS); // the line is fully output (E03_03 spec)
        for x in 0..LINE_PIXELS as usize {
            assert_eq!(p.line_buf[x], expected_color(0x55, 0xAA, x), "pixel {x}");
        }
    }

    #[test]
    fn e03_03_fetcher_reads_vram_directly_while_cpu_is_blocked() {
        use crate::machine::{Dmg, Machine};
        let rom = vec![0x00u8; Dmg::MIN_ROM_LEN]; // minimal ROM of NOPs (note 07a)
        let mut dmg = Dmg::new(&rom).unwrap();
        for i in 0..1024 {
            dmg.bus.vram[(0x9800 - 0x8000) + i] = 0; // map at $9800: tile index 0 (E03_03 spec)
        }
        for row in 0..16 {
            dmg.bus.vram[row * 2] = 0x55; // unsigned base $8000, tile index 0 (default LCDC bit 4 set)
            dmg.bus.vram[row * 2 + 1] = 0xAA;
        }
        // SCX ($FF43) is $00 at power-up (note 08): no fine scroll. Run to mid mode 3 of line 5,
        // where the CPU's VRAM access is blocked (E03_10 spec).
        for _ in 0..(LINE_DOTS * 5 + MODE2_DOTS + MODE3_LEN / 2) {
            dmg.tick();
        }
        assert_eq!(dmg.ppu.mode(), 3); // mode 3: the CPU cannot read VRAM here (E03_10 spec)
        for _ in 0..MODE3_LEN {
            dmg.tick(); // run out of mode 3: the fetcher keeps reading VRAM directly (E03_03 spec)
        }
        assert_eq!(dmg.ppu.mode(), 0); // mode 3 ended after exactly MODE3_LEN dots (E03_03 spec)
        for x in 0..LINE_PIXELS as usize {
            assert_eq!(
                dmg.ppu.line_buf[x],
                expected_color(0x55, 0xAA, x),
                "pixel {x}"
            );
        }
    }

    #[test]
    fn e03_03_bg_tile_map_selects_area_from_lcdc_bit_3() {
        let mut p = Ppu {
            lcdc: 0x91 | LCDC_BG_MAP,
            ..Ppu::default()
        }; // bit 3 set: map at $9C00 (E03_03 spec)
        let mut vram = [0x00u8; 0x2000];
        for i in 0..1024 {
            vram[(0x9800 - 0x8000) + i] = 0; // $9800 map: tile index 0 (would give color 0 everywhere)
        }
        for i in 0..32 {
            vram[(0x9C00 - 0x8000) + i] = 1; // $9C00 map, row 0: distinct tile index 1 (E03_03 spec)
        }
        vram[1] = 0x55; // tile index 1, line 0: low bit plane at the unsigned base $8000 (default LCDC bit 4 set)
        vram[2] = 0xAA; // high bit plane
        for dot in 0..LINE_DOTS {
            if dot > 0 {
                p.tick();
            }
            p.fetch_pixels(&vram, 0, 0); // SCX = 0 (E03_03 spec)
        }
        assert_eq!(p.pixels_output, LINE_PIXELS); // the line is fully output (E03_03 spec)
        for x in 0..LINE_PIXELS as usize {
            assert_eq!(p.line_buf[x], expected_color(0x55, 0xAA, x), "pixel {x}");
            // tile index 1 from $9C00 is used (E03_03 spec)
        }
    }

    #[test]
    fn e03_03_scy_wraps_past_256_selects_the_wrapped_tile_row() {
        const LY: u8 = 143; // last visible line (E03_03 spec)
        const SCY: u8 = 120; // (LY + SCY) & 0xFF = 7 wraps past 256 (E03_03 spec)
        let mut p = Ppu::default(); // SCX = 0 (E03_03 spec)
        for _ in 0..(u32::from(LY) * LINE_DOTS) {
            p.tick(); // reach line 143, dot 0
        }
        assert_eq!(p.ly(), LY);
        let mut vram = [0x00u8; 0x2000];
        for i in 0..1024 {
            let map_row = (i / 32) as u8;
            vram[(0x9800 - 0x8000) + i] = if map_row == 0 { 0 } else { 1 }; // $9800 map: row 0 -> tile 0, other rows -> tile 1 (E03_03 spec)
        }
        vram[14] = 0xFF; // tile 0, line 7: low bit plane $FF - the wrapped row (E03_03 spec)
        for i in 0..32 {
            vram[(0x9C00 - 0x8000) + i] = 8; // map row 32, reached only if LY + SCY did not wrap past 256
        }
        vram[23] = 0xFF; // tile 8, line 7: high bit plane - the no-wrap result (E03_03 spec)
        for dot in 0..LINE_DOTS {
            if dot > 0 {
                p.tick();
            }
            p.fetch_pixels(&vram, 0, SCY); // background pixel Y = (LY + SCY) & 0xFF (E03_03 spec)
        }
        assert_eq!(p.pixels_output, LINE_PIXELS); // the line is fully output (E03_03 spec)
        for x in 0..LINE_PIXELS as usize {
            assert_eq!(p.line_buf[x], 1, "pixel {x}"); // wrapped row 7 of tile 0: low bit plane $FF only (E03_03 spec)
        }
    }

    #[test]
    fn e03_03_scx_coarse_scroll_shifts_the_background_by_whole_tiles() {
        const SCX: u8 = 16; // multiple of 8: whole-tile shift, no fine scroll (E03_03 spec)
        let mut p = Ppu::default();
        let mut vram = [0x00u8; 0x2000];
        for i in 0..1024 {
            vram[(0x9800 - 0x8000) + i] = (i % 32) as u8; // $9800 map: column c -> tile index c (E03_03 spec)
        }
        for (c, byte) in vram.iter_mut().enumerate().take(31) {
            *byte = if (c % 4) < 2 { 0xFFu8 } else { 0x00 }; // uniform row bytes per column (note 04a "Tile data format")
        }
        for dot in 0..LINE_DOTS {
            if dot > 0 {
                p.tick();
            }
            p.fetch_pixels(&vram, SCX, 0); // pixel x shows the tile at column SCX/8 + x/8 (E03_03 spec)
        }
        assert_eq!(p.pixels_output, LINE_PIXELS); // the line is fully output (E03_03 spec)
        for x in 0..LINE_PIXELS as usize {
            let col = (2 + x / 8) % 4; // tile column SCX/8 + x/8, modulo the byte pattern period of 4
            let expected = match col {
                0 => 3, // both bit planes set
                1 => 1, // low bit plane only
                2 => 0, // no bit plane set
                _ => 2, // high bit plane only
            };
            assert_eq!(p.line_buf[x], expected, "pixel {x}");
        }
    }
}
