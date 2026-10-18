//! Save state slots 1-4 (E09_04, decision A_05). Slot files live next to the ROM; file I/O
//! stays in this frontend module, the core only serializes into buffers.

use puce8gb_core::StateError;
use std::path::{Path, PathBuf};

/// Number of save state slots: 1..=SLOT_COUNT.
pub const SLOT_COUNT: u32 = 4;

/// Slot file for `slot` (1..=SLOT_COUNT) next to the battery save file of the ROM: same
/// directory and stem, named `<stem>.slot<N>.state`.
pub fn slot_path(battery_save: &Path, slot: u32) -> PathBuf {
    let stem = battery_save
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("save");
    battery_save.with_file_name(format!("{stem}.slot{slot}.state"))
}

/// Reads a slot file; None when it does not exist or cannot be read.
pub fn read_slot(path: &Path) -> Option<Vec<u8>> {
    std::fs::read(path).ok()
}

/// Writes through a temporary file then renames, so a crash cannot truncate the slot.
pub fn write_slot(path: &Path, data: &[u8]) -> std::io::Result<()> {
    let tmp = path.with_extension("state.tmp");
    std::fs::write(&tmp, data)?;
    std::fs::rename(&tmp, path)
}

/// Human readable status message for a rejected state (decision A_05: never panic).
pub fn state_error_message(slot: u32, err: &StateError) -> String {
    let what = match err {
        StateError::BadMagic => "not a state file",
        StateError::BadVersion => "unsupported state version",
        StateError::BadSize => "truncated state file",
        StateError::InvalidState => "corrupted state file",
        StateError::RomMismatch => "state saved for another cartridge",
        StateError::BufferTooSmall => "internal error (buffer too small)",
    };
    format!("Slot {slot} rejected: {what}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn e09_04_slot_path_is_next_to_the_rom() {
        assert_eq!(
            slot_path(Path::new("roms/game.sav"), 1),
            PathBuf::from("roms/game.slot1.state")
        );
    }

    #[test]
    fn e09_04_slot_path_keeps_parent_dir_and_stem() {
        assert_eq!(
            slot_path(Path::new("a/game.tar.sav"), 2),
            PathBuf::from("a/game.tar.slot2.state")
        );
        assert_eq!(
            slot_path(Path::new("game.sav"), 3),
            PathBuf::from("game.slot3.state")
        );
    }

    #[test]
    fn e09_04_slot_path_without_extension() {
        assert_eq!(
            slot_path(Path::new("roms/savefile"), 4),
            PathBuf::from("roms/savefile.slot4.state")
        );
    }

    #[test]
    fn e09_04_slots_are_distinct_files() {
        let base = Path::new("roms/game.sav");
        for a in 1..=SLOT_COUNT {
            for b in 1..=SLOT_COUNT {
                if a != b {
                    assert_ne!(slot_path(base, a), slot_path(base, b));
                }
            }
        }
    }

    #[test]
    fn e09_04_mismatch_message_names_the_case() {
        let msg = state_error_message(2, &StateError::RomMismatch);
        assert_eq!(msg, "Slot 2 rejected: state saved for another cartridge");
    }

    #[test]
    fn e09_04_all_state_errors_have_a_message() {
        for err in [
            StateError::BufferTooSmall,
            StateError::InvalidState,
            StateError::BadMagic,
            StateError::BadVersion,
            StateError::BadSize,
            StateError::RomMismatch,
        ] {
            assert!(state_error_message(1, &err).starts_with("Slot 1 rejected: "));
        }
    }

    #[test]
    fn e09_04_write_then_read_roundtrip() {
        let path = std::env::temp_dir().join("puce8gb_e09_04_roundtrip.slot1.state");
        write_slot(&path, &[9, 8, 7]).unwrap();
        assert_eq!(read_slot(&path), Some(vec![9, 8, 7]));
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn e09_04_read_missing_slot_is_none() {
        let path = std::env::temp_dir().join("puce8gb_e09_04_missing.slot1.state");
        let _ = std::fs::remove_file(&path);
        assert_eq!(read_slot(&path), None);
    }
}
