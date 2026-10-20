//! Plain-text settings file next to the executable: volume and key bindings (E07_06).

use crate::display::clamp_scale;
use crate::input_map::{Button, KeyMap, BUTTONS};
use crate::lcd_filter::LcdMode;
use crate::palettes::PaletteChoice;
use crate::volume::{DEFAULT_VOLUME, MAX_VOLUME};
use eframe::egui::Key;
use std::path::{Path, PathBuf};

pub const SETTINGS_FILE: &str = "puce8gb.cfg";

/// Window scale used when the settings file has no valid `scale` line (UNKNOWN - to confirm).
const DEFAULT_WINDOW_SCALE: u32 = 3;

/// Persistent user settings.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Settings {
    pub volume: u8,
    pub keys: KeyMap,
    pub window_scale: u32,
    pub show_fps: bool,
    pub palette: PaletteChoice,
    pub lcd_mode: LcdMode,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            volume: DEFAULT_VOLUME,
            keys: KeyMap::default(),
            window_scale: DEFAULT_WINDOW_SCALE,
            show_fps: true,
            palette: PaletteChoice::CoreDefault,
            lcd_mode: LcdMode::Off,
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
        text.push_str(&format!("scale={}\n", self.window_scale));
        text.push_str(&format!("show_fps={}\n", self.show_fps));
        text.push_str(&format!("palette={}\n", self.palette.to_text()));
        text.push_str(&format!("lcd_filter={}\n", self.lcd_mode.as_str()));
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
            } else if name == "scale" {
                if let Ok(scale) = value.parse::<u32>() {
                    settings.window_scale = clamp_scale(scale);
                }
            } else if name == "show_fps" {
                if let Ok(show) = value.parse::<bool>() {
                    settings.show_fps = show;
                }
            } else if name == "palette" {
                settings.palette = PaletteChoice::from_text(value);
            } else if name == "lcd_filter" {
                settings.lcd_mode = LcdMode::from_str(value);
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
            ..Settings::default()
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

    #[test]
    fn e10_02_new_fields_round_trip() {
        let custom = Settings {
            window_scale: 5,
            show_fps: false,
            ..Settings::default()
        };
        assert_eq!(Settings::from_text(&custom.to_text()), custom);
    }

    #[test]
    fn e10_02_scale_is_clamped() {
        assert_eq!(Settings::from_text("scale=0\n").window_scale, 1);
        assert_eq!(Settings::from_text("scale=99\n").window_scale, 6);
        assert_eq!(Settings::from_text("scale=4\n").window_scale, 4);
    }

    #[test]
    fn e10_02_invalid_values_keep_defaults() {
        let parsed = Settings::from_text("scale=abc\nshow_fps=maybe\n");
        assert_eq!(parsed, Settings::default());
    }

    #[test]
    fn e10_02_missing_keys_give_defaults() {
        let parsed = Settings::from_text("volume=40\n");
        assert_eq!(parsed.volume, 40);
        assert_eq!(parsed.window_scale, 3);
        assert!(parsed.show_fps);
    }

    #[test]
    fn e10_02_unknown_key_is_ignored() {
        let parsed = Settings::from_text("bogus_key=1\nscale=2\nshow_fps=false\n");
        assert_eq!(parsed.window_scale, 2);
        assert!(!parsed.show_fps);
    }

    #[test]
    fn e11_03_palette_round_trip_in_settings() {
        for palette in [
            PaletteChoice::CoreDefault,
            PaletteChoice::Preset(1),
            PaletteChoice::Custom([0x112233, 0x445566, 0x778899, 0xAABBCC]),
        ] {
            let custom = Settings {
                palette,
                ..Settings::default()
            };
            assert_eq!(Settings::from_text(&custom.to_text()), custom);
        }
    }

    #[test]
    fn e11_03_unknown_palette_text_gives_default() {
        let parsed = Settings::from_text("palette=nope\nvolume=40\n");
        assert_eq!(parsed.palette, PaletteChoice::CoreDefault);
        assert_eq!(parsed.volume, 40);
    }

    #[test]
    fn e11_03_missing_palette_line_gives_default() {
        let parsed = Settings::from_text("volume=40\n");
        assert_eq!(parsed.palette, PaletteChoice::CoreDefault);
    }

    #[test]
    fn e12_04_lcd_mode_round_trip_in_settings() {
        for lcd_mode in [LcdMode::Off, LcdMode::Grid, LcdMode::Ghost] {
            let custom = Settings {
                lcd_mode,
                ..Settings::default()
            };
            assert_eq!(Settings::from_text(&custom.to_text()), custom);
        }
    }

    #[test]
    fn e12_04_unknown_lcd_text_gives_off() {
        let parsed = Settings::from_text("lcd_filter=banana\nvolume=40\n");
        assert_eq!(parsed.lcd_mode, LcdMode::Off);
        assert_eq!(parsed.volume, 40);
    }

    #[test]
    fn e12_04_missing_lcd_line_gives_off() {
        assert_eq!(Settings::from_text("volume=40\n").lcd_mode, LcdMode::Off);
    }
}
