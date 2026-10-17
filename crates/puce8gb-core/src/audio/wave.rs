//! Wave channel core, channel 3 (E04_03): frequency timer, 32 samples, volume shift, length, trigger. Not wired to the registers yet. Wave RAM access while the channel plays: see a later step.

/// First timer step after a sample read (2 dots per step) at which the CPU reaches the wave RAM
/// byte being read (DMG). Calibrated with dmg_sound 09, 10 and 12 (not documented in the sources).
pub const WAVE_ACCESS_MIN_STEPS: u8 = 1;

/// Last timer step after a sample read at which the CPU still reaches that byte (DMG). The gbdev
/// wiki only says "within a couple of clocks". Calibrated with dmg_sound 09, 10 and 12 (not documented in the sources).
pub const WAVE_ACCESS_MAX_STEPS: u8 = 1;

/// Extra timer steps before the first sample read after a trigger. Calibrated with dmg_sound 09, 10 and 12 (not documented in the sources).
pub const WAVE_TRIGGER_DELAY_STEPS: u16 = 2;

/// First timer step after a sample read (2 dots per step) at which a trigger corrupts wave RAM
/// (DMG). Calibrated with dmg_sound 09, 10 and 12 (not documented in the sources).
pub const WAVE_CORRUPT_MIN_STEPS: u8 = 0;

/// Last timer step after a sample read at which a trigger corrupts wave RAM (DMG).
/// Calibrated with dmg_sound 09, 10 and 12 (not documented in the sources).
pub const WAVE_CORRUPT_MAX_STEPS: u8 = 0;

/// CPU access rule for wave RAM $FF30-$FF3F (DMG, gbdev wiki "Gameboy sound hardware").
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WaveAccess {
    /// Channel off: normal access.
    Free,
    /// Channel on, outside the read window: reads return $FF, writes are ignored.
    Closed,
    /// Channel on, inside the read window: access goes to this byte index (0..=15).
    Open(u8),
}

/// DMG retrigger corruption: triggering while the channel reads byte `index` rewrites the first
/// four bytes. Index 0..=3: only byte 0 is rewritten with that byte. Index 4..=15: bytes 0..=3
/// are rewritten with the aligned four-byte block that contains `index` (Pan Docs, gbdev wiki).
pub fn corrupt_ram(ram: &mut [u8; 16], index: u8) {
    let i = usize::from(index) & 15;
    if i < 4 {
        ram[0] = ram[i];
    } else {
        let base = i & !3;
        ram.copy_within(base..base + 4, 0);
    }
}

/// State of the wave channel. `tick` is called once per M-cycle (two timer steps).
#[derive(Debug, Default)]
pub struct Wave {
    pub enabled: bool,
    pub dac_on: bool,
    pub length: u16,
    pub length_enable: bool,
    pub volume_code: u8,
    pub freq: u16,
    pub pos: u8,
    pub sample: u8,
    /// Timer steps since the last sample read; saturates at 255 (E04_07).
    pub since_read: u8,
    /// Byte index to corrupt, recorded when the channel is retriggered inside the read window;
    /// consumed by the machine (E04_07).
    pub pending_corruption: Option<u8>,
    /// Copy of wave RAM ($FF30-$FF3F), high nibble first.
    pub ram: [u8; 16],
    timer: u16,
}

impl Wave {
    /// NR30: bit 7 is the DAC enable; turning it off disables the channel.
    pub fn write_nr30(&mut self, v: u8) {
        self.dac_on = v & 0x80 != 0;
        if !self.dac_on {
            self.enabled = false;
        }
    }

    /// NR31: length load (length = 256 - value).
    pub fn write_nr31(&mut self, v: u8) {
        self.length = 256 - u16::from(v);
    }

    /// NR32: output level code in bits 6-5 (0 mute, 1 100%, 2 50%, 3 25%).
    pub fn write_nr32(&mut self, v: u8) {
        self.volume_code = (v >> 5) & 0x03;
    }

    /// 11-bit frequency value (NR33 plus NR34 bits 2-0).
    pub fn set_freq(&mut self, freq: u16) {
        self.freq = freq & 0x07FF;
    }

    fn period(&self) -> u16 {
        2048 - self.freq
    }

    /// Trigger (NR34 bit 7).
    pub fn trigger(&mut self) {
        self.trigger_with(false);
        self.since_read = u8::MAX;
    }

    /// The sample buffer is not reloaded here. Extra start-up delay of the first sample:
    /// UNKNOWN - to confirm. `early_length_clock` is true when the next frame sequencer step does
    /// not clock length.
    pub fn trigger_with(&mut self, early_length_clock: bool) {
        self.enabled = self.dac_on;
        if self.length == 0 {
            self.length = if early_length_clock && self.length_enable {
                255
            } else {
                256
            };
        }
        self.timer = self.period() + WAVE_TRIGGER_DELAY_STEPS;
        self.pos = 0;
    }

    fn nibble(&self, pos: u8) -> u8 {
        let byte = self.ram[usize::from(pos / 2)];
        if pos & 1 == 0 {
            byte >> 4
        } else {
            byte & 0x0F
        }
    }

    /// One step of the frequency timer (half an M-cycle); the next sample is read when it expires.
    fn step_timer(&mut self) {
        self.since_read = self.since_read.saturating_add(1);
        self.timer = self.timer.saturating_sub(1);
        if self.timer == 0 {
            self.timer = self.period();
            self.pos = (self.pos + 1) & 31;
            self.sample = self.nibble(self.pos);
            self.since_read = 0;
        }
    }

    /// One M-cycle: two timer steps.
    pub fn tick(&mut self) {
        self.step_timer();
        self.step_timer();
    }

    /// NR34 bit 6 written; `skips_length` is true when the next frame sequencer step does not clock
    /// length; `trigger` is the NR34 bit 7 of the same write.
    pub fn write_length_enable(&mut self, enable: bool, skips_length: bool, trigger: bool) {
        let was = self.length_enable;
        self.length_enable = enable;
        if skips_length && enable && !was && self.length > 0 {
            self.length -= 1;
            if self.length == 0 && !trigger {
                self.enabled = false;
            }
        }
    }

    /// Length clock (frame sequencer EV_LENGTH): expiry disables the channel.
    pub fn clock_length(&mut self) {
        if self.length_enable && self.length > 0 {
            self.length -= 1;
            if self.length == 0 {
                self.enabled = false;
            }
        }
    }

    /// Current digital output, 0..=15.
    pub fn output(&self) -> u8 {
        if !(self.enabled && self.dac_on) {
            return 0;
        }
        match self.volume_code {
            0 => 0,
            1 => self.sample,
            2 => self.sample >> 1,
            _ => self.sample >> 2,
        }
    }

    /// CPU access rule for wave RAM right now.
    pub fn access(&self) -> WaveAccess {
        if !self.enabled {
            WaveAccess::Free
        } else if (WAVE_ACCESS_MIN_STEPS..=WAVE_ACCESS_MAX_STEPS).contains(&self.since_read) {
            WaveAccess::Open(self.pos / 2)
        } else {
            WaveAccess::Closed
        }
    }

    /// Byte index that a trigger right now would corrupt (DMG retrigger corruption), if any.
    pub fn corruption_index(&self) -> Option<u8> {
        if self.enabled
            && (WAVE_CORRUPT_MIN_STEPS..=WAVE_CORRUPT_MAX_STEPS).contains(&self.since_read)
        {
            Some(self.pos / 2)
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn on() -> Wave {
        let mut w = Wave::default();
        w.write_nr30(0x80);
        w.write_nr32(0x20);
        w.set_freq(2047);
        w
    }

    #[test]
    fn e04_03_samples_are_read_high_nibble_first_and_wrap() {
        let mut w = on();
        w.ram[0] = 0x12;
        w.ram[1] = 0x34;
        w.trigger();
        let mut seen = Vec::new();
        for _ in 0..6 {
            w.step_timer();
            seen.push(w.sample);
        }
        assert_eq!(seen, vec![0, 0, 2, 3, 4, 0]);
        for _ in 0..28 {
            w.step_timer();
        }
        assert_eq!(w.pos, 0);
        assert_eq!(w.sample, 1);
    }

    #[test]
    fn e04_03_volume_codes_shift_the_sample() {
        let expected = [0u8, 12, 6, 3];
        for code in 0..4u8 {
            let mut w = on();
            w.write_nr32(code << 5);
            w.trigger();
            w.sample = 12;
            assert_eq!(w.output(), expected[usize::from(code)], "code {code}");
        }
    }

    #[test]
    fn e04_03_length_load_and_expiry() {
        let mut w = on();
        w.write_nr31(0x00);
        assert_eq!(w.length, 256);
        w.write_nr31(0xFF);
        assert_eq!(w.length, 1);
        w.length_enable = true;
        w.trigger();
        assert!(w.enabled);
        w.clock_length();
        assert!(!w.enabled);
    }

    #[test]
    fn e04_03_trigger_reloads_length_when_zero() {
        let mut w = on();
        w.trigger();
        assert_eq!(w.length, 256);
    }

    #[test]
    fn e04_03_timer_period_is_2048_minus_freq_in_half_m_cycles() {
        let mut w = on();
        w.set_freq(1000);
        w.trigger();
        for _ in 0..524 {
            w.tick();
        }
        assert_eq!(w.pos, 0);
        w.tick();
        assert_eq!(w.pos, 1);
    }

    #[test]
    fn e04_03_dac_off_blocks_trigger_and_disables() {
        let mut w = Wave::default();
        w.trigger();
        assert!(!w.enabled);
        w.write_nr30(0x80);
        w.trigger();
        assert!(w.enabled);
        w.write_nr30(0x00);
        assert!(!w.enabled);
        assert_eq!(w.output(), 0);
    }

    #[test]
    fn e04_03_trigger_resets_position() {
        let mut w = on();
        w.pos = 5;
        w.trigger();
        assert_eq!(w.pos, 0);
    }

    #[test]
    fn e04_06_wave_length_enable_extra_clock_to_zero_disables_channel() {
        let mut w = Wave::default();
        w.write_nr31(0xFF); // length = 1
        w.enabled = true;
        w.write_length_enable(true, true, false);
        assert_eq!(w.length, 0);
        assert!(!w.enabled);
    }

    #[test]
    fn e04_06_wave_trigger_reload_is_255_in_skipping_phase_with_length_enabled() {
        let mut w1 = Wave {
            length_enable: true,
            ..Default::default()
        };
        w1.trigger_with(true);
        assert_eq!(w1.length, 255);
        let mut w2 = Wave::default();
        w2.trigger_with(true); // length enable off: reload stays 256
        assert_eq!(w2.length, 256);
    }

    #[test]
    fn e04_07_wave_access_is_free_when_channel_is_off() {
        assert_eq!(Wave::default().access(), WaveAccess::Free);
    }

    #[test]
    fn e04_07_wave_access_is_open_right_after_a_sample_read() {
        let mut w = Wave {
            dac_on: true,
            freq: 0x7FD,
            ..Default::default()
        };
        w.trigger();
        for _ in 0..6 {
            w.tick();
        }
        assert_eq!(w.pos, 3);
        assert_eq!(w.since_read, 1);
        assert_eq!(w.access(), WaveAccess::Open(1));
    }

    #[test]
    fn e04_07_wave_access_is_closed_after_trigger_and_between_reads() {
        let mut w = Wave {
            dac_on: true,
            freq: 0x7FD,
            ..Default::default()
        };
        w.trigger();
        assert_eq!(w.access(), WaveAccess::Closed);
        w.tick();
        assert_eq!(w.access(), WaveAccess::Closed);
        w.tick();
        assert_eq!(w.pos, 0);
        assert_eq!(w.access(), WaveAccess::Closed);
        w.tick();
        assert_eq!(w.pos, 1);
        assert_eq!(w.access(), WaveAccess::Open(0));
        w.tick();
        assert_eq!(w.since_read, 0);
        assert_eq!(w.access(), WaveAccess::Closed);
        w.tick();
        assert_eq!(w.since_read, 2);
        assert_eq!(w.access(), WaveAccess::Closed);
    }

    #[test]
    fn e04_07_retrigger_corruption_from_first_block() {
        let mut ram = [0u8; 16];
        for (i, b) in ram.iter_mut().enumerate() {
            *b = (i as u8) * 0x11;
        }
        corrupt_ram(&mut ram, 2);
        assert_eq!(ram[0], 0x22);
        assert_eq!(ram[1], 0x11);
        assert_eq!(ram[3], 0x33);
    }

    #[test]
    fn e04_07_retrigger_corruption_from_later_block() {
        let mut ram = [0u8; 16];
        for (i, b) in ram.iter_mut().enumerate() {
            *b = (i as u8) * 0x11;
        }
        corrupt_ram(&mut ram, 9);
        assert_eq!(&ram[0..4], &[0x88, 0x99, 0xAA, 0xBB]);
        assert_eq!(ram[4], 0x44);
        assert_eq!(ram[8], 0x88);
    }

    #[test]
    fn e04_07_wave_corruption_index_follows_the_read_window() {
        let mut w = Wave {
            dac_on: true,
            freq: 0x7FD,
            ..Default::default()
        };
        w.trigger();
        assert_eq!(w.corruption_index(), None);
        for _ in 0..3 {
            w.tick();
        }
        assert_eq!(w.corruption_index(), None);
        w.tick();
        assert_eq!(w.corruption_index(), Some(1));
        w.tick();
        assert_eq!(w.corruption_index(), None);
    }
}
