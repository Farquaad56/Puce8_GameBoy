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

/// DMG output colors (E03_05 spec): u32 as 0xAARRGGBB with AA = 0xFF, neutral greys.
pub const PALETTE: [u32; 4] = [0xFFFF_FFFF, 0xFFAA_AAAA, 0xFF55_5555, 0xFF00_0000];

/// Output color of a color index through a palette byte (E03_05 spec): shade = (palette >> (index * 2)) & 3.
pub fn shade_color(palette: u8, index: u8) -> u32 {
    PALETTE[usize::from((palette >> ((index & 3) * 2)) & 3)]
}

/// One output pixel of a visible line (E03_04 spec): color index 0..3, whether the pixel comes
/// from a sprite and which object palette it uses (bit 4 of the OAM attributes: false = OBP0,
/// true = OBP1). Background and window pixels have `obj` clear.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PixelOut {
    pub color: u8,
    pub obj: bool,
    pub pal1: bool,
}

/// A background or window pixel with no sprite on top (E03_04 spec).
const PIXEL_BG_ZERO: PixelOut = PixelOut {
    color: 0,
    obj: false,
    pal1: false,
};

/// PPU state: framebuffer of 0xAARRGGBB pixels (E03_05), plus the dot
/// counter, LY position and current mode (E03_01), the per-line pixel fetcher with its FIFO
/// and line buffer (E03_03) and the sprite merge of the selected OAM entries into that line
/// buffer (E03_04).
#[derive(Debug)]
pub struct Ppu {
    pub frame: [u32; FRAMEBUFFER_SIZE],
    /// Output pixels of the current visible line: color index 0..3 plus whether the pixel comes
    /// from a sprite and which object palette (E03_04 spec); filled by the pixel fetcher, the
    /// window fetcher and the sprite renderer during mode 3.
    pub line_buf: [PixelOut; LINE_PIXELS as usize],
    /// Dot within the current scanline, 0..=LINE_DOTS-1 (decision A_01).
    dot_in_line: u32,
    /// Current scanline index = LY ($FF44), 0..=153 (note 04a "LY").
    ly: u8,
    /// Current PPU mode, 0-3 (note 04a "STAT bits"); reads as 0 while the LCD is off.
    mode: u8,
    /// true on line 0 right after the LCD was switched on: STAT reads mode 0 instead of 2 (mooneye stat_lyc_onoff, FIX_08)
    first_line: bool,
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
    /// Pixels produced this line by the fetcher, up to 160 + SCX mod 8 (note 04b).
    produce_count: u8,
    /// Fine scroll offset, 0..7: the low 3 bits of SCX latched at the first dot of mode 3
    /// (dot 80) of each visible line; a mid-line write changes only the high bits, which keep
    /// being re-read per tile fetch (note 04a "SCY / SCX").
    fine_scroll: u8,
    /// Pixel FIFO of color indices for the current line, up to 16 entries (note 04b "FIFO de pixels").
    fifo: [u8; FIFO_CAP],
    fifo_head: usize,
    fifo_count: usize,
    /// Tile column (0..31) whose row is cached in `tile_low`/`tile_high`; 0xFF forces a load.
    fetcher_col: u8,
    /// Cached low and high bit-plane bytes of the current tile row (note 04a "Tile data format").
    tile_low: u8,
    tile_high: u8,
    /// Window internal line counter (E03_12 spec): advances only on lines where a window pixel is
    /// produced; reset to 0 at the start of each frame and on an LCD off -> on restart.
    win_line: u8,
    /// Set while a visible window pixel has been produced on the current line (E03_12 spec); cleared
    /// at the start of each visible line; when set at end of line it advances `win_line`.
    win_drawn: bool,
    /// Set once the sprite pixels of the current line have been merged into `line_buf` (E03_04
    /// spec); cleared at the start of each visible line and on an LCD off -> on restart.
    sprites_rendered: bool,
    /// Per-pixel marker for the sprite merge (E03_04 spec): set once a pixel has been resolved by
    /// the best winner among the selected sprites; cleared at the start of each visible line.
    line_resolved: [bool; LINE_PIXELS as usize],
    /// Dots of mode 3 penalty incurred this line by sprite fetches (E03_13 spec): the fetcher is
    /// paused for that many dots, so mode 3 lasts longer and mode 0 shorter. Computed once per
    /// visible line from the selected sprites; cleared at the start of each visible line.
    obj_penalty: u32,
    /// Set once the finished line has been written into the framebuffer (E03_05 spec); cleared
    /// at the start of each visible line so every line is committed exactly once.
    line_committed: bool,
}

impl Default for Ppu {
    fn default() -> Self {
        let mut p = Ppu {
            frame: [PALETTE[0]; FRAMEBUFFER_SIZE],
            line_buf: [PIXEL_BG_ZERO; LINE_PIXELS as usize], // background pixels, color index 0 (E03_04 spec)
            dot_in_line: 0,
            ly: 0,
            mode: 0,
            first_line: false, // not on the first line after an LCD off -> on restart (FIX_08)
            lcd_on: true,      // post-boot LCDC ($FF40) is $91: bit 7 set (note 08)
            lcdc: 0x91,        // post-boot LCDC value (E03_09 spec); the machine keeps it current
            line_sprites: [0x00; 10],
            line_sprite_count: 0,
            oam_scan_pending: true, // line 0 starts in mode 2 at power-up (E03_01): the scan is due
            pixels_output: 0,
            produce_count: 0,
            fine_scroll: 0, // SCX is $00 at power-up (note 08): no fine scroll
            fifo: [0x00; FIFO_CAP],
            fifo_head: 0,
            fifo_count: 0,
            fetcher_col: 0xFF, // sentinel: force a tile load on the first production (E03_03)
            tile_low: 0,
            tile_high: 0,
            win_line: 0,      // window internal line counter starts at 0 (E03_12 spec)
            win_drawn: false, // no window pixel produced on the current line yet (E03_12 spec)
            sprites_rendered: false, // the sprite merge is due on line 0 (E03_04 spec)
            line_resolved: [false; LINE_PIXELS as usize], // no pixel resolved by a sprite yet (E03_04 spec)
            obj_penalty: 0, // no sprite penalty incurred before the first OAM scan (E03_13 spec)
            line_committed: false, // no finished line written into the framebuffer yet (E03_05 spec)
        };
        p.mode = Self::mode_for(p.ly, p.dot_in_line, p.pixels_output); // start of line 0 is OAM scan (E03_01)
        p
    }
}

/// Mode 3 extension caused by the sprites of one line (E03_13, note 04b "OBJ penalty").
fn line_obj_penalty(xs: &[u8], scx: u8) -> u32 {
    let n = xs.len().min(10);
    let mut sorted = [0u8; 10];
    sorted[..n].copy_from_slice(&xs[..n]);
    sorted[..n].sort_unstable();
    let mut seen = [u16::MAX; 10];
    let mut total = 0u32;
    for (i, &x) in sorted[..n].iter().enumerate() {
        if x >= 168 {
            continue;
        }
        if x == 0 {
            total += 11;
            continue;
        }
        let sum = u16::from(x) + u16::from(scx);
        let tile = sum / 8;
        let mut cost = 6u32;
        if !seen[..i].contains(&tile) {
            cost += 5u32.saturating_sub(u32::from(sum % 8));
            seen[i] = tile;
        }
        total += cost;
    }
    total
}

impl Ppu {
    /// Post-boot state (note 08): LY = 0 at the start of line 0, LCD enabled.
    pub fn reset(&mut self) {
        self.frame.fill(PALETTE[0]);
        self.line_buf.fill(PIXEL_BG_ZERO); // background pixels, color index 0 (E03_04 spec)
        self.dot_in_line = 0;
        self.ly = 0;
        self.first_line = false; // post-boot is not the first line after an LCD off -> on restart (FIX_08)
        self.mode = Self::mode_for(self.ly, self.dot_in_line, self.pixels_output); // mode 2 at start of line 0
        self.lcd_on = true; // post-boot LCDC ($FF40) is $91: bit 7 set (note 08)
        self.lcdc = 0x91; // post-boot LCDC value (E03_09 spec); the machine keeps it current
        self.line_sprites = [0x00; 10];
        self.line_sprite_count = 0;
        self.oam_scan_pending = true; // line 0 starts in mode 2 at power-up (E03_01): the scan is due
        self.reset_pixel_state();
    }

    /// Reset the pixel fetcher state for a new visible line (E03_03): no pixels produced or
    /// output yet, empty FIFO, fine scroll cleared until it is latched at dot 80 (E03_11), and
    /// a tile load forced on the first production; the sprite merge of this line is due again
    /// with every pixel unresolved (E03_04 spec).
    fn reset_pixel_state(&mut self) {
        self.pixels_output = 0;
        self.produce_count = 0;
        self.fine_scroll = 0; // re-latched from SCX at the first dot of mode 3 (E03_11 spec)
        self.fifo_head = 0;
        self.fifo_count = 0;
        self.fetcher_col = 0xFF; // sentinel: force a tile load on the first production (E03_03)
        self.win_drawn = false; // no window pixel produced on this line yet (E03_12 spec)
        self.sprites_rendered = false; // the sprite merge is due again on this line (E03_04 spec)
        self.line_resolved.fill(false); // no pixel resolved by a sprite yet on this line (E03_04 spec)
        self.obj_penalty = 0; // the sprite penalty of this line is recomputed at its OAM scan (E03_13 spec)
        self.line_committed = false; // the finished line of this line has not been written yet (E03_05 spec)
    }

    /// Framebuffer of 0xAARRGGBB pixels, 160 x 144 (E03_05 spec).
    pub fn framebuffer(&self) -> &[u32] {
        &self.frame
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
            self.first_line = true; // STAT reads mode 0 on this first line (mooneye stat_lyc_onoff, FIX_08)
            self.oam_scan_pending = true; // the restarted line 0 starts in mode 2: its scan is due (E03_02)
            self.reset_pixel_state(); // the restarted line 0 fetches fresh pixels (E03_03)
            self.win_line = 0; // window internal counter restarts with the frame (E03_12 spec)
        } else {
            self.dot_in_line = 0;
            self.ly = 0;
            self.mode = 0; // STAT mode bits read 0 while the PPU is disabled (note 04a "STAT")
            self.first_line = false; // no first-line state while the PPU is disabled (FIX_08)
            self.oam_scan_pending = false; // the line whose scan was due no longer exists (E03_02)
            self.frame.fill(PALETTE[0]);
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
            self.first_line = false; // the first line after an LCD on is over (FIX_08)
            let visible_line_ended = u32::from(self.ly) < u32::from(VBLANK_FIRST_LINE);
            let next_ly = u32::from(self.ly).wrapping_add(1);
            self.ly = if next_ly >= u32::from(LINES_PER_FRAME) {
                0
            } else {
                next_ly as u8
            };
            if self.ly == 0 {
                self.win_line = 0; // new frame: the window internal counter restarts (E03_12 spec)
            } else if visible_line_ended && self.win_drawn {
                self.win_line = self.win_line.wrapping_add(1); // advances only where a window pixel was produced (E03_12 spec)
            }
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

    /// Mode 3 extension of the current line from its sprites (E03_13 spec); call right after scan_oam.
    pub(crate) fn set_obj_penalty(&mut self, oam: &[u8], scx: u8) {
        let mut xs = [0u8; 10];
        for (i, &idx) in self.line_sprites().iter().enumerate() {
            xs[i] = oam[usize::from(idx) * 4 + 1];
        }
        self.obj_penalty = line_obj_penalty(&xs[..usize::from(self.line_sprite_count)], scx);
    }

    /// OAM indexes of the sprites selected for the current line by the mode 2 scan, in OAM
    /// order (E03_02); consumed by `render_sprites` (E03_04).
    pub(crate) fn line_sprites(&self) -> &[u8] {
        &self.line_sprites[..self.line_sprite_count as usize]
    }

    /// Write the finished line into framebuffer row LY (E03_05 spec): BGP for background pixels,
    /// OBP0 or OBP1 for sprite pixels. Called every dot; acts once per visible line, when its
    /// 160 pixels are all output (end of mode 3).
    pub(crate) fn commit_line(&mut self, bgp: u8, obp0: u8, obp1: u8) {
        if !self.lcd_on
            || u32::from(self.ly) >= u32::from(VBLANK_FIRST_LINE)
            || self.line_committed
            || u32::from(self.pixels_output) < u32::from(LINE_PIXELS)
        {
            return;
        }
        let row = usize::from(self.ly) * usize::from(LINE_PIXELS);
        for (x, px) in self.line_buf.iter().enumerate() {
            let palette = if !px.obj {
                bgp
            } else if px.pal1 {
                obp1
            } else {
                obp0
            };
            self.frame[row + x] = shade_color(palette, px.color);
        }
        self.line_committed = true;
    }

    /// One dot of pixel fetching (E03_03): during mode 3 the fetcher produces one color index
    /// per dot into its FIFO and, once the initial FETCH_INITIAL_DOTS dots are done, one pixel
    /// leaves the FIFO per dot into `line_buf`. The low 3 bits of SCX are latched at the first
    /// dot of mode 3; the first SCX mod 8 produced pixels are discarded from the leftmost tile
    /// and delay the first output by that many dots, so mode 3 lasts 172 + SCX mod 8 (E03_11
    /// spec, note 04a "SCY / SCX"). The fetcher is also paused for `obj_penalty` dots this line:
    /// each selected sprite costs a penalty of 6 to 11 dots from its tile fetch, so mode 3 lasts
    /// longer and mode 0 shorter by the same amount (E03_13 spec, note 04b "Duree variable du
    /// mode 3"). The tile map and tile data are read directly from VRAM, bypassing CPU access
    /// blocking (E03_10 spec). Windows arrive with E03_12.
    pub(crate) fn fetch_pixels(&mut self, vram: &[u8], scx: u8, scy: u8) {
        if !self.lcd_on || self.mode != 3 {
            return; // only the pixel phase of a visible line (E03_03 spec); LCD off forces mode 0
        }
        let dots_in_mode3 = self.dot_in_line - MODE2_DOTS; // 0 at dot 80, FETCH_INITIAL_DOTS at the first output dot
        if dots_in_mode3 == 0 {
            // The low 3 bits of SCX are read only at the beginning of the scanline (note 04a "SCY / SCX");
            // a mid-line write changes only the high bits, which keep being re-read per tile fetch.
            self.fine_scroll = scx & 7;
        }
        if u32::from(self.produce_count) < u32::from(LINE_PIXELS) + u32::from(self.fine_scroll)
            && self.fifo_count < FIFO_CAP
        {
            // The fetcher produces one pixel per dot until the line is done (note 04b); the first
            // SCX mod 8 pixels are discarded from the leftmost tile (E03_11 spec).
            let color = self.next_color(vram, scx, scy);
            if u32::from(self.produce_count) >= u32::from(self.fine_scroll) {
                // Discarded pixels are not pushed into the FIFO (E03_11 spec).
                self.fifo_push(color);
            }
            self.produce_count += 1;
        }
        if dots_in_mode3 >= FETCH_INITIAL_DOTS + u32::from(self.fine_scroll) + self.obj_penalty
            && self.fifo_count > 0
        {
            // One pixel leaves the FIFO per dot (E03_03 spec); the fine scroll pause delays the first
            // output by SCX mod 8 dots, so mode 3 lasts 172 + SCX mod 8 (E03_11 spec), and the sprite
            // fetch penalty of this line delays it further by `obj_penalty` dots (E03_13 spec).
            let color = self.fifo_pop();
            // Background pixel: no sprite on top yet (E03_04 spec); the sprite merge may replace it.
            self.line_buf[self.pixels_output as usize] = PixelOut {
                color,
                obj: false,
                pal1: false,
            };
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
        let pixel_in_tile = x & 7; // position within the tile: the fine scroll is handled by discarding produced pixels (E03_11 spec)
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
            // unsigned index from $8000, one tile is 16 bytes (E03_04 spec)
            u32::from(tile_index) * 16
        } else {
            // signed index from $9000: values 128-255 wrap below the base (E03_04 spec)
            (0x1000i32 + i32::from(tile_index as i8) * 16) as u32
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

    /// Dot position inside the current line (0..455) (FIX_07).
    pub(crate) fn line_dot(&self) -> u32 {
        self.dot_in_line
    }

    /// Current PPU mode, 0-3 (E03_01).
    pub fn mode(&self) -> u8 {
        self.mode
    }

    /// Mode bits as read through STAT: the first line after the LCD is switched on has no OAM scan phase and reads mode 0 (FIX_08; exact length of that phase: see lcdon_timing, UNKNOWN - to confirm).
    pub fn stat_mode(&self) -> u8 {
        if self.first_line && self.mode == 2 {
            0
        } else {
            self.mode
        }
    }

    /// True while in VBlank: LY = 144..=153 (note 01_timing.md).
    pub fn vblank(&self) -> bool {
        self.ly >= VBLANK_FIRST_LINE
    }

    /// Window line condition (E03_12 spec): window enabled in LCDC bits 5 and 0 (DMG), and LY >= WY.
    fn window_line_ok(&self, wy: u8) -> bool {
        self.lcdc & LCDC_WIN_ENABLE != 0 && self.lcdc & LCDC_BG_ENABLE != 0 && self.ly >= wy
    }

    /// Color index of window pixel x_w (E03_12 spec): from the window tile map row `win_line`
    /// with no SCX/SCY offset; tile data addressing and 2bpp decoding as for the background.
    fn window_color(&self, vram: &[u8], lcdc: u8, x_w: u32) -> u8 {
        let map_base = if lcdc & LCDC_WIN_MAP != 0 {
            0x9C00 - 0x8000 // window tile map area $9C00 (E03_12 spec, note 04a "Selection des tile maps")
        } else {
            0x9800 - 0x8000 // default window tile map area $9800 (E03_12 spec)
        };
        let row = u32::from(self.win_line);
        let tile_index = vram[(map_base + ((row >> 3) * 32 + ((x_w / 8) & 31))) as usize];
        let data_base = if lcdc & LCDC_TILE_DATA != 0 {
            u32::from(tile_index) * 16 // unsigned index from $8000, one tile is 16 bytes (E03_12 spec)
        } else {
            (0x1000i32 + i32::from(tile_index as i8) * 16) as u32 // signed index from $9000 (E03_12 spec)
        };
        let bit = (7 - (x_w % 8)) as usize; // pixel position within the tile, MSB first (note 04a "Tile data format")
        let row_off = (data_base + (row % 8) * 2) as usize;
        ((vram[row_off + 1] >> bit) & 1) << 1 | (vram[row_off] >> bit) & 1
    }

    /// One dot of window fetching (E03_12 spec): when the window is displayed on this line
    /// (`window_line_ok`) and the pixel position has reached WX - 7, every output pixel from
    /// that position on comes from the window tile data at row `win_line`. A visible window
    /// pixel sets `win_drawn` so the internal line counter advances at the end of the line.
    pub(crate) fn fetch_window(&mut self, vram: &[u8], wx: u8, wy: u8) {
        if !self.lcd_on || self.mode != 3 {
            return; // only the pixel phase of a visible line (E03_12 spec); LCD off forces mode 0
        }
        if !self.window_line_ok(wy) {
            return; // window not displayed on this line: disabled in LCDC or LY < WY (E03_12 spec)
        }
        let pos = u32::from(self.pixels_output); // pixels output so far on this line
        let start = u32::from(wx).saturating_sub(7); // the window starts when the pixel position reaches WX - 7 (E03_12 spec)
        if pos > start {
            let x_w = pos - 1 - start; // window pixel X, from the left edge of the window (E03_12 spec)
                                       // Window pixel: no sprite on top yet (E03_04 spec); the sprite merge may replace it.
            self.line_buf[(pos - 1) as usize] = PixelOut {
                color: self.window_color(vram, self.lcdc, x_w),
                obj: false,
                pal1: false,
            };
            self.win_drawn = true; // a visible window pixel was produced on this line (E03_12 spec)
        }
    }

    /// Merge the sprite pixels of the current line into `line_buf` (E03_04 spec): called once per dot
    /// like the other fetchers, but the merge itself runs only once per visible line. The sprite with
    /// the lowest X wins an overlap; on equal X the lowest OAM index wins (E03_04 spec), so sprites
    /// are processed from best to worst winner and each pixel is resolved at most once: a losing or
    /// hidden sprite never overwrites what already won its pixel. Sprite tile data is read directly
    /// from VRAM, always with unsigned $8000 addressing (note 04a "les objets utilisent toujours
    /// l'adressage $8000"); CPU access blocking does not apply (E03_10 spec). Mode 3 penalties arrive
    /// with E03_13.
    pub(crate) fn render_sprites(&mut self, oam: &[u8], vram: &[u8]) {
        if !self.lcd_on
            || u32::from(self.ly) >= u32::from(VBLANK_FIRST_LINE)
            || self.sprites_rendered
        {
            return;
        }
        if u32::from(self.pixels_output) < u32::from(LINE_PIXELS) {
            return;
        }
        self.sprites_rendered = true;
        if self.lcdc & LCDC_OBJ_ENABLE == 0 {
            return; // sprites disabled in LCDC bit 1 (E03_04 spec)
        }
        let height = if self.lcdc & LCDC_OBJ_SIZE != 0 {
            16u8
        } else {
            8
        }; // LCDC bit 2 (E03_09 spec)
        let count = self.line_sprites().len();
        // `order` holds OAM indexes in OAM order, sorted by X ascending (stable): the lowest X wins an
        // overlap and on equal X the lowest OAM index wins (E03_04 spec), so process from best to worst winner.
        let mut order = [0u8; 10];
        for (i, slot) in order.iter_mut().enumerate().take(count) {
            *slot = self.line_sprites[i];
        }
        for i in 1..count {
            let mut j = i;
            while j > 0 && self.sprite_x(oam, order[j - 1]) > self.sprite_x(oam, order[j]) {
                order.swap(j - 1, j);
                j -= 1;
            }
        }
        for &pos in order.iter().take(count) {
            self.render_sprite(oam, vram, usize::from(pos), height);
        }
    }

    /// Sprite X coordinate of an OAM entry (E03_04 spec: the sprite covers screen pixels X-8..X-1).
    fn sprite_x(&self, oam: &[u8], idx: u8) -> u8 {
        oam[usize::from(idx) * 4 + 1]
    }

    /// Merge one selected sprite's non-transparent pixels into `line_buf` (E03_04 spec). A pixel is
    /// resolved at most once per line: the first sprite to reach it in best-to-worst order wins, and
    /// a sprite hidden behind background colors 1..3 by its BG priority bit leaves the background.
    fn render_sprite(&mut self, oam: &[u8], vram: &[u8], idx: usize, height: u8) {
        let base = idx * 4;
        let left = i32::from(oam[base + 1]) - 8; // leftmost screen column (E03_04 spec: covers X-8..X-1)
        if left + 7 < 0 || left >= i32::from(LINE_PIXELS) {
            return; // fully off-screen by X (note 04a "OAM": X=0 or X>=168 is off-screen)
        }
        let row = u32::from(self.ly)
            .wrapping_add(16)
            .wrapping_sub(u32::from(oam[base])); // sprite row on this line (E03_02 selection: 0 <= row < height)
        if row >= u32::from(height) {
            return; // the OAM scan only selects sprites covering LY (E03_02 spec); guard a hand-set list
        }
        let tile_index = oam[base + 2];
        let attrs = oam[base + 3];
        let pal1 = attrs & 0x10 != 0; // bit 4: OBP1 (E03_04 spec)
        let flip_y = attrs & 0x40 != 0; // bit 6 (E03_04 spec)
        let flip_x = attrs & 0x20 != 0; // bit 5 (E03_04 spec)
        let bg_prio = attrs & 0x80 != 0; // bit 7: BG colors 1..3 are drawn over the sprite (E03_04 spec)
        let sprite_row = if flip_y {
            u32::from(height - 1).wrapping_sub(row)
        } else {
            row
        }; // Y flip reverses rows, and for 8x16 swaps the two halves (E03_04 spec)
        let tile_no = if height == 16 {
            // top half is tile (index & 0xFE), bottom half is tile (index | 0x01); bit 0 ignored (E03_04 spec)
            if sprite_row < 8 {
                u32::from(tile_index & 0xFE)
            } else {
                u32::from(tile_index | 0x01)
            }
        } else {
            u32::from(tile_index) // unsigned $8000 addressing, always (note 04a "les objets utilisent toujours l'adressage $8000")
        };
        let row_off = (tile_no * 16 + (sprite_row & 7) * 2) as usize; // tile N stores its rows at VRAM offset N*16, one tile is 16 bytes (E03_04 spec, note 04a "Tile data format")
        for dx in 0..8u32 {
            let sx = left + i32::from(dx as u8); // screen column (E03_04 spec)
            if sx < 0 || sx >= i32::from(LINE_PIXELS) {
                continue; // off-screen column: no pixel (E03_04 spec)
            }
            let col_in_tile = if flip_x { 7 - dx } else { dx }; // X flip reverses columns (E03_04 spec)
            let bit = (7 - col_in_tile) as usize; // MSB first within the tile row (note 04a "Tile data format")
            let color = ((vram[row_off + 1] >> bit) & 1) << 1 | (vram[row_off] >> bit) & 1; // note 04a "Tile data format"
            if color == 0 {
                continue; // sprite color index 0 is transparent (E03_04 spec)
            }
            let p = sx as usize;
            if self.line_resolved[p] {
                continue; // a better winner already resolved this pixel (E03_04 spec)
            }
            self.line_resolved[p] = true;
            if bg_prio && self.line_buf[p].color != 0 {
                continue; // BG colors 1..3 are drawn over the sprite: the background stays (E03_04 spec)
            }
            self.line_buf[p] = PixelOut {
                color,
                obj: true,
                pal1,
            };
        }
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

    // Existing assertions compare line_buf cells to color-index integers (E03_03/E03_12 style);
    // a cell equals the u8 of its color index (E03_04 spec).
    impl PartialEq<u8> for PixelOut {
        fn eq(&self, other: &u8) -> bool {
            self.color == *other
        }
    }

    #[test]
    fn e03_05_bgp_identity_and_reversed() {
        // BGP 0xE4 is the identity mapping: index i maps to shade i (E03_05 spec).
        assert_eq!(shade_color(0xE4, 0), PALETTE[0]);
        assert_eq!(shade_color(0xE4, 1), PALETTE[1]);
        assert_eq!(shade_color(0xE4, 2), PALETTE[2]);
        assert_eq!(shade_color(0xE4, 3), PALETTE[3]);
        // BGP 0x1B is the reversed mapping: index i maps to shade 3 - i (E03_05 spec).
        assert_eq!(shade_color(0x1B, 0), PALETTE[3]);
        assert_eq!(shade_color(0x1B, 1), PALETTE[2]);
        assert_eq!(shade_color(0x1B, 2), PALETTE[1]);
        assert_eq!(shade_color(0x1B, 3), PALETTE[0]);
    }

    #[test]
    fn e03_05_lcd_off_gives_blank_buffer() {
        let mut p = Ppu::default();
        p.reset();
        p.frame[5] = PALETTE[3]; // a non-blank pixel before the LCD is turned off (E03_05 spec)
        p.set_lcd_enabled(0x11); // bit 7 clear: LCD off (E03_09 spec)
        assert!(p.framebuffer().iter().all(|&c| c == PALETTE[0]));
    }

    #[test]
    fn e03_05_commit_applies_bgp() {
        let mut p = Ppu::default();
        p.reset();
        p.pixels_output = 160; // the line is finished: mode 3 has output all its pixels (E03_05 spec)
        for (x, color) in [0u8, 1, 2, 3].into_iter().enumerate() {
            p.line_buf[x] = PixelOut {
                color,
                obj: false,
                pal1: false,
            }; // background pixels with colors 0..3 (E03_05 spec)
        }
        p.commit_line(0xE4, 0, 0); // BGP identity mapping (E03_05 spec)
        assert_eq!(
            &p.frame[0..4],
            &[PALETTE[0], PALETTE[1], PALETTE[2], PALETTE[3]]
        );

        let mut p = Ppu::default();
        p.reset();
        p.pixels_output = 160; // the line is finished (E03_05 spec)
        for (x, color) in [0u8, 1, 2, 3].into_iter().enumerate() {
            p.line_buf[x] = PixelOut {
                color,
                obj: false,
                pal1: false,
            }; // background pixels with colors 0..3 (E03_05 spec)
        }
        p.commit_line(0x1B, 0, 0); // BGP reversed mapping (E03_05 spec)
        assert_eq!(
            &p.frame[0..4],
            &[PALETTE[3], PALETTE[2], PALETTE[1], PALETTE[0]]
        );
    }

    #[test]
    fn e03_05_commit_uses_obp0_obp1_for_sprites() {
        let mut p = Ppu::default();
        p.reset();
        p.pixels_output = 160; // the line is finished (E03_05 spec)
        p.line_buf[0] = PixelOut {
            color: 2,
            obj: true,
            pal1: false,
        }; // sprite pixel on OBP0 (E03_05 spec)
        p.line_buf[1] = PixelOut {
            color: 2,
            obj: true,
            pal1: true,
        }; // sprite pixel on OBP1 (E03_05 spec)
        p.line_buf[2] = PixelOut {
            color: 2,
            obj: false,
            pal1: false,
        }; // background pixel (E03_05 spec)
        p.commit_line(0x00, 0xE4, 0x1B); // BGP all shade 0, OBP0 identity, OBP1 reversed (E03_05 spec)
        assert_eq!(p.frame[0], PALETTE[2]);
        assert_eq!(p.frame[1], PALETTE[1]);
        assert_eq!(p.frame[2], PALETTE[0]);
    }

    #[test]
    fn e03_05_line_lands_at_row_ly_once() {
        let mut p = Ppu::default();
        p.reset();
        p.pixels_output = 160; // the line is finished (E03_05 spec)
        p.ly = 5; // a visible line in the middle of the frame (E03_05 spec)
        p.line_buf[0] = PixelOut {
            color: 3,
            obj: false,
            pal1: false,
        }; // background pixel, color 3 (E03_05 spec)
        p.commit_line(0xE4, 0, 0); // BGP identity mapping (E03_05 spec)
        assert_eq!(p.frame[5 * 160], PALETTE[3]); // the line lands at row LY (E03_05 spec)
        assert_eq!(p.frame[4 * 160], PALETTE[0]); // other rows are untouched (E03_05 spec)
        assert_eq!(p.frame[6 * 160], PALETTE[0]);

        p.line_buf[0] = PixelOut {
            color: 0,
            obj: false,
            pal1: false,
        }; // a new background pixel (E03_05 spec)
        p.commit_line(0xE4, 0, 0); // the line is already committed this line (E03_05 spec)
        assert_eq!(p.frame[5 * 160], PALETTE[3]); // a line is committed once per line (E03_05 spec)
    }

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
        for base in [0usize, 0x1000] {
            // tile index 1 sits at VRAM offset N*16 = 0x10 (unsigned) / 0x1010 (signed), different bytes (E03_04 spec)
            for row in 0..8 {
                vram[base + 0x10 + row * 2] = 0xA5; // tile index 1, both bases (E03_04 spec)
                vram[base + 0x10 + row * 2 + 1] = 0x3C;
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
    fn e03_03_tile_index_is_multiplied_by_16() {
        let mut p = Ppu::default(); // SCX = 0, unsigned $8000 tile data (E03_03 spec)
        let mut vram = vram_with_tile(0x55, 0xCC); // tile index 0 at offset 0, tile index 1 at offset 0x10 (E03_04 spec)
        for i in 0..1024 {
            vram[(0x9800 - 0x8000) + i] = 1; // $9800 map: tile index 1 everywhere (E03_04 spec)
        }
        for dot in 0..LINE_DOTS {
            if dot > 0 {
                p.tick();
            }
            p.fetch_pixels(&vram, 0, 0);
        }
        assert_eq!(p.pixels_output, LINE_PIXELS); // all 160 pixels of the line are output (E03_04 spec)
        for x in 0..LINE_PIXELS as usize {
            assert_eq!(p.line_buf[x], expected_color(0xA5, 0x3C, x), "pixel {x}");
            // tile-1 colours at offset N*16 (E03_04 spec)
        }
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
        let row = 0x1000usize - 128 * 16; // signed index 128 = -128 -> $9000 - 128*16 = $8800 (E03_04 spec)
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
        vram[0x10] = 0x55; // tile index 1, line 0: low bit plane at VRAM offset N*16 (E03_04 spec)
        vram[0x11] = 0xAA; // high bit plane
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
        for c in 0..32usize {
            // tile index c stores its row bytes at VRAM offset N*16 (E03_04 spec)
            let (low, high) = match c % 4 {
                0 => (0xFFu8, 0xFF), // both bit planes set: color index 3
                1 => (0xFF, 0x00),   // low bit plane only: color index 1
                2 => (0x00, 0x00),   // no bit plane set: color index 0
                _ => (0x00, 0xFF),   // high bit plane only: color index 2
            };
            vram[c * 16] = low;
            vram[c * 16 + 1] = high;
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

    #[test]
    fn e03_11_line_buf_is_shifted_by_the_fine_scroll_for_scx_1_to_7() {
        for scx in 1..=7u8 {
            let mut p = Ppu::default();
            let vram = vram_with_tile(0x55, 0xCC); // one tile row yields all four color indexes (E03_03 spec)
            for dot in 0..LINE_DOTS {
                if dot > 0 {
                    p.tick();
                }
                p.fetch_pixels(&vram, scx, 0);
            }
            assert_eq!(p.pixels_output, LINE_PIXELS); // all 160 pixels of the line are output (E03_11 spec)
            for x in 0..LINE_PIXELS as usize {
                // visible pixel x is background pixel SCX + x: shifted by the fine scroll (E03_11 spec)
                assert_eq!(
                    p.line_buf[x],
                    expected_color(0x55, 0xCC, x + scx as usize),
                    "scx {scx}, pixel {x}"
                );
            }
        }
    }

    /// Run `dots` dots of PPU advancement exactly as Machine::tick does (E03_12 spec): one tick,
    /// then one dot of pixel fetching and window fetching per dot.
    fn run_dots(p: &mut Ppu, vram: &[u8], scx: u8, scy: u8, wx: u8, wy: u8, dots: u32) {
        for _ in 0..dots {
            p.tick();
            p.fetch_pixels(vram, scx, scy);
            p.fetch_window(vram, wx, wy);
        }
    }

    /// Run one full line with sprites merged (E03_04 spec): OAM scan, then per dot a tick plus the
    /// pixel and sprite fetches.
    fn run_line_with_sprites(p: &mut Ppu, vram: &[u8], oam: &[u8]) {
        p.scan_oam(oam);
        for dot in 0..LINE_DOTS {
            if dot > 0 {
                p.tick();
            }
            p.fetch_pixels(vram, 0, 0);
            p.render_sprites(oam, vram);
        }
    }

    #[test]
    fn e03_04_single_sprite() {
        let vram = vram_with_tile(0x00, 0x00); // background tile 0 is colour 0; tile 1 at offset 0x10 (E03_04 spec)
        let mut oam = [0u8; 160];
        oam[0] = 16; // Y=16: screen line 0 with LY=0 (E03_02 spec)
        oam[1] = 8; // X=8: screen columns 0..7 (E03_04 spec)
        oam[2] = 1; // tile index 1 (E03_04 spec)
        let mut p = Ppu {
            lcdc: 0x93, // bits 7,4,1,0: LCD on, unsigned tile data, sprites on, BG on (E03_04 spec)
            ..Ppu::default()
        };
        run_line_with_sprites(&mut p, &vram, &oam);
        let expected = [1u8, 0, 3, 2, 2, 3, 0, 1]; // tile-1 row 0 colours (E03_04 spec)
        for x in [0usize, 2, 3, 4, 5, 7] {
            assert_eq!(p.line_buf[x].color, expected[x], "pixel {x}");
            assert!(
                p.line_buf[x].obj,
                "pixel {x} is a sprite pixel (E03_04 spec)"
            );
            assert!(
                !p.line_buf[x].pal1,
                "palette 1 not selected: attrs bit 3 clear (E03_04 spec)"
            );
        }
        for x in [1usize, 6] {
            // colour index 0 is transparent: background shows through (E03_04 spec)
            assert!(!p.line_buf[x].obj);
            assert_eq!(p.line_buf[x].color, 0);
        }
        assert!(!p.line_buf[8].obj); // column 8 is background again (E03_04 spec)
        assert_eq!(p.line_buf[8].color, 0);
    }

    #[test]
    fn e03_07_sprite_at_oam_index_5_is_drawn() {
        let vram = vram_with_tile(0x00, 0x00); // background tile 0 is colour 0; tile 1 at offset 0x10 (E03_04 spec)
        let mut oam = [0u8; 160];
        oam[20] = 16; // OAM entry 5, Y=16: screen line 0 with LY=0 (E03_02 spec)
        oam[21] = 8; // X=8: screen columns 0..7 (E03_04 spec)
        oam[22] = 1; // tile index 1 (E03_04 spec)
        let mut p = Ppu {
            lcdc: 0x93, // bits 7,4,1,0: LCD on, unsigned tile data, sprites on, BG on (E03_04 spec)
            ..Ppu::default()
        };
        run_line_with_sprites(&mut p, &vram, &oam);
        assert_eq!(p.line_buf[0].color, 1); // tile-1 row 0 colours [1,0,3,2,2,3,0,1] (E03_04 spec)
        assert_eq!(p.line_buf[2].color, 3);
        assert!(p.line_buf[0].obj);
        assert!(p.line_buf[2].obj);
    }

    #[test]
    fn e03_07_two_sprites_at_high_oam_indexes_are_both_drawn() {
        let vram = vram_with_tile(0x00, 0x00); // background tile 0 is colour 0; tile 1 at offset 0x10 (E03_04 spec)
        let mut oam = [0u8; 160];
        oam[12] = 16; // OAM entry 3, Y=16: screen line 0 with LY=0 (E03_02 spec)
        oam[13] = 16; // X=16: screen columns 8..15 (E03_04 spec)
        oam[14] = 1; // tile index 1 (E03_04 spec)
        oam[28] = 16; // OAM entry 7, Y=16: screen line 0 with LY=0 (E03_02 spec)
        oam[29] = 8; // X=8: screen columns 0..7 (E03_04 spec)
        oam[30] = 1; // tile index 1 (E03_04 spec)
        let mut p = Ppu {
            lcdc: 0x93, // bits 7,4,1,0: LCD on, unsigned tile data, sprites on, BG on (E03_04 spec)
            ..Ppu::default()
        };
        run_line_with_sprites(&mut p, &vram, &oam);
        assert_eq!(p.line_buf[0].color, 1); // entry 7: tile-1 row 0 colours [1,0,3,2,2,3,0,1] (E03_04 spec)
        assert_eq!(p.line_buf[2].color, 3);
        assert_eq!(p.line_buf[8].color, 1); // entry 3: same row at columns 8..15 (E03_04 spec)
        assert_eq!(p.line_buf[10].color, 3);
        assert!(p.line_buf[0].obj);
        assert!(p.line_buf[2].obj);
        assert!(p.line_buf[8].obj);
        assert!(p.line_buf[10].obj);
    }

    #[test]
    fn e03_05_sprite_index_0_shows_the_background() {
        let vram = vram_with_tile(0xFF, 0xFF); // background tile 0 is colour 3 everywhere; tile 1 keeps its own bytes (E03_05 spec)
        let mut oam = [0u8; 160];
        oam[0] = 16; // Y=16: screen line 0 with LY=0 (E03_02 spec)
        oam[1] = 8; // X=8: screen columns 0..7 (E03_04 spec)
        oam[2] = 1; // tile index 1 (E03_04 spec)
        let mut p = Ppu {
            lcdc: 0x93, // bits 7,4,1,0: LCD on, unsigned tile data, sprites on, BG on (E03_04 spec)
            ..Ppu::default()
        };
        run_line_with_sprites(&mut p, &vram, &oam);
        p.commit_line(0xE4, 0x1B, 0x1B); // BGP identity; OBP0/OBP1 reversed (E03_05 spec)
        let expected = [
            PALETTE[2], // sprite colour 1 through OBP0 = 0x1B (E03_05 spec)
            PALETTE[3], // transparent pixel: background colour 3 through BGP = 0xE4 (E03_05 spec)
            PALETTE[0], // sprite colour 3 through OBP0 = 0x1B (E03_05 spec)
            PALETTE[1], // sprite colour 2 through OBP0 = 0x1B (E03_05 spec)
            PALETTE[1], // sprite colour 2 through OBP0 = 0x1B (E03_05 spec)
            PALETTE[0], // sprite colour 3 through OBP0 = 0x1B (E03_05 spec)
            PALETTE[3], // transparent pixel: background colour 3 through BGP = 0xE4 (E03_05 spec)
            PALETTE[2], // sprite colour 1 through OBP0 = 0x1B (E03_05 spec)
            PALETTE[3], // column 8 is background colour 3 through BGP = 0xE4 (E03_05 spec)
        ];
        assert_eq!(&p.frame[0..9], &expected);
    }

    #[test]
    fn e03_04_x_flip() {
        let mut vram = vram_with_tile(0x00, 0x00); // background tile 0 is colour 0 (E03_04 spec)
        vram[0x20] = 0xF0; // tile 2 row 0 low byte: columns 0..3 are colour 1 (E03_04 spec)
        vram[0x21] = 0x00; // tile 2 row 0 high byte: all clear (E03_04 spec)
        let mut oam = [0u8; 160];
        oam[0] = 16; // Y=16: screen line 0 with LY=0 (E03_02 spec)
        oam[1] = 8; // X=8: screen columns 0..7 (E03_04 spec)
        oam[2] = 2; // tile index 2 at VRAM offset 0x20 (E03_04 spec)
        oam[3] = 0x20; // attrs bit 5: flip X (E03_04 spec)
        let mut p = Ppu {
            lcdc: 0x93, // bits 7,4,1,0: LCD on, unsigned tile data, sprites on, BG on (E03_04 spec)
            ..Ppu::default()
        };
        run_line_with_sprites(&mut p, &vram, &oam);
        for x in 0..4usize {
            // flipped row: transparent half now at columns 0..3 (E03_04 spec)
            assert!(!p.line_buf[x].obj, "pixel {x}");
            assert_eq!(p.line_buf[x].color, 0);
        }
        for x in 4..8usize {
            // flipped row: colour-1 half now at columns 4..7 (E03_04 spec)
            assert!(p.line_buf[x].obj, "pixel {x}");
            assert_eq!(p.line_buf[x].color, 1);
        }
    }

    #[test]
    fn e03_04_y_flip() {
        let mut vram = vram_with_tile(0x00, 0x00); // background tile 0 is colour 0 (E03_04 spec)
        vram[0x20] = 0xFF; // tile 2 row 0: colour 1 (E03_04 spec)
        vram[0x21] = 0x00;
        vram[0x2E] = 0x00; // tile 2 row 7: colour 2 (E03_04 spec)
        vram[0x2F] = 0xFF;
        let mut oam = [0u8; 160];
        oam[0] = 16; // Y=16: screen line 0 with LY=0 (E03_02 spec)
        oam[1] = 8; // X=8: screen columns 0..7 (E03_04 spec)
        oam[2] = 2; // tile index 2 at VRAM offset 0x20 (E03_04 spec)
        oam[3] = 0x40; // attrs bit 6: flip Y, screen line 0 reads tile row 7 (E03_04 spec)
        let mut p = Ppu {
            lcdc: 0x93, // bits 7,4,1,0: LCD on, unsigned tile data, sprites on, BG on (E03_04 spec)
            ..Ppu::default()
        };
        run_line_with_sprites(&mut p, &vram, &oam);
        for x in 0..8usize {
            // flipped sprite: row 7 colour 2 across all columns (E03_04 spec)
            assert!(p.line_buf[x].obj, "pixel {x}");
            assert_eq!(p.line_buf[x].color, 2);
        }
    }

    #[test]
    fn e03_04_overlap_lower_x_wins() {
        let mut vram = vram_with_tile(0x00, 0x00); // background tile 0 is colour 0 (E03_04 spec)
        vram[0x20] = 0xFF; // tile 2 uniform: colour 1 (E03_04 spec)
        vram[0x21] = 0x00;
        vram[0x30] = 0x00; // tile 3 uniform: colour 2 (E03_04 spec)
        vram[0x31] = 0xFF;
        let mut oam = [0u8; 160];
        oam[0] = 16; // Y=16: screen line 0 with LY=0 (E03_02 spec)
        oam[1] = 12; // X=12: screen columns 4..11, tile 2 colour 1 (E03_04 spec)
        oam[2] = 2;
        oam[4] = 16;
        oam[5] = 8; // X=8: screen columns 0..7, tile 3 colour 2 (E03_04 spec)
        oam[6] = 3;
        let mut p = Ppu {
            lcdc: 0x93, // bits 7,4,1,0: LCD on, unsigned tile data, sprites on, BG on (E03_04 spec)
            ..Ppu::default()
        };
        run_line_with_sprites(&mut p, &vram, &oam);
        for x in 0..8usize {
            // lowest X wins the overlap despite the higher OAM index (E03_04 spec)
            assert!(p.line_buf[x].obj, "pixel {x}");
            assert_eq!(p.line_buf[x].color, 2);
        }
        for x in 8..12usize {
            // only sprite 0 remains: colour 1 (E03_04 spec)
            assert!(p.line_buf[x].obj, "pixel {x}");
            assert_eq!(p.line_buf[x].color, 1);
        }
        assert!(!p.line_buf[12].obj); // column 12 is background again (E03_04 spec)
    }

    #[test]
    fn e03_04_equal_x_lowest_oam_index_wins() {
        let mut vram = vram_with_tile(0x00, 0x00); // background tile 0 is colour 0 (E03_04 spec)
        vram[0x20] = 0xFF; // tile 2 uniform: colour 1 (E03_04 spec)
        vram[0x21] = 0x00;
        vram[0x30] = 0x00; // tile 3 uniform: colour 2 (E03_04 spec)
        vram[0x31] = 0xFF;
        let mut oam = [0u8; 160];
        oam[0] = 16; // Y=16: screen line 0 with LY=0 (E03_02 spec)
        oam[1] = 8; // X=8: screen columns 0..7, tile 2 colour 1 (E03_04 spec)
        oam[2] = 2;
        oam[4] = 16;
        oam[5] = 8; // same X as OAM 0: the lowest OAM index wins (E03_04 spec)
        oam[6] = 3;
        let mut p = Ppu {
            lcdc: 0x93, // bits 7,4,1,0: LCD on, unsigned tile data, sprites on, BG on (E03_04 spec)
            ..Ppu::default()
        };
        run_line_with_sprites(&mut p, &vram, &oam);
        for x in 0..8usize {
            // equal X: OAM index 0 wins over index 1 (E03_04 spec)
            assert!(p.line_buf[x].obj, "pixel {x}");
            assert_eq!(p.line_buf[x].color, 1);
        }
    }

    #[test]
    fn e03_04_bg_priority_flag() {
        let mut vram = vram_with_tile(0xF0, 0x00); // background row 0: [1,1,1,1,0,0,0,0] (E03_04 spec)
        vram[0x20] = 0xFF; // tile 2 uniform: colour 3 (E03_04 spec)
        vram[0x21] = 0xFF;
        let mut oam = [0u8; 160];
        oam[0] = 16; // Y=16: screen line 0 with LY=0 (E03_02 spec)
        oam[1] = 8; // X=8: screen columns 0..7, tile 2 uniform colour 3 (E03_04 spec)
        oam[2] = 2;
        oam[3] = 0x80; // attrs bit 7: BG priority (E03_04 spec)
        let mut p = Ppu {
            lcdc: 0x93, // bits 7,4,1,0: LCD on, unsigned tile data, sprites on, BG on (E03_04 spec)
            ..Ppu::default()
        };
        run_line_with_sprites(&mut p, &vram, &oam);
        for x in 0..4usize {
            // BG colour 1 stays over the sprite: background wins (E03_04 spec)
            assert!(!p.line_buf[x].obj, "pixel {x}");
            assert_eq!(p.line_buf[x].color, 1);
        }
        for x in 4..8usize {
            // BG colour 0 is transparent: the sprite shows through (E03_04 spec)
            assert!(p.line_buf[x].obj, "pixel {x}");
            assert_eq!(p.line_buf[x].color, 3);
        }
    }

    #[test]
    fn e03_04_8x16_with_y_flip() {
        let mut vram = vram_with_tile(0x00, 0x00); // background tile 0 is colour 0 (E03_04 spec)
        for i in 0..16usize {
            // tile 4 uniform: colour 1 on every row (E03_04 spec)
            vram[0x40 + i] = if i % 2 == 0 { 0xFF } else { 0x00 };
        }
        vram[0x5E] = 0xFF; // tile 5: only row 7 is colour 3 (E03_04 spec)
        vram[0x5F] = 0xFF;
        let mut oam = [0u8; 160];
        oam[0] = 16; // Y=16: screen line 0 with LY=0 (E03_02 spec)
        oam[1] = 8; // X=8: screen columns 0..7 (E03_04 spec)
        oam[2] = 5; // tile index 5, bit 0 ignored: top half tile 4, bottom half tile 5 (E03_04 spec)
        oam[3] = 0x40; // attrs bit 6: flip Y swaps the two halves (E03_04 spec)
        let mut p = Ppu {
            lcdc: 0x97, // bits 7,4,2,1,0: LCD on, unsigned tile data, 8x16 sprites, BG on (E03_04 spec)
            ..Ppu::default()
        };
        run_line_with_sprites(&mut p, &vram, &oam);
        for x in 0..8usize {
            // flipped: screen line 0 reads tile-5 row 7, colour 3 (E03_04 spec)
            assert!(p.line_buf[x].obj, "pixel {x}");
            assert_eq!(p.line_buf[x].color, 3);
        }
    }

    /// VRAM with the BG map at $9800 pointing to tile 0 (row 0 = color index 2) and the window map
    /// at $9C00 pointing to tile 1 (row 0 = color index 3), from the unsigned base $8000. A tile of
    /// index N stores its row bytes starting at VRAM offset N*16, so tile 0 sits at 0x00 and tile 1
    /// at 0x10 (E03_04 spec).
    fn win_vram() -> [u8; 0x2000] {
        let mut vram = [0x00u8; 0x2000];
        for i in 0..1024 {
            vram[(0x9800 - 0x8000) + i] = 0; // BG map $9800 -> tile 0 (E03_12 spec)
        }
        for i in 0..1024 {
            vram[(0x9C00 - 0x8000) + i] = 1; // window map $9C00 -> tile 1 (E03_12 spec)
        }
        vram[0x00] = 0x00; // tile 0 row 0 low bit plane (E03_12 spec)
        vram[0x01] = 0xFF; // tile 0 row 0 high bit plane: background colour 2 (E03_12 spec)
        vram[0x10] = 0xFF; // tile 1 row 0 low bit plane, offset 1*16 (E03_04 spec)
        vram[0x11] = 0xFF; // tile 1 row 0 high bit plane: window colour 3 (E03_12 spec)
        vram
    }

    #[test]
    fn e03_12_window_starts_at_the_right_pixel_for_wx() {
        let vram = win_vram(); // background color 2, window color 3 on row 0 (E03_12 spec)
        for (wx, first_win_x) in [(7u8, 0usize), (20, 13), (166, 159)] {
            let mut p = Ppu {
                lcdc: 0xF1,
                ..Ppu::default()
            }; // window enabled, map $9C00 (E03_12 spec)
            run_dots(&mut p, &vram, 0, 0, wx, 0, LINE_DOTS); // line 0, WY = 0 (E03_12 spec)
            for x in 0..LINE_PIXELS as usize {
                let expected = if x >= first_win_x { 3 } else { 2 }; // window starts at screen X = WX - 7 (E03_12 spec)
                assert_eq!(p.line_buf[x], expected, "wx {wx}, pixel {x}");
            }
        }
    }

    #[test]
    fn e03_12_window_starts_on_the_wy_line() {
        let vram = win_vram(); // line 4 background is color 0 (tile 0 row 4); window color 3 (E03_12 spec)
        const WY: u8 = 5;
        let mut p = Ppu {
            lcdc: 0xF1,
            ..Ppu::default()
        }; // window enabled (E03_12 spec)
        run_dots(&mut p, &vram, 0, 0, 7, WY, 5 * LINE_DOTS); // lines 0..4: LY < WY, no window (E03_12 spec)
        assert!(
            p.line_buf.iter().all(|&c| c == 0),
            "line 4 (LY=4) is all background (E03_12 spec)"
        );
        run_dots(&mut p, &vram, 0, 0, 7, WY, LINE_DOTS); // line 5: LY = WY, the window starts (E03_12 spec)
        assert!(
            p.line_buf.iter().all(|&c| c == 3),
            "line 5 (LY=5) is all window (E03_12 spec)"
        );
    }

    #[test]
    fn e03_12_win_line_advances_only_on_window_lines() {
        let vram = win_vram();
        const WY: u8 = 2; // no window on lines 0 and 1 (LY < WY); the window starts at line 2 (E03_12 spec)
        let mut p = Ppu {
            lcdc: 0xF1,
            ..Ppu::default()
        }; // window enabled (E03_12 spec)
        run_dots(&mut p, &vram, 0, 0, 7, WY, 3 * LINE_DOTS); // lines 0..2 (E03_12 spec)
        assert_eq!(
            p.win_line, 1,
            "only line 2 drew a window pixel: the counter advances once (E03_12 spec)"
        );
    }

    #[test]
    fn e03_12_win_line_stays_zero_when_window_disabled() {
        let vram = win_vram();
        let mut p = Ppu {
            lcdc: 0x81,
            ..Ppu::default()
        }; // bit 5 clear: window disabled (E03_12 spec)
        run_dots(&mut p, &vram, 0, 0, 7, 0, 3 * LINE_DOTS); // WX = 7, WY = 0 but the window is off (E03_12 spec)
        assert_eq!(
            p.win_line, 0,
            "no window pixel drawn: the counter does not advance (E03_12 spec)"
        );
    }

    #[test]
    fn e03_12_window_map_selects_by_lcdc_bit6() {
        let mut vram = [0x00u8; 0x2000];
        for i in 0..1024 {
            vram[(0x9800 - 0x8000) + i] = 16; // $9800 map -> tile 16 (E03_12 spec)
        }
        for i in 0..1024 {
            vram[(0x9C00 - 0x8000) + i] = 48; // $9C00 map -> tile 48 (E03_12 spec)
        }
        // Tile data from the unsigned base $8000: a tile of index N stores its row bytes at VRAM
        // offset N*16, so tile 16 sits at 0x100 and tile 48 at 0x300 (E03_04 spec).
        vram[16 * 16] = 0x00; // tile 16 row 0 low bit plane: color index 2 (E03_12 spec)
        vram[16 * 16 + 1] = 0xFF; // tile 16 row 0 high bit plane: color index 2 (E03_12 spec)
        vram[48 * 16] = 0xFF; // tile 48 row 0 low bit plane: color index 3 (E03_12 spec)
        vram[48 * 16 + 1] = 0xFF; // tile 48 row 0 high bit plane: color index 3 (E03_12 spec)

        let mut p = Ppu {
            lcdc: 0xB9, // bits 7,5,4,3,0 set: window map $9800 (bit 6 clear), BG map $9C00 (E03_12 spec)
            ..Ppu::default()
        };
        run_dots(&mut p, &vram, 0, 0, 7, 0, LINE_DOTS); // WX = 7: the window covers the whole line (E03_12 spec)
        assert!(
            p.line_buf.iter().all(|&c| c == 2),
            "bit 6 clear: window pixels from the $9800 map, not the BG's $9C00 map (E03_12 spec)"
        );

        let mut p = Ppu {
            lcdc: 0xF1, // bits 7,6,5,4,0 set: window map $9C00 (bit 6 set), BG map $9800 (E03_12 spec)
            ..Ppu::default()
        };
        run_dots(&mut p, &vram, 0, 0, 7, 0, LINE_DOTS); // WX = 7: the window covers the whole line (E03_12 spec)
        assert!(
            p.line_buf.iter().all(|&c| c == 3),
            "bit 6 set: window pixels from the $9C00 map, not the BG's $9800 map (E03_12 spec)"
        );
    }

    #[test]
    fn e03_11_mode_boundaries_for_each_scx_mod_8() {
        for scx in 0..=7u8 {
            let mut p = Ppu::default();
            let vram = [0x00u8; 0x2000];
            // Last dot of mode 3: dots 80..=251+SCX mod 8, i.e. a duration of 172 + SCX mod 8 (E03_11 spec).
            let mode3_end = MODE2_DOTS + MODE3_LEN - 1 + u32::from(scx);
            for dot in 0..LINE_DOTS {
                if dot > 0 {
                    p.tick();
                }
                p.fetch_pixels(&vram, scx, 0);
                let expected = if dot < MODE2_DOTS {
                    2 // OAM scan: dots 0..79 (note 04a "STAT bits")
                } else if dot <= mode3_end {
                    3 // mode 3 lasts 172 + SCX mod 8 dots, i.e. 80..=251+SCX mod 8 (E03_11 spec)
                } else {
                    0 // HBlank: from dot 252+SCX mod 8 to dot 455 (note 04b)
                };
                assert_eq!(p.mode(), expected, "scx {scx}, mode at dot {dot}");
            }
        }
    }

    #[test]
    fn e03_11_mode_transitions_from_3_to_0_at_the_exact_dot() {
        for scx in 0..=7u8 {
            let mut p = Ppu::default();
            let vram = [0x00u8; 0x2000];
            let mode3_end = MODE2_DOTS + MODE3_LEN - 1 + u32::from(scx); // last dot of mode 3 (E03_11 spec)
            for dot in 0..=mode3_end {
                if dot > 0 {
                    p.tick();
                }
                p.fetch_pixels(&vram, scx, 0);
            }
            assert_eq!(
                p.mode(),
                3,
                "scx {scx}: mode is still 3 on its last dot {mode3_end}"
            );
            p.tick(); // dot mode3_end + 1: the exact transition dot from mode 3 to mode 0 (E03_11 spec)
            assert_eq!(p.mode(), 0, "scx {scx}: mode is 0 on dot {}", mode3_end + 1);
            for dot in mode3_end + 2..LINE_DOTS {
                p.tick(); // mode 0 runs from the transition dot to dot 455 (note 04b)
                assert_eq!(p.mode(), 0, "scx {scx}, mode at dot {dot}");
            }
        }
    }

    #[test]
    fn e03_11_mid_line_scx_write_does_not_change_the_fine_scroll() {
        let mut p = Ppu::default();
        let vram = vram_with_tile(0x55, 0xCC); // uniform tile row: only the bit position matters (E03_03 spec)
        let mut last_mode3_dot = 0u32;
        for dot in 0..LINE_DOTS {
            if dot > 0 {
                p.tick();
            }
            // The low bits of SCX are latched on the first dot of mode 3 (dot 80); a mid-line write
            // changes only the high bits, which keep being re-read per tile fetch (E03_11 spec).
            let scx = if dot <= MODE2_DOTS { 3u8 } else { 19 };
            p.fetch_pixels(&vram, scx, 0);
            if p.mode() == 3 {
                last_mode3_dot = dot;
            }
        }
        assert_eq!(p.pixels_output, LINE_PIXELS); // all 160 pixels of the line are output (E03_11 spec)
        for x in 0..LINE_PIXELS as usize {
            // fine scroll stays at the value captured on dot 80: a shift of 3 (E03_11 spec)
            assert_eq!(
                p.line_buf[x],
                expected_color(0x55, 0xCC, x + 3),
                "pixel {x}"
            );
        }
        // mode 3 still lasts 172 + 3 dots: it ends on dot 254 (E03_11 spec)
        assert_eq!(last_mode3_dot, MODE2_DOTS + MODE3_LEN + 3 - 1);
    }

    #[test]
    fn e03_13_line_obj_penalty_values() {
        // (sprite X positions, SCX) -> mode 3 extension in dots (E03_13 spec)
        assert_eq!(line_obj_penalty(&[], 0), 0);
        assert_eq!(line_obj_penalty(&[8], 0), 11);
        assert_eq!(line_obj_penalty(&[13], 0), 6);
        assert_eq!(line_obj_penalty(&[0], 0), 11);
        assert_eq!(line_obj_penalty(&[160], 0), 11);
        assert_eq!(line_obj_penalty(&[168], 0), 0);
        assert_eq!(line_obj_penalty(&[8, 9], 0), 17);
        assert_eq!(line_obj_penalty(&[16, 8], 0), 22);
        assert_eq!(line_obj_penalty(&[8], 3), 8);
        assert_eq!(line_obj_penalty(&[8; 10], 0), 65);
    }

    /// Run one visible line through the pixel pipeline with sprites at `xs` (all on screen line 0,
    /// i.e. OAM Y = 16) and SCX = scx; return the last dot of mode 3 (E03_13 spec). The sprite
    /// fetch penalty is computed from the selected sprites via scan_oam + set_obj_penalty, so this
    /// measures how far the penalty pushes the end of mode 3 through the real pipeline.
    fn mode3_last_dot_with_sprites(xs: &[u8], scx: u8) -> u32 {
        let vram = [0x00u8; 0x2000]; // background tile 0 is colour 0 (E03_13 spec)
        let mut oam = [0u8; 160];
        for (i, &x) in xs.iter().enumerate() {
            oam[i * 4] = 16; // Y=16: screen line 0 with LY=0 (E03_02 spec)
            oam[i * 4 + 1] = x; // sprite X position (E03_13 spec)
        }
        let mut p = Ppu {
            lcdc: 0x93, // bits 7,4,1,0: LCD on, unsigned tile data, sprites on, BG on (E03_13 spec)
            ..Ppu::default()
        };
        p.scan_oam(&oam);
        p.set_obj_penalty(&oam, scx); // the sprite penalty of this line (E03_13 spec)
        let mut last_mode3_dot = 0u32;
        for dot in 0..LINE_DOTS {
            if dot > 0 {
                p.tick();
            }
            p.fetch_pixels(&vram, scx, 0);
            if p.mode() == 3 {
                last_mode3_dot = dot;
            }
        }
        assert_eq!(p.pixels_output, LINE_PIXELS); // all 160 pixels of the line are output (E03_13 spec)
        last_mode3_dot
    }

    #[test]
    fn e03_13_mode3_length_with_zero_sprites() {
        // No sprites: mode 3 lasts exactly MODE3_LEN = 172 dots, ending on dot 251 (E03_13 spec).
        assert_eq!(
            mode3_last_dot_with_sprites(&[], 0),
            MODE2_DOTS + MODE3_LEN - 1
        );
    }

    #[test]
    fn e03_13_mode3_length_with_one_sprite() {
        // One sprite at X=8 (leftmost pixel column 0, the left edge of tile 0): penalty = 6 + 5 = 11
        // dots (E03_13 spec), so mode 3 lasts 172 + 11 = 183 dots, ending on dot 262.
        assert_eq!(
            mode3_last_dot_with_sprites(&[8], 0),
            MODE2_DOTS + MODE3_LEN - 1 + 11
        );
    }

    #[test]
    fn e03_13_mode3_length_with_ten_sprites() {
        // Ten sprites all at X=8: the first costs 6 + 5 = 11 dots (its tile not yet considered), each
        // of the other nine shares that tile and costs only the fixed 6-dot OBJ fetch, so the penalty
        // is 11 + 9 * 6 = 65 dots (E03_13 spec); mode 3 lasts 172 + 65 = 237 dots, ending on dot 316.
        assert_eq!(
            mode3_last_dot_with_sprites(&[8; 10], 0),
            MODE2_DOTS + MODE3_LEN - 1 + 65
        );
    }

    #[test]
    fn e03_13_sprite_at_x_zero_costs_eleven_dots_regardless_of_scx() {
        // A sprite at X=0 is completely off-screen to the left: it always costs 11 dots, whatever SCX
        // (E03_13 spec). With SCX=3 the fine scroll adds 3 more dots, so mode 3 lasts 172 + 11 + 3 =
        // 186 dots, ending on dot 265.
        assert_eq!(
            mode3_last_dot_with_sprites(&[0], 3),
            MODE2_DOTS + MODE3_LEN - 1 + 11 + 3
        );
    }

    #[test]
    fn e03_13_sprite_at_x_160_costs_eleven_dots() {
        // A sprite at X=160 has its leftmost pixel on column 152, the left edge of tile 19: penalty =
        // 6 + 5 = 11 dots (E03_13 spec); mode 3 lasts 172 + 11 = 183 dots, ending on dot 262.
        assert_eq!(
            mode3_last_dot_with_sprites(&[160], 0),
            MODE2_DOTS + MODE3_LEN - 1 + 11
        );
    }
}
