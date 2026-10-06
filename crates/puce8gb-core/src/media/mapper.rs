//! Cartridge mappers (decision A_06): ROM-only ($00) first; MBC1/MBC2/MBC3 and the
//! remaining 0147 codes arrive with later tasks. An unknown or unsupported code at
//! $0147 returns `CartridgeLoadError::UnsupportedCartridgeType` (bad ROM => Result).

use super::header::{CartridgeLoadError, Header};

/// External RAM size of a ROM-only cartridge: up to 8 KiB at A000-BFFF (note 07b
/// "No-MBC / ROM-only").
pub const ROM_ONLY_RAM_SIZE: usize = 8 * 1024;

/// Mapper state for the loaded cartridge. Enum by design, no trait objects
/// (decision A_06); later tasks add one variant per supported mapper.
#[derive(Debug)]
pub enum Mapper {
    /// No MBC chip (note 07b): ROM mapped directly at 0000-7FFF, optional RAM at
    /// A000-BFFF via a discrete logic decoder.
    RomOnly(RomOnly),
}

impl Mapper {
    /// Build the mapper for a cartridge from its header (decision A_06). Only the
    /// ROM-only type $00 is supported so far; every other code at $0147 is rejected
    /// with `CartridgeLoadError::UnsupportedCartridgeType`.
    pub fn new(rom: Vec<u8>, header: &Header) -> Result<Mapper, CartridgeLoadError> {
        match header.cartridge_type {
            0x00 => Ok(Mapper::RomOnly(RomOnly::new(rom, header.ram_size_code)?)),
            other => Err(CartridgeLoadError::UnsupportedCartridgeType(other)),
        }
    }

    /// Read a byte from the cartridge address space (ROM region 0000-7FFF or RAM
    /// region A000-BFFF); anything else reads open bus $FF (decision A_02).
    pub fn read(&self, addr: u16) -> u8 {
        match self {
            Mapper::RomOnly(m) => m.read(addr),
        }
    }

    /// Write a byte to the cartridge address space; ROM writes are ignored and RAM
    /// writes only land when RAM is present (note 07b).
    pub fn write(&mut self, addr: u16, val: u8) {
        match self {
            Mapper::RomOnly(m) => m.write(addr, val),
        }
    }
}

/// ROM-only cartridge (note 07b "No-MBC / ROM-only"): at most 32 KiB of ROM mapped
/// directly at 0000-7FFF, optionally up to 8 KiB of RAM at A000-BFFF.
#[derive(Debug)]
pub struct RomOnly {
    /// Full ROM image, bank 0 first (decision A_02).
    rom: Vec<u8>,
    /// External RAM at A000-BFFF when the header declares it ($0149 = $02); absent
    /// otherwise. Zeroed at power-up (deterministic, decision A_05).
    ram: Option<Vec<u8>>,
}

impl RomOnly {
    /// Build a ROM-only mapper from the ROM image and the RAM size code at $0149
    /// (note 07a "Taille RAM"): $00 = no RAM, $02 = 8 KiB. Every other code is
    /// rejected with `CartridgeLoadError::UnknownRamSize`, including $01 which is a
    /// CONFLIT between sources (note 07a) and $03-$05 which exceed the 8 KiB that a
    /// no-MBC cartridge can carry (note 07b).
    pub fn new(rom: Vec<u8>, ram_size_code: u8) -> Result<Self, CartridgeLoadError> {
        let ram = match ram_size_code {
            0x00 => None,
            0x02 => Some(vec![0u8; ROM_ONLY_RAM_SIZE]),
            other => return Err(CartridgeLoadError::UnknownRamSize(other)),
        };
        Ok(RomOnly { rom, ram })
    }

    /// The full ROM image.
    pub fn rom(&self) -> &[u8] {
        &self.rom
    }

    /// True when the cartridge carries external RAM at A000-BFFF (note 07b).
    pub fn has_ram(&self) -> bool {
        self.ram.is_some()
    }

    /// Read a byte from the ROM region 0000-7FFF (note 07b); past the end of the
    /// image reads open bus $FF (decision A_02).
    pub fn read_rom(&self, addr: u16) -> u8 {
        self.rom.get(addr as usize).copied().unwrap_or(0xFF)
    }

    /// Write to the ROM region is ignored: the ROM is read-only (note 03a "Adresse bus").
    pub fn write_rom(&mut self, _addr: u16, _val: u8) {}

    /// Read a byte from the RAM region A000-BFFF (offset from A000); when no RAM is
    /// present, or past its end, reads open bus $FF (decision A_02).
    pub fn read_ram(&self, addr: u16) -> u8 {
        self.ram
            .as_ref()
            .and_then(|ram| ram.get(addr as usize))
            .copied()
            .unwrap_or(0xFF)
    }

    /// Write to the RAM region A000-BFFF (offset from A000); ignored when no RAM is
    /// present or past its end (note 07b).
    pub fn write_ram(&mut self, addr: u16, val: u8) {
        if let Some(ram) = &mut self.ram {
            if ram.len() > addr as usize {
                ram[addr as usize] = val;
            }
        }
    }

    /// Read a byte from the cartridge address space (ROM 0000-7FFF, RAM A000-BFFF);
    /// anything else reads open bus $FF (decision A_02).
    pub fn read(&self, addr: u16) -> u8 {
        if addr <= 0x7FFF {
            self.read_rom(addr)
        } else if (0xA000..=0xBFFF).contains(&addr) {
            self.read_ram(addr - 0xA000)
        } else {
            0xFF
        }
    }

    /// Write a byte to the cartridge address space; ROM writes are ignored, RAM
    /// writes only land when RAM is present (note 07b).
    pub fn write(&mut self, addr: u16, val: u8) {
        if addr <= 0x7FFF {
            self.write_rom(addr, val);
        } else if (0xA000..=0xBFFF).contains(&addr) {
            self.write_ram(addr - 0xA000, val);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A header with the given cartridge type and RAM size code at $0149.
    fn header_with(cart_type: u8, ram_size: u8) -> Header {
        Header {
            title: [0u8; 16],
            cartridge_type: cart_type,
            rom_size_code: 0x00,
            ram_size_code: ram_size,
            header_checksum: 0x00,
            checksum_ok: true,
        }
    }

    /// ROM image of `len` bytes where byte i is i & $FF (distinct from open bus $FF).
    fn rom_image(len: usize) -> Vec<u8> {
        (0..len as u32).map(|i| (i & 0xFF) as u8).collect()
    }

    #[test]
    fn e01_04_rom_only_reads_across_0000_7fff() {
        let m = RomOnly::new(rom_image(32 * 1024), 0x00).expect("no RAM loads");
        for addr in 0u16..=0x7FFF {
            assert_eq!(m.read_rom(addr), (addr & 0xFF) as u8, "read at {addr:#06x}");
        }
    }

    #[test]
    fn e01_04_rom_only_read_past_image_is_open_bus() {
        let m = RomOnly::new(rom_image(16 * 1024), 0x00).expect("no RAM loads");
        assert_eq!(m.read_rom(0x3FFF), 0xFF); // last byte of the image: 0xFFFF & $FF
        assert_eq!(m.read_rom(0x4000), 0xFF); // past the end: open bus (decision A_02)
        assert_eq!(m.read_rom(0x7FFF), 0xFF);
    }

    #[test]
    fn e01_04_rom_only_writes_are_ignored() {
        let mut m = RomOnly::new(rom_image(32 * 1024), 0x00).expect("no RAM loads");
        m.write_rom(0x0100, 0x5A);
        assert_eq!(m.read_rom(0x0100), 0x00); // ROM is read-only (note 03a)
        m.write(0x7FFF, 0x3C);
        assert_eq!(m.read(0x7FFF), 0xFF); // write through the mapper is ignored too
    }

    #[test]
    fn e01_04_ram_absent_reads_open_bus_and_writes_ignored() {
        let mut m = RomOnly::new(rom_image(32 * 1024), 0x00).expect("no RAM loads");
        assert!(!m.has_ram());
        for addr in [0u16, 0x1000, ROM_ONLY_RAM_SIZE as u16 - 1] {
            assert_eq!(m.read_ram(addr), 0xFF); // no RAM: open bus $FF (decision A_02)
        }
        m.write_ram(0x0000, 0x3C);
        assert_eq!(m.read_ram(0x0000), 0xFF); // writes are ignored without RAM
        m.write(0xA000, 0x3C);
        assert_eq!(m.read(0xA000), 0xFF); // same through the mapper
    }

    #[test]
    fn e01_04_ram_present_is_8_kib_readable_writable() {
        let mut m = RomOnly::new(rom_image(32 * 1024), 0x02).expect("8 KiB RAM loads");
        assert!(m.has_ram());
        assert_eq!(m.read_ram(0xFFFF), 0xFF); // past the 8 KiB: open bus (decision A_02)
        m.write_ram(0x0000, 0xA5); // first byte of A000-BFFF
        assert_eq!(m.read_ram(0x0000), 0xA5);
        m.write_ram((ROM_ONLY_RAM_SIZE - 1) as u16, 0x3C); // last byte (BFFF)
        assert_eq!(m.read_ram((ROM_ONLY_RAM_SIZE - 1) as u16), 0x3C);
        m.write(0xBFFF, 0x90); // through the mapper at the top of the region
        assert_eq!(m.read(0xBFFF), 0x90);
    }

    #[test]
    fn e01_04_ram_size_code_zero_means_no_ram() {
        let m = RomOnly::new(rom_image(32 * 1024), 0x00).expect("$00 loads");
        assert!(!m.has_ram());
    }

    #[test]
    fn e01_04_ram_size_code_two_is_8_kib() {
        let mut m = RomOnly::new(rom_image(32 * 1024), 0x02).expect("$02 loads");
        assert!(m.has_ram());
        // The RAM is exactly the 8 KiB that a no-MBC cartridge can carry (note 07b).
        m.write_ram((ROM_ONLY_RAM_SIZE - 1) as u16, 0x3C);
        assert_eq!(m.read_ram((ROM_ONLY_RAM_SIZE - 1) as u16), 0x3C);
    }

    #[test]
    fn e01_04_ram_size_code_one_rejected() {
        // $01 is a CONFLIT between sources (note 07a "Taille RAM 0149 : conflit valeur $01"):
        // rejected as unknown rather than guessed.
        let err = RomOnly::new(rom_image(32 * 1024), 0x01).unwrap_err();
        assert!(matches!(err, CartridgeLoadError::UnknownRamSize(0x01)));
    }

    #[test]
    fn e01_04_ram_size_codes_three_four_five_rejected() {
        // $03/$04/$05 are documented for banked MBC RAM (note 07a) but exceed the 8 KiB
        // a no-MBC cartridge can carry (note 07b): rejected.
        for code in [0x03u8, 0x04, 0x05] {
            let err = RomOnly::new(rom_image(32 * 1024), code).unwrap_err();
            assert!(matches!(&err, CartridgeLoadError::UnknownRamSize(c) if *c == code));
        }
    }

    #[test]
    fn e01_04_ram_size_other_codes_rejected() {
        // Every remaining code ($06-$FF) is unknown for a ROM-only cartridge.
        for code in 0x06u8..=0xFF {
            let err = RomOnly::new(rom_image(32 * 1024), code).unwrap_err();
            assert!(matches!(&err, CartridgeLoadError::UnknownRamSize(c) if *c == code));
        }
    }

    #[test]
    fn e01_04_mapper_new_rom_only_type() {
        let header = header_with(0x00, 0x02);
        let m = Mapper::new(rom_image(32 * 1024), &header).expect("type $00 loads");
        match &m {
            Mapper::RomOnly(r) => assert!(r.has_ram()),
        }
    }

    #[test]
    fn e01_04_mapper_unsupported_cartridge_type_rejected() {
        // MBC1 and friends arrive with later tasks (decision A_06): rejected now.
        for code in [0x01u8, 0x05, 0x0F, 0xFF] {
            let header = header_with(code, 0x00);
            let err = Mapper::new(rom_image(32 * 1024), &header).unwrap_err();
            assert!(matches!(&err, CartridgeLoadError::UnsupportedCartridgeType(c) if *c == code));
        }
    }
}
