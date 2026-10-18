//! Read-only debug snapshots for frontends (E08_01). Nothing here changes machine state and
//! nothing allocates: every snapshot fills a caller-provided buffer or returns a plain copy.
//! Memory is read raw from the bus arrays, bypassing PPU-mode and OAM DMA gates (decision A_02).
use crate::machine::Dmg;

/// Tiles in VRAM $8000-$97FF (3 blocks of 128).
pub const TILE_COUNT: usize = 384;
/// Bytes per tile: 8 rows of 2 bitplane bytes.
pub const TILE_BYTES: usize = 16;
/// OAM entries at $FE00-$FE9F, 4 bytes each.
pub const OAM_ENTRIES: usize = 40;

/// Copy of the CPU register file.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CpuRegs {
    pub a: u8,
    pub f: u8,
    pub b: u8,
    pub c: u8,
    pub d: u8,
    pub e: u8,
    pub h: u8,
    pub l: u8,
    pub sp: u16,
    pub pc: u16,
}

/// One OAM entry as stored: raw bytes in memory order Y, X, tile, flags.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct OamEntry {
    pub y: u8,
    pub x: u8,
    pub tile: u8,
    pub flags: u8,
}

/// Decoded DMG palettes: for each color index 0..3 the shade index 0..3 (0 = lightest).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Palettes {
    pub bgp: [u8; 4],
    pub obp0: [u8; 4],
    pub obp1: [u8; 4],
}

/// Decodes one tile (16 bytes) into 64 color indices 0..3, row by row, leftmost pixel first.
/// Returns false and leaves `out` untouched when fewer than 16 bytes are given.
pub fn decode_tile(bytes: &[u8], out: &mut [u8; 64]) -> bool {
    if bytes.len() < TILE_BYTES {
        return false;
    }
    for row in 0..8 {
        let lo = bytes[row * 2];
        let hi = bytes[row * 2 + 1];
        for x in 0..8 {
            let bit = 7 - x;
            out[row * 8 + x] = (((hi >> bit) & 1) << 1) | ((lo >> bit) & 1);
        }
    }
    true
}

/// Splits a palette register (BGP, OBP0, OBP1) into the shade index of color 0..3.
pub fn decode_palette(reg: u8) -> [u8; 4] {
    [reg & 3, (reg >> 2) & 3, (reg >> 4) & 3, (reg >> 6) & 3]
}

impl Dmg {
    /// Raw read with no side effect and no gate (decision A_02).
    pub fn peek(&self, addr: u16) -> u8 {
        self.bus.peek(addr)
    }

    /// Copy of the CPU registers.
    pub fn debug_regs(&self) -> CpuRegs {
        CpuRegs {
            a: self.cpu.a,
            f: self.cpu.f,
            b: self.cpu.b,
            c: self.cpu.c,
            d: self.cpu.d,
            e: self.cpu.e,
            h: self.cpu.h,
            l: self.cpu.l,
            sp: self.cpu.sp,
            pc: self.cpu.pc,
        }
    }

    /// Decodes VRAM tile `index` (0..384, tile data at $8000 + index * 16) into `out`.
    /// Returns false when `index` is out of range.
    pub fn debug_tile(&self, index: usize, out: &mut [u8; 64]) -> bool {
        if index >= TILE_COUNT {
            return false;
        }
        let base = index * TILE_BYTES;
        decode_tile(&self.bus.vram[base..base + TILE_BYTES], out)
    }

    /// Current BGP ($FF47), OBP0 ($FF48) and OBP1 ($FF49) decoded to shade indices.
    pub fn debug_palettes(&self) -> Palettes {
        Palettes {
            bgp: decode_palette(self.peek(0xFF47)),
            obp0: decode_palette(self.peek(0xFF48)),
            obp1: decode_palette(self.peek(0xFF49)),
        }
    }

    /// Fills `out` with the 40 OAM entries.
    pub fn debug_oam(&self, out: &mut [OamEntry; OAM_ENTRIES]) {
        for (i, entry) in out.iter_mut().enumerate() {
            let base = i * 4;
            *entry = OamEntry {
                y: self.bus.oam[base],
                x: self.bus.oam[base + 1],
                tile: self.bus.oam[base + 2],
                flags: self.bus.oam[base + 3],
            };
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::machine::Machine;

    fn test_dmg() -> Dmg {
        Dmg::new(&[0u8; 0x8000]).unwrap()
    }

    const KNOWN_TILE: [u8; 16] = [0x3C, 0x7E, 0x42, 0x42, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];

    #[test]
    fn e08_01_decode_tile_known_rows() {
        let mut out = [9u8; 64];
        assert!(decode_tile(&KNOWN_TILE, &mut out));
        assert_eq!(out[0..8], [0, 2, 3, 3, 3, 3, 2, 0]);
        assert_eq!(out[8..16], [0, 3, 0, 0, 0, 0, 3, 0]);
        assert!(out[16..64].iter().all(|&p| p == 0));
    }

    #[test]
    fn e08_01_decode_tile_bit_planes_and_short_input() {
        let mut bytes = [0u8; 16];
        bytes[0] = 0x80; // row 0: low plane only, leftmost pixel
        bytes[3] = 0x80; // row 1: high plane only, leftmost pixel
        bytes[4] = 0x01; // row 2: both planes, rightmost pixel
        bytes[5] = 0x01;
        let mut out = [0u8; 64];
        assert!(decode_tile(&bytes, &mut out));
        assert_eq!(out[0], 1);
        assert_eq!(out[1], 0);
        assert_eq!(out[8], 2);
        assert_eq!(out[23], 3);
        let mut untouched = [7u8; 64];
        assert!(!decode_tile(&bytes[..15], &mut untouched));
        assert!(untouched.iter().all(|&p| p == 7));
    }

    #[test]
    fn e08_01_decode_palette_values() {
        assert_eq!(decode_palette(0xE4), [0, 1, 2, 3]);
        assert_eq!(decode_palette(0x1B), [3, 2, 1, 0]);
        assert_eq!(decode_palette(0xFC), [0, 3, 3, 3]);
        assert_eq!(decode_palette(0x00), [0, 0, 0, 0]);
    }

    #[test]
    fn e08_01_debug_tile_reads_vram() {
        let mut dmg = test_dmg();
        dmg.bus.vram[16..32].copy_from_slice(&KNOWN_TILE);
        let mut out = [0u8; 64];
        assert!(dmg.debug_tile(1, &mut out));
        assert_eq!(out[0..8], [0, 2, 3, 3, 3, 3, 2, 0]);
        assert!(dmg.debug_tile(TILE_COUNT - 1, &mut out));
        assert!(!dmg.debug_tile(TILE_COUNT, &mut out));
    }

    #[test]
    fn e08_01_debug_palettes_and_oam() {
        let mut dmg = test_dmg();
        dmg.bus.io[0x47] = 0xE4;
        dmg.bus.io[0x48] = 0x1B;
        dmg.bus.io[0x49] = 0xFC;
        let pal = dmg.debug_palettes();
        assert_eq!(pal.bgp, [0, 1, 2, 3]);
        assert_eq!(pal.obp0, [3, 2, 1, 0]);
        assert_eq!(pal.obp1, [0, 3, 3, 3]);
        dmg.bus.oam[0..4].copy_from_slice(&[16, 8, 0x42, 0xA0]);
        dmg.bus.oam[156..160].copy_from_slice(&[1, 2, 3, 4]);
        let mut entries = [OamEntry::default(); OAM_ENTRIES];
        dmg.debug_oam(&mut entries);
        assert_eq!(
            entries[0],
            OamEntry {
                y: 16,
                x: 8,
                tile: 0x42,
                flags: 0xA0
            }
        );
        assert_eq!(
            entries[39],
            OamEntry {
                y: 1,
                x: 2,
                tile: 3,
                flags: 4
            }
        );
    }

    #[test]
    fn e08_01_debug_regs_is_a_copy_of_the_cpu() {
        let mut dmg = test_dmg();
        dmg.cpu.a = 0x12;
        dmg.cpu.f = 0xB0;
        dmg.cpu.b = 0x34;
        dmg.cpu.c = 0x56;
        dmg.cpu.d = 0x78;
        dmg.cpu.e = 0x9A;
        dmg.cpu.h = 0xBC;
        dmg.cpu.l = 0xDE;
        dmg.cpu.sp = 0xCAFE;
        dmg.cpu.pc = 0x1234;
        let regs = dmg.debug_regs();
        assert_eq!(
            regs,
            CpuRegs {
                a: 0x12,
                f: 0xB0,
                b: 0x34,
                c: 0x56,
                d: 0x78,
                e: 0x9A,
                h: 0xBC,
                l: 0xDE,
                sp: 0xCAFE,
                pc: 0x1234
            }
        );
        dmg.cpu.a = 0x00;
        assert_eq!(regs.a, 0x12);
    }

    #[test]
    fn e08_01_peek_has_no_side_effects() {
        let mut dmg = test_dmg();
        dmg.run_frame();
        let before = format!("{dmg:?}");
        let live = [0xFF00u16, 0xFF04, 0xFF05, 0xFF0F, 0xFF41, 0xFF44];
        let first: Vec<u8> = live.iter().map(|&a| dmg.peek(a)).collect();
        for addr in 0..=0xFFFFu16 {
            assert_eq!(dmg.peek(addr), dmg.bus.peek(addr));
        }
        let second: Vec<u8> = live.iter().map(|&a| dmg.peek(a)).collect();
        assert_eq!(first, second);
        assert_eq!(before, format!("{dmg:?}"));
    }
}
