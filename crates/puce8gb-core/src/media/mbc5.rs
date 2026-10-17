//! MBC5 mapper (E05_05, note 07b): 9-bit ROM bank (bank 0 selectable) and banked RAM.

use super::header::CartridgeLoadError;
use super::{load_ram_payload, BatteryError};

/// Size of one ROM bank (note 07b).
const ROM_BANK_SIZE: usize = 0x4000;
/// Size of one RAM bank (note 07b).
const RAM_BANK_SIZE: usize = 0x2000;

/// MBC5 cartridge (note 07b): up to 8 MiB of ROM (banks $000-$1FF) and 8/32/128 KiB of RAM.
/// Bank numbers are reduced to the real ROM or RAM size, so an out-of-range number never panics.
/// UNKNOWN - to confirm: the ROM bank register value at power-up (bank 1 here, not asserted by any
/// test); which type codes carry a rumble motor on bit 3 of the RAM bank register (not modelled:
/// the raw 4-bit register is exposed by `ram_bank_register`); a value other than $0A or $00
/// written to 0000-1FFF leaves the enable flag unchanged; writes to 6000-7FFF are ignored.
#[derive(Debug)]
pub struct Mbc5 {
    /// Full ROM image, bank 0 first.
    rom: Vec<u8>,
    /// External RAM (empty when the header declares none), zeroed at power-up.
    ram: Vec<u8>,
    /// RAM enable flag (0000-1FFF), off at power-up.
    enabled: bool,
    /// 9-bit ROM bank register: low 8 bits at 2000-2FFF, bit 8 at 3000-3FFF.
    rom_bank: u16,
    /// 4-bit RAM bank register (4000-5FFF).
    ram_bank: u8,
}

impl Mbc5 {
    /// Build an MBC5 from the ROM image and the RAM size code at $0149 (note 07a): $00 = no RAM,
    /// $02 = 8 KiB, $03 = 32 KiB (4 banks), $04 = 128 KiB (16 banks). Every other code is rejected
    /// with `CartridgeLoadError::UnknownRamSize` ($01 is a CONFLIT in note 07a; $05 is outside the
    /// 8/32/128 KiB sizes of note 07b).
    pub fn new(rom: Vec<u8>, ram_size_code: u8) -> Result<Self, CartridgeLoadError> {
        let ram_len = match ram_size_code {
            0x00 => 0,
            0x02 => RAM_BANK_SIZE,
            0x03 => 4 * RAM_BANK_SIZE,
            0x04 => 16 * RAM_BANK_SIZE,
            other => return Err(CartridgeLoadError::UnknownRamSize(other)),
        };
        Ok(Mbc5 {
            rom,
            ram: vec![0u8; ram_len],
            enabled: false,
            rom_bank: 1,
            ram_bank: 0,
        })
    }

    /// The raw 4-bit RAM bank register (bit 3 drives the rumble motor on equipped cartridges).
    pub fn ram_bank_register(&self) -> u8 {
        self.ram_bank
    }

    fn rom_banks(&self) -> usize {
        (self.rom.len() / ROM_BANK_SIZE).max(1)
    }

    fn ram_banks(&self) -> usize {
        (self.ram.len() / RAM_BANK_SIZE).max(1)
    }

    /// ROM bank mapped at `addr` (0000-7FFF), reduced to the real number of banks. Unlike the
    /// other MBCs, register value 0 selects bank 0 (note 07b).
    fn rom_bank_for(&self, addr: u16) -> usize {
        let bank = if addr < 0x4000 {
            0
        } else {
            usize::from(self.rom_bank)
        };
        bank % self.rom_banks()
    }

    /// Read a byte of the ROM region 0000-7FFF; past the image reads open bus $FF.
    pub fn read_rom(&self, addr: u16) -> u8 {
        let offset =
            self.rom_bank_for(addr) * ROM_BANK_SIZE + (usize::from(addr) & (ROM_BANK_SIZE - 1));
        self.rom.get(offset).copied().unwrap_or(0xFF)
    }

    /// A write to 0000-5FFF sets one of the registers (note 07b); 6000-7FFF is ignored.
    pub fn write_rom(&mut self, addr: u16, val: u8) {
        match addr {
            0x0000..=0x1FFF => match val {
                0x0A => self.enabled = true,
                0x00 => self.enabled = false,
                _ => {}
            },
            0x2000..=0x2FFF => self.rom_bank = (self.rom_bank & 0x100) | u16::from(val),
            0x3000..=0x3FFF => self.rom_bank = (self.rom_bank & 0x0FF) | (u16::from(val & 1) << 8),
            0x4000..=0x5FFF => self.ram_bank = val & 0x0F,
            _ => {}
        }
    }

    /// Index in the RAM array for `offset` (from A000) in the selected bank, if there is RAM.
    /// UNKNOWN - to confirm: a bank number past the real RAM size wraps around it.
    fn ram_index(&self, offset: u16) -> Option<usize> {
        if !self.enabled || self.ram.is_empty() || usize::from(offset) >= RAM_BANK_SIZE {
            return None;
        }
        let bank = usize::from(self.ram_bank) % self.ram_banks();
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

    /// Write to the cartridge address space (registers 0000-5FFF, RAM A000-BFFF).
    pub fn write(&mut self, addr: u16, val: u8) {
        if addr <= 0x7FFF {
            self.write_rom(addr, val);
        } else if (0xA000..=0xBFFF).contains(&addr) {
            self.write_ram(addr - 0xA000, val);
        }
    }

    /// Persistent bytes of the cartridge (E05_06): the external RAM, or None when there is none.
    pub fn battery_payload(&self) -> Option<Vec<u8>> {
        if self.ram.is_empty() {
            None
        } else {
            Some(self.ram.clone())
        }
    }

    /// Load the persistent RAM; the length must match exactly (E05_06).
    pub fn load_battery_payload(&mut self, data: &[u8]) -> Result<(), BatteryError> {
        load_ram_payload(&mut self.ram, data)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// ROM of `banks` banks where the first two bytes of bank n are n (low byte, high byte).
    fn banked_rom(banks: usize) -> Vec<u8> {
        let mut rom = vec![0u8; banks * ROM_BANK_SIZE];
        for b in 0..banks {
            rom[b * ROM_BANK_SIZE] = (b & 0xFF) as u8;
            rom[b * ROM_BANK_SIZE + 1] = (b >> 8) as u8;
        }
        rom
    }

    fn mbc5(banks: usize, ram_code: u8) -> Mbc5 {
        Mbc5::new(banked_rom(banks), ram_code).expect("loads")
    }

    /// Number of the bank mapped at `addr` (reads the two tag bytes of that bank).
    fn bank_at(m: &Mbc5, addr: u16) -> usize {
        usize::from(m.read(addr)) | (usize::from(m.read(addr + 1)) << 8)
    }

    #[test]
    fn e05_05_power_up_has_ram_disabled_and_bank_0_low() {
        let m = mbc5(4, 0x03);
        assert_eq!(bank_at(&m, 0x0000), 0);
        assert_eq!(m.read(0xA000), 0xFF);
    }

    #[test]
    fn e05_05_low_register_selects_banks_and_zero_is_bank_0() {
        let mut m = mbc5(512, 0x00);
        m.write(0x2000, 0x05);
        assert_eq!(bank_at(&m, 0x4000), 5);
        m.write(0x2000, 0xFF);
        assert_eq!(bank_at(&m, 0x4000), 255);
        m.write(0x2000, 0x00);
        assert_eq!(bank_at(&m, 0x4000), 0);
        m.write(0x2FFF, 0x07);
        assert_eq!(bank_at(&m, 0x4000), 7);
    }

    #[test]
    fn e05_05_bit_8_extends_the_bank_up_to_0x1ff() {
        let mut m = mbc5(512, 0x00);
        m.write(0x3000, 0x01);
        m.write(0x2000, 0x00);
        assert_eq!(bank_at(&m, 0x4000), 256);
        m.write(0x2000, 0xFF);
        assert_eq!(bank_at(&m, 0x4000), 511);
        m.write(0x3FFF, 0x00);
        assert_eq!(bank_at(&m, 0x4000), 255);
        m.write(0x3000, 0xFE); // only bit 0 counts
        assert_eq!(bank_at(&m, 0x4000), 255);
        m.write(0x3000, 0x03);
        assert_eq!(bank_at(&m, 0x4000), 511);
    }

    #[test]
    fn e05_05_the_two_bank_registers_do_not_disturb_each_other() {
        let mut m = mbc5(512, 0x00);
        m.write(0x2000, 0x34);
        m.write(0x3000, 0x01);
        assert_eq!(bank_at(&m, 0x4000), 0x134);
        m.write(0x2000, 0x35);
        assert_eq!(bank_at(&m, 0x4000), 0x135);
        m.write(0x3000, 0x00);
        assert_eq!(bank_at(&m, 0x4000), 0x35);
    }

    #[test]
    fn e05_05_bank_numbers_are_reduced_to_the_real_rom_size_without_panic() {
        let mut m = mbc5(4, 0x00);
        m.write(0x2000, 0xFF);
        m.write(0x3000, 0x01);
        assert_eq!(bank_at(&m, 0x4000), 3); // 511 % 4
        let short = Mbc5::new(vec![0u8; 100], 0x00).expect("loads");
        assert_eq!(short.read(0x7FFF), 0xFF);
        assert_eq!(short.read(0x0000), 0);
    }

    #[test]
    fn e05_05_low_window_stays_on_bank_0() {
        let mut m = mbc5(64, 0x00);
        m.write(0x2000, 0x05);
        m.write(0x4000, 0x02);
        assert_eq!(bank_at(&m, 0x0000), 0);
        assert_eq!(bank_at(&m, 0x4000), 5);
    }

    #[test]
    fn e05_05_ram_enable_accepts_0a_and_00_only() {
        let mut m = mbc5(4, 0x03);
        assert_eq!(m.read(0xA000), 0xFF);
        m.write(0x0000, 0x0A);
        m.write(0xA000, 0x5A);
        assert_eq!(m.read(0xA000), 0x5A);
        m.write(0x1FFF, 0x05); // UNKNOWN value: the flag is left unchanged
        assert_eq!(m.read(0xA000), 0x5A);
        m.write(0x0000, 0x00);
        assert_eq!(m.read(0xA000), 0xFF);
        m.write(0xA000, 0x77);
        m.write(0x0000, 0x0A);
        assert_eq!(m.read(0xA000), 0x5A);
    }

    #[test]
    fn e05_05_sixteen_ram_banks_are_independent() {
        let mut m = mbc5(4, 0x04);
        m.write(0x0000, 0x0A);
        for b in 0..16u8 {
            m.write(0x4000, b);
            m.write(0xA000, 0x40 + b);
            m.write(0xBFFF, 0x80 + b);
        }
        for b in 0..16u8 {
            m.write(0x5FFF, b);
            assert_eq!(m.read(0xA000), 0x40 + b, "bank {b}");
            assert_eq!(m.read(0xBFFF), 0x80 + b, "bank {b} end");
        }
    }

    #[test]
    fn e05_05_ram_bank_register_has_4_bits_and_wraps_on_small_ram() {
        let mut m = mbc5(4, 0x03);
        m.write(0x0000, 0x0A);
        m.write(0x4000, 0xF3);
        assert_eq!(m.ram_bank_register(), 0x03);
        m.write(0x4000, 0x08);
        assert_eq!(m.ram_bank_register(), 0x08); // bit 3 is kept for a rumble motor
        m.write(0x4000, 0x01);
        m.write(0xA000, 0x66);
        m.write(0x4000, 0x05); // 5 % 4 = bank 1
        assert_eq!(m.read(0xA000), 0x66);
    }

    #[test]
    fn e05_05_no_ram_reads_open_bus_even_when_enabled() {
        let mut m = mbc5(4, 0x00);
        m.write(0x0000, 0x0A);
        m.write(0xA000, 0x5A);
        assert_eq!(m.read(0xA000), 0xFF);
    }

    #[test]
    fn e05_05_ram_size_codes_are_checked() {
        for code in [0x00u8, 0x02, 0x03, 0x04] {
            assert!(Mbc5::new(banked_rom(2), code).is_ok());
        }
        for code in [0x01u8, 0x05, 0x06, 0xFF] {
            let err = Mbc5::new(banked_rom(2), code).unwrap_err();
            assert!(matches!(err, CartridgeLoadError::UnknownRamSize(c) if c == code));
        }
    }

    #[test]
    fn e05_05_writes_to_6000_7fff_and_other_addresses_are_ignored() {
        let mut m = mbc5(4, 0x03);
        m.write(0x2000, 0x02);
        m.write(0x6000, 0x01);
        m.write(0x7FFF, 0x03);
        m.write(0x9000, 0x12);
        m.write(0xC000, 0x12);
        assert_eq!(bank_at(&m, 0x4000), 2);
        assert_eq!(m.ram_bank_register(), 0);
        assert_eq!(m.read(0x8000), 0xFF);
        assert_eq!(m.read(0xC000), 0xFF);
    }

    #[test]
    fn e05_06_mbc5_battery_payload_round_trips_sixteen_ram_banks() {
        let mut m = mbc5(4, 0x04);
        m.write(0x0000, 0x0A);
        for b in 0..16u8 {
            m.write(0x4000, b);
            m.write(0xA000, 0x40 + b);
        }
        let payload = m.battery_payload().expect("has RAM");
        assert_eq!(payload.len(), 128 * 1024);
        for b in 0..16usize {
            assert_eq!(payload[b * 0x2000], 0x40 + b as u8, "bank {b}");
        }
        let mut other = mbc5(4, 0x04);
        assert_eq!(other.load_battery_payload(&payload), Ok(()));
        other.write(0x0000, 0x0A);
        other.write(0x4000, 0x09);
        assert_eq!(other.read(0xA000), 0x49);
    }

    #[test]
    fn e05_06_mbc5_without_ram_has_no_payload() {
        let mut m = mbc5(4, 0x00);
        assert_eq!(m.battery_payload(), None);
        assert_eq!(
            m.load_battery_payload(&[0u8; 8]),
            Err(BatteryError::NoBatteryData)
        );
    }

    #[test]
    fn e05_06_mbc5_load_rejects_a_wrong_size() {
        let mut m = mbc5(4, 0x02);
        assert_eq!(
            m.load_battery_payload(&vec![0u8; 8193]),
            Err(BatteryError::SizeMismatch {
                expected: 8192,
                actual: 8193
            })
        );
    }
}
