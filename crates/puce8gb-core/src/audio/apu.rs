//! APU frame sequencer (E04_01): clocked by the falling edge of DIV bit 4 at 512 Hz,
//! running one step per edge over an 8-step cycle (note 05b).

use crate::audio::pulse::Pulse;
use crate::audio::wave::Wave;

/// Length counters clocked: steps 0, 2, 4, 6 (256 Hz).
pub const EV_LENGTH: u8 = 0x01;
/// CH1 sweep clocked: steps 2, 6 (128 Hz).
pub const EV_SWEEP: u8 = 0x02;
/// Envelopes clocked: step 7 (64 Hz).
pub const EV_ENVELOPE: u8 = 0x04;

// Step assignment follows the Pan Docs frame sequencer table; which step runs at the first edge after power-on: UNKNOWN - to confirm (we start at step 0).

/// Bit 4 of DIV: its falling edge clocks the frame sequencer at 512 Hz (note 05b).
const DIV_APU_BIT: u8 = 0x10;

/// Frame sequencer state (E04_01): counts whatever the APU power state is, because the
/// DIV-APU counter is not affected by power off (note 05b).
#[derive(Debug, Default)]
pub struct Apu {
    /// Index 0..=7 of the NEXT step to run.
    pub step: u8,
    /// Bit 4 of DIV as seen on the previous tick; a falling edge runs one step.
    prev_div_bit: bool,
    pub pulse1: Pulse,
    pub pulse2: Pulse,
    pub wave: Wave,
}

impl Apu {
    /// Synchronize with the current DIV value at power-up (E04_01).
    pub fn reset(&mut self, div: u8) {
        self.step = 0;
        self.prev_div_bit = div & DIV_APU_BIT != 0;
        self.pulse1 = Pulse::post_boot();
        self.pulse2 = Pulse::default();
        self.wave = Wave::default();
    }

    /// Call once per M-cycle, after the timer, with the current DIV value; returns the EV_* bits of the step that just ran, or 0.
    fn step_sequencer(&mut self, div: u8) -> u8 {
        let bit = div & DIV_APU_BIT != 0;
        let mut events: u8 = 0;
        if self.prev_div_bit && !bit {
            events = match self.step {
                0 | 4 => EV_LENGTH,
                2 | 6 => EV_LENGTH | EV_SWEEP,
                7 => EV_ENVELOPE,
                _ => 0,
            };
            self.step = (self.step + 1) % 8;
        }
        self.prev_div_bit = bit;
        events
    }

    /// One M-cycle: advance the frame sequencer, dispatch its events, clock the channel timers.
    pub fn tick(&mut self, div: u8) -> u8 {
        let events = self.step_sequencer(div);
        if events & EV_LENGTH != 0 {
            self.pulse1.clock_length();
            self.pulse2.clock_length();
            self.wave.clock_length();
        }
        if events & EV_SWEEP != 0 {
            self.pulse1.clock_sweep();
        }
        if events & EV_ENVELOPE != 0 {
            self.pulse1.clock_envelope();
            self.pulse2.clock_envelope();
        }
        self.pulse1.tick();
        self.pulse2.tick();
        if self.wave.enabled {
            self.wave.tick();
        }
        events
    }

    /// Bits 0-2 of NR52: channel 1, 2 and 3 enabled flags.
    pub fn status(&self) -> u8 {
        u8::from(self.pulse1.enabled)
            | (u8::from(self.pulse2.enabled) << 1)
            | (u8::from(self.wave.enabled) << 2)
    }

    fn write_nrx3(p: &mut Pulse, v: u8) {
        let f = (p.freq & 0x0700) | u16::from(v);
        p.set_freq(f);
    }

    fn write_nrx4(p: &mut Pulse, v: u8) {
        let f = (p.freq & 0x00FF) | (u16::from(v & 0x07) << 8);
        p.set_freq(f);
        p.length_enable = v & 0x40 != 0;
        if v & 0x80 != 0 {
            p.trigger();
        }
    }

    /// Accepted register write (NR10-NR25 only while powered; NR52 always). Behaviour of length
    /// registers while powered off and of NRx2 writes while a channel plays: UNKNOWN - to confirm.
    /// Wave RAM access while channel 3 plays: UNKNOWN - to confirm (note 03b).
    pub fn write_reg(&mut self, addr: u16, v: u8) {
        match addr {
            0xFF10 => self.pulse1.write_nr10(v),
            0xFF11 => self.pulse1.write_nrx1(v),
            0xFF12 => self.pulse1.write_nrx2(v),
            0xFF13 => Self::write_nrx3(&mut self.pulse1, v),
            0xFF14 => Self::write_nrx4(&mut self.pulse1, v),
            0xFF16 => self.pulse2.write_nrx1(v),
            0xFF17 => self.pulse2.write_nrx2(v),
            0xFF18 => Self::write_nrx3(&mut self.pulse2, v),
            0xFF19 => Self::write_nrx4(&mut self.pulse2, v),
            0xFF1A => self.wave.write_nr30(v),
            0xFF1B => self.wave.write_nr31(v),
            0xFF1C => self.wave.write_nr32(v),
            0xFF1D => {
                let f = (self.wave.freq & 0x0700) | u16::from(v);
                self.wave.set_freq(f);
            }
            0xFF1E => {
                let f = (self.wave.freq & 0x00FF) | (u16::from(v & 0x07) << 8);
                self.wave.set_freq(f);
                self.wave.length_enable = v & 0x40 != 0;
                if v & 0x80 != 0 {
                    self.wave.trigger();
                }
            }
            0xFF26 if v & 0x80 == 0 => {
                self.pulse1 = Pulse::default();
                self.pulse2 = Pulse::default();
                self.wave = Wave::default();
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::timer::Timer;

    #[test]
    fn e04_01_sequencer_step_order_over_8_steps() {
        let mut apu = Apu::default();
        apu.reset(0x00);
        let mut got = [0u8; 8];
        for slot in got.iter_mut() {
            apu.tick(0x10); // rising edge: no step runs
            *slot = apu.tick(0x00); // falling edge: the next step runs
        }
        assert_eq!(got, [0x01, 0x00, 0x03, 0x00, 0x01, 0x00, 0x03, 0x04]);
        assert_eq!(apu.step, 0); // the cycle wrapped back to step 0
        apu.tick(0x10);
        assert_eq!(apu.tick(0x00), EV_LENGTH); // wraps to step 0 again (E04_01 spec)
    }

    #[test]
    fn e04_01_no_event_without_a_falling_edge() {
        let mut apu = Apu::default();
        apu.reset(0x00);
        assert_eq!(apu.tick(0x00), 0); // bit 4 stays low: no edge (E04_01 spec)
        assert_eq!(apu.tick(0x10), 0); // rising edge only: nothing runs (E04_01 spec)
        assert_eq!(apu.tick(0x1F), 0); // bit 4 stays high: no falling edge (E04_01 spec)
        assert_eq!(apu.step, 0);
        assert_eq!(apu.tick(0x20), EV_LENGTH); // bit 4 falls: step 0 runs (E04_01 spec)
        assert_eq!(apu.step, 1);
    }

    #[test]
    fn e04_01_sequencer_runs_every_2048_m_cycles_from_div() {
        let mut t = Timer::default();
        t.write_div(0); // DIV starts at $00 (note 01b)
        let mut apu = Apu::default();
        apu.reset(t.div());
        let mut edges: Vec<(u32, u8)> = Vec::new();
        for m in 1..=16384u32 {
            t.mcycle(); // one M-cycle (decision A_01)
            let before = apu.step;
            let ev = apu.tick(t.div());
            if apu.step != before {
                edges.push((m, ev));
            }
        }
        assert_eq!(
            edges,
            vec![
                (2048, 0x01),  // step 0: length counters (E04_01 spec)
                (4096, 0x00),  // step 1: nothing
                (6144, 0x03),  // step 2: length + sweep
                (8192, 0x00),  // step 3: nothing
                (10240, 0x01), // step 4: length counters
                (12288, 0x00), // step 5: nothing
                (14336, 0x03), // step 6: length + sweep
                (16384, 0x04), // step 7: envelopes; DIV wraps $FF -> $00 here too
            ]
        );
    }

    #[test]
    fn e04_01_div_write_while_bit4_is_set_clocks_the_sequencer() {
        let mut t = Timer::default();
        t.write_div(0); // DIV starts at $00 (note 01b)
        let mut apu = Apu::default();
        apu.reset(t.div());
        for _ in 0..1024 {
            t.mcycle();
            assert_eq!(apu.tick(t.div()), 0); // no falling edge yet: rising at $10 only (E04_01 spec)
        }
        assert_eq!(t.div(), 0x10); // DIV bit 4 is set after 256 increments of 4 dots each
        t.write_div(0); // a write resets the counter: bit 4 falls on this M-cycle (note 01b)
        assert_eq!(apu.tick(t.div()), EV_LENGTH); // the falling edge runs step 0 (E04_01 spec)
        assert_eq!(apu.step, 1);
    }

    #[test]
    fn e04_02_write_reg_trigger_enables_channel_2() {
        let mut apu = Apu::default();
        apu.write_reg(0xFF17, 0xF0);
        apu.write_reg(0xFF18, 0xFF);
        apu.write_reg(0xFF19, 0x87);
        assert_eq!(apu.pulse2.freq, 2047);
        assert!(apu.pulse2.enabled);
        assert_eq!(apu.status(), 0x02);
    }

    #[test]
    fn e04_02_write_reg_length_enable_and_expiry() {
        let mut apu = Apu::default();
        apu.write_reg(0xFF12, 0xF0);
        apu.write_reg(0xFF11, 0x3F);
        apu.write_reg(0xFF14, 0xC0);
        assert_eq!(apu.status(), 0x01);
        apu.pulse1.clock_length();
        assert_eq!(apu.status(), 0x00);
    }

    #[test]
    fn e04_02_power_off_clears_channels() {
        let mut apu = Apu::default();
        apu.write_reg(0xFF12, 0xF0);
        apu.write_reg(0xFF14, 0x80);
        assert_eq!(apu.status(), 0x01);
        apu.write_reg(0xFF26, 0x00);
        assert_eq!(apu.status(), 0x00);
        assert!(!apu.pulse1.dac_on);
    }

    #[test]
    fn e04_02_tick_advances_duty_position_every_period() {
        let mut apu = Apu::default();
        apu.write_reg(0xFF17, 0xF0);
        apu.write_reg(0xFF18, 0xFF);
        apu.write_reg(0xFF19, 0x87);
        apu.tick(0x00);
        assert_eq!(apu.pulse2.duty_pos, 1);
        apu.tick(0x00);
        assert_eq!(apu.pulse2.duty_pos, 2);
    }

    #[test]
    fn e04_02_sequencer_step_7_clocks_the_envelope() {
        let mut apu = Apu::default();
        apu.write_reg(0xFF17, 0xF1);
        apu.write_reg(0xFF19, 0x80);
        for _ in 0..7 {
            apu.tick(0x10);
            apu.tick(0x00);
        }
        assert_eq!(apu.pulse2.volume, 15);
        apu.tick(0x10);
        apu.tick(0x00);
        assert_eq!(apu.pulse2.volume, 14);
    }

    #[test]
    fn e04_02_sequencer_step_2_clocks_the_sweep() {
        let mut apu = Apu::default();
        apu.write_reg(0xFF10, 0x11);
        apu.write_reg(0xFF12, 0xF0);
        apu.write_reg(0xFF13, 0x20);
        apu.write_reg(0xFF14, 0x83);
        for _ in 0..2 {
            apu.tick(0x10);
            apu.tick(0x00);
        }
        assert_eq!(apu.pulse1.freq, 800);
        apu.tick(0x10);
        apu.tick(0x00);
        assert_eq!(apu.pulse1.freq, 1200);
    }

    #[test]
    fn e04_03_write_reg_trigger_enables_channel_3() {
        let mut apu = Apu::default();
        apu.write_reg(0xFF1A, 0x80);
        apu.write_reg(0xFF1C, 0x20);
        apu.write_reg(0xFF1D, 0xFF);
        apu.write_reg(0xFF1E, 0x87);
        assert_eq!(apu.wave.freq, 2047);
        assert!(apu.wave.enabled);
        assert_eq!(apu.status(), 0x04);
    }

    #[test]
    fn e04_03_sequencer_length_event_expires_channel_3() {
        let mut apu = Apu::default();
        apu.write_reg(0xFF1A, 0x80);
        apu.write_reg(0xFF1B, 0xFF);
        apu.write_reg(0xFF1E, 0xC0);
        assert_eq!(apu.status(), 0x04);
        apu.tick(0x10);
        apu.tick(0x00);
        assert_eq!(apu.status(), 0x00);
    }

    #[test]
    fn e04_03_power_off_clears_channel_3() {
        let mut apu = Apu::default();
        apu.write_reg(0xFF1A, 0x80);
        apu.write_reg(0xFF1E, 0x80);
        assert_eq!(apu.status(), 0x04);
        apu.write_reg(0xFF26, 0x00);
        assert_eq!(apu.status(), 0x00);
        assert!(!apu.wave.dac_on);
    }

    #[test]
    fn e04_03_tick_reads_samples_only_while_enabled() {
        let mut apu = Apu::default();
        apu.wave.ram[0] = 0x12;
        apu.wave.ram[1] = 0x34;
        apu.tick(0x00);
        assert_eq!(apu.wave.pos, 0);
        apu.write_reg(0xFF1A, 0x80);
        apu.write_reg(0xFF1C, 0x20);
        apu.write_reg(0xFF1D, 0xFF);
        apu.write_reg(0xFF1E, 0x87);
        apu.tick(0x00);
        assert_eq!(apu.wave.pos, 2);
        assert_eq!(apu.wave.sample, 3);
    }
}
