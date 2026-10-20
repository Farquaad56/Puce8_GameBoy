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
}
