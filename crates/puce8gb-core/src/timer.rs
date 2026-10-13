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
    /// An overflow by increment happened at the end of the previous M-cycle; the reload of
    /// TIMA with TMA and the request of IF bit 2 land one M-cycle later (note 01b "Retard
    /// d'un M-cycle"). False at power-on.
    reload_pending: bool,
    /// The reload window is open during this M-cycle: a CPU write to TIMA is ignored and a
    /// CPU write to TMA copies its value into TIMA on the same M-cycle (note 01b points 2-3).
    /// False at power-on.
    reloading: bool,
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
        self.reload_pending = false; // no pending reload at power-on (note 01b)
        self.reloading = false;
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
    /// request an interrupt (note 01b "FF05 TIMA"). A CPU write landing on the M-cycle of
    /// the overflow cancels the reload and the IF bit 2 request (note 01b point 1); a CPU
    /// write during the reload window is ignored because TIMA ends equal to TMA anyway
    /// (note 01b point 2).
    pub fn write_tima(&mut self, val: u8) {
        if self.reloading {
            return; // ignored: TIMA is reloaded with TMA at the end of this M-cycle
        }
        if self.reload_pending {
            self.reload_pending = false; // the overflow acts as if it did not happen
        }
        self.tima = val;
    }

    /// TMA ($FF06): reload value.
    pub fn tma(&self) -> u8 {
        self.tma
    }

    /// Write TMA ($FF06). During the reload window, the new value is copied into TIMA on
    /// this same M-cycle (note 01b point 3).
    pub fn write_tma(&mut self, val: u8) {
        self.tma = val;
        if self.reloading {
            self.tima = val; // the new TMA value is copied into TIMA on this M-cycle
        }
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
    /// edge of the selected bit increments TIMA. The reload of TMA into TIMA and the request
    /// of IF bit 2 land one M-cycle after the increment that overflows (note 01b "Retard
    /// d'un M-cycle"): this tick returns true when that delayed request lands, so the caller
    /// sets IF bit 2. The CPU's bus access for this M-cycle happens before this tick and
    /// sees the timer state as it was at the end of the previous M-cycle (decision C_00).
    pub fn mcycle(&mut self) -> bool {
        // a) Close the reload window opened by the previous tick.
        self.reloading = false;
        // b) Land the delayed reload and interrupt request from an overflow one M-cycle ago:
        // TIMA takes TMA, IF bit 2 is requested, and the reload window opens for this tick.
        let mut request_interrupt = false;
        if self.reload_pending {
            self.reload_pending = false;
            self.tima = self.tma;
            request_interrupt = true;
            self.reloading = true;
        }
        // c) Advance the system counter; a falling edge of the selected bit increments TIMA.
        self.sysclk = self.sysclk.wrapping_add(DOTS_PER_MCYCLE);
        let signal = self.signal();
        if self.prev_signal && !signal {
            // Falling edge: TIMA increments (note 01b). On overflow it wraps to $00 and the
            // reload plus interrupt request are scheduled for one M-cycle later.
            let (value, overflow) = self.tima.overflowing_add(1);
            self.tima = value;
            if overflow {
                self.reload_pending = true;
            }
        }
        self.prev_signal = signal;
        request_interrupt
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
        assert!(!t.mcycle()); // that increment overflows TIMA: it wraps to $00 (note 01b)
        assert_eq!(t.tima(), 0x00); // the reload is not visible yet on this M-cycle
        assert!(t.mcycle()); // one M-cycle later: the delayed request lands (note 01b)
        assert_eq!(t.tima(), 0x7E); // reloaded with the current TMA value
    }

    #[test]
    fn c01_34_tac_write_preserves_unused_bits() {
        let mut t = post_boot();
        assert_eq!(t.tac(), 0xF8); // $F8 at power-up (note 08)
        t.write_tac(0x35);
        assert_eq!(t.tac(), 0x35); // the full byte is preserved and read back unchanged
    }

    /// Timer armed for an overflow: TAC = $05 (one increment every 4 M-cycles), TIMA = $FF,
    /// TMA = $23. The first falling edge lands at the end of M-cycle 4.
    fn armed_overflow() -> Timer {
        let mut t = post_boot();
        t.write_div(0x00); // zero the system counter: every selected bit starts at 0
        t.write_tac(0x05); // timer enabled, clock select 01 (note 01b)
        t.write_tima(0xFF); // the next increment overflows
        t.write_tma(0x23); // reload value
        t
    }

    #[test]
    fn c01_35_overflow_delay_reload_and_if_bit_lag_one_m_cycle() {
        let mut t = armed_overflow();
        for _ in 0..3 {
            assert!(!t.mcycle()); // no falling edge yet (first one at the end of M-cycle 4)
        }
        assert!(!t.mcycle()); // M-cycle N: the increment overflows; TIMA wraps to $00, IF bit 2 still clear
        assert_eq!(t.tima(), 0x00); // the reload is not visible yet (note 01b "Retard d'un M-cycle")
        assert!(t.mcycle()); // M-cycle N+1: the delayed request lands at its end; TIMA = TMA, IF bit 2 set
        assert_eq!(t.tima(), 0x23); // reloaded with the current TMA value (note 01b)
    }

    #[test]
    fn c01_35_write_tima_during_overflow_m_cycle_cancels_reload_and_request() {
        let mut t = armed_overflow();
        for _ in 0..4 {
            assert!(!t.mcycle()); // M-cycle N: the increment overflows at its end
        }
        t.write_tima(0x55); // a CPU write on M-cycle N+1 cancels the reload and the request (note 01b point 1)
        assert!(!t.mcycle()); // M-cycle N+1: no delayed request lands
        assert_eq!(t.tima(), 0x55); // the written value stays; TMA is not copied into TIMA
    }

    #[test]
    fn c01_35_write_tma_during_reload_m_cycle_copies_into_tima() {
        let mut t = armed_overflow();
        for _ in 0..4 {
            assert!(!t.mcycle()); // M-cycle N: the increment overflows at its end
        }
        assert!(t.mcycle()); // M-cycle N+1: the delayed request lands; TIMA = TMA, reload window open
        t.write_tma(0x77); // a CPU write to TMA on M-cycle N+2 copies the new value into TIMA (note 01b point 3)
        assert_eq!(t.tma(), 0x77);
        assert_eq!(t.tima(), 0x77); // TIMA takes the new TMA value on this same M-cycle
    }

    #[test]
    fn c01_35_write_tima_during_reload_m_cycle_is_ignored() {
        let mut t = armed_overflow();
        for _ in 0..4 {
            assert!(!t.mcycle()); // M-cycle N: the increment overflows at its end
        }
        assert!(t.mcycle()); // M-cycle N+1: the delayed request lands; TIMA = TMA, reload window open
        t.write_tima(0x99); // a CPU write to TIMA on M-cycle N+2 is ignored (note 01b point 2)
        assert_eq!(t.tima(), 0x23); // TIMA stays equal to TMA; the written $99 is lost
    }
}
