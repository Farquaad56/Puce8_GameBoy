//! MBC1 mapper (E05_02, note 07b): ROM banking, RAM enable, two banking modes.

use super::header::CartridgeLoadError;

/// Size of one ROM bank (note 07b).
const ROM_BANK_SIZE: usize = 0x4000;
/// Size of one RAM bank (note 07b).
const RAM_BANK_SIZE: usize = 0x2000;

/// MBC1 cartridge (note 07b): registers written in 0000-7FFF, ROM read through them.
/// Bank numbers are reduced to the real ROM or RAM size, so an out-of-range number never panics.
/// Access to an unmapped RAM bank wraps around: UNKNOWN - to confirm (note 07b "Omissions").
#[derive(Debug)]
pub struct Mbc1 {
    /// Full ROM image, bank 0 first.
    rom: Vec<u8>,
    /// External RAM (empty when the header declares none), zeroed at power-up.
    ram: Vec<u8>,
    /// RAM enable flag: set by writing a value whose low nibble is $A to 0000-1FFF.
    ram_enabled: bool,
    /// 5-bit ROM bank register (2000-3FFF); 0 is used as 1.
    bank_lo: u8,
    /// 2-bit register (4000-5FFF): RAM bank or upper ROM bank bits.
    bank_hi: u8,
    /// Banking mode (6000-7FFF): false = simple (default), true = advanced.
    advanced_mode: bool,
}

impl Mbc1 {
    /// Build an MBC1 from the ROM image and the RAM size code at $0149 (note 07a): $00 = no RAM,
    /// $02 = 8 KiB, $03 = 32 KiB (4 banks). Every other code is rejected with
    /// `CartridgeLoadError::UnknownRamSize` ($01 is a CONFLIT in note 07a; $04/$05 exceed the
    /// 32 KiB of the default MBC1 wiring, note 07b).
    pub fn new(rom: Vec<u8>, ram_size_code: u8) -> Result<Self, CartridgeLoadError> {
        let ram_len = match ram_size_code {
            0x00 => 0,
            0x02 => RAM_BANK_SIZE,
            0x03 => 4 * RAM_BANK_SIZE,
            other => return Err(CartridgeLoadError::UnknownRamSize(other)),
        };
        Ok(Mbc1 {
            rom,
            ram: vec![0u8; ram_len],
            ram_enabled: false,
            bank_lo: 0,
            bank_hi: 0,
            advanced_mode: false,
        })
    }

    fn rom_banks(&self) -> usize {
        (self.rom.len() / ROM_BANK_SIZE).max(1)
    }

    fn ram_banks(&self) -> usize {
        (self.ram.len() / RAM_BANK_SIZE).max(1)
    }

    /// The 5-bit register with the 00 -> 01 rule, applied on the full register before any
    /// masking to the ROM size (note 07b "banque 0").
    fn low_register(&self) -> usize {
        if self.bank_lo == 0 {
            1
        } else {
            usize::from(self.bank_lo)
        }
    }

    /// ROM bank mapped at `addr` (0000-7FFF), reduced to the real number of banks (equals
    /// masking for the power-of-two sizes of the header).
    fn rom_bank_for(&self, addr: u16) -> usize {
        let upper = usize::from(self.bank_hi) << 5;
        let bank = if addr < 0x4000 {
            if self.advanced_mode {
                upper
            } else {
                0
            }
        } else {
            upper | self.low_register()
        };
        bank % self.rom_banks()
    }

    /// Read a byte of the ROM region 0000-7FFF; past the image reads open bus $FF.
    pub fn read_rom(&self, addr: u16) -> u8 {
        let offset =
            self.rom_bank_for(addr) * ROM_BANK_SIZE + (usize::from(addr) & (ROM_BANK_SIZE - 1));
        self.rom.get(offset).copied().unwrap_or(0xFF)
    }

    /// A write to 0000-7FFF sets one of the four registers (note 07b); other addresses are ignored.
    pub fn write_rom(&mut self, addr: u16, val: u8) {
        match addr {
            0x0000..=0x1FFF => self.ram_enabled = val & 0x0F == 0x0A,
            0x2000..=0x3FFF => self.bank_lo = val & 0x1F,
            0x4000..=0x5FFF => self.bank_hi = val & 0x03,
            0x6000..=0x7FFF => self.advanced_mode = val & 0x01 != 0,
            _ => {}
        }
    }

    /// Index in the RAM array for `offset` (from A000), or None when RAM is disabled or absent.
    fn ram_index(&self, offset: u16) -> Option<usize> {
        if !self.ram_enabled || self.ram.is_empty() || usize::from(offset) >= RAM_BANK_SIZE {
            return None;
        }
        let bank = if self.advanced_mode {
            usize::from(self.bank_hi) % self.ram_banks()
        } else {
            0
        };
        Some(bank * RAM_BANK_SIZE + usize::from(offset))
    }

    /// Read a byte of A000-BFFF (offset from A000); disabled or absent RAM reads open bus $FF.
    pub fn read_ram(&self, offset: u16) -> u8 {
        self.ram_index(offset)
            .and_then(|i| self.ram.get(i))
            .copied()
            .unwrap_or(0xFF)
    }

    /// Write a byte of A000-BFFF (offset from A000); ignored while RAM is disabled or absent.
    pub fn write_ram(&mut self, offset: u16, val: u8) {
        if let Some(i) = self.ram_index(offset) {
            if let Some(b) = self.ram.get_mut(i) {
                *b = val;
            }
        }
    }

    /// Read from the cartridge address space (ROM 0000-7FFF, RAM A000-BFFF); else open bus $FF.
    pub fn read(&self, addr: u16) -> u8 {
        if addr <= 0x7FFF {
            self.read_rom(addr)
        } else if (0xA000..=0xBFFF).contains(&addr) {
            self.read_ram(addr - 0xA000)
        } else {
            0xFF
        }
    }

    /// Write to the cartridge address space (registers 0000-7FFF, RAM A000-BFFF).
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

    /// ROM of `banks` banks where the first byte of bank n is n.
    fn banked_rom(banks: usize) -> Vec<u8> {
        let mut rom = vec![0u8; banks * ROM_BANK_SIZE];
        for b in 0..banks {
            rom[b * ROM_BANK_SIZE] = b as u8;
        }
        rom
    }

    fn mbc(banks: usize, ram_code: u8) -> Mbc1 {
        Mbc1::new(banked_rom(banks), ram_code).expect("loads")
    }

    #[test]
    fn e05_02_power_up_maps_bank_0_low_and_bank_1_high() {
        let m = mbc(64, 0x00);
        assert_eq!(m.read(0x0000), 0);
        assert_eq!(m.read(0x4000), 1);
    }

    #[test]
    fn e05_02_rom_bank_register_selects_the_high_window() {
        let mut m = mbc(64, 0x00);
        m.write(0x2000, 0x05);
        assert_eq!(m.read(0x4000), 5);
        m.write(0x3FFF, 0x1F);
        assert_eq!(m.read(0x4000), 31);
        m.write(0x2000, 0xE5); // only the low 5 bits count
        assert_eq!(m.read(0x4000), 5);
        assert_eq!(m.read(0x0000), 0); // bank 0 stays at 0000-3FFF
    }

    #[test]
    fn e05_02_bank_register_zero_maps_to_bank_1() {
        let mut m = mbc(64, 0x00);
        m.write(0x2000, 0x00);
        assert_eq!(m.read(0x4000), 1);
        m.write(0x2000, 0x07);
        m.write(0x2000, 0x20); // low 5 bits are 0
        assert_eq!(m.read(0x4000), 1);
    }

    #[test]
    fn e05_02_upper_register_extends_the_rom_bank_on_a_1_mib_rom() {
        let mut m = mbc(64, 0x00);
        m.write(0x2000, 0x05);
        m.write(0x4000, 0x01);
        assert_eq!(m.read(0x4000), 37); // 0x25
        m.write(0x2000, 0x00);
        assert_eq!(m.read(0x4000), 33); // bank $20 reads as $21
        m.write(0x4000, 0x02);
        assert_eq!(m.read(0x4000), 1); // $41 reduced to the 64 real banks
        m.write(0x4000, 0x03);
        m.write(0x2000, 0x1F);
        assert_eq!(m.read(0x4000), 63); // $7F reduced to 64 banks
    }

    #[test]
    fn e05_02_upper_register_vanishes_on_a_512_kib_rom() {
        let mut m = mbc(32, 0x00);
        m.write(0x2000, 0x05);
        m.write(0x4000, 0x03);
        assert_eq!(m.read(0x4000), 5);
        m.write(0x2000, 0x00);
        m.write(0x4000, 0x01);
        assert_eq!(m.read(0x4000), 1);
    }

    #[test]
    fn e05_02_simple_mode_locks_the_low_window_on_bank_0() {
        let mut m = mbc(64, 0x00);
        m.write(0x4000, 0x01);
        m.write(0x6000, 0x00);
        assert_eq!(m.read(0x0000), 0);
    }

    #[test]
    fn e05_02_advanced_mode_banks_the_low_window() {
        let mut m = mbc(64, 0x00);
        m.write(0x4000, 0x01);
        m.write(0x6000, 0x01);
        assert_eq!(m.read(0x0000), 32);
        m.write(0x4000, 0x02);
        assert_eq!(m.read(0x0000), 0); // $40 reduced to the 64 real banks
        m.write(0x4000, 0x03);
        assert_eq!(m.read(0x0000), 32); // $60 reduced to 64 banks
        m.write(0x6000, 0x02); // only bit 0 selects the mode
        assert_eq!(m.read(0x0000), 0);
    }

    #[test]
    fn e05_02_bank_numbers_are_reduced_to_a_small_rom_without_panic() {
        let mut m = mbc(2, 0x00);
        m.write(0x2000, 0x1F);
        assert_eq!(m.read(0x4000), 1); // 31 % 2
        m.write(0x2000, 0x10);
        assert_eq!(m.read(0x4000), 0); // 00->01 is tested on the full register, then 16 % 2 = 0
        let short = Mbc1::new(vec![0u8; 100], 0x00).expect("loads");
        assert_eq!(short.read(0x7FFF), 0xFF);
        assert_eq!(short.read(0x0000), 0);
    }

    #[test]
    fn e05_02_ram_is_disabled_until_a_is_written() {
        let mut m = mbc(64, 0x03);
        assert_eq!(m.read_ram(0), 0xFF);
        m.write_ram(0, 0x5A);
        m.write(0x0000, 0x0A);
        assert_eq!(m.read_ram(0), 0x00); // the earlier write was ignored
        m.write_ram(0, 0x5A);
        assert_eq!(m.read_ram(0), 0x5A);
        m.write(0x1FFF, 0x1A); // only the low nibble counts
        assert_eq!(m.read_ram(0), 0x5A);
        m.write(0x0000, 0x0B);
        assert_eq!(m.read_ram(0), 0xFF);
        m.write(0x0000, 0xFA);
        assert_eq!(m.read_ram(0), 0x5A); // contents survive a disable
    }

    #[test]
    fn e05_02_ram_banks_follow_the_upper_register_in_advanced_mode_only() {
        let mut m = mbc(64, 0x03);
        m.write(0x0000, 0x0A);
        m.write_ram(0, 0x11);
        m.write(0x4000, 0x02);
        assert_eq!(m.read_ram(0), 0x11); // simple mode: locked on bank 0
        m.write(0x6000, 0x01);
        assert_eq!(m.read_ram(0), 0x00); // bank 2 is untouched
        m.write_ram(0, 0x22);
        m.write(0x4000, 0x00);
        assert_eq!(m.read_ram(0), 0x11);
        m.write(0x4000, 0x02);
        assert_eq!(m.read_ram(0), 0x22);
        m.write(0x4000, 0x03);
        m.write(0xBFFF, 0x33);
        assert_eq!(m.read(0xBFFF), 0x33);
        m.write(0x4000, 0x00);
        assert_eq!(m.read(0xBFFF), 0x00);
    }

    #[test]
    fn e05_02_eight_kib_ram_ignores_the_upper_register() {
        let mut m = mbc(64, 0x02);
        m.write(0x0000, 0x0A);
        m.write(0x6000, 0x01);
        m.write_ram(0, 0x44);
        m.write(0x4000, 0x03);
        assert_eq!(m.read_ram(0), 0x44);
        assert_eq!(m.read_ram(0x2000), 0xFF); // past the bank
    }

    #[test]
    fn e05_02_no_ram_reads_open_bus_even_when_enabled() {
        let mut m = mbc(64, 0x00);
        m.write(0x0000, 0x0A);
        m.write_ram(0, 0x5A);
        assert_eq!(m.read_ram(0), 0xFF);
    }

    #[test]
    fn e05_02_ram_size_codes_are_checked() {
        for code in [0x00u8, 0x02, 0x03] {
            assert!(Mbc1::new(banked_rom(2), code).is_ok());
        }
        for code in [0x01u8, 0x04, 0x05, 0x06, 0xFF] {
            let err = Mbc1::new(banked_rom(2), code).unwrap_err();
            assert!(matches!(err, CartridgeLoadError::UnknownRamSize(c) if c == code));
        }
    }

    #[test]
    fn e05_02_addresses_outside_the_cartridge_read_open_bus() {
        let mut m = mbc(2, 0x03);
        assert_eq!(m.read(0x8000), 0xFF);
        assert_eq!(m.read(0xC000), 0xFF);
        m.write(0x9000, 0x12);
        m.write(0xC000, 0x12);
        assert_eq!(m.read(0x4000), 1);
    }
}
