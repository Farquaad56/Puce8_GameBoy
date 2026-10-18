//! Save state of the PPU (E09_02, decision A_05). Layout: frame (23040 x u32), line_buf (160 x
//! color, obj, pal1), dot_in_line (u32), ly, mode, first_line, mode_hist (4), mode_lag, lcd_on,
//! lcdc, line_sprites (10), line_sprite_count, oam_scan_pending, pixels_output, produce_count,
//! fine_scroll, fifo (16), fifo_head, fifo_count, fetcher_col, tile_low, tile_high, win_line,
//! win_drawn, sprites_rendered, line_resolved (160 flags), obj_penalty (u32), line_committed.
//! The framebuffer does not influence future behavior but is saved so that a loaded state shows
//! the same image (UNKNOWN - to confirm). Values used as indexes are range checked.

use super::{PixelOut, Ppu, FIFO_CAP, LINES_PER_FRAME, LINE_DOTS, LINE_PIXELS};
use crate::machine::FRAMEBUFFER_SIZE;
use crate::savestate::{StateError, StateReader, StateWriter};

const FRAME_BYTES: usize = FRAMEBUFFER_SIZE * 4;
const LINE_BUF_BYTES: usize = LINE_PIXELS as usize * 3;
const OAM_ENTRIES: u8 = 40;

fn checked_bool(byte: u8) -> Result<bool, StateError> {
    match byte {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(StateError::InvalidState),
    }
}

fn index_byte(value: usize) -> Result<u8, StateError> {
    u8::try_from(value).map_err(|_| StateError::InvalidState)
}

impl Ppu {
    /// Writes every field listed in the module documentation.
    pub fn write_state(&self, w: &mut StateWriter<'_>) -> Result<(), StateError> {
        for pixel in &self.frame {
            w.put_u32(*pixel)?;
        }
        for pixel in &self.line_buf {
            w.put_u8(pixel.color)?;
            w.put_bool(pixel.obj)?;
            w.put_bool(pixel.pal1)?;
        }
        w.put_u32(self.dot_in_line)?;
        w.put_u8(self.ly)?;
        w.put_u8(self.mode)?;
        w.put_bool(self.first_line)?;
        w.put_bytes(&self.mode_hist)?;
        w.put_u8(self.mode_lag)?;
        w.put_bool(self.lcd_on)?;
        w.put_u8(self.lcdc)?;
        w.put_bytes(&self.line_sprites)?;
        w.put_u8(self.line_sprite_count)?;
        w.put_bool(self.oam_scan_pending)?;
        w.put_u8(self.pixels_output)?;
        w.put_u8(self.produce_count)?;
        w.put_u8(self.fine_scroll)?;
        w.put_bytes(&self.fifo)?;
        w.put_u8(index_byte(self.fifo_head)?)?;
        w.put_u8(index_byte(self.fifo_count)?)?;
        w.put_u8(self.fetcher_col)?;
        w.put_u8(self.tile_low)?;
        w.put_u8(self.tile_high)?;
        w.put_u8(self.win_line)?;
        w.put_bool(self.win_drawn)?;
        w.put_bool(self.sprites_rendered)?;
        for flag in &self.line_resolved {
            w.put_bool(*flag)?;
        }
        w.put_u32(self.obj_penalty)?;
        w.put_bool(self.line_committed)
    }

    /// Reads what `write_state` wrote; the PPU is left unchanged on error.
    pub fn read_state(&mut self, r: &mut StateReader<'_>) -> Result<(), StateError> {
        let mut frame = [0u32; FRAMEBUFFER_SIZE];
        let (frame_chunks, _rem) = r.get_bytes(FRAME_BYTES)?.as_chunks::<4>();
        for (pixel, bytes) in frame.iter_mut().zip(frame_chunks) {
            *pixel = u32::from_le_bytes(*bytes);
        }
        let mut line_buf = [PixelOut {
            color: 0,
            obj: false,
            pal1: false,
        }; LINE_PIXELS as usize];
        let (line_chunks, _rem) = r.get_bytes(LINE_BUF_BYTES)?.as_chunks::<3>();
        for (pixel, bytes) in line_buf.iter_mut().zip(line_chunks) {
            if bytes[0] > 3 {
                return Err(StateError::InvalidState);
            }
            *pixel = PixelOut {
                color: bytes[0],
                obj: checked_bool(bytes[1])?,
                pal1: checked_bool(bytes[2])?,
            };
        }
        let dot_in_line = r.get_u32()?;
        if dot_in_line >= LINE_DOTS {
            return Err(StateError::InvalidState);
        }
        let ly = r.get_u8()?;
        if ly >= LINES_PER_FRAME {
            return Err(StateError::InvalidState);
        }
        let mode = r.get_u8()?;
        if mode > 3 {
            return Err(StateError::InvalidState);
        }
        let first_line = r.get_bool()?;
        let mut mode_hist = [0u8; 4];
        mode_hist.copy_from_slice(r.get_bytes(4)?);
        let mode_lag = r.get_u8()?;
        let lcd_on = r.get_bool()?;
        let lcdc = r.get_u8()?;
        let mut line_sprites = [0u8; 10];
        line_sprites.copy_from_slice(r.get_bytes(10)?);
        if line_sprites.iter().any(|&index| index >= OAM_ENTRIES) {
            return Err(StateError::InvalidState);
        }
        let line_sprite_count = r.get_u8()?;
        if line_sprite_count > 10 {
            return Err(StateError::InvalidState);
        }
        let oam_scan_pending = r.get_bool()?;
        let pixels_output = r.get_u8()?;
        if pixels_output > LINE_PIXELS {
            return Err(StateError::InvalidState);
        }
        let produce_count = r.get_u8()?;
        let fine_scroll = r.get_u8()?;
        if fine_scroll > 7 {
            return Err(StateError::InvalidState);
        }
        let mut fifo = [0u8; FIFO_CAP];
        fifo.copy_from_slice(r.get_bytes(FIFO_CAP)?);
        let fifo_head = usize::from(r.get_u8()?);
        if fifo_head >= FIFO_CAP {
            return Err(StateError::InvalidState);
        }
        let fifo_count = usize::from(r.get_u8()?);
        if fifo_count > FIFO_CAP {
            return Err(StateError::InvalidState);
        }
        let fetcher_col = r.get_u8()?;
        let tile_low = r.get_u8()?;
        let tile_high = r.get_u8()?;
        let win_line = r.get_u8()?;
        let win_drawn = r.get_bool()?;
        let sprites_rendered = r.get_bool()?;
        let mut line_resolved = [false; LINE_PIXELS as usize];
        for (flag, byte) in line_resolved
            .iter_mut()
            .zip(r.get_bytes(LINE_PIXELS as usize)?)
        {
            *flag = checked_bool(*byte)?;
        }
        let obj_penalty = r.get_u32()?;
        let line_committed = r.get_bool()?;

        self.frame = frame;
        self.line_buf = line_buf;
        self.dot_in_line = dot_in_line;
        self.ly = ly;
        self.mode = mode;
        self.first_line = first_line;
        self.mode_hist = mode_hist;
        self.mode_lag = mode_lag;
        self.lcd_on = lcd_on;
        self.lcdc = lcdc;
        self.line_sprites = line_sprites;
        self.line_sprite_count = line_sprite_count;
        self.oam_scan_pending = oam_scan_pending;
        self.pixels_output = pixels_output;
        self.produce_count = produce_count;
        self.fine_scroll = fine_scroll;
        self.fifo = fifo;
        self.fifo_head = fifo_head;
        self.fifo_count = fifo_count;
        self.fetcher_col = fetcher_col;
        self.tile_low = tile_low;
        self.tile_high = tile_high;
        self.win_line = win_line;
        self.win_drawn = win_drawn;
        self.sprites_rendered = sprites_rendered;
        self.line_resolved = line_resolved;
        self.obj_penalty = obj_penalty;
        self.line_committed = line_committed;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Offset of dot_in_line: frame (92160) + line_buf (480).
    const T: usize = 92_640;

    /// Bytes from dot_in_line to sprites_rendered of `sample()`.
    const TAIL_HEAD: [u8; 53] = [
        0x02, 0x01, 0x00, 0x00, 0x90, 0x03, 0x01, 0x00, 0x01, 0x02, 0x03, 0x01, 0x01, 0x91, 0x00,
        0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x27, 0x0A, 0x00, 0x64, 0x66, 0x05, 0x01,
        0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0A, 0x0B, 0x0C, 0x0D, 0x0E, 0x0F, 0x10,
        0x03, 0x09, 0x1F, 0xA5, 0x5A, 0x07, 0x01, 0x00,
    ];

    #[allow(clippy::field_reassign_with_default)]
    fn sample() -> Ppu {
        let mut p = Ppu::default();
        p.frame[0] = 0x1122_3344;
        p.frame[FRAMEBUFFER_SIZE - 1] = 0xAABB_CCDD;
        p.line_buf[0] = PixelOut {
            color: 3,
            obj: true,
            pal1: false,
        };
        p.line_buf[159] = PixelOut {
            color: 1,
            obj: false,
            pal1: true,
        };
        p.dot_in_line = 258;
        p.ly = 0x90;
        p.mode = 3;
        p.first_line = true;
        p.mode_hist = [0, 1, 2, 3];
        p.mode_lag = 1;
        p.lcd_on = true;
        p.lcdc = 0x91;
        p.line_sprites = [0, 1, 2, 3, 4, 5, 6, 7, 8, 39];
        p.line_sprite_count = 10;
        p.oam_scan_pending = false;
        p.pixels_output = 100;
        p.produce_count = 102;
        p.fine_scroll = 5;
        p.fifo = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16];
        p.fifo_head = 3;
        p.fifo_count = 9;
        p.fetcher_col = 0x1F;
        p.tile_low = 0xA5;
        p.tile_high = 0x5A;
        p.win_line = 7;
        p.win_drawn = true;
        p.sprites_rendered = false;
        p.line_resolved = [false; 160];
        p.line_resolved[0] = true;
        p.line_resolved[159] = true;
        p.obj_penalty = 518;
        p.line_committed = true;
        p
    }

    fn saved(p: &Ppu) -> Vec<u8> {
        let mut buf = vec![0u8; 100_000];
        let mut w = StateWriter::new(&mut buf);
        p.write_state(&mut w).expect("room");
        let len = w.position();
        buf.truncate(len);
        buf
    }

    #[test]
    fn e09_02_ppu_layout_is_fixed() {
        let bytes = saved(&sample());
        assert_eq!(bytes.len(), 92_858);
        assert_eq!(&bytes[..4], &[0x44, 0x33, 0x22, 0x11]);
        assert_eq!(&bytes[92_156..92_160], &[0xDD, 0xCC, 0xBB, 0xAA]);
        assert_eq!(&bytes[92_160..92_163], &[3, 1, 0]);
        assert_eq!(&bytes[T - 3..T], &[1, 0, 1]);
        assert_eq!(&bytes[T..T + 53], &TAIL_HEAD);
        assert_eq!(bytes[T + 53], 1);
        assert_eq!(bytes[T + 54], 0);
        assert_eq!(bytes[T + 212], 1);
        assert_eq!(&bytes[T + 213..T + 217], &[0x06, 0x02, 0x00, 0x00]);
        assert_eq!(bytes[T + 217], 1);
    }

    #[test]
    fn e09_02_ppu_round_trip_is_byte_identical() {
        let bytes = saved(&sample());
        let mut loaded = Ppu::default();
        let mut r = StateReader::new(&bytes);
        loaded.read_state(&mut r).expect("valid");
        assert_eq!(r.remaining(), 0);
        assert_eq!(loaded.frame[0], 0x1122_3344);
        assert_eq!(loaded.ly, 0x90);
        assert_eq!(loaded.fifo_head, 3);
        assert_eq!(loaded.fifo_count, 9);
        assert_eq!(loaded.line_buf[159].color, 1);
        assert!(loaded.line_resolved[159]);
        assert_eq!(loaded.obj_penalty, 518);
        assert_eq!(saved(&loaded), bytes);
    }

    #[test]
    fn e09_02_ppu_truncated_data_is_bad_size_and_leaves_ppu_unchanged() {
        let bytes = saved(&sample());
        let last = bytes.len() - 1;
        for len in [
            0usize,
            1,
            3,
            92_159,
            92_160,
            92_639,
            T,
            T + 1,
            T + 53,
            T + 216,
            last,
        ] {
            let mut p = sample();
            assert_eq!(
                p.read_state(&mut StateReader::new(&bytes[..len])),
                Err(StateError::BadSize),
                "len {len}"
            );
            assert_eq!(saved(&p), bytes);
        }
    }

    #[test]
    fn e09_02_ppu_corrupted_flag_is_invalid_state() {
        let before = saved(&sample());
        for index in [
            92_161usize,
            92_162,
            T + 6,
            T + 12,
            T + 25,
            T + 51,
            T + 52,
            T + 53,
            T + 217,
        ] {
            let mut bytes = before.clone();
            bytes[index] = 2;
            let mut p = sample();
            assert_eq!(
                p.read_state(&mut StateReader::new(&bytes)),
                Err(StateError::InvalidState),
                "index {index}"
            );
            assert_eq!(saved(&p), before);
        }
    }

    #[test]
    fn e09_02_ppu_out_of_range_value_is_invalid_state() {
        let before = saved(&sample());
        let cases: [&[(usize, u8)]; 10] = [
            &[(92_160, 4)],
            &[(T, 0xC8), (T + 1, 0x01)],
            &[(T + 4, 154)],
            &[(T + 5, 4)],
            &[(T + 14, 40)],
            &[(T + 24, 11)],
            &[(T + 26, 161)],
            &[(T + 28, 8)],
            &[(T + 45, 16)],
            &[(T + 46, 17)],
        ];
        for case in cases {
            let mut bytes = before.clone();
            for &(index, value) in case {
                bytes[index] = value;
            }
            let mut p = sample();
            assert_eq!(
                p.read_state(&mut StateReader::new(&bytes)),
                Err(StateError::InvalidState),
                "case {case:?}"
            );
            assert_eq!(saved(&p), before);
        }
    }

    #[test]
    fn e09_02_ppu_write_to_small_buffer_fails() {
        let mut buf = vec![0u8; 92_857];
        let mut w = StateWriter::new(&mut buf);
        assert_eq!(
            sample().write_state(&mut w),
            Err(StateError::BufferTooSmall)
        );
    }
}
