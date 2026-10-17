//! Pulse channel core shared by channels 1 and 2 (E04_02): duty, frequency timer, length, envelope, trigger. Sweep is added in a later step. Not wired to the registers yet.

/// Duty waveforms, one row per duty value (0..=3), indexed by duty position 0..=7.
const DUTY: [[u8; 8]; 4] = [
    [0, 0, 0, 0, 0, 0, 0, 1],
    [1, 0, 0, 0, 0, 0, 0, 1],
    [1, 0, 0, 0, 0, 1, 1, 1],
    [0, 1, 1, 1, 1, 1, 1, 0],
];

/// State of one pulse channel. `tick` is called once per M-cycle.
#[derive(Debug, Default)]
pub struct Pulse {
    pub enabled: bool,
    pub dac_on: bool,
    pub duty: u8,
    pub duty_pos: u8,
    pub freq: u16,
    pub length: u8,
    pub length_enable: bool,
    pub volume: u8,
    timer: u16,
    env_initial: u8,
    env_add: bool,
    env_period: u8,
    env_timer: u8,
    sweep_pace: u8,
    sweep_negate: bool,
    sweep_shift: u8,
    sweep_timer: u8,
    sweep_enabled: bool,
    shadow: u16,
    sweep_negate_used: bool,
}

impl Pulse {
    /// Channel state right after the boot ROM: NR11=$BF, NR12=$F3, channel on (NR52 reads $F1).
    /// Initial frequency after boot: UNKNOWN - to confirm (set to the NR13/NR14 power-up bits).
    pub fn post_boot() -> Self {
        let mut p = Self::default();
        p.write_nrx1(0xBF);
        p.write_nrx2(0xF3);
        p.set_freq(0x07FF);
        p.trigger();
        p
    }

    /// NRx1: duty in bits 7-6, length load in bits 5-0 (length = 64 - value).
    pub fn write_nrx1(&mut self, v: u8) {
        self.duty = v >> 6;
        self.length = 64 - (v & 0x3F);
    }

    /// NRx2: initial volume bits 7-4, direction bit 3 (1 = increase), period bits 2-0.
    /// The DAC is on when bits 7-3 are not all zero; turning it off disables the channel.
    pub fn write_nrx2(&mut self, v: u8) {
        self.env_initial = v >> 4;
        self.env_add = v & 0x08 != 0;
        self.env_period = v & 0x07;
        self.dac_on = v & 0xF8 != 0;
        if !self.dac_on {
            self.enabled = false;
        }
    }

    /// NR10 (channel 1 only): pace bits 6-4, direction bit 3 (1 = decrease), shift bits 2-0.
    pub fn write_nr10(&mut self, v: u8) {
        let negate = v & 0x08 != 0;
        if self.sweep_negate && !negate && self.sweep_negate_used {
            self.enabled = false;
        }
        self.sweep_pace = (v >> 4) & 0x07;
        self.sweep_negate = negate;
        self.sweep_shift = v & 0x07;
    }

    fn sweep_calc(&mut self) -> u16 {
        let delta = self.shadow >> self.sweep_shift;
        if self.sweep_negate {
            self.sweep_negate_used = true;
            self.shadow - delta
        } else {
            self.shadow + delta
        }
    }

    /// Sweep clock (frame sequencer EV_SWEEP). Frequency overflow (> 2047) disables the channel.
    /// Behaviour when the negate bit is cleared after a negate calculation: UNKNOWN - to confirm.
    pub fn clock_sweep(&mut self) {
        if self.sweep_timer > 0 {
            self.sweep_timer -= 1;
        }
        if self.sweep_timer == 0 {
            self.sweep_timer = if self.sweep_pace == 0 {
                8
            } else {
                self.sweep_pace
            };
            if self.sweep_enabled && self.sweep_pace != 0 {
                let new = self.sweep_calc();
                if new > 2047 {
                    self.enabled = false;
                } else if self.sweep_shift != 0 {
                    self.shadow = new;
                    self.freq = new;
                    if self.sweep_calc() > 2047 {
                        self.enabled = false;
                    }
                }
            }
        }
    }

    /// 11-bit frequency value (NRx3 plus NRx4 bits 2-0).
    pub fn set_freq(&mut self, freq: u16) {
        self.freq = freq & 0x07FF;
    }

    fn period(&self) -> u16 {
        2048 - self.freq
    }

    /// Trigger (NRx4 bit 7).
    pub fn trigger(&mut self) {
        self.trigger_with(false);
    }

    /// Trigger; `early_length_clock` is true when the next frame sequencer step does not clock length.
    pub fn trigger_with(&mut self, early_length_clock: bool) {
        self.enabled = self.dac_on;
        if self.length == 0 {
            self.length = if early_length_clock && self.length_enable {
                63
            } else {
                64
            };
        }
        self.timer = self.period();
        self.env_timer = if self.env_period == 0 {
            8
        } else {
            self.env_period
        };
        self.volume = self.env_initial;
        self.sweep_negate_used = false;
        self.shadow = self.freq;
        self.sweep_timer = if self.sweep_pace == 0 {
            8
        } else {
            self.sweep_pace
        };
        self.sweep_enabled = self.sweep_pace != 0 || self.sweep_shift != 0;
        if self.sweep_shift != 0 && self.sweep_calc() > 2047 {
            self.enabled = false;
        }
    }

    /// One M-cycle of the frequency timer; the duty position advances when it expires.
    pub fn tick(&mut self) {
        self.timer = self.timer.saturating_sub(1);
        if self.timer == 0 {
            self.timer = self.period();
            self.duty_pos = (self.duty_pos + 1) & 7;
        }
    }

    /// NRx4 bit 6 written; `skips_length` is true when the next frame sequencer step does not clock
    /// length; `trigger` is the NRx4 bit 7 of the same write.
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

    /// Envelope clock (frame sequencer EV_ENVELOPE). Period 0 never changes the volume.
    pub fn clock_envelope(&mut self) {
        if self.env_timer > 0 {
            self.env_timer -= 1;
        }
        if self.env_timer == 0 {
            self.env_timer = if self.env_period == 0 {
                8
            } else {
                self.env_period
            };
            if self.enabled && self.env_period != 0 {
                if self.env_add && self.volume < 15 {
                    self.volume += 1;
                } else if !self.env_add && self.volume > 0 {
                    self.volume -= 1;
                }
            }
        }
    }

    /// Current digital output, 0..=15.
    pub fn output(&self) -> u8 {
        if self.enabled && self.dac_on {
            DUTY[self.duty as usize][self.duty_pos as usize] * self.volume
        } else {
            0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn loud() -> Pulse {
        let mut p = Pulse::default();
        p.write_nrx2(0xF0);
        p.set_freq(2047);
        p
    }

    #[test]
    fn e04_02_duty_waveforms_have_expected_high_steps() {
        let expected = [15u32, 30, 60, 90];
        for duty in 0..4u8 {
            let mut p = loud();
            p.duty = duty;
            p.trigger();
            let mut sum = 0u32;
            for _ in 0..8 {
                p.tick();
                sum += u32::from(p.output());
            }
            assert_eq!(sum, expected[duty as usize], "duty {duty}");
        }
    }

    #[test]
    fn e04_02_envelope_decrease_stops_at_zero() {
        let mut p = Pulse::default();
        p.write_nrx2(0x31);
        p.trigger();
        let mut seen = Vec::new();
        for _ in 0..4 {
            p.clock_envelope();
            seen.push(p.volume);
        }
        assert_eq!(seen, vec![2, 1, 0, 0]);
    }

    #[test]
    fn e04_02_envelope_increase_stops_at_fifteen() {
        let mut p = Pulse::default();
        p.write_nrx2(0xC9);
        p.trigger();
        let mut seen = Vec::new();
        for _ in 0..4 {
            p.clock_envelope();
            seen.push(p.volume);
        }
        assert_eq!(seen, vec![13, 14, 15, 15]);
    }

    #[test]
    fn e04_02_envelope_period_2_changes_every_second_clock() {
        let mut p = Pulse::default();
        p.write_nrx2(0x32);
        p.trigger();
        p.clock_envelope();
        assert_eq!(p.volume, 3);
        p.clock_envelope();
        assert_eq!(p.volume, 2);
    }

    #[test]
    fn e04_02_envelope_period_0_keeps_volume() {
        let mut p = Pulse::default();
        p.write_nrx2(0x30);
        p.trigger();
        for _ in 0..16 {
            p.clock_envelope();
        }
        assert_eq!(p.volume, 3);
    }

    #[test]
    fn e04_02_length_expiry_disables_channel() {
        let mut p = loud();
        p.write_nrx1(0x3F);
        p.length_enable = true;
        p.trigger();
        assert!(p.enabled);
        p.clock_length();
        assert!(!p.enabled);
    }

    #[test]
    fn e04_02_length_disabled_never_expires() {
        let mut p = loud();
        p.write_nrx1(0x3F);
        p.length_enable = false;
        p.trigger();
        p.clock_length();
        assert!(p.enabled);
    }

    #[test]
    fn e04_02_trigger_reloads_length_when_zero() {
        let mut p = loud();
        p.trigger();
        assert_eq!(p.length, 64);
    }

    #[test]
    fn e04_02_dac_off_blocks_trigger_and_disables() {
        let mut p = Pulse::default();
        p.write_nrx2(0x00);
        p.trigger();
        assert!(!p.enabled);
        p.write_nrx2(0xF0);
        p.trigger();
        assert!(p.enabled);
        p.write_nrx2(0x00);
        assert!(!p.enabled);
    }

    #[test]
    fn e04_02_sweep_increase_updates_frequency_then_overflows() {
        let mut p = loud();
        p.set_freq(800);
        p.write_nr10(0x11);
        p.trigger();
        assert!(p.enabled);
        p.clock_sweep();
        assert_eq!(p.freq, 1200);
        assert!(p.enabled);
        p.clock_sweep();
        assert_eq!(p.freq, 1800);
        assert!(!p.enabled);
    }

    #[test]
    fn e04_02_sweep_decrease_lowers_frequency() {
        let mut p = loud();
        p.set_freq(800);
        p.write_nr10(0x19);
        p.trigger();
        p.clock_sweep();
        assert_eq!(p.freq, 400);
        p.clock_sweep();
        assert_eq!(p.freq, 200);
        assert!(p.enabled);
    }

    #[test]
    fn e04_02_sweep_overflow_at_trigger_disables_channel() {
        let mut p = loud();
        p.set_freq(1500);
        p.write_nr10(0x11);
        p.trigger();
        assert!(!p.enabled);
    }

    #[test]
    fn e04_02_sweep_pace_0_keeps_frequency() {
        let mut p = loud();
        p.set_freq(800);
        p.write_nr10(0x01);
        p.trigger();
        for _ in 0..16 {
            p.clock_sweep();
        }
        assert_eq!(p.freq, 800);
        assert!(p.enabled);
    }

    #[test]
    fn e04_02_sweep_shift_0_only_checks_overflow() {
        let mut p = loud();
        p.set_freq(800);
        p.write_nr10(0x10);
        p.trigger();
        p.clock_sweep();
        assert_eq!(p.freq, 800);
        assert!(p.enabled);
        let mut q = loud();
        q.set_freq(1100);
        q.write_nr10(0x10);
        q.trigger();
        q.clock_sweep();
        assert!(!q.enabled);
    }

    #[test]
    fn e04_02_sweep_pace_2_updates_every_second_clock() {
        let mut p = loud();
        p.set_freq(800);
        p.write_nr10(0x21);
        p.trigger();
        p.clock_sweep();
        assert_eq!(p.freq, 800);
        p.clock_sweep();
        assert_eq!(p.freq, 1200);
    }

    #[test]
    fn e04_06_clearing_negate_after_a_negate_calculation_disables_channel() {
        let mut p = loud();
        p.set_freq(800);
        p.write_nr10(0x19);
        p.trigger();
        assert!(p.enabled);
        p.write_nr10(0x11);
        assert!(!p.enabled);
    }

    #[test]
    fn e04_06_clearing_negate_without_a_calculation_keeps_channel() {
        let mut p = loud();
        p.set_freq(800);
        p.write_nr10(0x18);
        p.trigger();
        p.write_nr10(0x10);
        assert!(p.enabled);
    }

    #[test]
    fn e04_06_sweep_clock_counts_as_a_negate_calculation() {
        let mut p = loud();
        p.set_freq(800);
        p.write_nr10(0x18);
        p.trigger();
        p.clock_sweep();
        assert!(p.enabled);
        p.write_nr10(0x10);
        assert!(!p.enabled);
    }

    #[test]
    fn e04_06_trigger_forgets_earlier_negate_calculations() {
        let mut p = loud();
        p.set_freq(800);
        p.write_nr10(0x19);
        p.trigger();
        p.write_nr10(0x11);
        assert!(!p.enabled);
        p.write_nr10(0x18);
        p.trigger();
        assert!(p.enabled);
        p.write_nr10(0x10);
        assert!(p.enabled);
    }

    #[test]
    fn e04_06_length_enable_in_skipping_phase_clocks_length() {
        let mut p = Pulse::default();
        p.write_nrx1(0x3E); // length = 2
        p.enabled = true;
        p.write_length_enable(true, true, false);
        assert_eq!(p.length, 1);
        assert!(p.enabled);
        assert!(p.length_enable);
    }

    #[test]
    fn e04_06_length_enable_extra_clock_to_zero_disables_channel() {
        let mut p = Pulse::default();
        p.write_nrx1(0x3F); // length = 1
        p.enabled = true;
        p.write_length_enable(true, true, false);
        assert_eq!(p.length, 0);
        assert!(!p.enabled);
    }

    #[test]
    fn e04_06_length_enable_extra_clock_to_zero_keeps_channel_when_triggering() {
        let mut p = Pulse::default();
        p.write_nrx1(0x3F); // length = 1
        p.enabled = true;
        p.write_length_enable(true, true, true);
        assert_eq!(p.length, 0);
        assert!(p.enabled);
    }

    #[test]
    fn e04_06_length_enable_without_skipping_phase_does_not_clock() {
        let mut p = Pulse::default();
        p.write_nrx1(0x3E); // length = 2
        p.write_length_enable(true, false, false);
        assert_eq!(p.length, 2);
    }

    #[test]
    fn e04_06_length_enable_already_set_does_not_clock() {
        let mut p = Pulse::default();
        p.write_nrx1(0x3E); // length = 2
        p.length_enable = true;
        p.write_length_enable(true, true, false);
        assert_eq!(p.length, 2);
    }

    #[test]
    fn e04_06_trigger_reload_is_63_in_skipping_phase_with_length_enabled() {
        let mut p = Pulse::default();
        p.write_nrx2(0xF3); // DAC on
        p.length_enable = true;
        p.trigger_with(true);
        assert_eq!(p.length, 63);
        assert!(p.enabled);
    }

    #[test]
    fn e04_06_trigger_reload_is_64_otherwise() {
        let mut p1 = Pulse::default();
        p1.write_nrx2(0xF3); // DAC on
        p1.length_enable = true;
        p1.trigger_with(false);
        assert_eq!(p1.length, 64);
        let mut p2 = Pulse::default();
        p2.write_nrx2(0xF3); // DAC on, length enable off
        p2.trigger_with(true);
        assert_eq!(p2.length, 64);
    }
}
