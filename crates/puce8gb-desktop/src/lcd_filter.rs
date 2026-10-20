//! LCD grid filter: a soft pixel-cell grid like a real LCD (E12_01, decision A_09).
//! Pure functions on RGBA bytes, no egui type, integer math only.

/// Weight (0..=256) of one position `i` inside a cell of `scale` output pixels along one axis.
/// 256 at the cell center, lower toward the cell border; `strength` 0..=255 sets the depth.
#[allow(dead_code)] // used by E12_03
pub fn cell_weight(i: u32, scale: u32, strength: u32) -> u32 {
    if scale <= 1 {
        return 256;
    }
    let c = (2 * i as i32 + 1 - scale as i32).unsigned_abs();
    let s = strength.min(255);
    256 - s * c * c / (scale * scale)
}

/// Blends one color channel toward the gap color: weight 256 keeps `src`, weight 0 gives `gap`.
fn blend(src: u8, gap: u8, w: u32) -> u8 {
    ((u32::from(src) * w + u32::from(gap) * (256 - w) + 128) >> 8) as u8
}

/// Scales `src` (width x height RGBA, row-major) by `scale` and darkens the borders of each
/// source pixel toward `gap`. Returns an empty Vec when `src.len() != width * height` or `scale == 0`.
#[allow(dead_code)] // used by E12_03
pub fn grid_filter(
    src: &[[u8; 4]],
    width: usize,
    height: usize,
    scale: u32,
    strength: u32,
    gap: [u8; 3],
) -> Vec<[u8; 4]> {
    if scale == 0 || width.checked_mul(height) != Some(src.len()) {
        return Vec::new();
    }
    let s = scale as usize;
    let axis: Vec<u32> = (0..scale)
        .map(|i| cell_weight(i, scale, strength))
        .collect();
    let out_w = width * s;
    let mut out = vec![[0u8; 4]; out_w * height * s];
    for y in 0..height {
        for x in 0..width {
            let px = src[y * width + x];
            for (iy, wy) in axis.iter().enumerate() {
                for (ix, wx) in axis.iter().enumerate() {
                    let w = (wx * wy) >> 8;
                    let o = (y * s + iy) * out_w + x * s + ix;
                    out[o] = [
                        blend(px[0], gap[0], w),
                        blend(px[1], gap[1], w),
                        blend(px[2], gap[2], w),
                        px[3],
                    ];
                }
            }
        }
    }
    out
}

/// Mixes one color channel: `p` (0..=255) is the share of the previous frame, out of 256.
fn mix(cur: u8, prev: u8, p: u32) -> u8 {
    ((u32::from(cur) * (256 - p) + u32::from(prev) * p + 128) >> 8) as u8
}

/// Pixel response: blends the previous frame into the current one. `persistence` 0..=255 is the
/// weight of the previous frame (0 = no ghosting). Alpha is copied from `cur`. When the two
/// frames differ in length the current frame is returned unchanged.
#[allow(dead_code)] // used by E12_03
pub fn ghost_blend(prev: &[[u8; 4]], cur: &[[u8; 4]], persistence: u32) -> Vec<[u8; 4]> {
    if prev.len() != cur.len() {
        return cur.to_vec();
    }
    let p = persistence.min(255);
    prev.iter()
        .zip(cur.iter())
        .map(|(old, new)| {
            [
                mix(new[0], old[0], p),
                mix(new[1], old[1], p),
                mix(new[2], old[2], p),
                new[3],
            ]
        })
        .collect()
}

/// LCD shader mode (E12_03).
#[allow(dead_code)] // used by E12_04
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LcdMode {
    #[default]
    Off,
    Grid,
    Ghost,
}

#[allow(dead_code)] // used by E12_04
impl LcdMode {
    /// Settings text: "off", "grid" or "ghost".
    pub fn as_str(self) -> &'static str {
        match self {
            LcdMode::Off => "off",
            LcdMode::Grid => "grid",
            LcdMode::Ghost => "ghost",
        }
    }

    /// Parses settings text (case-insensitive); unknown text gives Off.
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(text: &str) -> LcdMode {
        let trimmed = text.trim();
        if trimmed.eq_ignore_ascii_case("grid") {
            LcdMode::Grid
        } else if trimmed.eq_ignore_ascii_case("ghost") {
            LcdMode::Ghost
        } else {
            LcdMode::Off
        }
    }
}

/// Mode, filter parameters and the previous frame (E12_03).
#[allow(dead_code)] // used by E12_04
#[derive(Debug, Clone)]
pub struct LcdFilter {
    mode: LcdMode,
    prev: Vec<[u8; 4]>,
    strength: u32,
    persistence: u32,
    gap: [u8; 3],
}

#[allow(dead_code)] // used by E12_04
impl LcdFilter {
    /// Off, strength 160, persistence 96 (aesthetic values, UNKNOWN - to tune), black gap.
    pub fn new() -> LcdFilter {
        LcdFilter {
            mode: LcdMode::Off,
            prev: Vec::new(),
            strength: 160,
            persistence: 96,
            gap: [0, 0, 0],
        }
    }

    /// Current mode.
    pub fn mode(&self) -> LcdMode {
        self.mode
    }

    /// Changes the mode and forgets the previous frame.
    pub fn set_mode(&mut self, mode: LcdMode) {
        self.mode = mode;
        self.prev.clear();
    }

    /// Sets the color of the gaps between pixel cells.
    pub fn set_gap(&mut self, gap: [u8; 3]) {
        self.gap = gap;
    }

    /// Filters one frame (width x height RGBA, row-major) scaled by `scale`.
    /// Off gives None. Ghost blends the previous frame first and stores the blended frame
    /// (before the grid) for the next call.
    pub fn process(
        &mut self,
        frame: &[[u8; 4]],
        width: usize,
        height: usize,
        scale: u32,
    ) -> Option<Vec<[u8; 4]>> {
        match self.mode {
            LcdMode::Off => None,
            LcdMode::Grid => Some(grid_filter(
                frame,
                width,
                height,
                scale,
                self.strength,
                self.gap,
            )),
            LcdMode::Ghost => {
                let blended = ghost_blend(&self.prev, frame, self.persistence);
                let out = grid_filter(&blended, width, height, scale, self.strength, self.gap);
                self.prev = blended;
                Some(out)
            }
        }
    }
}

impl Default for LcdFilter {
    fn default() -> LcdFilter {
        LcdFilter::new()
    }
}

/// Gap color for the grid: the darkest palette color (index 3, 0xRRGGBB) as [r, g, b]; None gives black.
pub fn gap_for(colors: Option<[u32; 4]>) -> [u8; 3] {
    match colors {
        Some(palette) => {
            let c = palette[3];
            [(c >> 16) as u8, (c >> 8) as u8, c as u8]
        }
        None => [0, 0, 0],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn e12_01_cell_weight_scale_five() {
        assert_eq!(cell_weight(0, 5, 128), 175);
        assert_eq!(cell_weight(1, 5, 128), 236);
        assert_eq!(cell_weight(2, 5, 128), 256);
        assert_eq!(cell_weight(3, 5, 128), 236);
        assert_eq!(cell_weight(4, 5, 128), 175);
    }

    #[test]
    fn e12_01_cell_weight_small_scales() {
        assert_eq!(cell_weight(0, 1, 255), 256);
        assert_eq!(cell_weight(0, 2, 255), 193);
        assert_eq!(cell_weight(1, 2, 255), 193);
    }

    #[test]
    fn e12_01_grid_filter_single_pixel_scale_five() {
        let src = [[200u8, 200, 200, 255]];
        let out = grid_filter(&src, 1, 1, 5, 128, [100, 100, 100]);
        assert_eq!(out.len(), 25);
        assert_eq!(out[2 * 5 + 2][0], 200);
        assert_eq!(out[2 * 5][0], 168);
        assert_eq!(out[0][0], 146);
        assert!(out.iter().all(|p| p[3] == 255));
    }

    #[test]
    fn e12_01_grid_filter_scale_one_equals_source() {
        let src = [[1u8, 2, 3, 255], [40, 50, 60, 128]];
        let out = grid_filter(&src, 2, 1, 1, 200, [9, 9, 9]);
        assert_eq!(out, src.to_vec());
    }

    #[test]
    fn e12_01_grid_filter_wrong_length_gives_empty() {
        let src = [[0u8, 0, 0, 255]; 3];
        assert!(grid_filter(&src, 2, 2, 3, 128, [0, 0, 0]).is_empty());
    }

    #[test]
    fn e12_01_grid_filter_scale_zero_gives_empty() {
        let src = [[0u8, 0, 0, 255]];
        assert!(grid_filter(&src, 1, 1, 0, 128, [0, 0, 0]).is_empty());
    }

    #[test]
    fn e12_02_ghost_blend_halfway() {
        let prev = [[100u8, 100, 100, 255]];
        let cur = [[200u8, 200, 200, 255]];
        assert_eq!(ghost_blend(&prev, &cur, 128)[0], [150, 150, 150, 255]);
    }

    #[test]
    fn e12_02_ghost_blend_zero_persistence_is_identity() {
        let prev = [[100u8, 100, 100, 255]];
        let cur = [[200u8, 200, 200, 255]];
        assert_eq!(ghost_blend(&prev, &cur, 0), cur.to_vec());
    }

    #[test]
    fn e12_02_ghost_blend_mismatched_length_returns_cur() {
        let cur = [[200u8, 200, 200, 255], [10, 20, 30, 255]];
        assert_eq!(ghost_blend(&[], &cur, 128), cur.to_vec());
        assert_eq!(ghost_blend(&[[0u8, 0, 0, 255]], &cur, 128), cur.to_vec());
    }

    #[test]
    fn e12_02_ghost_blend_keeps_alpha_and_clamps_persistence() {
        let prev = [[100u8, 100, 100, 7]];
        let cur = [[200u8, 200, 200, 255]];
        let out = ghost_blend(&prev, &cur, 128);
        assert_eq!(out[0][3], 255);
        assert_eq!(
            ghost_blend(&prev, &cur, 1000),
            ghost_blend(&prev, &cur, 255)
        );
        assert_eq!(ghost_blend(&prev, &cur, 255)[0][0], 100);
    }

    const FRAME_A: [[u8; 4]; 4] = [
        [10, 20, 30, 255],
        [40, 50, 60, 255],
        [70, 80, 90, 255],
        [100, 110, 120, 255],
    ];
    const FRAME_B: [[u8; 4]; 4] = [[200, 210, 220, 255]; 4];

    #[test]
    fn e12_03_new_filter_defaults() {
        let filter = LcdFilter::new();
        assert_eq!(filter.mode(), LcdMode::Off);
        assert_eq!(filter.strength, 160);
        assert_eq!(filter.persistence, 96);
        assert_eq!(filter.gap, [0, 0, 0]);
    }

    #[test]
    fn e12_03_off_gives_none() {
        let mut filter = LcdFilter::new();
        assert_eq!(filter.process(&FRAME_A, 2, 2, 3), None);
    }

    #[test]
    fn e12_03_grid_on_two_by_two_gives_thirty_six_pixels() {
        let mut filter = LcdFilter::new();
        filter.set_mode(LcdMode::Grid);
        let out = filter.process(&FRAME_A, 2, 2, 3).expect("grid output");
        assert_eq!(out.len(), 36);
        assert_eq!(out, grid_filter(&FRAME_A, 2, 2, 3, 160, [0, 0, 0]));
    }

    #[test]
    fn e12_03_ghost_first_frame_equals_grid() {
        let mut ghost = LcdFilter::new();
        ghost.set_mode(LcdMode::Ghost);
        let mut grid = LcdFilter::new();
        grid.set_mode(LcdMode::Grid);
        assert_eq!(
            ghost.process(&FRAME_A, 2, 2, 3),
            grid.process(&FRAME_A, 2, 2, 3)
        );
    }

    #[test]
    fn e12_03_ghost_second_frame_differs_from_grid() {
        let mut ghost = LcdFilter::new();
        ghost.set_mode(LcdMode::Ghost);
        let _ = ghost.process(&FRAME_A, 2, 2, 3);
        let second = ghost.process(&FRAME_B, 2, 2, 3).expect("ghost output");
        let plain = grid_filter(&FRAME_B, 2, 2, 3, 160, [0, 0, 0]);
        assert_eq!(second.len(), plain.len());
        assert_ne!(second, plain);
    }

    #[test]
    fn e12_03_set_mode_clears_the_previous_frame() {
        let mut filter = LcdFilter::new();
        filter.set_mode(LcdMode::Ghost);
        let _ = filter.process(&FRAME_A, 2, 2, 3);
        filter.set_mode(LcdMode::Grid);
        filter.set_mode(LcdMode::Ghost);
        let out = filter.process(&FRAME_B, 2, 2, 3).expect("ghost output");
        assert_eq!(out, grid_filter(&FRAME_B, 2, 2, 3, 160, [0, 0, 0]));
    }

    #[test]
    fn e12_03_mode_text_round_trip() {
        for mode in [LcdMode::Off, LcdMode::Grid, LcdMode::Ghost] {
            assert_eq!(LcdMode::from_str(mode.as_str()), mode);
        }
        assert_eq!(LcdMode::from_str("GRID"), LcdMode::Grid);
        assert_eq!(LcdMode::from_str(" Ghost "), LcdMode::Ghost);
        assert_eq!(LcdMode::from_str("???"), LcdMode::Off);
        assert_eq!(LcdMode::default(), LcdMode::Off);
    }

    #[test]
    fn e12_04_gap_for_uses_the_darkest_color() {
        assert_eq!(
            gap_for(Some([0xFFFFFF, 0xAAAAAA, 0x555555, 0x102030])),
            [0x10, 0x20, 0x30]
        );
    }

    #[test]
    fn e12_04_gap_for_none_is_black() {
        assert_eq!(gap_for(None), [0, 0, 0]);
    }
}
