//! Bus: owns every addressable memory region (decision A_02).
//! read/write dispatch and side-effect-free peek() per decision A_02; PPU-mode blocking
//! and OAM DMA gating arrive with E03.

use crate::audio::wave::WaveAccess;
use crate::input::{p1_read, Input};
use crate::media::header::Header;
use crate::media::mapper::{Mapper, RomOnly};
use crate::timer::Timer;

/// IO register file FF00-FFFF size, including IF ($FF0F) and IE ($FFFF) (decision A_02).
pub const IO_SIZE: usize = 0x100;
#[cfg(test)]
mod boot_tests;
mod state;

/// Error returned by `Bus::set_boot_rom` (E14_02).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BootRomError {
    /// The boot ROM image does not have the expected length.
    BadSize,
}

/// Header checksum address in bank 0 (note 07a).
pub const ROM_HEADER_CHECKSUM: usize = 0x14D;

/// Read masks of $FF10-$FF26: a read returns the stored value OR the mask (unused and write-only bits read 1; consistent with the post-boot table of note 05a). Which value a write-only register returns: UNKNOWN - to confirm.
const APU_READ_MASK: [u8; 23] = [
    0x80, 0x3F, 0x00, 0xFF, 0xBF, 0xFF, 0x3F, 0x00, 0xFF, 0xBF, 0x7F, 0xFF, 0x9F, 0xFF, 0xBF, 0xFF,
    0xFF, 0x00, 0x00, 0xBF, 0x00, 0x00, 0x70,
]; // index = address - 0xFF10: NR10..NR52 with the unused $FF15 and $FF1F slots (E04_01 spec)

/// M-cycles (dma_step calls, counting the one of the write cycle) between a write to $FF46 and the first blocked CPU access; mooneye oam_dma_start: write at M=0, nothing at M=1, DMA active at M=2 (FIX_02).
const DMA_START_DELAY_STEPS: u8 = 2;

/// Bus segment an OAM DMA source address belongs to, for CPU access collision checks (E03_15 spec, FIX_02).
#[derive(Clone, Copy, PartialEq, Eq)]
enum DmaBus {
    /// ROM and cartridge RAM segments ($0000-$7FFF, $A000-$FDFF including echo RAM).
    External,
    /// VRAM segment ($8000-$9FFF).
    Video,
    /// Everything else (HRAM, I/O, IE): never blocked by the DMA.
    Other,
}

/// Bus segment of an address for OAM DMA collision checks (E03_15 spec, FIX_02).
fn dma_bus(addr: u16) -> DmaBus {
    match addr {
        0x0000..=0x7FFF | 0xA000..=0xFDFF => DmaBus::External,
        0x8000..=0x9FFF => DmaBus::Video,
        _ => DmaBus::Other,
    }
}

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
    /// Value of the STAT LY==LYC flag frozen while the LCD is off (None while the LCD is on: live compare) (mooneye stat_lyc_onoff, FIX_08).
    pub(crate) lyc_frozen: Option<bool>,
    /// true during the first 4 dots of a line: the STAT LY==LYC flag reads 0 there (mooneye lcdon_timing-GS, FIX_09)
    pub(crate) lyc_flag_hold: bool,
    /// CPU access to VRAM ($8000-$9FFF) allowed (E03_10): mirrored from the PPU by Machine::tick.
    vram_open: bool,
    /// CPU access to OAM ($FE00-$FE9F) allowed (E03_10): mirrored from the PPU by Machine::tick.
    oam_open: bool,
    /// CPU write access to VRAM ($8000-$9FFF) allowed (FIX_10): mirrored from the PPU by Machine::tick.
    pub(crate) vram_write_ok: bool,
    /// CPU write access to OAM ($FE00-$FE9F) allowed (FIX_10): mirrored from the PPU by Machine::tick.
    pub(crate) oam_write_ok: bool,
    /// OAM DMA transfer running (E03_06 spec): one byte copied per M-cycle via dma_step.
    dma_active: bool,
    /// OAM DMA source address base = V << 8 from the last write to $FF46 (E03_06 spec).
    dma_src: u16,
    /// Next destination index of the running OAM DMA transfer, 0..=159 (E03_06 spec).
    dma_index: u8,
    /// Source base pending for the next OAM DMA start, V << 8 from the last write to $FF46 (FIX_02).
    dma_pending_src: u16,
    /// M-cycles remaining until a new OAM DMA transfer starts after a write to $FF46 (FIX_02).
    dma_start_delay: u8,
    /// Last accepted APU register write (address, value), drained by the machine each M-cycle (E04_02).
    pub apu_write: Option<(u16, u8)>,
    /// Cartridge mapper for the loaded ROM (decision A_06): ROM banking, RAM and RTC;
    /// RomOnly fallback when the header is missing or unsupported (E05_07).
    pub mapper: Mapper,
    /// CPU access rule for wave RAM $FF30-$FF3F, mirrored from the wave channel by Machine::tick (E04_07).
    pub wave_access: WaveAccess,
    /// Debug tool for trace comparison, task C01_49: when Some(v), every read of $FF44
    /// returns v instead of the PPU scanline; None (default) keeps normal behaviour.
    ly_override: Option<u8>,
    /// Timer and divider registers DIV/TIMA/TMA/TAC ($FF04-$FF07, note 01b).
    pub timer: Timer,
    /// Current button state (E05_01).
    pub input: Input,
    /// Low nibble of P1 at the previous check, for falling-edge detection (E05_01).
    p1_prev: u8,
    /// Boot ROM image overlaid on $0000-$00FF while `boot_mapped` is true (E14_02, decision A_11).
    boot_rom: Option<Box<[u8]>>,
    /// True while the boot ROM is visible; a non-zero write to $FF50 clears it (E14_02).
    boot_mapped: bool,
}

impl Bus {
    /// Build a bus around the cartridge ROM image, all RAM zeroed.
    pub fn new(rom: Vec<u8>) -> Self {
        let mapper = Self::build_mapper(&rom);
        Bus {
            rom,
            vram: [0x00; 0x2000],
            wram: [0x00; 0x2000],
            oam: [0x00; 0x100],
            hram: [0x00; 0x7F],
            io: [0xFF; IO_SIZE],
            serial: [0x00; 256],
            serial_len: 0,
            ly: 0,                // LY ($FF44) post-boot value (note 08, C01_15)
            ppu_mode: 2, // PPU mode at the start of line 0 is OAM scan (E03_01); Machine::tick keeps it current
            lyc_frozen: None, // no frozen coincidence flag while the LCD is on (FIX_08)
            lyc_flag_hold: false, // LY==LYC flag not held low at power-up (FIX_09)
            vram_open: true, // VRAM access allowed until Machine::tick mirrors the PPU state (E03_10)
            oam_open: true, // OAM access allowed until Machine::tick mirrors the PPU state (E03_10)
            vram_write_ok: true, // VRAM writes allowed until Machine::tick mirrors the PPU state (FIX_10)
            oam_write_ok: true, // OAM writes allowed until Machine::tick mirrors the PPU state (FIX_10)
            dma_active: false,  // no OAM DMA transfer running at power-up (E03_06 spec)
            dma_src: 0,         // source base set by a write to $FF46 (E03_06 spec)
            dma_index: 0,       // next destination index of the running transfer (E03_06 spec)
            dma_pending_src: 0, // no pending OAM DMA source at power-up (FIX_02)
            dma_start_delay: 0, // no start delay pending at power-up (FIX_02)
            apu_write: None,    // no pending APU register write at power-up (E04_02)
            mapper,
            wave_access: WaveAccess::Free,
            ly_override: None,       // no debug override by default (C01_49)
            timer: Timer::default(), // DIV/TIMA/TMA/TAC are set in reset() (note 08)
            input: Input::default(), // no button pressed at power-up (E05_01)
            p1_prev: 0x0F,           // P1 low nibble before the first check (E05_01)
            boot_rom: None,          // no boot ROM installed by default (E14_02)
            boot_mapped: false,      // cartridge visible from power-up (E14_02)
        }
    }

    /// Build the cartridge mapper for a ROM image (E05_07): the header at $0134-$014F selects
    /// the type; a missing or unsupported header falls back to ROM-only so any image loads.
    fn build_mapper(rom: &[u8]) -> Mapper {
        match Header::parse(rom) {
            Ok(header) => {
                Mapper::new(rom.to_vec(), &header).unwrap_or_else(|_| Self::rom_only_fallback(rom))
            }
            Err(_) => Self::rom_only_fallback(rom),
        }
    }

    /// ROM-only fallback mapper: RAM size code $00 (no RAM) is always valid, so this never fails.
    fn rom_only_fallback(rom: &[u8]) -> Mapper {
        Mapper::RomOnly(
            RomOnly::new(rom.to_vec(), 0x00).expect("no-RAM ROM-only mapper always loads"),
        )
    }

    /// Post-boot state of the memory owned by the Bus (note 08, decision A_06).
    pub fn reset(&mut self) {
        // WRAM/HRAM filled deterministically with $00 at power-up (note 08 "RAM after
        // power-up", decision A_05); echo RAM mirrors the current WRAM bank by routing,
        // so it carries no separate state.
        self.wram.fill(0x00);
        self.hram.fill(0x00);
        // Cartridge RAM is NOT kept across reset (E05_07): rebuild the mapper from the ROM;
        // the frontend reloads battery RAM with load_battery_ram.
        self.mapper = Self::build_mapper(&self.rom);
        self.serial_len = 0; // no captured serial bytes at power-up (C01_03)
        self.ly = 0; // LY ($FF44) post-boot value (note 08, C01_15)
        self.ppu_mode = 2; // PPU mode restarts at the start of line 0: OAM scan (E03_01)
        self.lyc_frozen = None; // no frozen coincidence flag while the LCD is on (FIX_08)
        self.lyc_flag_hold = false; // LY==LYC flag not held low at power-up (FIX_09)
        self.vram_open = true; // VRAM access allowed until Machine::tick mirrors the PPU state (E03_10)
        self.oam_open = true; // OAM access allowed until Machine::tick mirrors the PPU state (E03_10)
        self.vram_write_ok = true; // VRAM writes allowed until Machine::tick mirrors the PPU state (FIX_10)
        self.oam_write_ok = true; // OAM writes allowed until Machine::tick mirrors the PPU state (FIX_10)
        self.dma_active = false; // no OAM DMA transfer running at power-up (E03_06 spec)
        self.apu_write = None; // no pending APU register write at power-up (E04_02)
        self.dma_index = 0; // next destination index of the running transfer (E03_06 spec)
        self.dma_pending_src = 0; // no pending OAM DMA source at power-up (FIX_02)
        self.dma_start_delay = 0; // no start delay pending at power-up (FIX_02)
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
        io[0x16] = 0x3F; // NR21
        io[0x17] = 0x00; // NR22
        io[0x18] = 0xFF; // NR23
        io[0x19] = 0xBF; // NR24
        io[0x1A] = 0x7F; // NR30
        io[0x1B] = 0xFF; // NR31
        io[0x1C] = 0x9F; // NR32
        io[0x1D] = 0xFF; // NR33
        io[0x1E] = 0xBF; // NR34
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
        self.p1_prev = p1_read(self.io[0x00], &self.input) & 0x0F; // P1 low nibble at power-up (E05_01)
    }

    /// Installs a boot ROM image and maps it over $0000-$00FF (E14_02). The image must have
    /// exactly `expected_len` bytes, else nothing changes.
    pub fn set_boot_rom(&mut self, data: &[u8], expected_len: usize) -> Result<(), BootRomError> {
        if data.len() != expected_len {
            return Err(BootRomError::BadSize);
        }
        self.boot_rom = Some(data.to_vec().into_boxed_slice());
        self.boot_mapped = true;
        Ok(())
    }

    /// Maps the installed boot ROM again (no effect when none is installed) (E14_02).
    pub fn remap_boot(&mut self) {
        self.boot_mapped = self.boot_rom.is_some();
    }

    /// True when a boot ROM image is installed (E14_03).
    pub fn has_boot_rom(&self) -> bool {
        self.boot_rom.is_some()
    }

    /// True while the boot ROM is visible over $0000-$00FF (E14_03).
    pub fn boot_mapped(&self) -> bool {
        self.boot_mapped
    }

    /// Power-on state of the Bus-owned I/O (E14_03, note 10): every I/O register is 0 (all values
    /// UNKNOWN in note 10 are 0, so the LCD is off), the timer restarts from zero and the boot
    /// ROM is mapped when one is installed. Call it after `reset`.
    pub fn power_on(&mut self) {
        self.io.fill(0x00);
        self.timer = Timer::default();
        self.p1_prev = p1_read(self.io[0x00], &self.input) & 0x0F;
        self.remap_boot();
    }

    /// Boot ROM byte at `addr` ($FF when out of range); never allocates (E14_02).
    fn boot_byte(&self, addr: u16) -> u8 {
        self.boot_rom
            .as_deref()
            .and_then(|rom| rom.get(usize::from(addr)))
            .copied()
            .unwrap_or(0xFF)
    }

    /// CPU accesses that collide with the OAM DMA: OAM and the bus of the DMA source. Hardware-verified by mooneye call_timing (DMA from VRAM leaves WRAM/echo usable). The value a colliding read returns is the DMA byte on a real DMG: UNKNOWN - to confirm, $FF is used. DMA sources $E0-$FF map to $C0-$DF (FIX_06).
    fn dma_blocks(&self, addr: u16) -> bool {
        if !self.dma_active {
            return false;
        }
        if (0xFE00..=0xFE9F).contains(&addr) {
            return true; // OAM is never readable during a DMA (E03_15 spec)
        }
        dma_bus(addr) != DmaBus::Other && dma_bus(addr) == dma_bus(self.dma_src)
    }

    /// Read a byte from the bus (decision A_02). PPU-mode blocking and OAM DMA gating: E03_15.
    pub fn read(&self, addr: u16) -> u8 {
        if self.dma_blocks(addr) {
            return 0xFF; // OAM DMA collision: reads $FF (E03_15 spec, FIX_02); HRAM/I/O/IE stay accessible
        }
        if self.boot_mapped && addr <= 0x00FF {
            return self.boot_byte(addr); // boot ROM overlay (E14_02)
        }
        match addr {
            0x0000..=0x7FFF => self.mapper.read(addr), // cartridge ROM region through the mapper (E05_07); past the image reads $FF
            0x8000..=0x9FFF => {
                if self.vram_open {
                    self.vram[(addr - 0x8000) as usize]
                } else {
                    0xFF // blocked by the PPU mode: reads $FF (E03_10 spec)
                }
            }
            0xA000..=0xBFFF => self.mapper.read(addr), // cartridge RAM region through the mapper (E05_07); absent or disabled RAM reads $FF
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
            0xFF00 => self.p1(), // P1 ($FF00): computed from the select bits and the buttons (E05_01)
            0xFF0F => 0xE0 | (self.io[0x0F] & 0x1F), // IF ($FF0F): bits 5-7 read as 1 (task C01_32)
            0xFF04 => self.timer.div(), // DIV ($FF04, note 01b)
            0xFF05 => self.timer.tima(), // TIMA ($FF05, note 01b)
            0xFF06 => self.timer.tma(), // TMA ($FF06, note 01b)
            0xFF07 => 0xF8 | self.timer.tac(), // TAC ($FF07): bits 7-3 are unused and read 1 (mooneye unused_hwio-GS, FIX_05)
            0xFF10..=0xFF26 => {
                self.io[(addr - 0xFF00) as usize] | APU_READ_MASK[(addr - 0xFF10) as usize]
            } // APU registers with read masks (E04_01 spec)
            // Unassigned I/O $FF27-$FF2F: always read $FF (dmg_sound 01-registers test 2).
            0xFF27..=0xFF2F => 0xFF,
            0xFF30..=0xFF3F => match self.wave_access {
                WaveAccess::Free => self.io[(addr - 0xFF00) as usize],
                WaveAccess::Closed => 0xFF,
                WaveAccess::Open(i) => self.io[0x30 + usize::from(i & 15)],
            },
            0xFF02 => self.io[0x02] | 0x7E, // SC ($FF02): bits 6-1 are unused and read 1 on DMG (mooneye unused_hwio-GS, FIX_05)
            0xFF03 | 0xFF08..=0xFF0E | 0xFF4C..=0xFF7F => 0xFF, // unmapped I/O reads $FF (mooneye unused_hwio-GS, FIX_05)
            0xFF01..=0xFF7F => self.io[(addr - 0xFF00) as usize],
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

    /// Mirror the PPU's CPU write access state for VRAM and OAM (FIX_10): called by Machine::tick
    /// after every PPU step, and at power-up. The PPU's own writes use the raw arrays directly.
    pub(crate) fn set_video_write_access(&mut self, vram: bool, oam: bool) {
        self.vram_write_ok = vram;
        self.oam_write_ok = oam;
    }

    /// Write a byte to the bus (decision A_02). ROM and unmapped/unusable ranges are
    /// ignored; PPU-mode blocking and OAM DMA gating: E03_15.
    pub fn write(&mut self, addr: u16, val: u8) {
        if self.dma_blocks(addr) {
            return; // OAM DMA collision: writes are ignored (E03_15 spec, FIX_02)
        }
        match addr {
            0x0000..=0x7FFF => self.mapper.write(addr, val), // cartridge registers through the mapper (E05_07)
            0x8000..=0x9FFF => {
                if self.vram_write_ok {
                    self.vram[(addr - 0x8000) as usize] = val;
                } // blocked by the PPU mode: writes are ignored (E03_10 spec, FIX_10)
            }
            0xA000..=0xBFFF => self.mapper.write(addr, val), // cartridge RAM through the mapper; absent or disabled RAM ignores it (E05_07)
            0xC000..=0xDFFF => self.wram[(addr - 0xC000) as usize] = val,
            0xE000..=0xFDFF => {
                // Echo RAM: only the lower 13 bits of the address are connected, so it wraps onto
                // the current WRAM bank (C000-C7FF on DMG) exactly like a read (note 03a "Echo RAM").
                self.wram[(addr as usize) & 0x1FFF] = val;
            }
            0xFE00..=0xFE9F => {
                if self.oam_write_ok {
                    self.oam[(addr - 0xFE00) as usize] = val;
                } // blocked by the PPU mode: writes are ignored (E03_10 spec, FIX_10)
            }
            0xFEA0..=0xFEFF => {} // unusable range: writes are ignored (note 03a)
            0xFF50 => {
                if val != 0 {
                    self.boot_mapped = false; // BOOT ($FF50): a non-zero write unmaps the boot ROM; only remap_boot brings it back (E14_02)
                }
            } // reading $FF50 is unchanged (E14_02)
            0xFF44 => {} // LY ($FF44) is read-only; writes are ignored (C01_15, note 04a "LY")
            0xFF41 => self.io[0x41] = val & 0x78, // STAT ($FF41): only the select bits 6..3 are writable (E03_08 spec); bit 2 and bits 1..0 read back live; LYC ($FF45) is plain R/W through the IO file
            0xFF00 => {
                self.io[0x00] = 0xC0 | (val & 0x30); // P1 ($FF00): only the select bits 5-4 are writable (E05_01)
                self.joypad_edge();
            }
            0xFF0F => self.io[0x0F] = val & 0x1F, // IF ($FF0F): only bits 0-4 writable (task C01_32)
            0xFF04 => self.timer.write_div(val), // DIV ($FF04): any value resets the counter (note 01b)
            0xFF05 => self.timer.write_tima(val), // TIMA ($FF05, note 01b)
            0xFF06 => self.timer.write_tma(val), // TMA ($FF06, note 01b)
            0xFF07 => self.timer.write_tac(val), // TAC ($FF07, note 01b)
            0xFF46 => {
                self.io[0x46] = val; // DMA reads back the last written value (E03_06 spec)
                                     // Sources $E0-$FF read the echo of WRAM: the high byte is mapped to $C0-$DF (mooneye oam_dma/sources-GS, FIX_06). The register keeps the written value.
                let src_hi = if val >= 0xE0 { val - 0x20 } else { val };
                self.dma_pending_src = u16::from(src_hi) << 8; // source base of the next transfer, V * 0x100 (FIX_02)
                self.dma_start_delay = DMA_START_DELAY_STEPS; // start delay after the write cycle (FIX_02); an earlier transfer keeps running until the new one starts
            }
            0xFF26 => {
                // NR52: bit 7 = APU power; bits 3-0 are read-only channel status, a write never changes them (E04_01 spec, note 05a/05b)
                let old = self.io[0x26];
                if val & 0x80 != 0 {
                    self.io[0x26] = 0x80 | (old & 0x0F);
                } else {
                    if old & 0x80 != 0 {
                        self.io[0x10..=0x25].fill(0x00); // power off clears every APU register; wave RAM FF30-FF3F and the DIV-APU counter are not affected (E04_01 spec, note 05b)
                    }
                    self.io[0x26] = 0x00; // power off: status bits 3-0 are 0 too
                }
                self.apu_write = Some((addr, val));
            }
            0xFF11 | 0xFF16 | 0xFF1B | 0xFF20 if self.io[0x26] & 0x80 == 0 => {
                // DMG: length counters can still be written while the APU is off; the register
                // itself stays cleared and the duty bits are not applied (gbdev wiki "Gameboy sound
                // hardware"). Only the length part is relayed to the APU.
                let length_part = if addr == 0xFF1B { val } else { val & 0x3F };
                self.apu_write = Some((addr, length_part));
            }
            0xFF10..=0xFF25 => {
                if self.io[0x26] & 0x80 != 0 {
                    self.io[(addr - 0xFF00) as usize] = val; // APU registers are read-only while the APU is off (E04_01 spec, note 05b)
                    self.apu_write = Some((addr, val));
                }
            }
            // Unassigned I/O $FF27-$FF2F: writes are ignored (dmg_sound 01-registers test 2).
            0xFF27..=0xFF2F => {}
            0xFF30..=0xFF3F => match self.wave_access {
                WaveAccess::Free => self.io[(addr - 0xFF00) as usize] = val,
                WaveAccess::Closed => {}
                WaveAccess::Open(i) => self.io[0x30 + usize::from(i & 15)] = val,
            },
            0xFF03 | 0xFF08..=0xFF0E | 0xFF4C..=0xFF7F => {} // unmapped I/O: writes are ignored (mooneye unused_hwio-GS, FIX_05)
            0xFF01..=0xFF7F => {
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

    /// P1 ($FF00) as read by the CPU (E05_01, note 06).
    pub fn p1(&self) -> u8 {
        p1_read(self.io[0x00], &self.input)
    }

    /// Store a new button state; a falling edge on P1 bits 3-0 requests the joypad interrupt.
    pub fn set_buttons(&mut self, input: Input) {
        self.input = input;
        self.joypad_edge();
    }

    /// IF bit 4 (joypad) is requested when a bit of the P1 low nibble goes from 1 to 0 (note 06).
    /// Contact bounce and the STOP wake-up are not modelled: UNKNOWN - to confirm.
    fn joypad_edge(&mut self) {
        let now = self.p1() & 0x0F;
        if self.p1_prev & !now != 0 {
            self.io[0x0F] |= 0x10;
        }
        self.p1_prev = now;
    }

    /// Clear one IF ($FF0F) request bit (task C01_32): the acknowledge at interrupt entry.
    pub fn clear_if_bit(&mut self, mask: u8) {
        self.io[0x0F] &= !(mask & 0x1F);
    }

    /// LY==LYC coincidence flag: frozen while the LCD is off, live otherwise (FIX_08).
    pub(crate) fn lyc_flag(&self) -> bool {
        match self.lyc_frozen {
            Some(f) => f,
            None => self.ly == self.io[0x45] && !self.lyc_flag_hold,
        }
    }

    /// STAT ($FF41) readback (E03_08): bit 7 reads as 1; bits 6..3 are the interrupt select
    /// bits as written; bit 2 is the coincidence flag, set while LY == LYC and updated
    /// continuously (note 04a "LYC"); bits 1..0 report the current PPU mode. The stored byte
    /// holds only the writable select bits, so the readback composes them with the live state.
    pub fn stat_read(&self) -> u8 {
        let mut v = 0x80 | (self.io[0x41] & 0x78); // bit 7 fixed at 1; select bits as written
        if self.lyc_flag() {
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

    /// Mirror the channel 1 to 4 enabled flags into NR52 bits 0-3.
    pub fn set_apu_status(&mut self, bits: u8) {
        self.io[0x26] = (self.io[0x26] & !0x0F) | (bits & 0x0F);
    }

    /// Copy of wave RAM ($FF30-$FF3F) for the wave channel. Access rules while channel 3 plays: UNKNOWN - to confirm.
    pub fn wave_ram(&self) -> [u8; 16] {
        let mut ram = [0u8; 16];
        ram.copy_from_slice(&self.io[0x30..0x40]);
        ram
    }

    /// DMG retrigger corruption of wave RAM (E04_07): see audio::wave::corrupt_ram.
    pub fn corrupt_wave_ram(&mut self, index: u8) {
        let mut ram = self.wave_ram();
        crate::audio::wave::corrupt_ram(&mut ram, index);
        self.io[0x30..0x40].copy_from_slice(&ram);
    }

    /// Raw NR50 (master volume) and NR51 (panning) for the mixer. Both read 0 while the APU is off.
    pub fn audio_pan(&self) -> (u8, u8) {
        (self.io[0x24], self.io[0x25])
    }

    /// One M-cycle of the OAM DMA (E03_06 spec): copy source byte `dma_index` into OAM, not blocked by
    /// the PPU mode. 160 steps in total. Start delay after a write to $FF46: DMA_START_DELAY_STEPS (FIX_02).
    pub fn dma_step(&mut self) {
        if self.dma_active {
            let byte = self.peek(self.dma_src.wrapping_add(u16::from(self.dma_index)));
            self.oam[usize::from(self.dma_index)] = byte;
            self.dma_index += 1;
            if self.dma_index == 160 {
                self.dma_active = false;
            }
        }
        if self.dma_start_delay > 0 {
            self.dma_start_delay -= 1;
            if self.dma_start_delay == 0 {
                self.dma_src = self.dma_pending_src; // the pending source starts now (FIX_02)
                self.dma_index = 0;
                self.dma_active = true;
            }
        }
    }

    /// Read the raw memory at `addr` with no side effect and no gate (decision A_02):
    /// for debuggers and viewers. PPU-mode blocking and OAM DMA gating: E03_15.
    pub fn peek(&self, addr: u16) -> u8 {
        if self.boot_mapped && addr <= 0x00FF {
            return self.boot_byte(addr); // boot ROM overlay (E14_02)
        }
        match addr {
            0x0000..=0x7FFF => self.mapper.read(addr), // cartridge ROM region through the mapper (E05_07); past the image reads $FF
            0x8000..=0x9FFF => self.vram[(addr - 0x8000) as usize],
            0xA000..=0xBFFF => self.mapper.read(addr), // cartridge RAM region through the mapper (E05_07); absent or disabled RAM reads $FF
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
            0xFF00 => self.p1(), // P1 ($FF00): computed from the select bits and the buttons (E05_01)
            0xFF0F => 0xE0 | (self.io[0x0F] & 0x1F), // IF ($FF0F): bits 5-7 read as 1 (task C01_32)
            0xFF04 => self.timer.div(), // DIV ($FF04, note 01b)
            0xFF05 => self.timer.tima(), // TIMA ($FF05, note 01b)
            0xFF06 => self.timer.tma(), // TMA ($FF06, note 01b)
            0xFF07 => 0xF8 | self.timer.tac(),
            0xFF02 => self.io[0x02] | 0x7E, // same as read() (FIX_05)
            0xFF03 | 0xFF08..=0xFF0E | 0xFF4C..=0xFF7F => 0xFF, // same as read() (FIX_05)
            0xFF01..=0xFF7F => self.io[(addr - 0xFF00) as usize],
            0xFF80..=0xFFFE => self.hram[(addr - 0xFF80) as usize],
            0xFFFF => self.io[IO_SIZE - 1], // IE (decision A_02)
        }
    }

    /// Blargg result block in cartridge RAM (dmg_sound ROMs): when $A001-$A003 hold
    /// $DE $B0 $61 and the status byte $A000 is not $80 (still running), copy the
    /// zero-terminated text that starts at $A004 into `out` and return its length.
    /// Returns 0 when there is no SRAM, no signature, or the ROM is still running.
    pub fn cart_result(&self, out: &mut [u8]) -> usize {
        let s = self.mapper.ram_bytes(); // cartridge RAM through the mapper (E05_07); empty on a ROM-only cart
        if s.len() < 4 || s[1] != 0xDE || s[2] != 0xB0 || s[3] != 0x61 || s[0] == 0x80 {
            return 0;
        }
        let mut n = 0;
        for &b in &s[4..] {
            if b == 0 || n == out.len() {
                break;
            }
            out[n] = b;
            n += 1;
        }
        n
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
        bus.write(0xFF01, 0xC5); // SB
        assert_eq!(bus.read(0xFF01), 0xC5);
        bus.write(0xFF47, 0x3C);
        assert_eq!(bus.read(0xFF47), 0x3C);
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
        bus.write(0xFF01, 0xC5);
        assert_eq!(bus.peek(0xFF01), 0xC5); // IO
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
        bus.write(0xFF01, 0xC5);
        for _ in 0..8 {
            assert_eq!(bus.peek(0xFF01), 0xC5); // peek has no side effect (decision A_02)
        }
        assert_eq!(bus.read(0xFF01), 0xC5);
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
        bus.write(0xFF07, 0x04); // TAC: bits 7-3 read 1 (FIX_05)
        assert_eq!(bus.read(0xFF07), 0xFC);

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
        for _ in 0..162 {
            bus.dma_step(); // two start-delay steps plus one byte per M-cycle: 160 copy steps (FIX_02)
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
        assert!(!bus.dma_active()); // not active yet: the start delay is pending (FIX_02)
        bus.dma_step(); // M-cycle 1 of the start delay after the write cycle
        assert!(!bus.dma_active());
        bus.dma_step(); // M-cycle 2: the transfer starts here (mooneye oam_dma_start, FIX_02)
        assert!(bus.dma_active());
        for _ in 0..159 {
            bus.dma_step();
        }
        assert!(bus.dma_active()); // still running on step 159 of 160 copy steps
        bus.dma_step(); // the 160th and final copy step
        assert!(!bus.dma_active()); // inactive after exactly 160 copy steps (FIX_02)
    }

    #[test]
    fn e03_15_read_outside_hram_returns_ff_during_dma() {
        let mut bus = Bus::new(test_rom(32 * 1024));
        bus.write(0xC000, 0x42); // WRAM byte before the transfer starts
        assert_eq!(bus.read(0xC000), 0x42); // normal read outside HRAM
        bus.write(0xFF46, 0xC1); // start the OAM DMA (E03_06 spec)
        bus.dma_step(); // M-cycle 1 of the start delay after the write cycle (FIX_02)
        bus.dma_step(); // M-cycle 2: the transfer is now running (FIX_02)
        assert_eq!(bus.read(0xC000), 0xFF); // during the transfer: reads $FF (E03_15 spec)
    }

    #[test]
    fn e03_15_hram_and_ie_stay_usable_during_dma() {
        let mut bus = Bus::new(test_rom(32 * 1024));
        bus.write(0xFF46, 0xC1); // start the OAM DMA (E03_06 spec)
        bus.write(0xFF80, 0x77); // HRAM stays usable during the transfer (E03_15 spec)
        assert_eq!(bus.read(0xFF80), 0x77);
        bus.write(0xFFFF, 0x1F); // IE: UNKNOWN - to confirm; kept accessible here
        assert_eq!(bus.read(0xFFFF), 0x1F);
    }

    #[test]
    fn e03_15_write_outside_hram_is_ignored_during_dma() {
        let mut bus = Bus::new(test_rom(32 * 1024));
        bus.write(0xC000, 0x42); // WRAM byte before the transfer starts
        bus.write(0xFF46, 0xC1); // start the OAM DMA (E03_06 spec)
        bus.dma_step(); // M-cycle 1 of the start delay after the write cycle (FIX_02)
        bus.dma_step(); // M-cycle 2: the transfer is now running (FIX_02)
        bus.write(0xC000, 0x99); // ignored while the transfer runs (E03_15 spec)
        for _ in 0..160 {
            bus.dma_step(); // run the whole transfer to completion
        }
        assert_eq!(bus.read(0xC000), 0x42); // the ignored write left WRAM untouched
        bus.write(0xC000, 0x99); // after the transfer: writes work again
        assert_eq!(bus.read(0xC000), 0x99);
    }

    #[test]
    fn e03_15_restriction_ends_exactly_with_the_transfer() {
        let mut bus = Bus::new(test_rom(32 * 1024));
        bus.write(0xC000, 0x42); // WRAM byte before the transfer starts
        bus.write(0xFF46, 0xC0); // start the OAM DMA (E03_06 spec)
        bus.dma_step(); // M-cycle 1 of the start delay after the write cycle (FIX_02)
        bus.dma_step(); // M-cycle 2: the transfer is now running (FIX_02)
        for _ in 0..159 {
            bus.dma_step(); // all but the final step: still active
        }
        assert_eq!(bus.read(0xC000), 0xFF); // restricted on the last M-cycle of the transfer
        bus.dma_step(); // the 160th and final step ends the transfer (E03_06 spec)
        assert_eq!(bus.read(0xC000), 0x42); // the restriction is gone exactly with it
    }

    #[test]
    fn e04_01_post_boot_apu_registers_read_as_in_note() {
        let mut bus = Bus::new(vec![0u8; 0x8000]);
        bus.reset();
        for (address, expected) in [
            (0xFF10, 0x80), // NR10
            (0xFF11, 0xBF), // NR11
            (0xFF12, 0xF3), // NR12
            (0xFF13, 0xFF), // NR13
            (0xFF14, 0xBF), // NR14
            (0xFF15, 0xFF), // unused $FF15
            (0xFF16, 0x3F), // NR21
            (0xFF17, 0x00), // NR22
            (0xFF18, 0xFF), // NR23
            (0xFF19, 0xBF), // NR24
            (0xFF1A, 0x7F), // NR30
            (0xFF1B, 0xFF), // NR31
            (0xFF1C, 0x9F), // NR32
            (0xFF1D, 0xFF), // NR33
            (0xFF1E, 0xBF), // NR34
            (0xFF1F, 0xFF), // unused $FF1F
            (0xFF20, 0xFF), // NR41
            (0xFF21, 0x00), // NR42
            (0xFF22, 0x00), // NR43
            (0xFF23, 0xBF), // NR44
            (0xFF24, 0x77), // NR50
            (0xFF25, 0xF3), // NR51
            (0xFF26, 0xF1), // NR52
        ] {
            assert_eq!(bus.read(address), expected, "address {:#06X}", address);
        }
    }

    #[test]
    fn e04_01_unused_apu_addresses_read_ff() {
        let mut bus = Bus::new(vec![0u8; 0x8000]);
        bus.reset();
        assert_eq!(bus.read(0xFF15), 0xFF); // unused $FF15: open-bus default (E04_01 spec)
        assert_eq!(bus.read(0xFF1F), 0xFF); // unused $FF1F: open-bus default (E04_01 spec)
        assert_eq!(bus.read(0xFF27), 0xFF); // past NR52, before wave RAM: open bus (E04_01 spec)
        assert_eq!(bus.read(0xFF2F), 0xFF); // unused $FF2F: open-bus default (E04_01 spec)
    }

    #[test]
    fn e04_01_power_off_clears_registers_and_keeps_wave_ram() {
        let mut bus = Bus::new(vec![0u8; 0x8000]);
        bus.reset();
        bus.write(0xFF30, 0x5A); // wave RAM byte before the power-off (E04_01 spec)
        bus.write(0xFF26, 0x00); // APU power off: every APU register is cleared (note 05b)
        assert_eq!(bus.read(0xFF10), 0x80); // NR10: stored $00 OR mask $80
        assert_eq!(bus.read(0xFF11), 0x3F); // NR11
        assert_eq!(bus.read(0xFF12), 0x00); // NR12
        assert_eq!(bus.read(0xFF13), 0xFF); // NR13
        assert_eq!(bus.read(0xFF14), 0xBF); // NR14
        assert_eq!(bus.read(0xFF16), 0x3F); // NR21
        assert_eq!(bus.read(0xFF17), 0x00); // NR22
        assert_eq!(bus.read(0xFF19), 0xBF); // NR24
        assert_eq!(bus.read(0xFF1A), 0x7F); // NR30
        assert_eq!(bus.read(0xFF1C), 0x9F); // NR32
        assert_eq!(bus.read(0xFF21), 0x00); // NR42
        assert_eq!(bus.read(0xFF22), 0x00); // NR43
        assert_eq!(bus.read(0xFF23), 0xBF); // NR44
        assert_eq!(bus.read(0xFF24), 0x00); // NR50: cleared, and its read mask is $00 (E04_01 spec)
        assert_eq!(bus.read(0xFF25), 0x00); // NR51
        assert_eq!(bus.read(0xFF26), 0x70); // NR52: power off, status bits 3-0 are 0 too (E04_01 spec)
        assert_eq!(bus.read(0xFF30), 0x5A); // wave RAM is not affected by the power-off (note 05b)
    }

    #[test]
    fn e04_01_writes_are_ignored_while_off_except_nr52_and_wave_ram() {
        let mut bus = Bus::new(vec![0u8; 0x8000]);
        bus.reset();
        bus.write(0xFF26, 0x00); // APU power off (E04_01 spec)
        bus.write(0xFF12, 0x77); // ignored while the APU is off (note 05b)
        bus.write(0xFF24, 0x55); // ignored while the APU is off (note 05b)
        bus.write(0xFF30, 0x12); // wave RAM stays writable (E04_01 spec)
        assert_eq!(bus.read(0xFF12), 0x00);
        assert_eq!(bus.read(0xFF24), 0x00);
        assert_eq!(bus.read(0xFF30), 0x12);
        bus.write(0xFF26, 0x80); // APU power on again (E04_01 spec)
        assert_eq!(bus.read(0xFF26), 0xF0); // NR52: bit 7 set, status bits read back as $F0 here (E04_01 spec)
    }

    #[test]
    fn e04_01_registers_are_writable_again_after_power_on() {
        let mut bus = Bus::new(vec![0u8; 0x8000]);
        bus.reset();
        bus.write(0xFF26, 0x00); // APU power off (E04_01 spec)
        bus.write(0xFF26, 0x80); // APU power on again (E04_01 spec)
        bus.write(0xFF12, 0x77); // writable while the APU is on (note 05b)
        bus.write(0xFF24, 0x55);
        bus.write(0xFF25, 0xAA);
        bus.write(0xFF11, 0x40);
        assert_eq!(bus.read(0xFF12), 0x77);
        assert_eq!(bus.read(0xFF24), 0x55);
        assert_eq!(bus.read(0xFF25), 0xAA);
        assert_eq!(bus.read(0xFF11), 0x7F); // stored $40 OR mask $3F (E04_01 spec)
    }

    #[test]
    fn e04_01_nr52_status_bits_are_read_only() {
        let mut bus = Bus::new(vec![0u8; 0x8000]);
        bus.reset();
        assert_eq!(bus.read(0xFF26), 0xF1); // post-boot NR52 (note 08)
        bus.write(0xFF26, 0x80); // a write never changes the status bits 3-0 (E04_01 spec)
        assert_eq!(bus.read(0xFF26), 0xF1);
        bus.write(0xFF26, 0x8F); // even writing the status bits themselves has no effect (E04_01 spec)
        assert_eq!(bus.read(0xFF26), 0xF1);
    }

    #[test]
    fn e04_02_bus_records_accepted_apu_write() {
        let mut bus = Bus::new(vec![0u8; 0x8000]);
        bus.reset();
        bus.write(0xFF19, 0x80);
        assert_eq!(bus.apu_write.take(), Some((0xFF19, 0x80)));
    }

    #[test]
    fn e04_02_bus_records_nothing_while_apu_is_off() {
        let mut bus = Bus::new(vec![0u8; 0x8000]);
        bus.reset();
        bus.write(0xFF26, 0x00);
        bus.apu_write = None;
        bus.write(0xFF19, 0x80);
        assert_eq!(bus.apu_write, None);
    }

    #[test]
    fn e04_07_length_registers_are_relayed_while_apu_is_off() {
        let mut bus = Bus::new(vec![0u8; 0x8000]);
        bus.reset();
        bus.write(0xFF26, 0x00);
        bus.apu_write = None;
        bus.write(0xFF11, 0xFE);
        assert_eq!(bus.apu_write.take(), Some((0xFF11, 0x3E)));
        assert_eq!(bus.read(0xFF11), 0x3F);
        bus.write(0xFF16, 0xC5);
        assert_eq!(bus.apu_write.take(), Some((0xFF16, 0x05)));
        bus.write(0xFF1B, 0xF0);
        assert_eq!(bus.apu_write.take(), Some((0xFF1B, 0xF0)));
        assert_eq!(bus.read(0xFF1B), 0xFF);
        bus.write(0xFF20, 0xFF);
        assert_eq!(bus.apu_write.take(), Some((0xFF20, 0x3F)));
        assert_eq!(bus.read(0xFF20), 0xFF);
        bus.write(0xFF12, 0x77);
        assert_eq!(bus.apu_write, None);
    }

    #[test]
    fn e04_02_bus_status_bits_follow_set_apu_status() {
        let mut bus = Bus::new(vec![0u8; 0x8000]);
        bus.reset();
        bus.set_apu_status(0x03);
        assert_eq!(bus.read(0xFF26) & 0x03, 0x03);
        bus.set_apu_status(0x00);
        assert_eq!(bus.read(0xFF26) & 0x03, 0x00);
    }

    #[test]
    fn e04_03_bus_wave_ram_copy_matches_written_bytes() {
        let mut bus = Bus::new(vec![0u8; 0x8000]);
        bus.reset();
        bus.write(0xFF30, 0xAB);
        bus.write(0xFF3F, 0xCD);
        let ram = bus.wave_ram();
        assert_eq!(ram[0], 0xAB);
        assert_eq!(ram[15], 0xCD);
    }

    #[test]
    fn e04_03_bus_status_bit_2_follows_set_apu_status() {
        let mut bus = Bus::new(vec![0u8; 0x8000]);
        bus.reset();
        bus.set_apu_status(0x07);
        assert_eq!(bus.read(0xFF26) & 0x07, 0x07);
        bus.set_apu_status(0x00);
        assert_eq!(bus.read(0xFF26) & 0x07, 0x00);
    }

    #[test]
    fn e04_04_bus_status_bit_3_follows_set_apu_status() {
        let mut bus = Bus::new(vec![0u8; 0x8000]);
        bus.reset();
        bus.set_apu_status(0x0F);
        assert_eq!(bus.read(0xFF26) & 0x0F, 0x0F);
        bus.set_apu_status(0x00);
        assert_eq!(bus.read(0xFF26) & 0x0F, 0x00);
    }

    #[test]
    fn e04_05_bus_audio_pan_returns_nr50_and_nr51() {
        let mut bus = Bus::new(vec![0u8; 0x8000]);
        bus.reset();
        bus.write(0xFF24, 0x5A);
        bus.write(0xFF25, 0xA5);
        assert_eq!(bus.audio_pan(), (0x5A, 0xA5));
    }

    #[test]
    fn e04_06_registers_read_back_written_value_or_mask() {
        let mut bus = Bus::new(vec![0u8; 0x8000]);
        bus.reset();
        let masks: [u8; 22] = [
            0x80, 0x3F, 0x00, 0xFF, 0xBF, 0xFF, 0x3F, 0x00, 0xFF, 0xBF, 0x7F, 0xFF, 0x9F, 0xFF,
            0xBF, 0xFF, 0xFF, 0x00, 0x00, 0xBF, 0x00, 0x00,
        ];
        for d in 0..=255u8 {
            for addr in 0xFF10u16..=0xFF3F {
                if addr == 0xFF26 {
                    continue;
                }
                let mask = match addr {
                    0xFF10..=0xFF25 => masks[usize::from(addr - 0xFF10)],
                    0xFF27..=0xFF2F => 0xFF,
                    _ => 0x00,
                };
                bus.write(addr, d);
                assert_eq!(bus.read(addr), d | mask, "addr {addr:04X} value {d:02X}");
                bus.write(0xFF25, 0x00);
                bus.write(0xFF1A, 0x00);
            }
        }
    }

    #[test]
    fn e04_06_sram_exists_only_on_mbc1_ram_cart() {
        let mut r = test_rom(32 * 1024);
        r[0x147] = 0x03;
        r[0x149] = 0x02;
        let mut bus = Bus::new(r);
        bus.write(0x0000, 0x0A); // enable the MBC1 RAM (note 07b)
        bus.write(0xA000, 0x5A);
        bus.write(0xBFFF, 0x3C);
        assert_eq!(bus.read(0xA000), 0x5A);
        assert_eq!(bus.read(0xBFFF), 0x3C);
        assert_eq!(bus.peek(0xA000), 0x5A);
        let mut plain = Bus::new(test_rom(32 * 1024));
        plain.write(0xA000, 0x5A);
        assert_eq!(plain.read(0xA000), 0xFF);
    }

    #[test]
    fn e04_06_cart_result_needs_signature_and_finished_status() {
        let mut r = test_rom(32 * 1024);
        r[0x147] = 0x03;
        r[0x149] = 0x02;
        let mut bus = Bus::new(r);
        bus.write(0x0000, 0x0A); // enable the MBC1 RAM (note 07b)
        let mut out = [0u8; 16];
        for (i, b) in b"Passed\0".iter().enumerate() {
            bus.write(0xA004 + i as u16, *b);
        }
        assert_eq!(bus.cart_result(&mut out), 0); // no signature yet
        bus.write(0xA001, 0xDE);
        bus.write(0xA002, 0xB0);
        bus.write(0xA003, 0x61);
        bus.write(0xA000, 0x80);
        assert_eq!(bus.cart_result(&mut out), 0); // still running
        bus.write(0xA000, 0x00);
        assert_eq!(bus.cart_result(&mut out), 6);
        assert_eq!(&out[..6], b"Passed");
        let mut small = [0u8; 3];
        assert_eq!(bus.cart_result(&mut small), 3); // truncated to the buffer
    }

    #[test]
    fn e04_07_wave_ram_is_free_when_channel_is_off() {
        let mut bus = Bus::new(vec![0u8; 0x8000]);
        bus.reset();
        bus.write(0xFF30, 0x5A);
        assert_eq!(bus.read(0xFF30), 0x5A);
    }

    #[test]
    fn e04_07_wave_ram_reads_ff_and_ignores_writes_outside_the_window() {
        let mut bus = Bus::new(vec![0u8; 0x8000]);
        bus.reset();
        bus.write(0xFF31, 0x12);
        bus.wave_access = WaveAccess::Closed;
        assert_eq!(bus.read(0xFF31), 0xFF);
        bus.write(0xFF31, 0x34);
        bus.wave_access = WaveAccess::Free;
        assert_eq!(bus.read(0xFF31), 0x12);
    }

    #[test]
    fn e04_07_wave_ram_access_goes_to_the_current_byte_inside_the_window() {
        let mut bus = Bus::new(vec![0u8; 0x8000]);
        bus.reset();
        bus.write(0xFF35, 0xAB);
        let before = bus.read(0xFF3A);
        bus.wave_access = WaveAccess::Open(5);
        assert_eq!(bus.read(0xFF30), 0xAB);
        assert_eq!(bus.read(0xFF3F), 0xAB);
        bus.write(0xFF3A, 0xCD);
        bus.wave_access = WaveAccess::Free;
        assert_eq!(bus.read(0xFF35), 0xCD);
        assert_eq!(bus.read(0xFF3A), before);
    }

    #[test]
    fn e04_07_corrupt_wave_ram_rewrites_first_bytes() {
        let mut bus = Bus::new(vec![0u8; 0x8000]);
        bus.reset();
        for i in 0..16u16 {
            bus.write(0xFF30 + i, (i as u8) * 0x11);
        }
        bus.corrupt_wave_ram(9);
        assert_eq!(bus.read(0xFF30), 0x88);
        assert_eq!(bus.read(0xFF31), 0x99);
        assert_eq!(bus.read(0xFF32), 0xAA);
        assert_eq!(bus.read(0xFF33), 0xBB);
        assert_eq!(bus.read(0xFF34), 0x44);
    }

    #[test]
    fn e05_01_p1_write_keeps_only_the_select_bits() {
        let mut bus = Bus::new(test_rom(32 * 1024));
        bus.write(0xFF00, 0xFF);
        assert_eq!(bus.read(0xFF00), 0xFF);
        bus.write(0xFF00, 0x00);
        assert_eq!(bus.read(0xFF00), 0xCF);
        bus.write(0xFF00, 0x2F);
        assert_eq!(bus.read(0xFF00), 0xEF);
        assert_eq!(bus.peek(0xFF00), 0xEF);
    }

    #[test]
    fn e05_01_p1_reads_pressed_buttons_of_the_selected_family() {
        let mut bus = Bus::new(test_rom(32 * 1024));
        bus.write(0xFF00, 0x10);
        bus.set_buttons(Input {
            a: true,
            start: true,
            right: true,
            ..Input::default()
        });
        assert_eq!(bus.read(0xFF00), 0xD6);
        bus.write(0xFF00, 0x20);
        assert_eq!(bus.read(0xFF00), 0xEE);
    }

    #[test]
    fn e05_01_joypad_interrupt_on_press_of_a_selected_button() {
        let mut bus = Bus::new(test_rom(32 * 1024));
        bus.write(0xFF00, 0x10);
        bus.io[0x0F] &= !0x10;
        bus.set_buttons(Input {
            a: true,
            ..Input::default()
        });
        assert_eq!(bus.io[0x0F] & 0x10, 0x10);
    }

    #[test]
    fn e05_01_no_joypad_interrupt_for_an_unselected_family() {
        let mut bus = Bus::new(test_rom(32 * 1024));
        bus.write(0xFF00, 0x10);
        bus.io[0x0F] &= !0x10;
        bus.set_buttons(Input {
            right: true,
            ..Input::default()
        });
        assert_eq!(bus.io[0x0F] & 0x10, 0x00);
    }

    #[test]
    fn e05_01_no_joypad_interrupt_on_release_or_while_held() {
        let mut bus = Bus::new(test_rom(32 * 1024));
        bus.write(0xFF00, 0x10);
        let pressed = Input {
            a: true,
            ..Input::default()
        };
        bus.set_buttons(pressed);
        bus.io[0x0F] &= !0x10;
        bus.set_buttons(pressed);
        assert_eq!(bus.io[0x0F] & 0x10, 0x00);
        bus.set_buttons(Input::default());
        assert_eq!(bus.io[0x0F] & 0x10, 0x00);
    }

    #[test]
    fn e05_01_selecting_a_family_with_a_held_button_requests_the_interrupt() {
        let mut bus = Bus::new(test_rom(32 * 1024));
        bus.write(0xFF00, 0x30);
        bus.set_buttons(Input {
            a: true,
            ..Input::default()
        });
        bus.io[0x0F] &= !0x10;
        assert_eq!(bus.read(0xFF00), 0xFF);
        bus.write(0xFF00, 0x10);
        assert_eq!(bus.read(0xFF00), 0xDE);
        assert_eq!(bus.io[0x0F] & 0x10, 0x10);
    }

    /// ROM image with the cartridge header fields set for mapper selection (E05_07): type at
    /// $0147, ROM size code at $0148, RAM size code at $0149 and a valid checksum at $014D.
    fn e05_07_rom(cart_type: u8, rom_size_code: u8, ram_code: u8, banks: usize) -> Vec<u8> {
        let mut rom = vec![0x00u8; banks * 0x4000];
        rom[0x147] = cart_type;
        rom[0x148] = rom_size_code;
        rom[0x149] = ram_code;
        let mut x = 0u8;
        for &b in &rom[0x134..=0x14C] {
            x = x.wrapping_sub(b).wrapping_sub(1);
        }
        rom[0x14D] = x; // header checksum (note 07a)
        rom
    }

    #[test]
    fn e05_07_mbc1_rom_bank_switch_is_visible_on_the_bus() {
        let mut rom = e05_07_rom(0x01, 0x01, 0x00, 4);
        for n in 1..=3 {
            rom[0x4000 * n] = 0x10 + n as u8; // bank marker (note 07b)
        }
        let mut bus = Bus::new(rom);
        bus.write(0x2000, 2);
        assert_eq!(bus.read(0x4000), 0x12);
        bus.write(0x2000, 3);
        assert_eq!(bus.read(0x4000), 0x13);
        bus.write(0x2000, 0); // register value 0 maps to bank 1 (note 07b)
        assert_eq!(bus.read(0x4000), 0x11);
    }

    #[test]
    fn e05_07_mbc1_ram_is_gated_by_the_enable_register() {
        let mut bus = Bus::new(e05_07_rom(0x03, 0x00, 0x02, 2));
        assert_eq!(bus.read(0xA000), 0xFF); // RAM closed at power-up (note 07b)
        bus.write(0x0000, 0x0A); // enable the RAM
        bus.write(0xA000, 0x42);
        assert_eq!(bus.read(0xA000), 0x42);
        bus.write(0x0000, 0x00); // close the RAM again
        assert_eq!(bus.read(0xA000), 0xFF);
        bus.write(0x0000, 0x0A); // reopen: the byte survives (note 07b)
        assert_eq!(bus.read(0xA000), 0x42);
    }

    #[test]
    fn e05_07_rom_only_has_no_cartridge_ram() {
        let mut bus = Bus::new(e05_07_rom(0x00, 0x00, 0x00, 2));
        bus.write(0xA000, 0x42); // ignored: no RAM on a ROM-only cart (note 07b)
        assert_eq!(bus.read(0xA000), 0xFF);
    }

    #[test]
    fn e05_07_mbc5_ninth_bank_bit_reaches_the_bus() {
        let mut rom = e05_07_rom(0x19, 0x08, 0x00, 512); // 8 MiB (note 07b)
        rom[0x4000 * 256] = 0xAB;
        rom[0x4000] = 0x77; // bank 1 marker (note 07b)
        let mut bus = Bus::new(rom);
        bus.write(0x2000, 0x00);
        bus.write(0x3000, 0x01); // bit 8: bank 256 (note 07b)
        assert_eq!(bus.read(0x4000), 0xAB);
        bus.write(0x3000, 0x00);
        bus.write(0x2000, 0x01); // bank 1
        assert_eq!(bus.read(0x4000), 0x77);
    }

    #[test]
    fn e05_07_unsupported_cartridge_type_falls_back_to_rom_only() {
        let mut rom = e05_07_rom(0x20, 0x00, 0x00, 2); // type $20 is unsupported (E05_07)
        rom[0x4000] = 0x99;
        rom[0x0200] = 0x55;
        let bus = Bus::new(rom); // must not panic: ROM-only fallback
        assert_eq!(bus.read(0x0200), 0x55);
        assert_eq!(bus.read(0xA000), 0xFF); // no cartridge RAM in the fallback
    }

    #[test]
    fn fix_02_dma_starts_two_steps_after_the_ff46_write() {
        let mut bus = Bus::new(test_rom(32 * 1024));
        bus.write(0xFF46, 0xC0); // start the transfer from $C000 (E03_06 spec)
        assert!(!bus.dma_active()); // not active yet: the start delay is pending (FIX_02)
        bus.dma_step(); // M-cycle 1 of the start delay after the write cycle
        assert!(!bus.dma_active());
        bus.dma_step(); // M-cycle 2: the transfer starts here (mooneye oam_dma_start, FIX_02)
        assert!(bus.dma_active());
    }

    #[test]
    fn fix_02_oam_is_readable_until_the_dma_starts() {
        let mut bus = Bus::new(test_rom(32 * 1024));
        bus.oam[0] = 0x33; // OAM byte before the transfer starts
        bus.write(0xFF46, 0xC0); // start the transfer from $C000 (E03_06 spec)
        assert_eq!(bus.read(0xFE00), 0x33); // OAM stays readable during the start delay (FIX_02)
        bus.dma_step(); // M-cycle 1 of the start delay after the write cycle
        assert_eq!(bus.read(0xFE00), 0x33);
        bus.dma_step(); // M-cycle 2: the transfer starts here (FIX_02)
        assert_eq!(bus.read(0xFE00), 0xFF); // OAM is never readable during a DMA (E03_15 spec)
    }

    #[test]
    fn fix_02_dma_from_vram_leaves_wram_usable() {
        let mut bus = Bus::new(test_rom(32 * 1024));
        bus.write(0xC000, 0x42); // WRAM byte before the transfer starts
        bus.write(0xFF46, 0x80); // start the transfer from VRAM $8000 (E03_06 spec)
        bus.dma_step(); // M-cycle 1 of the start delay after the write cycle (FIX_02)
        bus.dma_step(); // M-cycle 2: the transfer is now running (FIX_02)
        assert_eq!(bus.read(0xC000), 0x42); // WRAM stays usable: the DMA source is VRAM, not WRAM (mooneye call_timing)
        assert_eq!(bus.read(0xFE00), 0xFF); // OAM is blocked during the transfer (E03_15 spec)
        bus.write(0xC000, 0x43); // writes to WRAM work too
        assert_eq!(bus.read(0xC000), 0x43);
    }

    #[test]
    fn fix_02_dma_from_wram_leaves_vram_usable() {
        let mut bus = Bus::new(test_rom(32 * 1024));
        bus.write(0xFF46, 0xC0); // start the transfer from WRAM $C000 (E03_06 spec)
        bus.dma_step(); // M-cycle 1 of the start delay after the write cycle (FIX_02)
        bus.dma_step(); // M-cycle 2: the transfer is now running (FIX_02)
        bus.write(0x8000, 0x55); // VRAM stays usable: the DMA source is WRAM, not VRAM (mooneye call_timing)
        assert_eq!(bus.read(0x8000), 0x55);
        assert_eq!(bus.read(0xC000), 0xFF); // WRAM is blocked during the transfer (E03_15 spec)
    }

    #[test]
    fn fix_02_dma_from_vram_blocks_vram() {
        let mut bus = Bus::new(test_rom(32 * 1024));
        bus.write(0xFF46, 0x80); // start the transfer from VRAM $8000 (E03_06 spec)
        bus.dma_step(); // M-cycle 1 of the start delay after the write cycle (FIX_02)
        bus.dma_step(); // M-cycle 2: the transfer is now running (FIX_02)
        assert_eq!(bus.read(0x8000), 0xFF); // VRAM is blocked: same bus segment as the DMA source (mooneye call_timing)
    }

    #[test]
    fn fix_02_restart_keeps_the_old_transfer_until_the_new_one_starts() {
        let mut bus = Bus::new(test_rom(32 * 1024));
        bus.write(0xFF46, 0xC0); // start the transfer from $C000 (E03_06 spec)
        bus.dma_step(); // M-cycle 1 of the start delay after the write cycle (FIX_02)
        bus.dma_step(); // M-cycle 2: the transfer is now running (FIX_02)
        for _ in 0..10 {
            bus.dma_step(); // ten bytes copied from $C000
        }
        bus.write(0xFF46, 0xC1); // restart request: source $C100 pending (E03_06 spec)
        assert!(bus.dma_active()); // the old transfer keeps running until the new one starts (FIX_02)
        bus.dma_step(); // M-cycle 1 of the start delay: the old transfer continues
        assert!(bus.dma_active());
        bus.dma_step(); // M-cycle 2: the new transfer from $C100 starts at index 0 (FIX_02)
        assert!(bus.dma_active());
        for _ in 0..159 {
            bus.dma_step(); // the new transfer runs to completion
            assert!(bus.dma_active());
        }
        bus.dma_step(); // the final step ends the transfer (E03_06 spec)
        assert!(!bus.dma_active());
    }

    #[test]
    fn fix_05_unmapped_io_reads_ff_and_ignores_writes() {
        let mut bus = Bus::new(test_rom(32 * 1024));
        for addr in [
            0xFF03u16, 0xFF08, 0xFF0A, 0xFF0E, 0xFF4C, 0xFF4D, 0xFF50, 0xFF60, 0xFF7F,
        ] {
            bus.write(addr, 0x00);
            assert_eq!(bus.read(addr), 0xFF); // unmapped I/O reads $FF (mooneye unused_hwio-GS)
            bus.write(addr, 0x55);
            assert_eq!(bus.read(addr), 0xFF); // writes to unmapped I/O are ignored (FIX_05)
        }
    }

    #[test]
    fn fix_05_sc_unused_bits_read_one() {
        let mut bus = Bus::new(test_rom(32 * 1024));
        bus.write(0xFF02, 0x00); // SC ($FF02)
        assert_eq!(bus.read(0xFF02), 0x7E); // bits 6-1 are unused and read 1 (mooneye unused_hwio-GS)
        bus.write(0xFF02, 0x7E);
        assert_eq!(bus.read(0xFF02), 0x7E);
    }

    #[test]
    fn fix_05_tac_unused_bits_read_one() {
        let mut bus = Bus::new(test_rom(32 * 1024));
        bus.write(0xFF07, 0x00); // TAC ($FF07)
        assert_eq!(bus.read(0xFF07), 0xF8); // bits 7-3 are unused and read 1 (mooneye unused_hwio-GS)
        bus.write(0xFF07, 0x05);
        assert_eq!(bus.read(0xFF07), 0xFD);
    }

    #[test]
    fn fix_05_mapped_registers_are_unchanged() {
        let mut bus = Bus::new(test_rom(32 * 1024));
        bus.write(0xFF45, 0x12); // LYC ($FF45) stays plain R/W through the IO file (FIX_05)
        assert_eq!(bus.read(0xFF45), 0x12);
        bus.write(0xFF47, 0x1B); // SCY ($FF47) stays plain R/W through the IO file (FIX_05)
        assert_eq!(bus.read(0xFF47), 0x1B);
    }

    fn fix_06_run(bus: &mut Bus, v: u8) {
        bus.write(0xFF46, v); // start the OAM DMA from V * 0x100 (E03_06 spec)
        for _ in 0..162 {
            bus.dma_step(); // two M-cycles of start delay plus the 160-byte transfer (FIX_02)
        }
    }

    #[test]
    fn fix_06_source_e0_is_the_echo_of_c000() {
        let mut bus = Bus::new(test_rom(32 * 1024));
        bus.write(0xC000, 0x11); // WRAM byte at $C000 (mooneye oam_dma/sources-GS)
        bus.write(0xC09F, 0x22); // WRAM byte at $C09F
        fix_06_run(&mut bus, 0xE0); // source $E0 reads the echo of $C000 (FIX_06)
        assert_eq!(bus.oam[0], 0x11);
        assert_eq!(bus.oam[159], 0x22);
    }

    #[test]
    fn fix_06_source_fe_is_the_echo_of_de00() {
        let mut bus = Bus::new(test_rom(32 * 1024));
        bus.write(0xDE00, 0x33); // WRAM byte at $DE00 (mooneye oam_dma/sources-GS)
        fix_06_run(&mut bus, 0xFE); // source $FE reads the echo of $DE00 (FIX_06)
        assert_eq!(bus.oam[0], 0x33);
    }

    #[test]
    fn fix_06_source_ff_is_the_echo_of_df00() {
        let mut bus = Bus::new(test_rom(32 * 1024));
        bus.write(0xDF00, 0x44); // WRAM byte at $DF00 (mooneye oam_dma/sources-GS)
        fix_06_run(&mut bus, 0xFF); // source $FF reads the echo of $DF00 (FIX_06)
        assert_eq!(bus.oam[0], 0x44);
        assert_eq!(bus.read(0xFF46), 0xFF); // the register keeps the written value (E03_06 spec)
    }

    #[test]
    fn fix_06_source_ff_blocks_the_external_bus() {
        let mut bus = Bus::new(test_rom(32 * 1024));
        bus.write(0xC000, 0x55); // WRAM byte before the transfer starts (mooneye oam_dma/sources-GS)
        bus.write(0xFF46, 0xFF); // start the transfer from the echo of $DF00 (FIX_06)
        bus.dma_step(); // M-cycle 1 of the start delay after the write cycle (FIX_02)
        bus.dma_step(); // M-cycle 2: the transfer is now running (FIX_02)
        assert_eq!(bus.read(0xC000), 0xFF); // WRAM is blocked: same external bus as the DMA source (E03_15 spec)
        assert!(bus.dma_active());
    }
}
