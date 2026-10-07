//! Bus: owns every addressable memory region (decision A_02).
//! read/write dispatch and side-effect-free peek() per decision A_02; PPU-mode blocking
//! and OAM DMA gating arrive with E03.

/// IO register file FF00-FFFF size, including IF ($FF0F) and IE ($FFFF) (decision A_02).
pub const IO_SIZE: usize = 0x100;

/// Header checksum address in bank 0 (note 07a).
pub const ROM_HEADER_CHECKSUM: usize = 0x14D;

/// Every memory region of the DMG bus (decision A_02, note 03a "Adresse bus").
#[derive(Debug)]
pub struct Bus {
    /// Cartridge ROM image, bank 0 first (decisions A_02/A_06).
    pub rom: Vec<u8>,
    /// VRAM 8000-9FFF.
    pub vram: [u8; 0x2000],
    /// WRAM C000-DFFF; the echo RAM E000-FDFF mirrors it by routing (note 03a "Echo RAM").
    pub wram: [u8; 0x2000],
    /// OAM FE00-FE9F.
    pub oam: [u8; 0x100],
    /// HRAM FF80-FFFE.
    pub hram: [u8; 0x7F],
    /// IO register file FF00-FFFF (decision A_02).
    pub io: [u8; IO_SIZE],
    /// Serial output capture (C01_03): bytes sent on SB, drained via take_serial.
    pub serial: [u8; 256],
    /// Number of captured bytes currently held in `serial` (C01_03).
    pub serial_len: usize,
}

impl Bus {
    /// Build a bus around the cartridge ROM image, all RAM zeroed.
    pub fn new(rom: Vec<u8>) -> Self {
        Bus {
            rom,
            vram: [0x00; 0x2000],
            wram: [0x00; 0x2000],
            oam: [0x00; 0x100],
            hram: [0x00; 0x7F],
            io: [0xFF; IO_SIZE],
            serial: [0x00; 256],
            serial_len: 0,
        }
    }

    /// Post-boot state of the memory owned by the Bus (note 08, decision A_06).
    pub fn reset(&mut self) {
        // WRAM/HRAM filled deterministically with $00 at power-up (note 08 "RAM after
        // power-up", decision A_05); echo RAM mirrors the current WRAM bank by routing,
        // so it carries no separate state.
        self.wram.fill(0x00);
        self.hram.fill(0x00);
        self.serial_len = 0; // no captured serial bytes at power-up (C01_03)

        // Unassigned IO reads $FF by default (decision A_02), then the documented
        // post-boot values from note 08 "Registres I/O apres boot".
        self.io.fill(0xFF);
        let io = &mut self.io;
        io[0x00] = 0xCF; // P1
        io[0x01] = 0x00; // SB
        io[0x02] = 0x7E; // SC
        io[0x04] = 0xAB; // DIV
        io[0x05] = 0x00; // TIMA
        io[0x06] = 0x00; // TMA
        io[0x07] = 0xF8; // TAC (timer active)
        io[0x0F] = 0xE1; // IF
        io[0x10] = 0x80; // NR10
        io[0x11] = 0xBF; // NR11
        io[0x12] = 0xF3; // NR12
        io[0x13] = 0xFF; // NR13
        io[0x14] = 0xBF; // NR14
        io[0x15] = 0x3F; // NR21
        io[0x16] = 0x00; // NR22
        io[0x17] = 0xFF; // NR23
        io[0x18] = 0xBF; // NR24
        io[0x19] = 0x7F; // NR30
        io[0x1A] = 0xFF; // NR31
        io[0x1B] = 0x9F; // NR32
        io[0x1C] = 0xFF; // NR33
        io[0x1D] = 0xBF; // NR34
        io[0x20] = 0xFF; // NR41
        io[0x21] = 0x00; // NR42
        io[0x22] = 0x00; // NR43
        io[0x23] = 0xBF; // NR44
        io[0x24] = 0x77; // NR50
        io[0x25] = 0xF3; // NR51
        io[0x26] = 0xF1; // NR52
                         // Wave RAM FF30-FF3F power-up value is not listed in note 08: UNKNOWN - to confirm,
                         // left at the open-bus default $FF (decision A_02).
        io[0x40] = 0x91; // LCDC
        io[0x41] = 0x85; // STAT
        io[0x42] = 0x00; // SCY
        io[0x43] = 0x00; // SCX
        io[0x44] = 0x00; // LY
        io[0x45] = 0x00; // LYC
        io[0x46] = 0xFF; // DMA
        io[0x47] = 0xFC; // BGP
                         // OBP0/OBP1: specs 2001 give $FF/$FF, pandocs says left uninitialized (CONFLIT,
                         // note 08); the specs value is used until mooneye boot_hwio-dmgABCmgb settles it.
        io[0x48] = 0xFF; // OBP0
        io[0x49] = 0xFF; // OBP1
        io[0x4A] = 0x00; // WY
        io[0x4B] = 0x00; // WX
        io[0xFF] = 0x00; // IE
    }

    /// Read a byte from the bus (decision A_02). PPU-mode blocking and OAM DMA gating
    /// arrive with E03.
    pub fn read(&self, addr: u16) -> u8 {
        match addr {
            0x0000..=0x7FFF => self.rom.get(addr as usize).copied().unwrap_or(0xFF), // ROM-only cart (decision A_06): past the image is open bus
            0x8000..=0x9FFF => self.vram[(addr - 0x8000) as usize],
            0xA000..=0xBFFF => 0xFF, // cartridge SRAM: unmapped on a ROM-only cart (decision A_06)
            0xC000..=0xDFFF => {
                // WRAM: lower 14 bits of the address index into the 8 KiB array.
                self.wram[((addr - 0xC000) as usize) & 0x3FFF]
            }
            0xE000..=0xFDFF => {
                // Echo RAM: only the lower 13 bits of the address are connected, so it wraps onto
                // the current WRAM bank (C000-C7FF on DMG) exactly like a read (note 03a "Echo RAM").
                self.wram[(addr as usize) & 0x1FFF]
            }
            0xFE00..=0xFE9F => self.oam[(addr - 0xFE00) as usize],
            0xFEA0..=0xFEFF => 0x00, // unusable range: $00 on DMG outside OAM block (note 03a)
            0xFF00..=0xFF7F => self.io[(addr - 0xFF00) as usize],
            0xFF80..=0xFFFE => self.hram[(addr - 0xFF80) as usize],
            0xFFFF => self.io[IO_SIZE - 1], // IE (decision A_02)
        }
    }

    /// Write a byte to the bus (decision A_02). ROM and unmapped/unusable ranges are
    /// ignored; PPU-mode blocking and OAM DMA gating arrive with E03.
    pub fn write(&mut self, addr: u16, val: u8) {
        match addr {
            0x0000..=0x7FFF => {} // ROM is read-only (note 03a "Adresse bus")
            0x8000..=0x9FFF => self.vram[(addr - 0x8000) as usize] = val,
            0xA000..=0xBFFF => {} // cartridge SRAM: unmapped on a ROM-only cart (decision A_06)
            0xC000..=0xDFFF => self.wram[(addr - 0xC000) as usize] = val,
            0xE000..=0xFDFF => {
                // Echo RAM: only the lower 13 bits of the address are connected, so it wraps onto
                // the current WRAM bank (C000-C7FF on DMG) exactly like a read (note 03a "Echo RAM").
                self.wram[(addr as usize) & 0x1FFF] = val;
            }
            0xFE00..=0xFE9F => self.oam[(addr - 0xFE00) as usize] = val,
            0xFEA0..=0xFEFF => {} // unusable range: writes are ignored (note 03a)
            0xFF00..=0xFF7F => {
                self.io[(addr - 0xFF00) as usize] = val;
                // SC ($FF02): a transfer started with the internal clock (bit7 + bit0 set,
                // note 06 "SC") completes instantly (documented simplification, C01_03):
                // the current SB is captured and the enable bit cleared at once. The exact
                // duration (4096 dots) and the serial interrupt arrive with a later task.
                if addr == 0xFF02 && val & 0x80 != 0 && val & 0x01 != 0 {
                    if self.serial_len < self.serial.len() {
                        self.serial[self.serial_len] = self.io[0x01]; // SB ($FF01)
                        self.serial_len += 1;
                    } else {
                        // Buffer full: the new byte is dropped (C01_03).
                    }
                    self.io[0x02] &= !0x80;
                }
            }
            0xFF80..=0xFFFE => self.hram[(addr - 0xFF80) as usize] = val,
            0xFFFF => self.io[IO_SIZE - 1] = val, // IE (decision A_02)
        }
    }

    /// Drain up to `out.len()` captured serial bytes, oldest first (C01_03); returns the
    /// number of bytes drained. The buffer is a fixed ring: no allocation.
    pub fn take_serial(&mut self, out: &mut [u8]) -> usize {
        let n = out.len().min(self.serial_len);
        out[..n].copy_from_slice(&self.serial[..n]);
        self.serial.copy_within(n..self.serial_len, 0);
        self.serial_len -= n;
        n
    }

    /// Read the raw memory at `addr` with no side effect and no gate (decision A_02):
    /// for debuggers and viewers. PPU-mode blocking and OAM DMA gating arrive with E03.
    pub fn peek(&self, addr: u16) -> u8 {
        match addr {
            0x0000..=0x7FFF => self.rom.get(addr as usize).copied().unwrap_or(0xFF), // ROM-only cart (decision A_06): past the image is open bus
            0x8000..=0x9FFF => self.vram[(addr - 0x8000) as usize],
            0xA000..=0xBFFF => 0xFF, // cartridge SRAM: unmapped on a ROM-only cart (decision A_06)
            0xC000..=0xDFFF => self.wram[(addr - 0xC000) as usize],
            0xE000..=0xFDFF => {
                // Echo RAM: only the lower 13 bits of the address are connected, so it wraps onto
                // the current WRAM bank (C000-C7FF on DMG) exactly like a read/write (note 03a "Echo RAM").
                self.wram[(addr as usize) & 0x1FFF]
            }
            0xFE00..=0xFE9F => self.oam[(addr - 0xFE00) as usize],
            0xFEA0..=0xFEFF => 0x00, // unusable range: $00 on DMG outside OAM block (note 03a)
            0xFF00..=0xFF7F => self.io[(addr - 0xFF00) as usize],
            0xFF80..=0xFFFE => self.hram[(addr - 0xFF80) as usize],
            0xFFFF => self.io[IO_SIZE - 1], // IE (decision A_02)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// ROM image of `len` bytes filled with $A5 (distinct from the open-bus $FF), header
    /// checksum byte $014D set to $5A.
    fn test_rom(len: usize) -> Vec<u8> {
        let mut r = vec![0xA5u8; len];
        if len > ROM_HEADER_CHECKSUM {
            r[ROM_HEADER_CHECKSUM] = 0x5A;
        }
        r
    }

    #[test]
    fn e01_03_rom_reads_via_cartridge() {
        let bus = Bus::new(test_rom(32 * 1024));
        assert_eq!(bus.read(0x0000), 0xA5); // bank 0 first byte
        assert_eq!(bus.read(0x014D), 0x5A); // header checksum byte (note 07a)
        assert_eq!(bus.read(0x3FFF), 0xA5); // last byte of bank 0
        assert_eq!(bus.read(0x4000), 0xA5); // ROM-only cart: the image continues in bank 1
        assert_eq!(bus.read(0x7FFF), 0xA5); // end of the ROM area (note 03a)
    }

    #[test]
    fn e01_03_rom_read_past_image_is_open_bus() {
        let bus = Bus::new(test_rom(16 * 1024));
        assert_eq!(bus.read(0x3FFF), 0xA5); // last byte of the image
        assert_eq!(bus.read(0x4000), 0xFF); // past the end: open bus (decision A_02)
    }

    #[test]
    fn e01_03_rom_writes_are_ignored() {
        let mut bus = Bus::new(test_rom(32 * 1024));
        bus.write(0x0100, 0x5A);
        assert_eq!(bus.read(0x0100), 0xA5); // ROM is read-only (note 03a "Adresse bus")
    }

    #[test]
    fn e01_03_vram_read_write() {
        let mut bus = Bus::new(test_rom(32 * 1024));
        bus.write(0x8000, 0xA5);
        assert_eq!(bus.read(0x8000), 0xA5);
        bus.write(0x9FFF, 0x3C);
        assert_eq!(bus.read(0x9FFF), 0x3C);
    }

    #[test]
    fn e01_03_wram_read_write() {
        let mut bus = Bus::new(test_rom(32 * 1024));
        bus.write(0xC000, 0xA5);
        assert_eq!(bus.read(0xC000), 0xA5);
        bus.write(0xDFFF, 0x3C);
        assert_eq!(bus.read(0xDFFF), 0x3C);
    }

    #[test]
    fn e01_03_echo_mirrors_wram_both_ways() {
        let mut bus = Bus::new(test_rom(32 * 1024));
        // WRAM -> echo: a value (not $00/$FF) written to WRAM is mirrored in the echo RAM
        // and absent from cartridge SRAM (note 03a "Echo RAM").
        bus.write(0xC050, 0xA5);
        assert_eq!(bus.read(0xE050), 0xA5);
        assert_eq!(bus.read(0xA050), 0xFF);
        // echo -> WRAM: writing the echo RAM changes the WRAM bank.
        bus.write(0xE051, 0x3C);
        assert_eq!(bus.read(0xC051), 0x3C);
        // The mirror wraps on the lower 13 bits over the full echo range E000-FDFF (note 03a "Echo RAM").
        bus.write(0xFDFF, 0xA5);
        assert_eq!(bus.read(0xDDFF), 0xA5); // FDFF mirrors DDFF: only the lower 13 bits are connected
    }

    #[test]
    fn e01_03_oam_read_write() {
        let mut bus = Bus::new(test_rom(32 * 1024));
        bus.write(0xFE00, 0x90); // OAM y coordinate (note 03a)
        assert_eq!(bus.read(0xFE00), 0x90);
        bus.write(0xFE9F, 0x88);
        assert_eq!(bus.read(0xFE9F), 0x88);
    }

    #[test]
    fn e01_03_unusable_range_reads_zero_and_writes_ignored() {
        let mut bus = Bus::new(test_rom(32 * 1024));
        assert_eq!(bus.read(0xFEA0), 0x00); // DMG outside OAM block (note 03a)
        assert_eq!(bus.read(0xFEFF), 0x00);
        bus.write(0xFEB0, 0xA5);
        assert_eq!(bus.read(0xFEB0), 0x00); // writes to the unusable range are ignored
    }

    #[test]
    fn e01_03_io_read_write() {
        let mut bus = Bus::new(test_rom(32 * 1024));
        bus.write(0xFF00, 0xC5); // P1
        assert_eq!(bus.read(0xFF00), 0xC5);
        bus.write(0xFF7F, 0x3C);
        assert_eq!(bus.read(0xFF7F), 0x3C);
    }

    #[test]
    fn e01_03_ie_is_last_byte_of_io_file() {
        let mut bus = Bus::new(test_rom(32 * 1024));
        bus.write(0xFFFF, 0x81); // IE (decision A_02)
        assert_eq!(bus.read(0xFFFF), 0x81);
        assert_eq!(bus.io[IO_SIZE - 1], 0x81);
    }

    #[test]
    fn e01_03_hram_read_write() {
        let mut bus = Bus::new(test_rom(32 * 1024));
        bus.write(0xFF80, 0xA5);
        assert_eq!(bus.read(0xFF80), 0xA5);
        bus.write(0xFFFE, 0x3C);
        assert_eq!(bus.read(0xFFFE), 0x3C);
    }

    #[test]
    fn e01_03_unmapped_cartridge_sram_reads_open_bus() {
        let mut bus = Bus::new(test_rom(32 * 1024));
        assert_eq!(bus.read(0xA000), 0xFF); // unmapped on a ROM-only cart (decision A_06)
        assert_eq!(bus.read(0xBFFF), 0xFF);
        bus.write(0xA000, 0x3C);
        assert_eq!(bus.read(0xA000), 0xFF); // writes are ignored
    }

    #[test]
    fn e01_03_region_boundaries_do_not_overlap() {
        let mut bus = Bus::new(test_rom(32 * 1024));
        // VRAM last byte does not leak into the unmapped SRAM after it.
        bus.write(0x9FFF, 0x3C);
        assert_eq!(bus.read(0xA000), 0xFF);
        // WRAM first byte does not leak into the unmapped SRAM before it.
        bus.write(0xC000, 0xA5);
        assert_eq!(bus.read(0xBFFF), 0xFF);
        // Echo RAM mirrors by offset: its last byte is C7FF, not DFFF.
        bus.write(0xDFFF, 0x3C);
        assert_eq!(bus.read(0xFDFF), 0x00);
        assert_eq!(bus.read(0xC7FF), 0x00);
        // OAM first byte does not leak into the unusable range before it.
        bus.write(0xFE00, 0x90);
        assert_eq!(bus.read(0xFEA0), 0x00);
        // IO first byte does not leak into the unusable range before it.
        bus.write(0xFF00, 0xC5);
        assert_eq!(bus.read(0xFEFF), 0x00);
        // HRAM is separate from the IO file: writing FF7F leaves FF80 untouched.
        bus.write(0xFF7F, 0x3C);
        assert_eq!(bus.read(0xFF80), 0x00);
        // IE (FFFF) is separate from HRAM: writing FFFE leaves FFFF at its open-bus value.
        bus.write(0xFFFE, 0xA5);
        assert_eq!(bus.read(0xFFFF), 0xFF);
    }

    #[test]
    fn e01_03_peek_reads_raw_memory_across_regions() {
        let mut bus = Bus::new(test_rom(32 * 1024));
        assert_eq!(bus.peek(0x0000), 0xA5); // ROM via cartridge
        bus.write(0x8000, 0x3C);
        assert_eq!(bus.peek(0x8000), 0x3C); // VRAM
        bus.write(0xC000, 0x90);
        assert_eq!(bus.peek(0xE000), 0x90); // echo mirrors WRAM
        bus.write(0xFE00, 0x81);
        assert_eq!(bus.peek(0xFE00), 0x81); // OAM
        assert_eq!(bus.peek(0xFEA0), 0x00); // unusable range reads $00 on DMG
        bus.write(0xFF00, 0xC5);
        assert_eq!(bus.peek(0xFF00), 0xC5); // IO
        bus.write(0xFF80, 0xA1);
        assert_eq!(bus.peek(0xFF80), 0xA1); // HRAM
        assert_eq!(bus.peek(0xFFFF), 0xFF); // IE at its open-bus value
        assert_eq!(bus.peek(0xA000), 0xFF); // unmapped: open bus
    }

    #[test]
    fn e01_03_peek_equals_read_for_plain_memory() {
        let mut bus = Bus::new(test_rom(32 * 1024));
        for addr in [0x8000u16, 0xC000, 0xE000, 0xFE00, 0xFF00, 0xFF80, 0xFFFF] {
            bus.write(addr, 0xA5);
        }
        // No gates yet (E03): peek equals read across the whole map.
        for addr in 0x0000u16..=0xFFFF {
            assert_eq!(bus.peek(addr), bus.read(addr));
        }
    }

    #[test]
    fn e01_03_peek_does_not_change_state() {
        let mut bus = Bus::new(test_rom(32 * 1024));
        bus.write(0xFF00, 0xC5);
        for _ in 0..8 {
            assert_eq!(bus.peek(0xFF00), 0xC5); // peek has no side effect (decision A_02)
        }
        assert_eq!(bus.read(0xFF00), 0xC5);
    }

    #[test]
    fn c01_03_internal_clock_write_captures_sb_and_clears_bit7() {
        let mut bus = Bus::new(test_rom(32 * 1024));
        bus.write(0xFF01, 0x41); // SB: the byte to send (note 06 "SB")
        bus.write(0xFF02, 0x81); // SC: enable + internal clock (note 06 "SC")
        assert_eq!(bus.io[0x02], 0x01); // bit7 cleared at once; unused bits keep the written value
        let mut out = [0u8; 4];
        assert_eq!(bus.take_serial(&mut out), 1);
        assert_eq!(&out[..1], &[0x41]);
    }

    #[test]
    fn c01_03_external_clock_write_captures_nothing() {
        let mut bus = Bus::new(test_rom(32 * 1024));
        bus.write(0xFF01, 0x41); // SB
        bus.write(0xFF02, 0x80); // SC: enable + external clock (bit0 clear) - no capture (C01_03)
        assert_eq!(bus.io[0x02], 0x80); // bit7 stays as written
        let mut out = [0u8; 4];
        assert_eq!(bus.take_serial(&mut out), 0);
    }

    #[test]
    fn c01_03_drain_empties_buffer() {
        let mut bus = Bus::new(test_rom(32 * 1024));
        for (i, byte) in [0x41u8, 0x42, 0x43].into_iter().enumerate() {
            bus.write(0xFF01, byte);
            bus.write(0xFF02, 0x81); // internal clock: one captured byte per write (C01_03)
            assert_eq!(bus.serial_len, i + 1);
        }
        let mut out = [0u8; 2];
        assert_eq!(bus.take_serial(&mut out), 2); // oldest first
        assert_eq!(&out[..], &[0x41, 0x42]);
        assert_eq!(bus.serial_len, 1);
        let mut rest = [0u8; 8];
        assert_eq!(bus.take_serial(&mut rest), 1); // the buffer is now empty
        assert_eq!(&rest[..1], &[0x43]);
        assert_eq!(bus.take_serial(&mut rest), 0);
    }

    #[test]
    fn c01_03_overflow_drops_new_bytes() {
        let mut bus = Bus::new(test_rom(32 * 1024));
        // Fill the fixed buffer to capacity, then send one more byte: it is dropped (C01_03).
        for i in 0u8..=255 {
            bus.write(0xFF01, i);
            bus.write(0xFF02, 0x81);
        }
        assert_eq!(bus.serial_len, bus.serial.len());
        bus.write(0xFF01, 0xEE);
        bus.write(0xFF02, 0x81);
        assert_eq!(bus.serial_len, bus.serial.len()); // still full: the new byte was dropped
        let mut out = vec![0u8; bus.serial.len()];
        assert_eq!(bus.take_serial(&mut out), bus.serial.len());
        assert_eq!(out[0], 0x00); // oldest byte kept, not overwritten
        assert_ne!(out.last(), Some(&0xEE));
    }

    #[test]
    fn c01_03_reset_clears_capture_buffer() {
        let mut bus = Bus::new(test_rom(32 * 1024));
        bus.write(0xFF01, 0x41);
        bus.write(0xFF02, 0x81);
        assert_eq!(bus.serial_len, 1);
        bus.reset();
        assert_eq!(bus.serial_len, 0); // the capture buffer is part of the post-boot state (C01_03)
    }
}
