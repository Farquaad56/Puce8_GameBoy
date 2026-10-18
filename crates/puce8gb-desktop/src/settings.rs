//! Plain-text settings file next to the executable: volume and key bindings (E07_06).

use crate::input_map::{Button, KeyMap, BUTTONS};
use crate::volume::{DEFAULT_VOLUME, MAX_VOLUME};
use eframe::egui::Key;
use std::path::{Path, PathBuf};

pub const SETTINGS_FILE: &str = "puce8gb.cfg";

/// Persistent user settings.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Settings {
    pub volume: u8,
    pub keys: KeyMap,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            volume: DEFAULT_VOLUME,
            keys: KeyMap::default(),
        }
    }
}

fn button_name(button: Button) -> &'static str {
    match button {
        Button::Up => "up",
        Button::Down => "down",
        Button::Left => "left",
        Button::Right => "right",
        Button::A => "a",
        Button::B => "b",
        Button::Start => "start",
        Button::Select => "select",
    }
}

/// Settings file path: next to the executable.
pub fn settings_path(exe: &Path) -> PathBuf {
    exe.with_file_name(SETTINGS_FILE)
}

impl Settings {
    /// One `key=value` line per setting.
    pub fn to_text(self) -> String {
        let mut text = format!("volume={}\n", self.volume);
        for button in BUTTONS {
            text.push_str(&format!(
                "key_{}={}\n",
                button_name(button),
                self.keys.key_for(button).name()
            ));
        }
        text
    }

    /// Parses settings text; unknown or invalid lines are ignored and keep the default.
    pub fn from_text(text: &str) -> Settings {
        let mut settings = Settings::default();
        for line in text.lines() {
            let Some((name, value)) = line.split_once('=') else {
                continue;
            };
            let (name, value) = (name.trim(), value.trim());
            if name == "volume" {
                if let Ok(volume) = value.parse::<u8>() {
                    settings.volume = volume.min(MAX_VOLUME);
                }
            } else if let Some(button_text) = name.strip_prefix("key_") {
                let button = BUTTONS
                    .iter()
                    .copied()
                    .find(|b| button_name(*b) == button_text);
                if let (Some(button), Some(key)) = (button, Key::from_name(value)) {
                    settings.keys = settings.keys.with_key(button, key);
                }
            }
        }
        settings
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn e07_06_settings_round_trip() {
        let custom = Settings {
            volume: 35,
            keys: KeyMap::default().with_key(Button::A, Key::Space),
        };
        assert_eq!(Settings::from_text(&custom.to_text()), custom);
        let default = Settings::default();
        assert_eq!(Settings::from_text(&default.to_text()), default);
    }

    #[test]
    fn e07_06_settings_ignore_invalid_lines() {
        let parsed = Settings::from_text("volume=250\nkey_a=Q\nbogus\nkey_b=NotAKey\nvolume=abc\n");
        assert_eq!(parsed.volume, 100);
        assert_eq!(parsed.keys.a, Key::Q);
        assert_eq!(parsed.keys.b, KeyMap::default().b);
    }

    #[test]
    fn e07_06_settings_empty_text_gives_defaults() {
        assert_eq!(Settings::from_text(""), Settings::default());
    }

    #[test]
    fn e07_06_settings_path_is_next_to_the_executable() {
        assert_eq!(
            settings_path(Path::new("C:/tools/puce8gb.exe")),
            PathBuf::from("C:/tools/puce8gb.cfg")
        );
    }

    #[test]
    fn e07_06_rebind_swaps_keys_to_stay_unique() {
        let map = KeyMap::default();
        let swapped = map.rebind(Button::A, Key::X);
        assert_eq!(swapped.a, Key::X);
        assert_eq!(swapped.b, Key::W);
        let free = map.rebind(Button::A, Key::Q);
        assert_eq!(free.a, Key::Q);
        assert_eq!(free.b, Key::X);
        assert_eq!(map.rebind(Button::A, Key::W), map);
    }
}
