//! MBC2 mapper (E05_03, note 07b): ROM banking plus 512 x 4-bit built-in RAM.

use super::{load_ram_payload, BatteryError};

/// Size of one ROM bank (note 07b).
const ROM_BANK_SIZE: usize = 0x4000;
/// Number of 4-bit RAM cells at A000-A1FF (note 07b); A200-BFFF echoes them.
const RAM_CELLS: usize = 512;
mod state;

/// MBC2 cartridge (note 07b): registers written in 0000-3FFF, role chosen by address bit 8.
/// The RAM size code of the header is ignored: the RAM is built into the chip. Bank numbers are
/// reduced to the real ROM size, so an out-of-range number never panics.
/// UNKNOWN - to confirm: value of the upper 4 bits on a RAM read (read as 1 here) and value read
/// while the RAM is disabled ($FF here); note 07b does not document them.
#[derive(Debug)]
pub struct Mbc2 {
    /// Full ROM image, bank 0 first.
    rom: Vec<u8>,
    /// Built-in RAM; only the low 4 bits of each cell are kept.
    ram: Vec<u8>,
    /// RAM enable flag (default off).
    ram_enabled: bool,
    /// 4-bit ROM bank register (default 1); 0 is used as 1.
    rom_bank: u8,
}

impl Mbc2 {
    /// Build an MBC2 from the ROM image (RAM zeroed, disabled; ROM bank 1, note 07b).
    pub fn new(rom: Vec<u8>) -> Self {
        Mbc2 {
            rom,
            ram: vec![0u8; RAM_CELLS],
            ram_enabled: false,
            rom_bank: 1,
        }
    }

    fn rom_banks(&self) -> usize {
        (self.rom.len() / ROM_BANK_SIZE).max(1)
    }

    /// ROM bank mapped at `addr` (0000-7FFF), reduced to the real number of banks.
    fn rom_bank_for(&self, addr: u16) -> usize {
        let bank = if addr < 0x4000 {
            0
        } else if self.rom_bank == 0 {
            1
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

    /// A write to 0000-3FFF: address bit 8 = 0 sets the RAM enable (low nibble $A enables),
    /// address bit 8 = 1 sets the ROM bank (low 4 bits); other addresses are ignored (note 07b).
    pub fn write_rom(&mut self, addr: u16, val: u8) {
        if addr <= 0x3FFF {
            if addr & 0x0100 == 0 {
                self.ram_enabled = val & 0x0F == 0x0A;
            } else {
                self.rom_bank = val & 0x0F;
            }
        }
    }

    /// Read a byte of A000-BFFF (offset from A000); the 512 cells repeat every 512 bytes.
    pub fn read_ram(&self, offset: u16) -> u8 {
        if !self.ram_enabled {
            return 0xFF;
        }
        0xF0 | (self.ram[usize::from(offset) & (RAM_CELLS - 1)] & 0x0F)
    }

    /// Write a byte of A000-BFFF (offset from A000); only the low 4 bits are kept, ignored while
    /// the RAM is disabled.
    pub fn write_ram(&mut self, offset: u16, val: u8) {
        if self.ram_enabled {
            self.ram[usize::from(offset) & (RAM_CELLS - 1)] = val & 0x0F;
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

    /// Write to the cartridge address space (registers 0000-3FFF, RAM A000-BFFF).
    pub fn write(&mut self, addr: u16, val: u8) {
        if addr <= 0x7FFF {
            self.write_rom(addr, val);
        } else if (0xA000..=0xBFFF).contains(&addr) {
            self.write_ram(addr - 0xA000, val);
        }
    }

    /// The built-in RAM: 512 cells, one low nibble each.
    pub fn ram(&self) -> &[u8] {
        &self.ram
    }

    /// Persistent bytes of the cartridge (E05_06): the 512 built-in RAM cells, one low nibble each.
    pub fn battery_payload(&self) -> Option<Vec<u8>> {
        Some(self.ram.clone())
    }

    /// Load the 512 RAM cells; the length must match exactly, only the low nibble of each byte is
    /// kept (note 07b) (E05_06).
    pub fn load_battery_payload(&mut self, data: &[u8]) -> Result<(), BatteryError> {
        load_ram_payload(&mut self.ram, data)?;
        for cell in &mut self.ram {
            *cell &= 0x0F;
        }
        Ok(())
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

    fn mbc2(banks: usize) -> Mbc2 {
        Mbc2::new(banked_rom(banks))
    }

    #[test]
    fn e05_03_power_up_state_is_bank_1_and_ram_disabled() {
        let m = mbc2(16);
        assert_eq!(m.read(0x0000), 0);
        assert_eq!(m.read(0x4000), 1);
        assert_eq!(m.read(0xA000), 0xFF);
    }

    #[test]
    fn e05_03_rom_bank_is_selected_when_address_bit_8_is_set() {
        let mut m = mbc2(16);
        m.write(0x2100, 0x05);
        assert_eq!(m.read(0x4000), 5);
        m.write(0x0100, 0x03);
        assert_eq!(m.read(0x4000), 3);
        m.write(0x3FFF, 0x0F);
        assert_eq!(m.read(0x4000), 15);
        assert_eq!(m.read(0x0000), 0);
    }

    #[test]
    fn e05_03_rom_bank_zero_maps_to_bank_1() {
        let mut m = mbc2(16);
        m.write(0x2100, 0x05);
        m.write(0x2100, 0x00);
        assert_eq!(m.read(0x4000), 1);
        m.write(0x2100, 0x05);
        m.write(0x2100, 0x10); // low 4 bits are 0
        assert_eq!(m.read(0x4000), 1);
        m.write(0x2100, 0xF7); // only the low 4 bits count
        assert_eq!(m.read(0x4000), 7);
    }

    #[test]
    fn e05_03_ram_enable_is_selected_when_address_bit_8_is_clear() {
        let mut m = mbc2(16);
        m.write(0x0000, 0x0A);
        m.write(0xA000, 0x05);
        assert_eq!(m.read(0xA000) & 0x0F, 5);
        m.write(0x1E00, 0x1A); // low nibble $A keeps it enabled
        assert_eq!(m.read(0xA000) & 0x0F, 5);
        m.write(0x0000, 0x0B);
        assert_eq!(m.read(0xA000), 0xFF);
        m.write(0x0100, 0x0A); // bit 8 set: a ROM bank write, not an enable
        assert_eq!(m.read(0xA000), 0xFF);
        assert_eq!(m.read(0x4000), 10);
        m.write(0x2000, 0x0A); // bit 8 clear: enables, contents are kept
        assert_eq!(m.read(0xA000) & 0x0F, 5);
    }

    #[test]
    fn e05_03_writes_while_ram_is_disabled_are_ignored() {
        let mut m = mbc2(16);
        m.write(0xA000, 0x07);
        m.write(0x0000, 0x0A);
        assert_eq!(m.read(0xA000) & 0x0F, 0);
    }

    #[test]
    fn e05_03_ram_keeps_only_the_low_nibble() {
        let mut m = mbc2(16);
        m.write(0x0000, 0x0A);
        m.write(0xA000, 0xA5);
        assert_eq!(m.read(0xA000) & 0x0F, 0x05);
        m.write(0xA001, 0xFF);
        assert_eq!(m.read(0xA001) & 0x0F, 0x0F);
        m.write(0xA002, 0xB0);
        assert_eq!(m.read(0xA002) & 0x0F, 0x00);
        assert_eq!(m.read(0xA003) & 0x0F, 0x00);
    }

    #[test]
    fn e05_03_ram_is_echoed_every_512_bytes() {
        let mut m = mbc2(16);
        m.write(0x0000, 0x0A);
        m.write(0xA000, 0x07);
        assert_eq!(m.read(0xA200) & 0x0F, 7);
        assert_eq!(m.read(0xBE00) & 0x0F, 7);
        m.write(0xBFFF, 0x09);
        assert_eq!(m.read(0xA1FF) & 0x0F, 9);
        assert_eq!(m.read(0xBDFF) & 0x0F, 9);
    }

    #[test]
    fn e05_03_all_512_cells_hold_their_own_value() {
        let mut m = mbc2(16);
        m.write(0x0000, 0x0A);
        for i in 0..0x200usize {
            m.write(0xA000 + i as u16, ((i * 7) & 0x0F) as u8);
        }
        for i in 0..0x200usize {
            assert_eq!(
                m.read(0xA000 + i as u16) & 0x0F,
                ((i * 7) & 0x0F) as u8,
                "cell {i}"
            );
        }
    }

    #[test]
    fn e05_03_ram_contents_survive_a_disable() {
        let mut m = mbc2(16);
        m.write(0x0000, 0x0A);
        m.write(0xA010, 0x0C);
        m.write(0x0000, 0x00);
        assert_eq!(m.read(0xA010), 0xFF);
        m.write(0x0000, 0x0A);
        assert_eq!(m.read(0xA010) & 0x0F, 0x0C);
    }

    #[test]
    fn e05_03_bank_numbers_are_reduced_to_a_small_rom_without_panic() {
        let mut m = mbc2(4);
        m.write(0x2100, 0x0F);
        assert_eq!(m.read(0x4000), 3); // 15 % 4
        let short = Mbc2::new(vec![0u8; 100]);
        assert_eq!(short.read(0x7FFF), 0xFF);
        assert_eq!(short.read(0x0000), 0);
    }

    #[test]
    fn e05_03_writes_outside_the_registers_are_ignored() {
        let mut m = mbc2(16);
        m.write(0x4000, 0x0A);
        m.write(0x7FFF, 0x05);
        m.write(0x8000, 0x01);
        assert_eq!(m.read(0xA000), 0xFF);
        assert_eq!(m.read(0x4000), 1);
        assert_eq!(m.read(0x8000), 0xFF);
        assert_eq!(m.read(0xC000), 0xFF);
    }

    #[test]
    fn e05_06_mbc2_battery_payload_holds_the_512_nibbles() {
        let mut m = mbc2(16);
        m.write(0x0000, 0x0A);
        m.write(0xA000, 0xA5);
        m.write(0xA1FF, 0x3C);
        let payload = m.battery_payload().expect("built-in RAM");
        assert_eq!(payload.len(), 512);
        assert_eq!(payload[0], 0x05);
        assert_eq!(payload[511], 0x0C);
    }

    #[test]
    fn e05_06_mbc2_load_keeps_only_the_low_nibble_and_checks_the_size() {
        let mut m = mbc2(16);
        assert_eq!(m.load_battery_payload(&[0xFFu8; 512]), Ok(()));
        m.write(0x0000, 0x0A);
        assert_eq!(m.read(0xA000) & 0x0F, 0x0F);
        assert_eq!(m.battery_payload().expect("RAM")[0], 0x0F);
        assert_eq!(
            m.load_battery_payload(&[0u8; 511]),
            Err(BatteryError::SizeMismatch {
                expected: 512,
                actual: 511
            })
        );
    }
}
