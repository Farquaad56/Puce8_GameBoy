//! Battery RAM persistence (E07_06). File I/O lives in the frontend only.
use puce8gb_core::Dmg;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

/// Minimum delay between two dirty checks while a game runs.
pub const SAVE_INTERVAL: Duration = Duration::from_secs(5);

/// True when the cartridge RAM differs from the bytes last written.
pub fn is_dirty(current: &[u8], last_saved: &[u8]) -> bool {
    current != last_saved
}

/// Reads a save file; None when it does not exist or cannot be read.
pub fn read_save(path: &Path) -> Option<Vec<u8>> {
    std::fs::read(path).ok()
}

/// Writes through a temporary file then renames, so a crash cannot truncate the save.
pub fn write_save(path: &Path, data: &[u8]) -> std::io::Result<()> {
    let tmp = path.with_extension("sav.tmp");
    std::fs::write(&tmp, data)?;
    std::fs::rename(&tmp, path)
}

pub struct BatterySaver {
    path: Option<PathBuf>,
    last_saved: Vec<u8>,
    last_check: Instant,
}

impl Default for BatterySaver {
    fn default() -> Self {
        Self::new()
    }
}

impl BatterySaver {
    pub fn new() -> Self {
        BatterySaver {
            path: None,
            last_saved: Vec::new(),
            last_check: Instant::now(),
        }
    }

    /// Binds a freshly built Dmg to a save path and loads the save file when present.
    pub fn attach(&mut self, dmg: &mut Dmg, path: Option<PathBuf>) {
        self.last_saved = Vec::new();
        self.last_check = Instant::now();
        self.path = None;
        let Some(path) = path else { return };
        let Some(current) = dmg.battery_ram() else {
            return;
        };
        self.last_saved = current;
        if let Some(data) = read_save(&path).filter(|d| dmg.load_battery_ram(d).is_ok()) {
            self.last_saved = data;
        }
        self.path = Some(path);
    }

    /// Call every emulation pass; writes at most once per SAVE_INTERVAL and only if dirty.
    pub fn poll(&mut self, dmg: &Dmg, now: Instant) {
        if now.duration_since(self.last_check) < SAVE_INTERVAL {
            return;
        }
        self.last_check = now;
        self.save_if_dirty(dmg);
    }

    /// Writes immediately if dirty (exit, ROM change).
    pub fn flush(&mut self, dmg: &Dmg) {
        self.save_if_dirty(dmg);
    }

    fn save_if_dirty(&mut self, dmg: &Dmg) {
        let Some(path) = self.path.clone() else {
            return;
        };
        let Some(ram) = dmg.battery_ram() else { return };
        if is_dirty(&ram, &self.last_saved) && write_save(&path, &ram).is_ok() {
            self.last_saved = ram;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn e07_06_is_dirty_detects_change() {
        assert!(!is_dirty(&[1, 2, 3], &[1, 2, 3]));
        assert!(is_dirty(&[1, 2, 4], &[1, 2, 3]));
        assert!(is_dirty(&[1, 2], &[1, 2, 3]));
    }

    #[test]
    fn e07_06_write_then_read_roundtrip() {
        let path = std::env::temp_dir().join("puce8gb_e07_06_roundtrip.sav");
        write_save(&path, &[9, 8, 7]).unwrap();
        write_save(&path, &[1, 2, 3, 4]).unwrap();
        assert_eq!(read_save(&path), Some(vec![1, 2, 3, 4]));
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn e07_06_read_missing_save_is_none() {
        let path = std::env::temp_dir().join("puce8gb_e07_06_missing_does_not_exist.sav");
        let _ = std::fs::remove_file(&path);
        assert_eq!(read_save(&path), None);
    }
}
