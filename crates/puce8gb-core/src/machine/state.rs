//! Save state of the whole machine (E09_02, decision A_05). Layout: header (magic, version,
//! cartridge type $0147), cpu, bus (RAMs, IO, serial, DMA, timer, input, mapper with its
//! cartridge RAM and RTC), ppu (with its framebuffer), audio, dot_phase (0..3),
//! stat_line_prev, frame_count (u64), input. The ROM is not saved. Cartridge RAM is part of the
//! state: UNKNOWN - to confirm (A_05 only says the battery file is separate), so loading a
//! state replaces the current cartridge RAM. Loading builds a fresh machine from the same ROM,
//! reads everything into it and replaces `self` only on success, so an error never changes the
//! machine. Bytes after the state are ignored on load.

use super::Dmg;
use crate::savestate::{StateError, StateReader, StateWriter};

impl Dmg {
    /// Upper bound of a serialized state: fixed parts (about 111 KiB) plus the largest
    /// cartridge RAM (128 KiB) plus a full audio ring (16 KiB), rounded up.
    pub const MAX_STATE_BYTES: usize = 262_144;

    pub(super) fn write_machine_state(&self, w: &mut StateWriter<'_>) -> Result<(), StateError> {
        w.put_header(self.bus.peek(0x0147))?;
        self.cpu.write_state(w)?;
        self.bus.write_state(w)?;
        self.ppu.write_state(w)?;
        self.audio.write_state(w)?;
        w.put_u8(self.dot_phase)?;
        w.put_bool(self.stat_line_prev)?;
        w.put_u64(self.frame_count)?;
        self.input.write_state(w)
    }

    pub(super) fn read_machine_state(&mut self, r: &mut StateReader<'_>) -> Result<(), StateError> {
        r.get_header(self.bus.peek(0x0147))?;
        let mut fresh = Dmg::new(&self.bus.rom).map_err(|_| StateError::InvalidState)?;
        fresh.cpu.read_state(r)?;
        fresh.bus.read_state(r)?;
        fresh.ppu.read_state(r)?;
        fresh.audio.read_state(r)?;
        let dot_phase = r.get_u8()?;
        if dot_phase > 3 {
            return Err(StateError::InvalidState);
        }
        fresh.dot_phase = dot_phase;
        fresh.stat_line_prev = r.get_bool()?;
        fresh.frame_count = r.get_u64()?;
        fresh.input.read_state(r)?;
        std::mem::swap(&mut fresh.trace, &mut self.trace);
        *self = fresh;
        self.trace.clear();
        Ok(())
    }

    /// Serializes the machine into a new vector of exactly the state length (for frontends).
    pub fn save_state_vec(&self) -> Result<Vec<u8>, StateError> {
        let mut buf = vec![0u8; Self::MAX_STATE_BYTES];
        let mut w = StateWriter::new(&mut buf);
        self.write_machine_state(&mut w)?;
        let len = w.position();
        buf.truncate(len);
        Ok(buf)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::machine::Machine;

    /// Program: set TAC, then loop incrementing A and storing it at $C000.
    fn rom() -> Vec<u8> {
        let mut r = vec![0u8; 0x8000];
        r[0x100..0x103].copy_from_slice(&[0xC3, 0x50, 0x01]);
        r[0x150..0x15A]
            .copy_from_slice(&[0x3E, 0x05, 0xE0, 0x07, 0x3C, 0xEA, 0x00, 0xC0, 0x18, 0xFA]);
        r
    }

    fn running(frames: usize) -> Dmg {
        let mut dmg = Dmg::new(&rom()).expect("valid");
        for _ in 0..frames {
            dmg.run_frame();
        }
        dmg
    }

    fn fresh() -> Dmg {
        Dmg::new(&rom()).expect("valid")
    }

    #[test]
    fn e09_02_dmg_save_load_save_is_byte_identical() {
        let a = running(2);
        let bytes = a.save_state_vec().expect("room");
        assert_eq!(&bytes[4..9], &[1, 0, 0, 0, 0]);
        let mut b = fresh();
        b.load_state(&bytes).expect("valid");
        assert_eq!(b.save_state_vec().expect("room"), bytes);
        assert_eq!(b.frame_count, a.frame_count);
        assert_eq!(b.cpu.pc, a.cpu.pc);
        assert_eq!(b.bus.wram[0], a.bus.wram[0]);
    }

    #[test]
    fn e09_02_dmg_loaded_machine_continues_identically() {
        let mut a = running(2);
        let bytes = a.save_state_vec().expect("room");
        let mut b = fresh();
        b.load_state(&bytes).expect("valid");
        a.run_frame();
        b.run_frame();
        assert_eq!(a.framebuffer(), b.framebuffer());
        assert_eq!(
            a.save_state_vec().expect("room"),
            b.save_state_vec().expect("room")
        );
    }

    #[test]
    fn e09_02_dmg_truncated_state_is_bad_size_and_leaves_machine_unchanged() {
        let mut a = running(1);
        let bytes = a.save_state_vec().expect("room");
        let before = bytes.clone();
        for len in [
            0usize,
            1,
            8,
            9,
            10,
            100,
            17_000,
            bytes.len() - 11,
            bytes.len() - 1,
        ] {
            assert_eq!(
                a.load_state(&bytes[..len]),
                Err(StateError::BadSize),
                "len {len}"
            );
            assert_eq!(a.save_state_vec().expect("room"), before);
        }
    }

    #[test]
    fn e09_02_dmg_header_errors_are_reported_and_leave_machine_unchanged() {
        let mut a = running(1);
        let bytes = a.save_state_vec().expect("room");
        let mut bad_magic = bytes.clone();
        bad_magic[0] ^= 0xFF;
        let mut bad_version = bytes.clone();
        bad_version[4] = 2;
        let mut bad_cartridge = bytes.clone();
        bad_cartridge[8] = 0x13;
        for (data, expected) in [
            (bad_magic, StateError::BadMagic),
            (bad_version, StateError::BadVersion),
            (bad_cartridge, StateError::RomMismatch),
        ] {
            assert_eq!(a.load_state(&data), Err(expected));
            assert_eq!(a.save_state_vec().expect("room"), bytes);
        }
    }

    #[test]
    fn e09_02_dmg_corrupted_tail_is_invalid_state_and_leaves_machine_unchanged() {
        let mut a = running(1);
        let bytes = a.save_state_vec().expect("room");
        let n = bytes.len();
        for (index, value) in [(n - 11, 4u8), (n - 10, 2u8)] {
            let mut data = bytes.clone();
            data[index] = value;
            assert_eq!(
                a.load_state(&data),
                Err(StateError::InvalidState),
                "index {index}"
            );
            assert_eq!(a.save_state_vec().expect("room"), bytes);
        }
    }

    #[test]
    fn e09_02_dmg_save_to_small_or_exact_buffer() {
        let a = running(1);
        let bytes = a.save_state_vec().expect("room");
        let mut small = vec![0u8; bytes.len() - 1];
        assert_eq!(a.save_state(&mut small), Err(StateError::BufferTooSmall));
        let mut exact = vec![0u8; bytes.len()];
        a.save_state(&mut exact).expect("room");
        assert_eq!(exact, bytes);
    }

    #[test]
    fn e09_02_dmg_fixed_size_buffer_round_trip_ignores_trailing_bytes() {
        let a = running(2);
        let mut big = vec![0u8; Dmg::MAX_STATE_BYTES];
        a.save_state(&mut big).expect("room");
        let mut b = fresh();
        b.load_state(&big).expect("valid");
        assert_eq!(
            b.save_state_vec().expect("room"),
            a.save_state_vec().expect("room")
        );
    }

    #[test]
    fn e09_02_dmg_state_size_stays_below_the_bound() {
        let a = running(3);
        let len = a.save_state_vec().expect("room").len();
        assert!(len > 100_000, "len {len}");
        assert!(len <= Dmg::MAX_STATE_BYTES, "len {len}");
    }
}
