#![forbid(unsafe_code)]

//! Cycle-accurate Game Boy (DMG) emulator core.
//! No external dependencies, no I/O, deterministic.

pub mod audio;
pub mod bus;
pub mod cpu;
pub mod input;
pub mod machine;
pub mod media;
pub mod savestate;
pub mod video;

pub use input::Input;
pub use machine::{Dmg, LoadError, Machine};
pub use savestate::StateError;

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
