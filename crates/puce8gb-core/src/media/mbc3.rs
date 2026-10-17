//! MBC3 mapper with a real-time clock (E05_04, note 07b). The clock advances only from emulated
//! dots, never from the system clock (determinism, decision A_05).

use super::header::CartridgeLoadError;

/// Size of one ROM bank (note 07b).
const ROM_BANK_SIZE: usize = 0x4000;
/// Size of one RAM bank (note 07b).
const RAM_BANK_SIZE: usize = 0x2000;
/// Dots of emulated time per second of RTC time: the DMG master clock (decision A_01).
pub const RTC_DOTS_PER_SECOND: u64 = 4_194_304;
/// Seconds in one day.
const SECONDS_PER_DAY: u64 = 86_400;

/// The five RTC registers (note 07b): seconds, minutes, hours, 9-bit day counter, halt, carry.
/// UNKNOWN - to confirm: behaviour of out-of-range values written by a program (the register
/// widths below are masked, then the time rolls over by plain arithmetic).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct Rtc {
    seconds: u8,
    minutes: u8,
    hours: u8,
    /// 9-bit day counter, 0-511.
    day: u16,
    halt: bool,
    /// Day overflow flag: stays set until the program clears it (note 07b).
    carry: bool,
}

impl Rtc {
    /// Register values $08-$0C: seconds, minutes, hours, day low, day high + halt + carry.
    /// UNKNOWN - to confirm: bits 1-5 of $0C (read as 0 here).
    fn read_reg(&self, reg: u8) -> u8 {
        match reg {
            0x08 => self.seconds,
            0x09 => self.minutes,
            0x0A => self.hours,
            0x0B => (self.day & 0xFF) as u8,
            0x0C => {
                (((self.day >> 8) & 1) as u8)
                    | (u8::from(self.halt) << 6)
                    | (u8::from(self.carry) << 7)
            }
            _ => 0xFF,
        }
    }

    fn write_reg(&mut self, reg: u8, val: u8) {
        match reg {
            0x08 => self.seconds = val & 0x3F,
            0x09 => self.minutes = val & 0x3F,
            0x0A => self.hours = val & 0x1F,
            0x0B => self.day = (self.day & 0x100) | u16::from(val),
            0x0C => {
                self.day = (self.day & 0x00FF) | (u16::from(val & 1) << 8);
                self.halt = val & 0x40 != 0;
                self.carry = val & 0x80 != 0;
            }
            _ => {}
        }
    }

    /// Advance the clock by `secs` seconds; nothing moves while halted. A day counter past 511
    /// wraps to 0 and sets the carry flag, which stays set (note 07b).
    fn add_seconds(&mut self, secs: u64) {
        if self.halt || secs == 0 {
            return;
        }
        let total = u64::from(self.seconds)
            + 60 * u64::from(self.minutes)
            + 3600 * u64::from(self.hours)
            + SECONDS_PER_DAY * u64::from(self.day)
            + secs;
        let days = total / SECONDS_PER_DAY;
        let rest = total % SECONDS_PER_DAY;
        self.hours = (rest / 3600) as u8;
        self.minutes = ((rest % 3600) / 60) as u8;
        self.seconds = (rest % 60) as u8;
        if days >= 512 {
            self.carry = true;
        }
        self.day = (days % 512) as u16;
    }
}

/// MBC3 cartridge (note 07b): ROM banking, up to 32 KiB of RAM in 4 banks, and an RTC.
/// Bank numbers are reduced to the real ROM or RAM size, so an out-of-range number never panics.
/// UNKNOWN - to confirm: RAM banks $04-$07 wrap around the real RAM size; a value other than $0A
/// or $00 written to 0000-1FFF leaves the enable flag unchanged; RTC writes are accepted without
/// the halt flag being set first and go to the running counters; select values $0D-$FF read $FF.
/// The delay between RTC register accesses (CONFLIT in note 07b) is not modelled.
#[derive(Debug)]
pub struct Mbc3 {
    /// Full ROM image, bank 0 first.
    rom: Vec<u8>,
    /// External RAM (empty when the header declares none), zeroed at power-up.
    ram: Vec<u8>,
    /// RAM and RTC enable flag (0000-1FFF), off at power-up.
    enabled: bool,
    /// 7-bit ROM bank register (2000-3FFF); 0 is used as 1.
    rom_bank: u8,
    /// Value written to 4000-5FFF: $00-$07 selects a RAM bank, $08-$0C an RTC register.
    select: u8,
    /// True after $00 was written to 6000-7FFF, waiting for $01 (latch sequence).
    latch_armed: bool,
    /// Running clock.
    rtc: Rtc,
    /// Snapshot read by the program, refreshed by the latch sequence.
    latched: Rtc,
    /// Dots accumulated towards the next RTC second (always below RTC_DOTS_PER_SECOND).
    dot_acc: u64,
}

impl Mbc3 {
    /// Build an MBC3 from the ROM image and the RAM size code at $0149 (note 07a): $00 = no RAM,
    /// $02 = 8 KiB, $03 = 32 KiB (4 banks). Every other code is rejected with
    /// `CartridgeLoadError::UnknownRamSize`.
    pub fn new(rom: Vec<u8>, ram_size_code: u8) -> Result<Self, CartridgeLoadError> {
        let ram_len = match ram_size_code {
            0x00 => 0,
            0x02 => RAM_BANK_SIZE,
            0x03 => 4 * RAM_BANK_SIZE,
            other => return Err(CartridgeLoadError::UnknownRamSize(other)),
        };
        Ok(Mbc3 {
            rom,
            ram: vec![0u8; ram_len],
            enabled: false,
            rom_bank: 0,
            select: 0,
            latch_armed: false,
            rtc: Rtc::default(),
            latched: Rtc::default(),
            dot_acc: 0,
        })
    }

    /// Advance the RTC by `dots` emulated dots (4_194_304 dots = 1 second). The remainder below
    /// one second is kept for the next call; a halted clock does not advance.
    pub fn tick(&mut self, dots: u64) {
        if self.rtc.halt {
            return;
        }
        self.dot_acc += dots;
        let secs = self.dot_acc / RTC_DOTS_PER_SECOND;
        self.dot_acc %= RTC_DOTS_PER_SECOND;
        self.rtc.add_seconds(secs);
    }

    fn rom_banks(&self) -> usize {
        (self.rom.len() / ROM_BANK_SIZE).max(1)
    }

    fn ram_banks(&self) -> usize {
        (self.ram.len() / RAM_BANK_SIZE).max(1)
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

    /// A write to 0000-7FFF sets one of the four registers (note 07b).
    pub fn write_rom(&mut self, addr: u16, val: u8) {
        match addr {
            0x0000..=0x1FFF => match val {
                0x0A => self.enabled = true,
                0x00 => self.enabled = false,
                _ => {}
            },
            0x2000..=0x3FFF => self.rom_bank = val & 0x7F,
            0x4000..=0x5FFF => self.select = val,
            0x6000..=0x7FFF => {
                if val == 0x01 && self.latch_armed {
                    self.latched = self.rtc;
                }
                self.latch_armed = val == 0x00;
            }
            _ => {}
        }
    }

    /// Index in the RAM array for `offset` (from A000) in the selected bank, if there is RAM.
    fn ram_index(&self, offset: u16) -> Option<usize> {
        if self.ram.is_empty() || usize::from(offset) >= RAM_BANK_SIZE {
            return None;
        }
        let bank = usize::from(self.select) % self.ram_banks();
        Some(bank * RAM_BANK_SIZE + usize::from(offset))
    }

    /// Read a byte of A000-BFFF (offset from A000): a RAM byte or the latched RTC register chosen
    /// by the select register; disabled, absent or unknown targets read open bus $FF.
    pub fn read_ram(&self, offset: u16) -> u8 {
        if !self.enabled {
            return 0xFF;
        }
        match self.select {
            0x00..=0x07 => self
                .ram_index(offset)
                .and_then(|i| self.ram.get(i))
                .copied()
                .unwrap_or(0xFF),
            0x08..=0x0C => self.latched.read_reg(self.select),
            _ => 0xFF,
        }
    }

    /// Write a byte of A000-BFFF (offset from A000): a RAM byte or a running RTC register.
    pub fn write_ram(&mut self, offset: u16, val: u8) {
        if !self.enabled {
            return;
        }
        match self.select {
            0x00..=0x07 => {
                if let Some(i) = self.ram_index(offset) {
                    if let Some(b) = self.ram.get_mut(i) {
                        *b = val;
                    }
                }
            }
            0x08..=0x0C => self.rtc.write_reg(self.select, val),
            _ => {}
        }
    }

    /// Read from the cartridge address space (ROM 0000-7FFF, RAM/RTC A000-BFFF); else open bus $FF.
    pub fn read(&self, addr: u16) -> u8 {
        if addr <= 0x7FFF {
            self.read_rom(addr)
        } else if (0xA000..=0xBFFF).contains(&addr) {
            self.read_ram(addr - 0xA000)
        } else {
            0xFF
        }
    }

    /// Write to the cartridge address space (registers 0000-7FFF, RAM/RTC A000-BFFF).
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

    fn mbc3(banks: usize, ram_code: u8) -> Mbc3 {
        Mbc3::new(banked_rom(banks), ram_code).expect("loads")
    }

    /// An enabled MBC3 with 32 KiB of RAM.
    fn clock_on() -> Mbc3 {
        let mut m = mbc3(2, 0x03);
        m.write(0x0000, 0x0A);
        m
    }

    fn run_seconds(m: &mut Mbc3, s: u64) {
        m.tick(s * RTC_DOTS_PER_SECOND);
    }

    fn latch(m: &mut Mbc3) {
        m.write(0x6000, 0x00);
        m.write(0x6000, 0x01);
    }

    /// Select an RTC register and read its latched value.
    fn reg(m: &mut Mbc3, r: u8) -> u8 {
        m.write(0x4000, r);
        m.read(0xA000)
    }

    /// Select an RTC register and write the running value.
    fn put(m: &mut Mbc3, r: u8, v: u8) {
        m.write(0x4000, r);
        m.write(0xA000, v);
    }

    #[test]
    fn e05_04_power_up_state_is_bank_1_and_ram_timer_disabled() {
        let m = mbc3(128, 0x03);
        assert_eq!(m.read(0x0000), 0);
        assert_eq!(m.read(0x4000), 1);
        assert_eq!(m.read(0xA000), 0xFF);
    }

    #[test]
    fn e05_04_rom_bank_register_is_7_bits_and_zero_maps_to_bank_1() {
        let mut m = mbc3(128, 0x00);
        m.write(0x2000, 0x7F);
        assert_eq!(m.read(0x4000), 127);
        m.write(0x2000, 0xFF);
        assert_eq!(m.read(0x4000), 127);
        m.write(0x2000, 0x00);
        assert_eq!(m.read(0x4000), 1);
        m.write(0x2000, 0x05);
        assert_eq!(m.read(0x4000), 5);
        assert_eq!(m.read(0x0000), 0);
    }

    #[test]
    fn e05_04_bank_numbers_are_reduced_to_a_small_rom_without_panic() {
        let mut m = mbc3(4, 0x00);
        m.write(0x2000, 0x7F);
        assert_eq!(m.read(0x4000), 3); // 127 % 4
        let short = Mbc3::new(vec![0u8; 100], 0x00).expect("loads");
        assert_eq!(short.read(0x7FFF), 0xFF);
        assert_eq!(short.read(0x0000), 0);
    }

    #[test]
    fn e05_04_ram_timer_enable_accepts_0a_and_00_only() {
        let mut m = mbc3(2, 0x03);
        assert_eq!(m.read(0xA000), 0xFF);
        m.write(0x0000, 0x0A);
        m.write(0xA000, 0x5A);
        assert_eq!(m.read(0xA000), 0x5A);
        m.write(0x0000, 0x05); // UNKNOWN value: the flag is left unchanged
        assert_eq!(m.read(0xA000), 0x5A);
        m.write(0x0000, 0x00);
        assert_eq!(m.read(0xA000), 0xFF);
        m.write(0xA000, 0x77);
        m.write(0x0000, 0x0A);
        assert_eq!(m.read(0xA000), 0x5A);
    }

    #[test]
    fn e05_04_ram_banks_are_independent() {
        let mut m = clock_on();
        m.write(0x4000, 0x00);
        m.write(0xA000, 0x11);
        m.write(0x4000, 0x01);
        m.write(0xA000, 0x22);
        m.write(0x4000, 0x03);
        m.write(0xBFFF, 0x33);
        assert_eq!(m.read(0xBFFF), 0x33);
        m.write(0x4000, 0x00);
        assert_eq!(m.read(0xA000), 0x11);
        m.write(0x4000, 0x01);
        assert_eq!(m.read(0xA000), 0x22);
        m.write(0x4000, 0x02);
        assert_eq!(m.read(0xA000), 0x00);
    }

    #[test]
    fn e05_04_ram_absent_reads_open_bus_but_rtc_registers_work() {
        let mut m = mbc3(2, 0x00);
        m.write(0x0000, 0x0A);
        m.write(0x4000, 0x00);
        m.write(0xA000, 0x5A);
        assert_eq!(m.read(0xA000), 0xFF);
        put(&mut m, 0x08, 7);
        latch(&mut m);
        assert_eq!(reg(&mut m, 0x08), 7);
    }

    #[test]
    fn e05_04_select_values_above_0c_read_open_bus() {
        let mut m = clock_on();
        m.write(0x4000, 0x0D);
        assert_eq!(m.read(0xA000), 0xFF);
        m.write(0x4000, 0xFF);
        assert_eq!(m.read(0xA000), 0xFF);
        m.write(0xA000, 0x12); // ignored, no panic
        assert_eq!(m.read(0xA000), 0xFF);
    }

    #[test]
    fn e05_04_latch_needs_00_then_01() {
        let mut m = clock_on();
        run_seconds(&mut m, 5);
        assert_eq!(reg(&mut m, 0x08), 0); // not latched yet
        latch(&mut m);
        assert_eq!(reg(&mut m, 0x08), 5);
        run_seconds(&mut m, 3);
        assert_eq!(reg(&mut m, 0x08), 5); // frozen until the next latch
        m.write(0x6000, 0x01); // $01 alone does not latch
        assert_eq!(reg(&mut m, 0x08), 5);
        latch(&mut m);
        assert_eq!(reg(&mut m, 0x08), 8);
        run_seconds(&mut m, 4);
        m.write(0x6000, 0x00);
        m.write(0x6000, 0x02); // breaks the sequence
        m.write(0x6000, 0x01);
        assert_eq!(reg(&mut m, 0x08), 8);
        latch(&mut m);
        assert_eq!(reg(&mut m, 0x08), 12);
    }

    #[test]
    fn e05_04_clock_rolls_over_seconds_minutes_hours_and_days() {
        let mut m = clock_on();
        run_seconds(&mut m, 59);
        latch(&mut m);
        assert_eq!(reg(&mut m, 0x08), 59);
        assert_eq!(reg(&mut m, 0x09), 0);
        run_seconds(&mut m, 1);
        latch(&mut m);
        assert_eq!(reg(&mut m, 0x08), 0);
        assert_eq!(reg(&mut m, 0x09), 1);
        let mut d = clock_on();
        run_seconds(&mut d, 86_399);
        latch(&mut d);
        assert_eq!(reg(&mut d, 0x08), 59);
        assert_eq!(reg(&mut d, 0x09), 59);
        assert_eq!(reg(&mut d, 0x0A), 23);
        assert_eq!(reg(&mut d, 0x0B), 0);
        run_seconds(&mut d, 1);
        latch(&mut d);
        assert_eq!(reg(&mut d, 0x08), 0);
        assert_eq!(reg(&mut d, 0x09), 0);
        assert_eq!(reg(&mut d, 0x0A), 0);
        assert_eq!(reg(&mut d, 0x0B), 1);
    }

    #[test]
    fn e05_04_day_counter_overflow_sets_a_persistent_carry() {
        let mut m = clock_on();
        run_seconds(&mut m, 511 * 86_400);
        latch(&mut m);
        assert_eq!(reg(&mut m, 0x0B), 0xFF);
        assert_eq!(reg(&mut m, 0x0C), 0x01);
        run_seconds(&mut m, 86_400);
        latch(&mut m);
        assert_eq!(reg(&mut m, 0x0B), 0x00);
        assert_eq!(reg(&mut m, 0x0C), 0x80);
        run_seconds(&mut m, 86_400);
        latch(&mut m);
        assert_eq!(reg(&mut m, 0x0B), 0x01);
        assert_eq!(reg(&mut m, 0x0C), 0x80);
        put(&mut m, 0x0C, 0x00); // the program clears the carry
        latch(&mut m);
        assert_eq!(reg(&mut m, 0x0C), 0x00);
        assert_eq!(reg(&mut m, 0x0B), 0x01);
    }

    #[test]
    fn e05_04_halt_stops_the_clock() {
        let mut m = clock_on();
        put(&mut m, 0x0C, 0x40);
        run_seconds(&mut m, 100);
        latch(&mut m);
        assert_eq!(reg(&mut m, 0x08), 0);
        assert_eq!(reg(&mut m, 0x0C), 0x40);
        put(&mut m, 0x0C, 0x00);
        run_seconds(&mut m, 5);
        latch(&mut m);
        assert_eq!(reg(&mut m, 0x08), 5);
        assert_eq!(reg(&mut m, 0x0C), 0x00);
    }

    #[test]
    fn e05_04_rtc_registers_can_be_written() {
        let mut m = clock_on();
        put(&mut m, 0x0C, 0x41); // halt, day bit 8
        put(&mut m, 0x08, 30);
        put(&mut m, 0x09, 59);
        put(&mut m, 0x0A, 23);
        put(&mut m, 0x0B, 0xFF);
        put(&mut m, 0x0C, 0x01); // run again, day = 511
        run_seconds(&mut m, 30); // 23:59:30 + 30 s of day 511 rolls over
        latch(&mut m);
        assert_eq!(reg(&mut m, 0x08), 0);
        assert_eq!(reg(&mut m, 0x09), 0);
        assert_eq!(reg(&mut m, 0x0A), 0);
        assert_eq!(reg(&mut m, 0x0B), 0);
        assert_eq!(reg(&mut m, 0x0C), 0x80);
    }

    #[test]
    fn e05_04_advance_is_independent_of_the_tick_chunk_size() {
        let mut a = clock_on();
        let mut b = clock_on();
        a.tick(2 * RTC_DOTS_PER_SECOND + 10);
        for _ in 0..8192 {
            b.tick(1024);
        }
        b.tick(10);
        latch(&mut a);
        latch(&mut b);
        assert_eq!(reg(&mut a, 0x08), 2);
        assert_eq!(reg(&mut b, 0x08), 2);
        assert_eq!(a.rtc, b.rtc);
        assert_eq!(a.dot_acc, b.dot_acc);
    }

    #[test]
    fn e05_04_sub_second_remainder_is_kept() {
        let mut m = clock_on();
        m.tick(RTC_DOTS_PER_SECOND - 1);
        latch(&mut m);
        assert_eq!(reg(&mut m, 0x08), 0);
        m.tick(1);
        latch(&mut m);
        assert_eq!(reg(&mut m, 0x08), 1);
    }

    #[test]
    fn e05_04_ram_size_codes_are_checked() {
        for code in [0x00u8, 0x02, 0x03] {
            assert!(Mbc3::new(banked_rom(2), code).is_ok());
        }
        for code in [0x01u8, 0x04, 0x05, 0x06, 0xFF] {
            let err = Mbc3::new(banked_rom(2), code).unwrap_err();
            assert!(matches!(err, CartridgeLoadError::UnknownRamSize(c) if c == code));
        }
    }
}
