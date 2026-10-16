//! Noise channel core, channel 4 (E04_04): LFSR (15-bit and 7-bit width modes), clock divisor, length, envelope, trigger. Not wired to the registers yet.

/// Base divisors in M-cycles, indexed by NR43 bits 2-0 (8, 16, 32, ... 112 dots; divisor 0 is 0.5 units).
const BASE_PERIOD: [u32; 8] = [2, 4, 8, 12, 16, 20, 24, 28];

/// State of the noise channel. `tick` is called once per M-cycle.
#[derive(Debug, Default)]
pub struct Noise {
    pub enabled: bool,
    pub dac_on: bool,
    pub length: u8,
    pub length_enable: bool,
    pub volume: u8,
    pub lfsr: u16,
    pub width_short: bool,
    pub shift: u8,
    pub divider: u8,
    timer: u32,
    env_initial: u8,
    env_add: bool,
    env_period: u8,
    env_timer: u8,
}

impl Noise {
    /// NR41: length load in bits 5-0 (length = 64 - value).
    pub fn write_nr41(&mut self, v: u8) {
        self.length = 64 - (v & 0x3F);
    }

    /// NR42 (same layout as NR12): initial volume bits 7-4, direction bit 3, period bits 2-0.
    /// The DAC is on when bits 7-3 are not all zero; turning it off disables the channel.
    pub fn write_nr42(&mut self, v: u8) {
        self.env_initial = v >> 4;
        self.env_add = v & 0x08 != 0;
        self.env_period = v & 0x07;
        self.dac_on = v & 0xF8 != 0;
        if !self.dac_on {
            self.enabled = false;
        }
    }

    /// NR43: clock shift bits 7-4, width bit 3 (1 = 7-bit), divider bits 2-0.
    pub fn write_nr43(&mut self, v: u8) {
        self.shift = v >> 4;
        self.width_short = v & 0x08 != 0;
        self.divider = v & 0x07;
    }

    /// LFSR clock period in M-cycles; None when the clock is stopped (shift 14 or 15).
    pub fn period(&self) -> Option<u32> {
        if self.shift >= 14 {
            return None;
        }
        Some(BASE_PERIOD[usize::from(self.divider)] << self.shift)
    }

    /// Trigger (NR44 bit 7): the LFSR is reset to zero.
    pub fn trigger(&mut self) {
        self.enabled = self.dac_on;
        if self.length == 0 {
            self.length = 64;
        }
        self.lfsr = 0;
        self.timer = self.period().unwrap_or(0);
        self.env_timer = if self.env_period == 0 {
            8
        } else {
            self.env_period
        };
        self.volume = self.env_initial;
    }

    /// One LFSR step: XNOR of bits 0 and 1, shift right, the result goes into bit 15 (and bit 7 in
    /// the 7-bit mode). Exact feedback bit positions (15/7 as in the note): UNKNOWN - to confirm.
    /// The lock when the 7 low bits are all ones follows from this rule, no special case.
    fn step_lfsr(&mut self) {
        let fb: u16 = if (self.lfsr ^ (self.lfsr >> 1)) & 1 == 0 {
            1
        } else {
            0
        };
        self.lfsr >>= 1;
        self.lfsr = (self.lfsr & 0x7FFF) | (fb << 15);
        if self.width_short {
            self.lfsr = (self.lfsr & !0x0080) | (fb << 7);
        }
    }

    /// One M-cycle of the LFSR clock. Nothing happens while the clock is stopped.
    pub fn tick(&mut self) {
        if let Some(p) = self.period() {
            self.timer = self.timer.saturating_sub(1);
            if self.timer == 0 {
                self.timer = p;
                self.step_lfsr();
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

    /// Current digital output: the envelope volume when LFSR bit 0 is 1, else 0.
    pub fn output(&self) -> u8 {
        if self.enabled && self.dac_on && self.lfsr & 1 == 1 {
            self.volume
        } else {
            0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn loud() -> Noise {
        let mut n = Noise::default();
        n.write_nr42(0xF0);
        n
    }

    #[test]
    fn e04_04_lfsr_15_bit_first_outputs() {
        let mut n = loud();
        n.write_nr43(0x00);
        n.trigger();
        let mut seen = Vec::new();
        for _ in 0..31 {
            n.step_lfsr();
            seen.push(u8::from(n.lfsr & 1 == 1));
        }
        let mut expected = vec![0u8; 15];
        expected.extend([1u8; 15]);
        expected.push(0);
        assert_eq!(seen, expected);
    }

    #[test]
    fn e04_04_lfsr_7_bit_first_outputs() {
        let mut n = loud();
        n.write_nr43(0x08);
        n.trigger();
        let mut seen = Vec::new();
        for _ in 0..15 {
            n.step_lfsr();
            seen.push(u8::from(n.lfsr & 1 == 1));
        }
        let mut expected = vec![0u8; 7];
        expected.extend([1u8; 7]);
        expected.push(0);
        assert_eq!(seen, expected);
    }

    #[test]
    fn e04_04_trigger_resets_the_lfsr() {
        let mut n = loud();
        n.lfsr = 0x1234;
        n.trigger();
        assert_eq!(n.lfsr, 0);
    }

    #[test]
    fn e04_04_divisor_table_and_shift() {
        let cases: [(u8, u8, u32); 11] = [
            (0, 0, 2),
            (1, 0, 4),
            (2, 0, 8),
            (3, 0, 12),
            (4, 0, 16),
            (5, 0, 20),
            (6, 0, 24),
            (7, 0, 28),
            (0, 1, 4),
            (3, 2, 48),
            (7, 13, 229_376),
        ];
        for (div, shift, expected) in cases {
            let mut n = Noise::default();
            n.write_nr43((shift << 4) | div);
            assert_eq!(n.period(), Some(expected), "div {div} shift {shift}");
        }
        let mut n = Noise::default();
        n.write_nr43(0xE0);
        assert_eq!(n.period(), None);
        n.write_nr43(0xF0);
        assert_eq!(n.period(), None);
    }

    #[test]
    fn e04_04_tick_steps_the_lfsr_every_period() {
        let mut n = loud();
        n.write_nr43(0x00);
        n.trigger();
        n.tick();
        assert_eq!(n.lfsr, 0);
        n.tick();
        assert_eq!(n.lfsr, 0x8000);
    }

    #[test]
    fn e04_04_stopped_clock_never_steps_the_lfsr() {
        let mut n = loud();
        n.write_nr43(0xE0);
        n.trigger();
        for _ in 0..1000 {
            n.tick();
        }
        assert_eq!(n.lfsr, 0);
    }

    #[test]
    fn e04_04_output_follows_lfsr_bit_0_and_volume() {
        let mut n = loud();
        n.trigger();
        n.lfsr = 1;
        assert_eq!(n.output(), 15);
        n.lfsr = 0;
        assert_eq!(n.output(), 0);
    }

    #[test]
    fn e04_04_length_expiry_disables_channel() {
        let mut n = loud();
        n.write_nr41(0x3F);
        n.length_enable = true;
        n.trigger();
        assert!(n.enabled);
        n.clock_length();
        assert!(!n.enabled);
    }

    #[test]
    fn e04_04_envelope_decrease_and_dac_off() {
        let mut n = Noise::default();
        n.write_nr42(0x31);
        n.trigger();
        n.clock_envelope();
        assert_eq!(n.volume, 2);
        n.write_nr42(0x00);
        assert!(!n.enabled);
        n.trigger();
        assert!(!n.enabled);
    }
}
