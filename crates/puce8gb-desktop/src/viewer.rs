//! Tile and background viewer: pure helpers and snapshot capture (E08_02).
//! Only the core snapshot API is used (Dmg::debug_tile, Dmg::debug_palettes, Dmg::peek).

use eframe::egui;
use puce8gb_core::debug::{OamEntry, Palettes, OAM_ENTRIES, TILE_COUNT};
use puce8gb_core::Dmg;

/// Tile sheet layout: 16 tiles per row, 24 rows.
pub const SHEET_COLS: usize = 16;
pub const SHEET_ROWS: usize = TILE_COUNT / SHEET_COLS;
pub const SHEET_W: usize = SHEET_COLS * 8;
pub const SHEET_H: usize = SHEET_ROWS * 8;
/// Tile map: 32 x 32 entries, 256 x 256 pixels.
pub const MAP_TILES: usize = 32;
pub const MAP_PX: usize = MAP_TILES * 8;
pub const MAP0_BASE: u16 = 0x9800;
pub const MAP1_BASE: u16 = 0x9C00;
/// Display scale of the tile sheet and of each tile map in the viewer window.
pub const SHEET_SCALE: usize = 3;
pub const MAP_SCALE: usize = 2;

/// Copy of what the viewer needs, built on the emulation thread.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Snapshot {
    /// TILE_COUNT * 64 color indices 0..3, tile after tile.
    pub tiles: Vec<u8>,
    /// The two 32 x 32 tile maps ($9800 and $9C00), 1024 raw entries each.
    pub maps: [Vec<u8>; 2],
    /// LCDC ($FF40), used for the tile data addressing mode (bit 4).
    pub lcdc: u8,
    /// BGP ($FF47) decoded: shade index of color 0..3.
    pub bgp: [u8; 4],
    /// The 40 OAM entries as stored.
    pub oam: Vec<OamEntry>,
    /// BGP, OBP0 and OBP1 decoded.
    pub palettes: Palettes,
    /// All 65536 addresses read with Dmg::peek, index = address.
    pub memory: Vec<u8>,
}

/// Pixel position (x, y) of the top-left corner of tile `index` in a sheet of `cols` tiles per row.
pub fn tile_origin(index: usize, cols: usize) -> (usize, usize) {
    if cols == 0 {
        return (0, 0);
    }
    ((index % cols) * 8, (index / cols) * 8)
}

/// Tile number 0..384 selected by a tile map entry. LCDC bit 4 set: unsigned index into the
/// $8000 area (tiles 0..255). Bit 4 clear: signed index relative to $9000 (tiles 128..383).
pub fn map_tile_index(entry: u8, lcdc: u8) -> usize {
    if lcdc & 0x10 != 0 {
        usize::from(entry)
    } else {
        (256i32 + i32::from(entry as i8)) as usize
    }
}

/// Gray level shown for a shade index 0..3 (0 = lightest). A display choice of the viewer.
pub fn shade_gray(shade: u8) -> u8 {
    match shade & 3 {
        0 => 0xFF,
        1 => 0xAA,
        2 => 0x55,
        _ => 0x00,
    }
}

/// Builds a snapshot from the core snapshot API.
pub fn capture(dmg: &Dmg) -> Snapshot {
    let mut tiles = vec![0u8; TILE_COUNT * 64];
    let mut buf = [0u8; 64];
    for (index, chunk) in tiles.chunks_mut(64).enumerate() {
        if dmg.debug_tile(index, &mut buf) {
            chunk.copy_from_slice(&buf);
        }
    }
    let mut map0 = vec![0u8; MAP_TILES * MAP_TILES];
    let mut map1 = vec![0u8; MAP_TILES * MAP_TILES];
    for (i, (a, b)) in map0.iter_mut().zip(map1.iter_mut()).enumerate() {
        *a = dmg.peek(MAP0_BASE + i as u16);
        *b = dmg.peek(MAP1_BASE + i as u16);
    }
    let mut oam = [OamEntry::default(); OAM_ENTRIES];
    dmg.debug_oam(&mut oam);
    let memory: Vec<u8> = (0..=0xFFFFu16).map(|addr| dmg.peek(addr)).collect();
    Snapshot {
        tiles,
        maps: [map0, map1],
        lcdc: dmg.peek(0xFF40),
        bgp: dmg.debug_palettes().bgp,
        oam: oam.to_vec(),
        palettes: dmg.debug_palettes(),
        memory,
    }
}

fn blit_tile(out: &mut [u8], width: usize, origin: (usize, usize), tile: &[u8], bgp: &[u8; 4]) {
    for (i, &color) in tile.iter().enumerate() {
        let x = origin.0 + i % 8;
        let y = origin.1 + i / 8;
        out[y * width + x] = bgp[usize::from(color & 3)];
    }
}

/// Shade index (after BGP) of every pixel of the 384-tile sheet, SHEET_W * SHEET_H values.
pub fn sheet_shades(snap: &Snapshot) -> Vec<u8> {
    let mut out = vec![0u8; SHEET_W * SHEET_H];
    for index in 0..TILE_COUNT {
        let origin = tile_origin(index, SHEET_COLS);
        let tile = &snap.tiles[index * 64..index * 64 + 64];
        blit_tile(&mut out, SHEET_W, origin, tile, &snap.bgp);
    }
    out
}

/// Shade index (after BGP) of every pixel of tile map `which` (0 or 1), MAP_PX * MAP_PX values.
pub fn map_shades(snap: &Snapshot, which: usize) -> Vec<u8> {
    let mut out = vec![0u8; MAP_PX * MAP_PX];
    let map = &snap.maps[which.min(1)];
    for (entry_index, &entry) in map.iter().enumerate() {
        let tile = map_tile_index(entry, snap.lcdc);
        let origin = tile_origin(entry_index, MAP_TILES);
        let data = &snap.tiles[tile * 64..tile * 64 + 64];
        blit_tile(&mut out, MAP_PX, origin, data, &snap.bgp);
    }
    out
}

/// Converts shade indices to an egui gray image.
pub fn to_color_image(shades: &[u8], width: usize, height: usize) -> egui::ColorImage {
    let grays: Vec<u8> = shades.iter().map(|&s| shade_gray(s)).collect();
    egui::ColorImage::from_gray([width, height], &grays)
}

/// Updates `slot` in place with `.set()`; the texture is created only the first time.
pub fn upload(
    ctx: &egui::Context,
    slot: &mut Option<egui::TextureHandle>,
    name: &str,
    image: egui::ColorImage,
) {
    if let Some(texture) = slot {
        texture.set(image, egui::TextureOptions::NEAREST);
    } else {
        *slot = Some(ctx.load_texture(name, image, egui::TextureOptions::NEAREST));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const KNOWN_TILE: [u8; 16] = [0x3C, 0x7E, 0x42, 0x42, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];

    fn test_dmg() -> Dmg {
        let mut dmg = Dmg::new(&[0u8; 0x8000]).unwrap();
        dmg.bus.vram[16..32].copy_from_slice(&KNOWN_TILE); // tile 1
        dmg.bus.vram[0x1800] = 1; // map 0, entry 0 -> tile 1
        dmg.bus.io[0x40] = 0x10; // LCDC bit 4 set: unsigned tile indices
        dmg.bus.io[0x47] = 0xE4; // BGP identity
        dmg
    }

    #[test]
    fn e08_02_tile_origin_positions() {
        assert_eq!(tile_origin(0, 16), (0, 0));
        assert_eq!(tile_origin(1, 16), (8, 0));
        assert_eq!(tile_origin(15, 16), (120, 0));
        assert_eq!(tile_origin(16, 16), (0, 8));
        assert_eq!(tile_origin(383, 16), (120, 184));
        assert_eq!(tile_origin(33, 32), (8, 8));
        assert_eq!(tile_origin(5, 0), (0, 0));
    }

    #[test]
    fn e08_02_map_tile_index_modes() {
        assert_eq!(map_tile_index(0, 0x10), 0);
        assert_eq!(map_tile_index(255, 0x10), 255);
        assert_eq!(map_tile_index(0, 0x00), 256);
        assert_eq!(map_tile_index(127, 0x00), 383);
        assert_eq!(map_tile_index(128, 0x00), 128);
        assert_eq!(map_tile_index(255, 0x00), 255);
    }

    #[test]
    fn e08_02_capture_reads_snapshot_api() {
        let snap = capture(&test_dmg());
        assert_eq!(snap.tiles.len(), TILE_COUNT * 64);
        assert_eq!(snap.tiles[64..72], [0, 2, 3, 3, 3, 3, 2, 0]);
        assert_eq!(snap.maps[0].len(), 1024);
        assert_eq!(snap.maps[0][0], 1);
        assert_eq!(snap.maps[1][0], 0);
        assert_eq!(snap.lcdc, 0x10);
        assert_eq!(snap.bgp, [0, 1, 2, 3]);
    }

    #[test]
    fn e08_02_sheet_places_tiles_and_applies_bgp() {
        let sheet = sheet_shades(&capture(&test_dmg()));
        assert_eq!(sheet.len(), SHEET_W * SHEET_H);
        assert_eq!(sheet[8..16], [0, 2, 3, 3, 3, 3, 2, 0]);
        assert_eq!(sheet[SHEET_W + 8..SHEET_W + 16], [0, 3, 0, 0, 0, 0, 3, 0]);
        let mut dmg = test_dmg();
        dmg.bus.io[0x47] = 0x1B; // BGP reversed: shades [3, 2, 1, 0]
        let reversed = sheet_shades(&capture(&dmg));
        assert_eq!(reversed[8..16], [3, 1, 0, 0, 0, 0, 1, 3]);
    }

    #[test]
    fn e08_02_map_uses_tile_of_each_entry() {
        let map = map_shades(&capture(&test_dmg()), 0);
        assert_eq!(map.len(), MAP_PX * MAP_PX);
        assert_eq!(map[0..8], [0, 2, 3, 3, 3, 3, 2, 0]);
        assert_eq!(map[8..16], [0, 0, 0, 0, 0, 0, 0, 0]);
        assert_eq!(map[MAP_PX..MAP_PX + 8], [0, 3, 0, 0, 0, 0, 3, 0]);
    }

    #[test]
    fn e08_02_to_color_image_maps_shades() {
        let image = to_color_image(&[0, 1, 2, 3], 2, 2);
        assert_eq!(image.pixels.len(), 4);
        assert_eq!(image.pixels[0], egui::Color32::from_gray(0xFF));
        assert_eq!(image.pixels[1], egui::Color32::from_gray(0xAA));
        assert_eq!(image.pixels[2], egui::Color32::from_gray(0x55));
        assert_eq!(image.pixels[3], egui::Color32::from_gray(0x00));
    }

    #[test]
    fn e08_03_capture_includes_oam_palettes_and_memory() {
        let mut dmg = test_dmg();
        dmg.bus.oam[0..4].copy_from_slice(&[16, 8, 0x42, 0xA0]);
        dmg.bus.io[0x48] = 0x1B;
        dmg.bus.vram[0] = 0x77;
        dmg.bus.wram[0] = 0x5A;
        let snap = capture(&dmg);
        assert_eq!(snap.oam.len(), 40);
        assert_eq!(
            snap.oam[0],
            OamEntry {
                y: 16,
                x: 8,
                tile: 0x42,
                flags: 0xA0
            }
        );
        assert_eq!(snap.palettes.obp0, [3, 2, 1, 0]);
        assert_eq!(snap.memory.len(), 0x10000);
        assert_eq!(snap.memory[0x8000], 0x77);
        assert_eq!(snap.memory[0xC000], 0x5A);
        assert_eq!(snap.memory[0xE000], 0x5A);
        assert_eq!(snap.memory[0xFF48], 0x1B);
    }
}
