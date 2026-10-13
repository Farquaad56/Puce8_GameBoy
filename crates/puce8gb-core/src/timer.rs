//! Timer and divider registers of the DMG: DIV ($FF04), TIMA ($FF05), TMA ($FF06) and
//! TAC ($FF07). The internal system counter is counted in dots; DIV is its upper byte
//! (note 01b, decision A_01: one tick = one dot).

/// Dots per M-cycle (decision A_01): the system counter advances by this much each M-cycle.
const DOTS_PER_MCYCLE: u16 = 4;

/// Bit of the system counter selected for each TAC[1:0] clock select value, giving TIMA
/// periods of 256, 4, 16 and 64 M-cycles (note 01b "FF07 TAC bits 1-0").
const SELECTED_BIT: [u32; 4] = [9, 3, 5, 7];

/// Timer and divider registers (note 01b).
#[derive(Debug, Default)]
pub struct Timer {
    /// Internal system counter in dots (note 01b "System counter"); DIV is its upper byte.
    sysclk: u16,
    /// Value of the TIMA input signal at the end of the previous M-cycle; a falling edge
    /// (1 -> 0) increments TIMA (note 01b).
    prev_signal: bool,
    /// TIMA ($FF05): timer counter.
    tima: u8,
    /// TMA ($FF06): value reloaded into TIMA when it overflows by increment (note 01b).
    tma: u8,
    /// TAC ($FF07): bit 2 = timer enable, bits 1-0 = clock select; the unused bits are
    /// preserved as written and read back unchanged.
    tac: u8,
}

impl Timer {
    /// Post-boot state (note 08): DIV=$AB, TIMA=$00, TMA=$00, TAC=$F8 (timer active).
    pub fn reset(&mut self) {
        // The system counter sits at a value whose upper byte reads $AB; its lower byte is
        // not documented (note 01b "Largeur du system counter interne": UNKNOWN), left at $00.
        self.sysclk = 0xAB00;
        self.tima = 0x00;
        self.tma = 0x00;
        self.tac = 0xF8; // timer active, clock select 11 (note 08)
        self.prev_signal = self.signal();
    }

    /// DIV ($FF04): the upper byte of the system counter (note 01b "FF04 DIV").
    pub fn div(&self) -> u8 {
        (self.sysclk >> 8) as u8
    }

    /// Writing any value to DIV resets the system counter; the written value is ignored
    /// (note 01b "FF04 DIV").
    pub fn write_div(&mut self, _val: u8) {
        // The written value is ignored (note 01b); the falling-edge detector follows the
        // reset counter so no spurious tick fires on the next M-cycle.
        self.sysclk = 0;
        self.prev_signal = self.signal();
    }

    /// TIMA ($FF05): timer counter.
    pub fn tima(&self) -> u8 {
        self.tima
    }

    /// Write TIMA ($FF05): a plain store; only the hardware increment can overflow it and
    /// request an interrupt (note 01b "FF05 TIMA").
    pub fn write_tima(&mut self, val: u8) {
        self.tima = val;
    }

    /// TMA ($FF06): reload value.
    pub fn tma(&self) -> u8 {
        self.tma
    }

    /// Write TMA ($FF06).
    pub fn write_tma(&mut self, val: u8) {
        self.tma = val;
    }

    /// TAC ($FF07): the full byte is preserved and read back unchanged (note 08: $F8 at boot).
    pub fn tac(&self) -> u8 {
        self.tac
    }

    /// Write TAC ($FF07); only bit 2 (enable) and bits 1-0 (clock select) affect the timer
    /// (note 01b "FF07 TAC").
    pub fn write_tac(&mut self, val: u8) {
        self.tac = val;
        // The selected bit (or the enable) changed: keep the falling-edge detector in sync.
        self.prev_signal = self.signal();
    }

    /// Advance one M-cycle (decision A_01): the system counter gains 4 dots and a falling
    /// edge of the selected bit increments TIMA. Returns true when that increment overflows
    /// TIMA, so the caller can request the timer interrupt (IF bit 2, note 01b).
    pub fn mcycle(&mut self) -> bool {
        self.sysclk = self.sysclk.wrapping_add(DOTS_PER_MCYCLE);
        let signal = self.signal();
        let overflowed;
        if self.prev_signal && !signal {
            // Falling edge of the selected bit: TIMA increments (note 01b). On overflow it is
            // reloaded with the current TMA value and an interrupt is requested.
            overflowed = self.tima == 0xFF;
            self.tima = if overflowed {
                self.tma
            } else {
                self.tima.wrapping_add(1)
            };
        } else {
            overflowed = false;
        }
        self.prev_signal = signal;
        overflowed
    }

    /// TIMA input signal: the timer enable (TAC bit 2) AND the selected bit of the system
    /// counter. DIV is never gated by the enable bit (note 01b "FF07 TAC bit 2").
    fn signal(&self) -> bool {
        if self.tac & 0x04 == 0 {
            return false; // timer disabled: TIMA does not increment
        }
        let bit = SELECTED_BIT[(self.tac & 0x03) as usize];
        (self.sysclk >> bit) & 1 == 1
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Post-boot timer (note 08): DIV=$AB, TIMA/TMA=$00, TAC=$F8.
    fn post_boot() -> Timer {
        let mut t = Timer::default();
        t.reset();
        t
    }

    #[test]
    fn c01_34_post_boot_registers_match_note_08() {
        let t = post_boot();
        assert_eq!(t.div(), 0xAB); // DIV (note 08)
        assert_eq!(t.tima(), 0x00); // TIMA (note 08)
        assert_eq!(t.tma(), 0x00); // TMA (note 08)
        assert_eq!(t.tac(), 0xF8); // TAC: timer active, clock select 11 (note 08)
    }

    #[test]
    fn c01_34_div_increments_at_documented_rate() {
        let mut t = post_boot();
        t.write_div(0x5A); // zero the counter so the rate is measured from $00
        assert_eq!(t.div(), 0x00);
        for _ in 0..63 {
            t.mcycle();
        }
        assert_eq!(t.div(), 0x00); // DIV changes every 256 dots = 64 M-cycles (note 01b)
        t.mcycle(); // the 64th M-cycle: the counter crosses $0100
        assert_eq!(t.div(), 0x01);
    }

    #[test]
    fn c01_34_div_write_resets_counter_and_ignores_value() {
        let mut t = post_boot();
        for _ in 0..100 {
            t.mcycle();
        }
        assert_ne!(t.div(), 0x00); // the counter has moved on from $AB
        t.write_div(0x5A);
        assert_eq!(t.div(), 0x00); // any value resets it to $00; the value is ignored (note 01b)
        for _ in 0..64 {
            t.mcycle();
        }
        assert_eq!(t.div(), 0x01); // counting resumes from zero
    }

    #[test]
    fn c01_34_tima_rate_for_each_tac_value() {
        for (select, period_mcycles) in [(0u8, 256u32), (1, 4), (2, 16), (3, 64)] {
            let mut t = post_boot();
            t.write_div(0x00); // zero the counter: every selected bit starts at 0
            t.write_tac(0x04 | select); // timer enabled + clock select (note 01b)
            for _ in 0..3 * period_mcycles {
                assert!(!t.mcycle()); // TMA=$00 and three periods: no overflow yet
            }
            assert_eq!(
                t.tima(),
                3,
                "TAC select {} must tick every {} M-cycles",
                select,
                period_mcycles
            );
        }
    }

    #[test]
    fn c01_34_disabled_timer_does_not_count() {
        let mut t = post_boot();
        t.write_div(0x00);
        t.write_tac(0x05 & !0x04); // clock select 01, enable clear (note 01b)
        for _ in 0..256 * 4 {
            t.mcycle();
        }
        assert_eq!(t.tima(), 0x00); // TIMA never increments while disabled
        assert_ne!(t.div(), 0x00); // DIV keeps counting regardless of the enable bit (note 01b)
    }

    #[test]
    fn c01_34_tima_overflow_reloads_tma_and_requests_interrupt() {
        let mut t = post_boot();
        t.write_div(0x00);
        t.write_tac(0x05); // one increment every 4 M-cycles (note 01b)
        t.write_tima(0xFF);
        t.write_tma(0x7E);
        for _ in 0..3 {
            assert!(!t.mcycle()); // the first falling edge is at the end of M-cycle 4
        }
        assert!(t.mcycle()); // that increment overflows TIMA (note 01b)
        assert_eq!(t.tima(), 0x7E); // reloaded with the current TMA value
    }

    #[test]
    fn c01_34_tac_write_preserves_unused_bits() {
        let mut t = post_boot();
        assert_eq!(t.tac(), 0xF8); // $F8 at power-up (note 08)
        t.write_tac(0x35);
        assert_eq!(t.tac(), 0x35); // the full byte is preserved and read back unchanged
    }
}
