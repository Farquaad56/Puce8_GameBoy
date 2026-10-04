#![forbid(unsafe_code)]

//! Cycle-accurate Game Boy (DMG) emulator core.
//! No external dependencies, no I/O, deterministic.

/// Emulator core version string.
pub fn version() -> &'static str {
    "0.1.0"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn e00_01_version() {
        assert_eq!(version(), "0.1.0");
    }
}
