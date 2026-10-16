//! APU frame sequencer (E04_01): clocked by the falling edge of DIV bit 4 at 512 Hz,
//! running one step per edge over an 8-step cycle (note 05b).

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
}

impl Apu {
    /// Synchronize with the current DIV value at power-up (E04_01).
    pub fn reset(&mut self, div: u8) {
        self.step = 0;
        self.prev_div_bit = div & DIV_APU_BIT != 0;
    }

    /// Call once per M-cycle, after the timer, with the current DIV value; returns the EV_* bits of the step that just ran, or 0.
    pub fn tick(&mut self, div: u8) -> u8 {
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
}
