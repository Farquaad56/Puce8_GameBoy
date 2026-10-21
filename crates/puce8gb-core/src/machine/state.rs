//! Save state of the whole machine (E09_02, decision A_05). Layout: header (magic, version,
//! cartridge type $0147), cpu, bus (RAMs, IO, serial, DMA, timer, input, mapper with its
//! cartridge RAM and RTC), ppu (with its framebuffer), audio, dot_phase (0..3),
//! stat_line_prev, frame_count (u64), input, then the extension block of decision A_11 (model
//! u8, boot_mapped u8; E14_04). The ROM is not saved. Cartridge RAM is part of the
//! state: UNKNOWN - to confirm (A_05 only says the battery file is separate), so loading a
//! state replaces the current cartridge RAM. Loading builds a fresh machine from the same ROM,
//! reads everything into it and replaces `self` only on success, so an error never changes the
//! machine. Bytes after the state are ignored on load.

use super::{Dmg, Model};
use crate::savestate::{StateError, StateReader, StateWriter};

impl Dmg {
    /// Upper bound of a serialized state: fixed parts (about 111 KiB) plus the largest
    /// cartridge RAM (128 KiB) plus a full audio ring (16 KiB), rounded up.
    // E15_19: the Cgb block adds about 32 KiB (VRAM bank 1, WRAM banks 2-7, palettes), so the bound is 320 KiB.
    pub const MAX_STATE_BYTES: usize = 327_680;

    pub(super) fn write_machine_state(&self, w: &mut StateWriter<'_>) -> Result<(), StateError> {
        // Temporary guard: the Sgb model cannot be saved yet. Removed by E16_17 (the Cgb guard was removed by E15_19).
        if self.model == Model::Sgb {
            return Err(StateError::InvalidState);
        }
        w.put_header(self.bus.peek(0x0147))?;
        self.cpu.write_state(w)?;
        self.bus.write_state(w)?;
        self.ppu.write_state(w)?;
        self.audio.write_state(w)?;
        w.put_u8(self.dot_phase)?;
        w.put_bool(self.stat_line_prev)?;
        w.put_u64(self.frame_count)?;
        self.input.write_state(w)?;
        // Extension block (decision A_11, E14_04): model byte, then the boot ROM mapped flag.
        w.put_u8(self.model.to_byte())?;
        w.put_bool(self.bus.boot_mapped())?;
        // Cgb block (E15_19): only for the Cgb model, so Dmg states keep their size and layout.
        if self.model == Model::Cgb {
            self.bus.write_cgb_state(w)?;
            self.ppu.write_cgb_state(w)?;
        }
        Ok(())
    }

    pub(super) fn read_machine_state(&mut self, r: &mut StateReader<'_>) -> Result<(), StateError> {
        r.get_header(self.bus.peek(0x0147))?;
        let mut fresh = Dmg::new(&self.bus.rom).map_err(|_| StateError::InvalidState)?;
        fresh.model = self.model;
        fresh.bus.cgb = self.bus.cgb; // Cgb register arms stay active in the fresh machine (E15_19)
        if let Some(image) = &self.boot_image {
            fresh
                .bus
                .set_boot_rom(image, self.model.boot_rom_len())
                .map_err(|_| StateError::InvalidState)?;
            fresh.boot_image = Some(image.clone());
        }
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
        // Extension block (decision A_11, E14_04).
        let model = Model::from_byte(r.get_u8()?).ok_or(StateError::InvalidState)?;
        let boot_mapped = r.get_bool()?;
        if model != self.model {
            return Err(StateError::RomMismatch);
        }
        // Cgb block (E15_19), read after the model check so a Cgb state loaded into a Dmg machine is a RomMismatch.
        if model == Model::Cgb {
            fresh.bus.read_cgb_state(r)?;
            fresh.ppu.read_cgb_state(r)?;
        }
        if boot_mapped && fresh.boot_image.is_none() {
            return Err(StateError::RomMismatch);
        }
        if !boot_mapped && fresh.boot_image.is_some() {
            fresh.bus.write(0xFF50, 0x01); // a non-zero write to BOOT unmaps the boot ROM
        }
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

    /// 32 KiB ROM-only cartridge image: byte 0 is $AA, the rest is $00 (E14_04).
    fn e14_04_cart() -> Vec<u8> {
        let mut image = vec![0x00u8; 32 * 1024];
        image[0] = 0xAA;
        image
    }

    /// Synthetic boot ROM: NOPs, JP $00FC at $0000, then LD A,$01 and LDH ($50),A at $00FC (E14_04).
    fn e14_04_boot() -> Vec<u8> {
        let mut boot = vec![0x00u8; 256];
        boot[0x00] = 0xC3;
        boot[0x01] = 0xFC;
        boot[0x02] = 0x00;
        boot[0xFC] = 0x3E;
        boot[0xFD] = 0x01;
        boot[0xFE] = 0xE0;
        boot[0xFF] = 0x50;
        boot
    }

    fn e14_04_with_boot() -> Dmg {
        Dmg::with_boot_rom(&e14_04_cart(), &e14_04_boot()).expect("valid boot ROM")
    }

    fn e14_04_run_mcycles(dmg: &mut Dmg, count: usize) {
        for _ in 0..count {
            for _ in 0..4 {
                dmg.tick();
            }
        }
    }

    #[test]
    fn e15_04_only_dmg_machines_can_be_saved() {
        let cfg = crate::machine::MachineConfig {
            model: Model::Sgb,
            boot_rom: None,
        };
        let sgb = Dmg::with_config(&e14_04_cart(), cfg).expect("valid");
        assert_eq!(sgb.save_state_vec(), Err(StateError::InvalidState));
        let dmg = Dmg::new(&e14_04_cart()).expect("valid");
        assert!(dmg.save_state_vec().is_ok());
    }

    fn e15_19_cgb(flag: u8) -> Dmg {
        let mut image = rom();
        image[0x143] = flag;
        let cfg = crate::machine::MachineConfig {
            model: Model::Cgb,
            boot_rom: None,
        };
        Dmg::with_config(&image, cfg).expect("valid")
    }

    fn e15_19_dmg_bytes(flag: u8) -> Vec<u8> {
        let mut image = rom();
        image[0x143] = flag;
        Dmg::new(&image)
            .expect("valid")
            .save_state_vec()
            .expect("room")
    }

    /// Size of the Cgb block: bus 32922 bytes + PPU attributes 337 bytes.
    const E15_19_CGB_BLOCK: usize = 33_259;

    fn e15_19_prepare(m: &mut Dmg) {
        m.bus.write(0xFF70, 3);
        m.bus.write(0xD123, 0x77); // WRAM bank 3
        m.bus.write(0xFF56, 0xC1);
        m.bus.write(0xFF72, 0x12);
        m.bus.write(0xFF73, 0x34);
        m.bus.write(0xFF74, 0x56);
        m.bus.write(0xFF75, 0x70);
        m.bus.write(0xFF51, 0xC1);
        m.bus.write(0xFF52, 0x00);
        m.bus.write(0xFF53, 0x00);
        m.bus.write(0xFF54, 0x00);
        m.bus.write(0xFF55, 0xFF); // HBlank transfer of 128 blocks, still running after a few lines
    }

    #[test]
    fn e15_19_cgb_round_trip_continues_identically() {
        let mut a = e15_19_cgb(0x80);
        e15_19_prepare(&mut a);
        for _ in 0..(456 * 20) {
            a.tick();
        }
        let status = a.bus.read(0xFF55);
        assert!(status != 0xFF && status & 0x80 == 0, "status {status:#04x}");
        let bytes = a.save_state_vec().expect("room");
        let mut b = e15_19_cgb(0x80);
        b.load_state(&bytes).expect("valid");
        assert_eq!(b.bus.read(0xFF55), status);
        assert_eq!(b.bus.read(0xFF70), 0xFB);
        assert_eq!(b.bus.read(0xD123), 0x77);
        assert_eq!(b.bus.read(0xFF56), 0xFF);
        assert_eq!(b.bus.read(0xFF72), 0x12);
        assert_eq!(b.bus.read(0xFF73), 0x34);
        assert_eq!(b.bus.read(0xFF74), 0x56);
        assert_eq!(b.bus.read(0xFF75), 0xFF);
        assert_eq!(b.save_state_vec().expect("room"), bytes);
        for _ in 0..2 {
            a.run_frame();
            b.run_frame();
        }
        assert_eq!(a.framebuffer(), b.framebuffer());
        assert_eq!(
            a.save_state_vec().expect("room"),
            b.save_state_vec().expect("room")
        );
    }

    #[test]
    fn e15_19_cgb_state_is_the_dmg_state_plus_the_cgb_block() {
        let cgb = e15_19_cgb(0x80).save_state_vec().expect("room");
        assert_eq!(cgb.len(), e15_19_dmg_bytes(0x80).len() + E15_19_CGB_BLOCK);
        assert!(cgb.len() <= Dmg::MAX_STATE_BYTES);
        assert_eq!(cgb[cgb.len() - E15_19_CGB_BLOCK - 2], 1); // model byte
    }

    #[test]
    fn e15_19_dmg_state_layout_is_unchanged() {
        let bytes = e15_19_dmg_bytes(0x00);
        assert_eq!(&bytes[4..9], &[2, 0, 0, 0, 0]);
        assert_eq!(&bytes[bytes.len() - 2..], &[0, 0]); // model Dmg, boot ROM not mapped
        let cfg = crate::machine::MachineConfig {
            model: Model::Dmg,
            boot_rom: None,
        };
        let same = Dmg::with_config(&rom(), cfg).expect("valid");
        assert_eq!(same.save_state_vec().expect("room").len(), bytes.len());
    }

    #[test]
    fn e15_19_cgb_state_into_a_dmg_machine_is_rom_mismatch_and_leaves_it_unchanged() {
        let bytes = e15_19_cgb(0x80).save_state_vec().expect("room");
        let mut image = rom();
        image[0x143] = 0x80;
        let mut dmg = Dmg::new(&image).expect("valid");
        let before = dmg.save_state_vec().expect("room");
        assert_eq!(dmg.load_state(&bytes), Err(StateError::RomMismatch));
        assert_eq!(dmg.save_state_vec().expect("room"), before);
    }

    #[test]
    fn e15_19_truncated_cgb_state_is_bad_size_and_leaves_the_machine_unchanged() {
        let mut a = e15_19_cgb(0x80);
        let bytes = a.save_state_vec().expect("room");
        let base = bytes.len() - E15_19_CGB_BLOCK;
        for len in [
            base,
            base + 1,
            base + 10_000,
            bytes.len() - 337,
            bytes.len() - 1,
        ] {
            assert_eq!(
                a.load_state(&bytes[..len]),
                Err(StateError::BadSize),
                "len {len}"
            );
            assert_eq!(a.save_state_vec().expect("room"), bytes);
        }
    }

    #[test]
    fn e15_19_corrupted_cgb_bytes_are_invalid_state_and_leave_the_machine_unchanged() {
        let mut a = e15_19_cgb(0x80);
        let bytes = a.save_state_vec().expect("room");
        let base = bytes.len() - E15_19_CGB_BLOCK;
        for (index, value) in [(base, 5u8), (base + 1, 2u8)] {
            let mut data = bytes.clone();
            data[index] = value; // compat flag 5, then VBK 2
            assert_eq!(
                a.load_state(&data),
                Err(StateError::InvalidState),
                "index {index}"
            );
            assert_eq!(a.save_state_vec().expect("room"), bytes);
        }
    }

    #[test]
    fn e15_19_compat_machine_round_trips_and_rejects_a_color_rom() {
        let mut a = e15_19_cgb(0x00);
        a.run_frame();
        let bytes = a.save_state_vec().expect("room");
        let mut b = e15_19_cgb(0x00);
        b.load_state(&bytes).expect("valid");
        assert!(b.compat());
        a.run_frame();
        b.run_frame();
        assert_eq!(a.framebuffer(), b.framebuffer());
        let mut color = e15_19_cgb(0x80);
        assert_eq!(color.load_state(&bytes), Err(StateError::RomMismatch));
    }

    #[test]
    fn e14_04_round_trip_while_boot_rom_is_mapped() {
        let mut a = e14_04_with_boot();
        e14_04_run_mcycles(&mut a, 3);
        assert!(a.bus.boot_mapped());
        let bytes = a.save_state_vec().expect("room");
        let mut b = e14_04_with_boot();
        b.load_state(&bytes).expect("valid");
        assert!(b.bus.boot_mapped());
        assert_eq!(b.cpu.regs(), a.cpu.regs());
        e14_04_run_mcycles(&mut a, 100);
        e14_04_run_mcycles(&mut b, 100);
        assert_eq!(b.cpu.regs(), a.cpu.regs());
        assert_eq!(b.bus.boot_mapped(), a.bus.boot_mapped());
        assert_eq!(
            b.save_state_vec().expect("room"),
            a.save_state_vec().expect("room")
        );
    }

    #[test]
    fn e14_04_unmapped_boot_state_round_trip() {
        let mut a = e14_04_with_boot();
        e14_04_run_mcycles(&mut a, 40);
        assert!(!a.bus.boot_mapped());
        let bytes = a.save_state_vec().expect("room");
        let mut b = e14_04_with_boot();
        b.load_state(&bytes).expect("valid");
        assert!(!b.bus.boot_mapped());
        assert_eq!(b.bus.read(0x0000), 0xAA);
        let mut plain = Dmg::new(&e14_04_cart()).expect("valid");
        plain.load_state(&bytes).expect("valid");
        assert_eq!(plain.cpu.regs(), a.cpu.regs());
    }

    #[test]
    fn e14_04_mapped_boot_state_needs_a_boot_rom() {
        let mut a = e14_04_with_boot();
        e14_04_run_mcycles(&mut a, 3);
        let bytes = a.save_state_vec().expect("room");
        let mut plain = Dmg::new(&e14_04_cart()).expect("valid");
        let before = plain.save_state_vec().expect("room");
        assert_eq!(plain.load_state(&bytes), Err(StateError::RomMismatch));
        assert_eq!(plain.save_state_vec().expect("room"), before);
    }

    #[test]
    fn e14_04_model_byte_7_is_invalid_and_leaves_machine_unchanged() {
        let mut a = running(1);
        let bytes = a.save_state_vec().expect("room");
        let mut data = bytes.clone();
        data[bytes.len() - 2] = 7;
        assert_eq!(a.load_state(&data), Err(StateError::InvalidState));
        assert_eq!(a.save_state_vec().expect("room"), bytes);
    }

    #[test]
    fn e14_04_boot_flag_byte_2_is_invalid() {
        let mut a = running(1);
        let bytes = a.save_state_vec().expect("room");
        let mut data = bytes.clone();
        data[bytes.len() - 1] = 2;
        assert_eq!(a.load_state(&data), Err(StateError::InvalidState));
        assert_eq!(a.save_state_vec().expect("room"), bytes);
    }

    #[test]
    fn e14_04_other_model_is_rom_mismatch() {
        let mut a = running(1);
        let bytes = a.save_state_vec().expect("room");
        let mut data = bytes.clone();
        data[bytes.len() - 2] = 1;
        assert_eq!(a.load_state(&data), Err(StateError::RomMismatch));
        assert_eq!(a.save_state_vec().expect("room"), bytes);
    }

    #[test]
    fn e14_04_version_1_header_is_bad_version() {
        let mut a = running(1);
        let bytes = a.save_state_vec().expect("room");
        assert_eq!(bytes[4], 2);
        let mut data = bytes.clone();
        data[4] = 1;
        assert_eq!(a.load_state(&data), Err(StateError::BadVersion));
        assert_eq!(a.save_state_vec().expect("room"), bytes);
    }

    #[test]
    fn e09_02_dmg_save_load_save_is_byte_identical() {
        let a = running(2);
        let bytes = a.save_state_vec().expect("room");
        assert_eq!(&bytes[4..9], &[2, 0, 0, 0, 0]);
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
        bad_version[4] = 1;
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
        for (index, value) in [(n - 13, 4u8), (n - 12, 2u8)] {
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
