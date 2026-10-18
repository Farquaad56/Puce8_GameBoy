//! Save state of the Bus (E09_02, decision A_05). Layout: vram (8192), wram (8192), oam (256),
//! hram (127), io (256), serial (256), serial_len (u16), ly, ppu_mode, lyc_frozen (0 None,
//! 1 Some(false), 2 Some(true)), lyc_flag_hold, vram_open, oam_open, vram_write_ok,
//! oam_write_ok, dma_active, dma_src (u16), dma_index, dma_pending_src (u16), dma_start_delay,
//! apu_write (flag, u16 address, value), wave_access (tag 0 Free, 1 Closed, 2 Open + index),
//! ly_override (flag, value), p1_prev, timer (8 bytes), input (1 byte), then the mapper.
//! The ROM image is not saved. Everything is read into locals first, so an error leaves the
//! bus unchanged.

use super::Bus;
use crate::audio::wave::WaveAccess;
use crate::input::Input;
use crate::savestate::{StateError, StateReader, StateWriter};
use crate::timer::Timer;

const DMA_BYTES: u8 = 160;

impl Bus {
    /// Writes every field listed in the module documentation.
    pub fn write_state(&self, w: &mut StateWriter<'_>) -> Result<(), StateError> {
        w.put_bytes(&self.vram)?;
        w.put_bytes(&self.wram)?;
        w.put_bytes(&self.oam)?;
        w.put_bytes(&self.hram)?;
        w.put_bytes(&self.io)?;
        w.put_bytes(&self.serial)?;
        let serial_len = u16::try_from(self.serial_len).map_err(|_| StateError::InvalidState)?;
        w.put_u16(serial_len)?;
        w.put_u8(self.ly)?;
        w.put_u8(self.ppu_mode)?;
        w.put_u8(match self.lyc_frozen {
            None => 0,
            Some(false) => 1,
            Some(true) => 2,
        })?;
        w.put_bool(self.lyc_flag_hold)?;
        w.put_bool(self.vram_open)?;
        w.put_bool(self.oam_open)?;
        w.put_bool(self.vram_write_ok)?;
        w.put_bool(self.oam_write_ok)?;
        w.put_bool(self.dma_active)?;
        w.put_u16(self.dma_src)?;
        w.put_u8(self.dma_index)?;
        w.put_u16(self.dma_pending_src)?;
        w.put_u8(self.dma_start_delay)?;
        let (apu_present, apu_addr, apu_value) = match self.apu_write {
            Some((addr, value)) => (true, addr, value),
            None => (false, 0, 0),
        };
        w.put_bool(apu_present)?;
        w.put_u16(apu_addr)?;
        w.put_u8(apu_value)?;
        let (wave_tag, wave_index) = match self.wave_access {
            WaveAccess::Free => (0, 0),
            WaveAccess::Closed => (1, 0),
            WaveAccess::Open(index) => (2, index),
        };
        w.put_u8(wave_tag)?;
        w.put_u8(wave_index)?;
        let (override_present, override_value) = match self.ly_override {
            Some(value) => (true, value),
            None => (false, 0),
        };
        w.put_bool(override_present)?;
        w.put_u8(override_value)?;
        w.put_u8(self.p1_prev)?;
        self.timer.write_state(w)?;
        self.input.write_state(w)?;
        self.mapper.write_state(w)
    }

    /// Reads what `write_state` wrote. The mapper variant and RAM sizes must match the loaded
    /// cartridge (`RomMismatch`). The bus is left unchanged on error.
    pub fn read_state(&mut self, r: &mut StateReader<'_>) -> Result<(), StateError> {
        let mut vram = self.vram;
        let n = vram.len();
        vram.copy_from_slice(r.get_bytes(n)?);
        let mut wram = self.wram;
        let n = wram.len();
        wram.copy_from_slice(r.get_bytes(n)?);
        let mut oam = self.oam;
        let n = oam.len();
        oam.copy_from_slice(r.get_bytes(n)?);
        let mut hram = self.hram;
        let n = hram.len();
        hram.copy_from_slice(r.get_bytes(n)?);
        let mut io = self.io;
        let n = io.len();
        io.copy_from_slice(r.get_bytes(n)?);
        let mut serial = self.serial;
        let n = serial.len();
        serial.copy_from_slice(r.get_bytes(n)?);
        let serial_len = usize::from(r.get_u16()?);
        if serial_len > serial.len() {
            return Err(StateError::InvalidState);
        }
        let ly = r.get_u8()?;
        let ppu_mode = r.get_u8()?;
        if ppu_mode > 3 {
            return Err(StateError::InvalidState);
        }
        let lyc_frozen = match r.get_u8()? {
            0 => None,
            1 => Some(false),
            2 => Some(true),
            _ => return Err(StateError::InvalidState),
        };
        let lyc_flag_hold = r.get_bool()?;
        let vram_open = r.get_bool()?;
        let oam_open = r.get_bool()?;
        let vram_write_ok = r.get_bool()?;
        let oam_write_ok = r.get_bool()?;
        let dma_active = r.get_bool()?;
        let dma_src = r.get_u16()?;
        let dma_index = r.get_u8()?;
        if dma_index > DMA_BYTES || (dma_active && dma_index >= DMA_BYTES) {
            return Err(StateError::InvalidState);
        }
        let dma_pending_src = r.get_u16()?;
        let dma_start_delay = r.get_u8()?;
        let apu_present = r.get_bool()?;
        let apu_addr = r.get_u16()?;
        let apu_value = r.get_u8()?;
        let apu_write = if apu_present {
            Some((apu_addr, apu_value))
        } else {
            None
        };
        let wave_tag = r.get_u8()?;
        let wave_index = r.get_u8()?;
        let wave_access = match wave_tag {
            0 => WaveAccess::Free,
            1 => WaveAccess::Closed,
            2 if wave_index <= 15 => WaveAccess::Open(wave_index),
            _ => return Err(StateError::InvalidState),
        };
        let override_present = r.get_bool()?;
        let override_value = r.get_u8()?;
        let ly_override = if override_present {
            Some(override_value)
        } else {
            None
        };
        let p1_prev = r.get_u8()?;
        let mut timer = Timer::default();
        timer.read_state(r)?;
        let mut input = Input::default();
        input.read_state(r)?;
        self.mapper.read_state(r)?;

        self.vram = vram;
        self.wram = wram;
        self.oam = oam;
        self.hram = hram;
        self.io = io;
        self.serial = serial;
        self.serial_len = serial_len;
        self.ly = ly;
        self.ppu_mode = ppu_mode;
        self.lyc_frozen = lyc_frozen;
        self.lyc_flag_hold = lyc_flag_hold;
        self.vram_open = vram_open;
        self.oam_open = oam_open;
        self.vram_write_ok = vram_write_ok;
        self.oam_write_ok = oam_write_ok;
        self.dma_active = dma_active;
        self.dma_src = dma_src;
        self.dma_index = dma_index;
        self.dma_pending_src = dma_pending_src;
        self.dma_start_delay = dma_start_delay;
        self.apu_write = apu_write;
        self.wave_access = wave_access;
        self.ly_override = ly_override;
        self.p1_prev = p1_prev;
        self.timer = timer;
        self.input = input;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Offset of serial_len: 8192 + 8192 + 256 + 127 + 256 + 256.
    const S: usize = 17_279;
    /// Offset of the mapper tag byte: after the 8-byte timer and the input byte.
    const M: usize = 17_314;

    /// Bytes from serial_len to p1_prev of `sample()`.
    const MID: [u8; 26] = [
        0x03, 0x00, 0x11, 0x02, 0x02, 0x01, 0x00, 0x01, 0x00, 0x01, 0x01, 0x00, 0xC1, 0x05, 0x00,
        0xD2, 0x01, 0x01, 0x12, 0xFF, 0x34, 0x02, 0x07, 0x01, 0x42, 0x0E,
    ];

    fn sample() -> Bus {
        let mut b = Bus::new(vec![0u8; 0x8000]);
        b.vram[0] = 1;
        b.vram[8191] = 2;
        b.wram[0] = 3;
        b.wram[8191] = 4;
        b.oam[0] = 5;
        b.oam[255] = 6;
        b.hram[0] = 7;
        b.hram[126] = 8;
        b.io[0] = 9;
        b.io[255] = 10;
        b.serial[0] = 11;
        b.serial[255] = 12;
        b.serial_len = 3;
        b.ly = 17;
        b.ppu_mode = 2;
        b.lyc_frozen = Some(true);
        b.lyc_flag_hold = true;
        b.vram_open = false;
        b.oam_open = true;
        b.vram_write_ok = false;
        b.oam_write_ok = true;
        b.dma_active = true;
        b.dma_src = 0xC100;
        b.dma_index = 5;
        b.dma_pending_src = 0xD200;
        b.dma_start_delay = 1;
        b.apu_write = Some((0xFF12, 0x34));
        b.wave_access = WaveAccess::Open(7);
        b.ly_override = Some(0x42);
        b.p1_prev = 0x0E;
        b
    }

    fn saved(b: &Bus) -> Vec<u8> {
        let mut buf = vec![0u8; 20_000];
        let mut w = StateWriter::new(&mut buf);
        b.write_state(&mut w).expect("room");
        let len = w.position();
        buf.truncate(len);
        buf
    }

    #[test]
    fn e09_02_bus_layout_is_fixed() {
        let bytes = saved(&sample());
        assert_eq!(bytes.len(), 17_319);
        assert_eq!(bytes[0], 1);
        assert_eq!(bytes[8191], 2);
        assert_eq!(bytes[8192], 3);
        assert_eq!(bytes[16_383], 4);
        assert_eq!(bytes[16_384], 5);
        assert_eq!(bytes[16_639], 6);
        assert_eq!(bytes[16_640], 7);
        assert_eq!(bytes[16_766], 8);
        assert_eq!(bytes[16_767], 9);
        assert_eq!(bytes[17_022], 10);
        assert_eq!(bytes[17_023], 11);
        assert_eq!(bytes[17_278], 12);
        assert_eq!(&bytes[S..S + 26], &MID);
        assert_eq!(&bytes[M..], &[0, 0, 0, 0, 0]);
    }

    #[test]
    fn e09_02_bus_round_trip_is_byte_identical() {
        let bytes = saved(&sample());
        let mut loaded = Bus::new(vec![0u8; 0x8000]);
        let mut r = StateReader::new(&bytes);
        loaded.read_state(&mut r).expect("valid");
        assert_eq!(r.remaining(), 0);
        assert_eq!(loaded.ly, 17);
        assert_eq!(loaded.dma_src, 0xC100);
        assert_eq!(loaded.apu_write, Some((0xFF12, 0x34)));
        assert_eq!(loaded.wave_access, WaveAccess::Open(7));
        assert_eq!(loaded.ly_override, Some(0x42));
        assert_eq!(loaded.lyc_frozen, Some(true));
        assert_eq!(loaded.serial_len, 3);
        assert_eq!(saved(&loaded), bytes);
    }

    #[test]
    fn e09_02_bus_none_values_round_trip() {
        let mut b = sample();
        b.lyc_frozen = None;
        b.apu_write = None;
        b.ly_override = None;
        b.wave_access = WaveAccess::Closed;
        let bytes = saved(&b);
        let mut loaded = sample();
        loaded
            .read_state(&mut StateReader::new(&bytes))
            .expect("valid");
        assert_eq!(loaded.lyc_frozen, None);
        assert_eq!(loaded.apu_write, None);
        assert_eq!(loaded.ly_override, None);
        assert_eq!(loaded.wave_access, WaveAccess::Closed);
        assert_eq!(saved(&loaded), bytes);
    }

    #[test]
    fn e09_02_bus_truncated_data_is_bad_size_and_leaves_bus_unchanged() {
        let bytes = saved(&sample());
        for len in [
            0usize, 1, 8191, 8192, 17_278, S, 17_304, 17_312, 17_313, M, 17_315, 17_318,
        ] {
            let mut b = sample();
            assert_eq!(
                b.read_state(&mut StateReader::new(&bytes[..len])),
                Err(StateError::BadSize),
                "len {len}"
            );
            assert_eq!(saved(&b), bytes);
        }
    }

    #[test]
    fn e09_02_bus_corrupted_flag_is_invalid_state() {
        let before = saved(&sample());
        for index in [
            17_284usize,
            17_285,
            17_286,
            17_287,
            17_288,
            17_289,
            17_296,
            17_302,
        ] {
            let mut bytes = before.clone();
            bytes[index] = 2;
            let mut b = sample();
            assert_eq!(
                b.read_state(&mut StateReader::new(&bytes)),
                Err(StateError::InvalidState),
                "index {index}"
            );
            assert_eq!(saved(&b), before);
        }
    }

    #[test]
    fn e09_02_bus_out_of_range_value_is_invalid_state() {
        let before = saved(&sample());
        let cases: [&[(usize, u8)]; 6] = [
            &[(S, 0x01), (S + 1, 0x01)],
            &[(17_282, 4)],
            &[(17_283, 3)],
            &[(17_292, 160)],
            &[(17_300, 3)],
            &[(17_301, 16)],
        ];
        for case in cases {
            let mut bytes = before.clone();
            for &(index, value) in case {
                bytes[index] = value;
            }
            let mut b = sample();
            assert_eq!(
                b.read_state(&mut StateReader::new(&bytes)),
                Err(StateError::InvalidState),
                "case {case:?}"
            );
            assert_eq!(saved(&b), before);
        }
    }

    #[test]
    fn e09_02_bus_mapper_errors_leave_bus_unchanged() {
        let before = saved(&sample());
        let mut other_variant = before.clone();
        other_variant[M] = 1;
        let mut invalid_tag = before.clone();
        invalid_tag[M] = 9;
        for (bytes, expected) in [
            (other_variant, StateError::RomMismatch),
            (invalid_tag, StateError::InvalidState),
        ] {
            let mut b = sample();
            assert_eq!(b.read_state(&mut StateReader::new(&bytes)), Err(expected));
            assert_eq!(saved(&b), before);
        }
    }

    #[test]
    fn e09_02_bus_write_to_small_buffer_fails() {
        let mut buf = vec![0u8; 17_318];
        let mut w = StateWriter::new(&mut buf);
        assert_eq!(
            sample().write_state(&mut w),
            Err(StateError::BufferTooSmall)
        );
    }
}
