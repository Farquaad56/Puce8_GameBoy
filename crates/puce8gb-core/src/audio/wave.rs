//! Wave channel core, channel 3 (E04_03): frequency timer, 32 samples, volume shift, length, trigger. Not wired to the registers yet. Wave RAM access while the channel plays: see a later step.

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

    /// Trigger (NR34 bit 7). The sample buffer is not reloaded here.
    /// Extra start-up delay of the first sample: UNKNOWN - to confirm.
    pub fn trigger(&mut self) {
        self.enabled = self.dac_on;
        if self.length == 0 {
            self.length = 256;
        }
        self.timer = self.period();
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
        self.timer = self.timer.saturating_sub(1);
        if self.timer == 0 {
            self.timer = self.period();
            self.pos = (self.pos + 1) & 31;
            self.sample = self.nibble(self.pos);
        }
    }

    /// One M-cycle: two timer steps.
    pub fn tick(&mut self) {
        self.step_timer();
        self.step_timer();
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
        for _ in 0..4 {
            w.step_timer();
            seen.push(w.sample);
        }
        assert_eq!(seen, vec![2, 3, 4, 0]);
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
        for _ in 0..523 {
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
}
