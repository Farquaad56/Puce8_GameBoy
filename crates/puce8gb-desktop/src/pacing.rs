//! Integer pacing of the emulation thread: wall-clock nanoseconds to dots, no floating point (E07_02).

/// Master clock: 4194304 dots per second (decision A_01).
pub const DOT_HZ: u64 = 4_194_304;
const NS_PER_S: u128 = 1_000_000_000;
/// Dots per frame (decision A_01, note 01_timing.md).
const FRAME_DOTS: u64 = 70_224;
/// Longest backlog kept, in whole frames: beyond this, frames are dropped instead of replayed.
pub const MAX_BACKLOG_FRAMES: u64 = 4;

/// Accumulator turning elapsed wall time into dots to run.
#[derive(Debug, Default)]
pub struct Pacer {
    remainder: u128,
}

impl Pacer {
    /// Dots to run for `elapsed_ns` of wall time; the sub-dot remainder is carried over so no time is lost.
    pub fn cycles_to_run(&mut self, elapsed_ns: u64) -> u64 {
        let total = u128::from(elapsed_ns) * u128::from(DOT_HZ) + self.remainder;
        self.remainder = total % NS_PER_S;
        u64::try_from(total / NS_PER_S).unwrap_or(u64::MAX)
    }
}

/// Caps the dots owed so a slow host drops frames instead of replaying an ever-growing backlog.
pub fn clamp_owed(owed: u64) -> u64 {
    owed.min(MAX_BACKLOG_FRAMES * FRAME_DOTS)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn e07_02_one_second_is_exactly_4194304_dots() {
        let mut pacer = Pacer::default();
        assert_eq!(pacer.cycles_to_run(1_000_000_000), 4_194_304);
    }

    #[test]
    fn e07_02_remainder_is_carried_between_calls() {
        let mut pacer = Pacer::default();
        let mut total = 0;
        for _ in 0..1000 {
            total += pacer.cycles_to_run(1_000_000);
        }
        assert_eq!(total, 4_194_304);
        let mut pacer = Pacer::default();
        let split = [333_333_333u64, 333_333_333, 333_333_333, 1];
        let total: u64 = split.iter().map(|&ns| pacer.cycles_to_run(ns)).sum();
        assert_eq!(total, 4_194_304);
    }

    #[test]
    fn e07_02_zero_elapsed_runs_nothing() {
        let mut pacer = Pacer::default();
        assert_eq!(pacer.cycles_to_run(0), 0);
    }

    #[test]
    fn e07_02_one_frame_needs_16742707_ns() {
        let mut pacer = Pacer::default();
        assert_eq!(pacer.cycles_to_run(16_742_706), 70_223);
        let mut pacer = Pacer::default();
        assert_eq!(pacer.cycles_to_run(16_742_707), 70_224);
    }

    #[test]
    fn e07_02_backlog_is_clamped() {
        assert_eq!(clamp_owed(100), 100);
        assert_eq!(clamp_owed(10 * 70_224), 4 * 70_224);
    }

    #[test]
    fn e07_02_huge_elapsed_does_not_overflow() {
        let mut pacer = Pacer::default();
        assert!(pacer.cycles_to_run(u64::MAX) > 0);
    }
}
