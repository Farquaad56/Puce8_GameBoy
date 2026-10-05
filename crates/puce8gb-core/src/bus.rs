//! Bus: owns every addressable memory region (decision A_02).
//! read/write dispatch and side-effect-free peek() arrive with later tasks.

/// IO register file FF00-FFFF, including IF ($FF0F) and IE ($FFFF) (decision A_02).
pub const IO_SIZE: usize = 0x100;

/// Echo RAM E000-FDFF size (decision A_02).
pub const ECHO_RAM_SIZE: usize = 0x800;

/// Header checksum address in bank 0 (note 07a).
pub const ROM_HEADER_CHECKSUM: usize = 0x14D;

/// Every memory region of the DMG bus (decision A_02).
#[derive(Debug)]
pub struct Bus {
    /// Cartridge ROM image, bank 0 first (decisions A_02/A_06).
    pub rom: Vec<u8>,
    /// VRAM 8000-9FFF.
    pub vram: [u8; 0x2000],
    /// WRAM C000-DFFF.
    pub wram: [u8; 0x2000],
    /// Echo RAM E000-FDFF, mirror of the current WRAM bank (decision A_02).
    pub echo_ram: [u8; ECHO_RAM_SIZE],
    /// OAM FE00-FE9F.
    pub oam: [u8; 0x100],
    /// HRAM FF80-FFFE.
    pub hram: [u8; 0x7F],
    /// IO register file FF00-FFFF (decision A_02).
    pub io: [u8; IO_SIZE],
}

impl Bus {
    /// Build a bus around the cartridge ROM image, all RAM zeroed.
    pub fn new(rom: Vec<u8>) -> Self {
        Bus {
            rom,
            vram: [0x00; 0x2000],
            wram: [0x00; 0x2000],
            echo_ram: [0x00; ECHO_RAM_SIZE],
            oam: [0x00; 0x100],
            hram: [0x00; 0x7F],
            io: [0xFF; IO_SIZE],
        }
    }

    /// Post-boot state of the memory owned by the Bus (note 08, decision A_06).
    pub fn reset(&mut self) {
        // WRAM/HRAM filled deterministically with $00 at power-up (note 08 "RAM after
        // power-up", decision A_05); echo RAM mirrors the current WRAM bank.
        self.wram.fill(0x00);
        self.hram.fill(0x00);
        self.echo_ram.copy_from_slice(&self.wram[..ECHO_RAM_SIZE]);

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
}
