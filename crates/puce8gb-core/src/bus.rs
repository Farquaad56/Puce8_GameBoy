//! Bus: owns every addressable memory region (decision A_02).
//! read/write dispatch and side-effect-free peek() per decision A_02; PPU-mode blocking
//! and OAM DMA gating arrive with E03.

use crate::timer::Timer;

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
    /// Current PPU scanline = LY ($FF44), owned by the PPU and mirrored here so reads are
    /// side-effect-free; writes to $FF44 are ignored (E03_01, note 04a "LY").
    pub ly: u8,
    /// Current PPU mode 0-3, mirrored from the PPU for the STAT ($FF41) readback so reads stay
    /// side-effect-free (decision A_02, E03_08); updated by Machine::tick.
    pub ppu_mode: u8,
    /// CPU access to VRAM ($8000-$9FFF) allowed (E03_10): mirrored from the PPU by Machine::tick.
    vram_open: bool,
    /// CPU access to OAM ($FE00-$FE9F) allowed (E03_10): mirrored from the PPU by Machine::tick.
    oam_open: bool,
    /// OAM DMA transfer running (E03_06 spec): one byte copied per M-cycle via dma_step.
    dma_active: bool,
    /// OAM DMA source address base = V << 8 from the last write to $FF46 (E03_06 spec).
    dma_src: u16,
    /// Next destination index of the running OAM DMA transfer, 0..=159 (E03_06 spec).
    dma_index: u8,
    /// Debug tool for trace comparison, task C01_49: when Some(v), every read of $FF44
    /// returns v instead of the PPU scanline; None (default) keeps normal behaviour.
    ly_override: Option<u8>,
    /// Timer and divider registers DIV/TIMA/TMA/TAC ($FF04-$FF07, note 01b).
    pub timer: Timer,
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
            ly: 0,                   // LY ($FF44) post-boot value (note 08, C01_15)
            ppu_mode: 2, // PPU mode at the start of line 0 is OAM scan (E03_01); Machine::tick keeps it current
            vram_open: true, // VRAM access allowed until Machine::tick mirrors the PPU state (E03_10)
            oam_open: true, // OAM access allowed until Machine::tick mirrors the PPU state (E03_10)
            dma_active: false, // no OAM DMA transfer running at power-up (E03_06 spec)
            dma_src: 0,     // source base set by a write to $FF46 (E03_06 spec)
            dma_index: 0,   // next destination index of the running transfer (E03_06 spec)
            ly_override: None, // no debug override by default (C01_49)
            timer: Timer::default(), // DIV/TIMA/TMA/TAC are set in reset() (note 08)
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
        self.ly = 0; // LY ($FF44) post-boot value (note 08, C01_15)
        self.ppu_mode = 2; // PPU mode restarts at the start of line 0: OAM scan (E03_01)
        self.vram_open = true; // VRAM access allowed until Machine::tick mirrors the PPU state (E03_10)
        self.oam_open = true; // OAM access allowed until Machine::tick mirrors the PPU state (E03_10)
        self.dma_active = false; // no OAM DMA transfer running at power-up (E03_06 spec)
        self.dma_index = 0; // next destination index of the running transfer (E03_06 spec)
        self.timer.reset(); // DIV/TIMA/TMA/TAC post-boot values (note 08, note 01b)

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
            0x8000..=0x9FFF => {
                if self.vram_open {
                    self.vram[(addr - 0x8000) as usize]
                } else {
                    0xFF // blocked by the PPU mode: reads $FF (E03_10 spec)
                }
            }
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
            0xFE00..=0xFE9F => {
                if self.oam_open {
                    self.oam[(addr - 0xFE00) as usize]
                } else {
                    0xFF // blocked by the PPU mode: reads $FF (E03_10 spec)
                }
            }
            0xFEA0..=0xFEFF => 0x00, // unusable range: $00 on DMG outside OAM block (note 03a)
            0xFF44 => self.ly_override.unwrap_or(self.ly), // LY ($FF44): PPU-owned scanline, read-only (C01_15); debug override wins (C01_49)
            0xFF41 => self.stat_read(), // STAT ($FF41): composed readback, side-effect-free (E03_08 spec)
            0xFF0F => 0xE0 | (self.io[0x0F] & 0x1F), // IF ($FF0F): bits 5-7 read as 1 (task C01_32)
            0xFF04 => self.timer.div(), // DIV ($FF04, note 01b)
            0xFF05 => self.timer.tima(), // TIMA ($FF05, note 01b)
            0xFF06 => self.timer.tma(), // TMA ($FF06, note 01b)
            0xFF07 => self.timer.tac(), // TAC ($FF07, note 01b)
            0xFF00..=0xFF7F => self.io[(addr - 0xFF00) as usize],
            0xFF80..=0xFFFE => self.hram[(addr - 0xFF80) as usize],
            0xFFFF => self.io[IO_SIZE - 1], // IE (decision A_02)
        }
    }

    /// Debug tool for trace comparison, task C01_49: force every $FF44 read to `v`, or
    /// restore the normal PPU scanline with None.
    #[doc(hidden)]
    pub fn debug_set_ly_override(&mut self, v: Option<u8>) {
        self.ly_override = v;
    }

    /// Mirror the PPU's CPU access state for VRAM and OAM (E03_10): called by Machine::tick
    /// after every PPU step, and at power-up. The PPU's own reads use the raw arrays directly.
    pub(crate) fn set_video_access(&mut self, vram_open: bool, oam_open: bool) {
        self.vram_open = vram_open;
        self.oam_open = oam_open;
    }

    /// Write a byte to the bus (decision A_02). ROM and unmapped/unusable ranges are
    /// ignored; PPU-mode blocking and OAM DMA gating arrive with E03.
    pub fn write(&mut self, addr: u16, val: u8) {
        match addr {
            0x0000..=0x7FFF => {} // ROM is read-only (note 03a "Adresse bus")
            0x8000..=0x9FFF => {
                if self.vram_open {
                    self.vram[(addr - 0x8000) as usize] = val;
                } // blocked by the PPU mode: writes are ignored (E03_10 spec)
            }
            0xA000..=0xBFFF => {} // cartridge SRAM: unmapped on a ROM-only cart (decision A_06)
            0xC000..=0xDFFF => self.wram[(addr - 0xC000) as usize] = val,
            0xE000..=0xFDFF => {
                // Echo RAM: only the lower 13 bits of the address are connected, so it wraps onto
                // the current WRAM bank (C000-C7FF on DMG) exactly like a read (note 03a "Echo RAM").
                self.wram[(addr as usize) & 0x1FFF] = val;
            }
            0xFE00..=0xFE9F => {
                if self.oam_open {
                    self.oam[(addr - 0xFE00) as usize] = val;
                } // blocked by the PPU mode: writes are ignored (E03_10 spec)
            }
            0xFEA0..=0xFEFF => {} // unusable range: writes are ignored (note 03a)
            0xFF44 => {} // LY ($FF44) is read-only; writes are ignored (C01_15, note 04a "LY")
            0xFF41 => self.io[0x41] = val & 0x78, // STAT ($FF41): only the select bits 6..3 are writable (E03_08 spec); bit 2 and bits 1..0 read back live; LYC ($FF45) is plain R/W through the IO file
            0xFF0F => self.io[0x0F] = val & 0x1F, // IF ($FF0F): only bits 0-4 writable (task C01_32)
            0xFF04 => self.timer.write_div(val), // DIV ($FF04): any value resets the counter (note 01b)
            0xFF05 => self.timer.write_tima(val), // TIMA ($FF05, note 01b)
            0xFF06 => self.timer.write_tma(val), // TMA ($FF06, note 01b)
            0xFF07 => self.timer.write_tac(val), // TAC ($FF07, note 01b)
            0xFF46 => {
                self.io[0x46] = val; // DMA reads back the last written value (E03_06 spec)
                self.dma_src = u16::from(val) << 8; // source = V * 0x100; sources above 0xDF: UNKNOWN - to confirm (note 03a)
                self.dma_index = 0; // a write during a transfer restarts it: UNKNOWN - to confirm (A_02 is silent)
                self.dma_active = true;
            }
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

    /// Interrupts pending for dispatch (task C01_32): IE ($FFFF) & IF ($FF0F), masked to the
    /// five interrupt bits. Note 02b: a handler runs only when IME and this are both set;
    /// bit 0 has priority, bit 4 last.
    pub fn pending_interrupts(&self) -> u8 {
        self.io[IO_SIZE - 1] & (self.io[0x0F] & 0x1F)
    }

    /// Clear one IF ($FF0F) request bit (task C01_32): the acknowledge at interrupt entry.
    pub fn clear_if_bit(&mut self, mask: u8) {
        self.io[0x0F] &= !(mask & 0x1F);
    }

    /// STAT ($FF41) readback (E03_08): bit 7 reads as 1; bits 6..3 are the interrupt select
    /// bits as written; bit 2 is the coincidence flag, set while LY == LYC and updated
    /// continuously (note 04a "LYC"); bits 1..0 report the current PPU mode. The stored byte
    /// holds only the writable select bits, so the readback composes them with the live state.
    pub fn stat_read(&self) -> u8 {
        let mut v = 0x80 | (self.io[0x41] & 0x78); // bit 7 fixed at 1; select bits as written
        if self.ly == self.io[0x45] {
            v |= 0x04; // coincidence flag: LY ($FF44) equals LYC ($FF45) (note 04a "LYC")
        }
        v | (self.ppu_mode & 0x03) // bits 1..0: current PPU mode, mirrored by Machine::tick
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

    /// True while an OAM DMA transfer is running (E03_06 spec).
    pub fn dma_active(&self) -> bool {
        self.dma_active
    }

    /// One M-cycle of the OAM DMA (E03_06 spec): copy source byte `dma_index` into OAM, not blocked by
    /// the PPU mode. 160 steps in total. Start delay: UNKNOWN - to confirm (A_02 gives none), so 0 here.
    pub fn dma_step(&mut self) {
        if !self.dma_active {
            return;
        }
        let byte = self.peek(self.dma_src.wrapping_add(u16::from(self.dma_index)));
        self.oam[usize::from(self.dma_index)] = byte;
        self.dma_index += 1;
        if self.dma_index == 160 {
            self.dma_active = false;
        }
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
            0xFF44 => self.ly_override.unwrap_or(self.ly), // LY ($FF44): PPU-owned scanline, read-only (C01_15); debug override wins (C01_49)
            0xFF41 => self.stat_read(), // STAT ($FF41): composed readback, side-effect-free (E03_08 spec)
            0xFF0F => 0xE0 | (self.io[0x0F] & 0x1F), // IF ($FF0F): bits 5-7 read as 1 (task C01_32)
            0xFF04 => self.timer.div(), // DIV ($FF04, note 01b)
            0xFF05 => self.timer.tima(), // TIMA ($FF05, note 01b)
            0xFF06 => self.timer.tma(), // TMA ($FF06, note 01b)
            0xFF07 => self.timer.tac(), // TAC ($FF07, note 01b)
            0xFF00..=0xFF7F => self.io[(addr - 0xFF00) as usize],
            0xFF80..=0xFFFE => self.hram[(addr - 0xFF80) as usize],
            0xFFFF => self.io[IO_SIZE - 1], // IE (decision A_02)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::machine::{Dmg, Machine};

    /// ROM image of `len` bytes filled with $A5 (distinct from the open-bus $FF), header
    /// checksum byte $014D set to $5A.
    fn test_rom(len: usize) -> Vec<u8> {
        let mut r = vec![0xA5u8; len];
        if len > ROM_HEADER_CHECKSUM {
            r[ROM_HEADER_CHECKSUM] = 0x5A;
        }
        r
    }

    /// Zero-filled ROM image of `len` bytes: full of unknown opcodes (task C01_34).
    fn zero_rom(len: usize) -> Vec<u8> {
        vec![0u8; len]
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
    fn c01_15_ff44_reads_ly_and_writes_are_ignored() {
        let mut bus = Bus::new(test_rom(32 * 1024));
        assert_eq!(bus.read(0xFF44), 0x00); // post-boot LY (note 08)
        bus.ly = 0x90; // the PPU advances the scanline (C01_15)
        assert_eq!(bus.read(0xFF44), 0x90); // $FF44 reflects the current line
        assert_eq!(bus.peek(0xFF44), 0x90); // peek is side-effect-free too
        bus.write(0xFF44, 0x5A); // LY is read-only: writes are ignored (note 04a "LY")
        assert_eq!(bus.read(0xFF44), 0x90);
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
        bus.write(0xFF01, 0x41); // SB
        bus.write(0xFF02, 0x81); // SC: internal clock (note 06)
        assert_eq!(bus.serial_len, 1);
        bus.reset();
        assert_eq!(bus.serial_len, 0); // the capture buffer is part of the post-boot state (C01_03)
    }

    #[test]
    fn c01_34_timer_registers_live_on_their_bus_addresses() {
        let mut bus = Bus::new(test_rom(32 * 1024));
        bus.reset(); // post-boot values (note 08)
        assert_eq!(bus.read(0xFF04), 0xAB); // DIV
        assert_eq!(bus.read(0xFF05), 0x00); // TIMA
        assert_eq!(bus.read(0xFF06), 0x00); // TMA
        assert_eq!(bus.read(0xFF07), 0xF8); // TAC

        bus.write(0xFF04, 0x5A); // DIV: the written value is ignored (note 01b)
        assert_eq!(bus.read(0xFF04), 0x00);
        bus.write(0xFF05, 0x3C); // TIMA
        assert_eq!(bus.read(0xFF05), 0x3C);
        bus.write(0xFF06, 0x7E); // TMA
        assert_eq!(bus.read(0xFF06), 0x7E);
        bus.write(0xFF07, 0x04); // TAC: timer enabled, clock select 00 (note 01b)
        assert_eq!(bus.read(0xFF07), 0x04);

        // peek is side-effect-free and agrees with read on the timer registers.
        for addr in [0xFF04u16, 0xFF05, 0xFF06, 0xFF07] {
            assert_eq!(bus.peek(addr), bus.read(addr));
        }
    }

    #[test]
    fn c01_34_div_counts_through_the_machine() {
        let mut dmg = Dmg::new(&zero_rom(32 * 1024)).expect("ROM loads");
        assert_eq!(dmg.bus.read(0xFF04), 0xAB); // post-boot DIV (note 08)
        dmg.bus.write(0xFF04, 0x5A); // reset the system counter to $00 (note 01b)
        for _ in 0..(64 * 4 - 1) {
            dmg.tick();
        }
        assert_eq!(dmg.bus.read(0xFF04), 0x00); // DIV changes every 256 dots = 64 M-cycles (note 01b)
        dmg.tick(); // the 64th M-cycle: the counter crosses $0100
        assert_eq!(dmg.bus.read(0xFF04), 0x01);
    }

    #[test]
    fn c01_34_tima_counts_through_the_machine_for_each_tac() {
        for (select, period_mcycles) in [(0u8, 256u32), (1, 4), (2, 16), (3, 64)] {
            let mut dmg = Dmg::new(&zero_rom(32 * 1024)).expect("ROM loads");
            dmg.bus.write(0xFF04, 0x00); // zero the system counter: every selected bit starts at 0
            dmg.bus.write(0xFF07, 0x04 | select); // timer enabled + clock select (note 01b)
            for _ in 0..(2 * period_mcycles * 4) {
                dmg.tick();
            }
            assert_eq!(
                dmg.bus.read(0xFF05),
                2,
                "TAC select {} must tick every {} M-cycles",
                select,
                period_mcycles
            );
        }
    }

    #[test]
    fn c01_34_disabled_timer_does_not_count_through_the_machine() {
        let mut dmg = Dmg::new(&zero_rom(32 * 1024)).expect("ROM loads");
        dmg.bus.write(0xFF04, 0x00); // zero the system counter
        dmg.bus.write(0xFF07, 0x05 & !0x04); // clock select 01, enable clear (note 01b)
        for _ in 0..(256 * 4 * 4) {
            dmg.tick();
        }
        assert_eq!(dmg.bus.read(0xFF05), 0x00); // TIMA never increments while disabled
        assert_ne!(dmg.bus.read(0xFF04), 0x00); // DIV keeps counting regardless (note 01b)
    }

    #[test]
    fn c01_34_tima_overflow_requests_timer_interrupt_through_the_machine() {
        let mut dmg = Dmg::new(&zero_rom(32 * 1024)).expect("ROM loads");
        dmg.bus.write(0xFF04, 0x00); // zero the system counter
        dmg.bus.write(0xFF07, 0x05); // one increment every 4 M-cycles (note 01b)
        dmg.bus.write(0xFF05, 0xFF); // TIMA at its maximum: the next increment overflows
        dmg.bus.write(0xFF06, 0x7E); // TMA reload value (note 01b)
        assert_eq!(dmg.bus.io[0x0F] & 0x04, 0x00); // no timer request yet
        for _ in 0..(4 * 4) {
            dmg.tick(); // M-cycle N: the increment overflows at its end (note 01b "Retard d'un M-cycle")
        }
        assert_eq!(dmg.bus.io[0x0F] & 0x04, 0x00); // IF bit 2 not set yet on this M-cycle
        assert_eq!(dmg.bus.read(0xFF05), 0x00); // TIMA wrapped to $00; the reload is not visible yet
        for _ in 0..4 {
            dmg.tick(); // M-cycle N+1: the delayed request lands at its end (note 01b)
        }
        assert_eq!(dmg.bus.io[0x0F] & 0x04, 0x04); // IF bit 2: timer interrupt requested (note 01b)
        assert_eq!(dmg.bus.read(0xFF05), 0x7E); // TIMA reloaded with the current TMA value
    }

    #[test]
    fn c01_35_cpu_serves_timer_interrupt_through_the_machine() {
        let mut dmg = Dmg::new(&zero_rom(32 * 1024)).expect("ROM loads"); // all-NOP ROM, PC at $0100
        dmg.bus.write(0xFF04, 0x00); // zero the system counter
        dmg.bus.write(0xFF07, 0x05); // one increment every 4 M-cycles (note 01b)
        dmg.bus.write(0xFF05, 0xFF); // TIMA at its maximum: the next increment overflows
        dmg.bus.write(0xFF06, 0x23); // TMA reload value (note 01b)
        dmg.bus.io[0x0F] = 0xE0; // IF: no request yet (bits 5-7 read as 1)
        dmg.bus.write(0xFFFF, 0x04); // IE bit 2: timer interrupt enabled (note 02b)
        dmg.bus.write(0xC000, 0xFB); // EI in WRAM; the CPU runs NOPs from $C001 on
        for i in 1..8u16 {
            dmg.bus.write(0xC000 + i, 0x00); // explicit NOPs after the EI
        }
        dmg.cpu.sp = 0xDFFE; // IME is false by default (note 02b: cleared at reset)
        dmg.cpu.pc = 0xC000;

        let mut dispatched = false;
        for _ in 0..200 {
            dmg.tick(); // one M-cycle per tick (decision C_00)
            if dmg.cpu.pc == 0x0050 {
                dispatched = true; // timer vector reached: the dispatch completed (note 02b)
                break;
            }
        }
        assert!(dispatched, "no timer dispatch");

        assert_eq!(dmg.bus.io[0x0F] & 0x04, 0x00); // IF bit 2 acknowledged at entry (note 02b)
        assert_eq!(dmg.cpu.sp, 0xDFFC); // SP = SP - 2
        let pushed = ((dmg.bus.read(0xDFFD) as u16) << 8) | dmg.bus.read(0xDFFC) as u16;
        assert!(
            (0xC001..=0xC010).contains(&pushed),
            "pushed PC {:#06x} not in $C001..=$C010",
            pushed
        );
        // TIMA was reloaded with TMA ($23); the timer keeps counting during the entry, so at
        // most one further increment (TAC select 1: every 4 M-cycles) can have landed by the
        // time the dispatch completes. The exact reload timing is covered by c01_35 unit tests.
        assert!(
            dmg.bus.read(0xFF05).wrapping_sub(0x23) <= 1,
            "TIMA {:#04x} not reloaded from TMA",
            dmg.bus.read(0xFF05)
        );

        dmg.bus.io[0x0F] |= 0x01; // IF bit 0 (vblank): a second request, pending
        dmg.bus.write(0xFFFF, 0x05); // IE bits 0 and 2 enabled (note 02b)
        for _ in 0..20 {
            dmg.tick(); // IME was cleared by the service: no second dispatch to $0040 (note 02b)
        }
        assert!(
            (0x0050..=0x0070).contains(&dmg.cpu.pc),
            "second dispatch to vblank: PC {:#06x}",
            dmg.cpu.pc
        );
    }

    #[test]
    fn e03_08_stat_readback_composes_bit7_flag_and_mode() {
        let mut bus = Bus::new(vec![0xA5; 0x150]); // ROM-only cart, header present (decision A_06)
        bus.reset(); // post-boot STAT value $85 (note 08)
        assert_eq!(bus.io[0x41], 0x85); // raw post-boot value stored in the IO file, not masked by the write path
                                        // Bit 7 reads as 1; select bits 6..3 from $85 & 0x78 = 0; bit 2 = coincidence flag (LY=0 == LYC=0); mode bits from the mirror.
        assert_eq!(bus.read(0xFF41), 0x80 | 0x04 | 0b10); // $8C: bit7 + no select bits + LYC flag + mode 2 at line start
        bus.write(0xFF41, 0x3F); // only bits 6..3 are writable (E03_08 spec)
        assert_eq!(bus.io[0x41], 0x38); // stored select bits: b5+b4+b3 set, bit7 and bit2 not stored
        bus.ly = 0x90; // LY no longer equals LYC ($00): the flag clears continuously (note 04a "LYC")
        assert_eq!(bus.read(0xFF41), 0x80 | 0x38 | 0b10); // bit7 + select bits as written, flag clear
    }

    #[test]
    fn e03_08_stat_write_masks_bits_2_1_and_0() {
        let mut bus = Bus::new(vec![0xA5; 0x150]);
        bus.reset();
        for val in [0xFFu8, 0x7F, 0x43, 0x0C] {
            bus.write(0xFF41, val);
            assert_eq!(bus.io[0x41], val & 0x78); // bits 2..0 and bit 7 are not stored (E03_08 spec)
        }
        // The readback still reports the live state: bit 7 = 1, flag from LY/LYC, mode from the mirror.
        bus.ly = 0x90;
        assert_eq!(bus.read(0xFF41), 0x80 | (bus.io[0x41] & 0x78) | 0b10); // select bits as written, flag clear
    }

    #[test]
    fn e03_08_lyc_is_plain_rw_at_ff45() {
        let mut bus = Bus::new(vec![0xA5; 0x150]);
        bus.reset(); // LYC post-boot value $00 (note 08)
        assert_eq!(bus.read(0xFF45), 0x00);
        bus.write(0xFF45, 0xC3); // R/W: the full byte is stored (E03_08 spec)
        assert_eq!(bus.io[0x45], 0xC3);
        assert_eq!(bus.read(0xFF45), 0xC3);
    }

    #[test]
    fn e03_08_coincidence_flag_set_and_cleared_when_ly_equals_lyc() {
        let mut bus = Bus::new(vec![0xA5; 0x150]);
        bus.reset(); // LY = 0, LYC = 0: coincidence at power-up (note 04a "LYC")
        assert_eq!(bus.read(0xFF41) & 0x04, 0x04); // flag set while LY == LYC
        bus.ly = 5; // the PPU advances past the compare value: the flag clears continuously
        assert_eq!(bus.read(0xFF41) & 0x04, 0x00);
        bus.write(0xFF45, 5); // writing LYC to the current line sets it again (note 04a "LYC")
        assert_eq!(bus.read(0xFF41) & 0x04, 0x04);
    }

    #[test]
    fn e03_08_stat_readback_reports_mode_bits() {
        let mut bus = Bus::new(vec![0xA5; 0x150]);
        bus.reset(); // mode mirror starts at OAM scan (E03_01)
        assert_eq!(bus.read(0xFF41) & 0x03, 0b10); // mode 2 at the start of line 0
        for (mode, expected) in [(0u8, 0b00), (1, 0b01), (2, 0b10), (3, 0b11)] {
            bus.ppu_mode = mode; // Machine::tick keeps the mirror current (decision A_02)
            assert_eq!(bus.read(0xFF41) & 0x03, expected); // bits 1..0 report the PPU mode (E03_08 spec)
        }
    }

    #[test]
    fn e03_06_ff46_reads_back_the_written_value() {
        let mut bus = Bus::new(test_rom(32 * 1024));
        bus.write(0xFF46, 0xC1); // DMA register: the written value is stored (E03_06 spec)
        assert_eq!(bus.read(0xFF46), 0xC1); // read-back returns the last written value
    }

    #[test]
    fn e03_06_copy_from_wram() {
        let mut bus = Bus::new(test_rom(32 * 1024));
        for i in 0..160u16 {
            bus.write(0xC100 + i, (i as u8) ^ 0x5A); // source pattern in WRAM at $C100-$C19F
        }
        bus.write(0xFF46, 0xC1); // start the transfer from $C100 (E03_06 spec)
        for _ in 0..160 {
            bus.dma_step(); // one byte per M-cycle: 160 steps total
        }
        assert!(!bus.dma_active()); // the transfer is complete after exactly 160 steps
        for i in 0..160usize {
            assert_eq!(bus.oam[i], (i as u8) ^ 0x5A); // every OAM byte copied from WRAM
        }
        assert_eq!(bus.oam[160], 0); // the transfer stops at 160 bytes: OAM $FEA0 untouched
    }

    #[test]
    fn e03_06_takes_exactly_160_steps() {
        let mut bus = Bus::new(test_rom(32 * 1024));
        bus.write(0xFF46, 0xC0); // start the transfer from $C000 (E03_06 spec)
        assert!(bus.dma_active()); // active immediately after the write: no start delay here
        for _ in 0..159 {
            bus.dma_step();
        }
        assert!(bus.dma_active()); // still running on step 159 of 160
        bus.dma_step(); // the 160th and final step
        assert!(!bus.dma_active()); // inactive after exactly 160 steps
    }
}
