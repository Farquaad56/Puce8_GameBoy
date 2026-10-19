//! DMG palette presets and color text parsing (E11_02, decision A_09).
//! Pure data and text, no egui type. Colors are 0xRRGGBB, index 0 = lightest shade.
//! The presets are aesthetic choices, not hardware facts.

/// A named set of four colors for shades 0..3.
#[allow(dead_code)] // used by E11_03
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Preset {
    pub name: &'static str,
    pub colors: [u32; 4],
}

/// Built-in presets, in menu order.
#[allow(dead_code)] // used by E11_03
pub const PRESETS: [Preset; 4] = [
    Preset {
        name: "Grayscale",
        colors: [0xFFFFFF, 0xAAAAAA, 0x555555, 0x000000],
    },
    Preset {
        name: "Classic green",
        colors: [0x9BBC0F, 0x8BAC0F, 0x306230, 0x0F380F],
    },
    Preset {
        name: "Pocket",
        colors: [0xC4CFA1, 0x8B956D, 0x4D533C, 0x1F1F1F],
    },
    Preset {
        name: "Kirby light",
        colors: [0xE0F8D0, 0x88C070, 0x346856, 0x081820],
    },
];

/// Index of a preset by name (case-insensitive, trimmed).
#[allow(dead_code)] // used by E11_03
pub fn preset_index(name: &str) -> Option<usize> {
    let wanted = name.trim();
    PRESETS
        .iter()
        .position(|preset| preset.name.eq_ignore_ascii_case(wanted))
}

/// Parses "#RRGGBB" or "RRGGBB" (upper or lower case) into 0xRRGGBB.
#[allow(dead_code)] // used by E11_03
pub fn parse_hex_color(text: &str) -> Option<u32> {
    let trimmed = text.trim();
    let digits = trimmed.strip_prefix('#').unwrap_or(trimmed);
    if digits.len() != 6 || !digits.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    u32::from_str_radix(digits, 16).ok()
}

/// Formats 0xRRGGBB as "#RRGGBB" in upper case.
#[allow(dead_code)] // used by E11_03
pub fn format_hex_color(color: u32) -> String {
    format!("#{:06X}", color & 0x00FF_FFFF)
}

/// Parses four colors separated by commas.
#[allow(dead_code)] // used by E11_03
pub fn parse_palette(text: &str) -> Option<[u32; 4]> {
    let mut colors = [0u32; 4];
    let mut count = 0usize;
    for part in text.split(',') {
        if count >= 4 {
            return None;
        }
        colors[count] = parse_hex_color(part)?;
        count += 1;
    }
    if count == 4 {
        Some(colors)
    } else {
        None
    }
}

/// Formats four colors joined by commas.
#[allow(dead_code)] // used by E11_03
pub fn format_palette(palette: [u32; 4]) -> String {
    palette
        .iter()
        .map(|&color| format_hex_color(color))
        .collect::<Vec<_>>()
        .join(",")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn e11_02_parse_hex_color_accepts_valid_text() {
        assert_eq!(parse_hex_color("#9BBC0F"), Some(0x9BBC0F));
        assert_eq!(parse_hex_color("9bbc0f"), Some(0x9BBC0F));
        assert_eq!(parse_hex_color("  #0f380f "), Some(0x0F380F));
    }

    #[test]
    fn e11_02_parse_hex_color_rejects_invalid_text() {
        assert_eq!(parse_hex_color("#9BBC0"), None);
        assert_eq!(parse_hex_color("GGGGGG"), None);
        assert_eq!(parse_hex_color("#9BBC0F00"), None);
        assert_eq!(parse_hex_color("+12345"), None);
        assert_eq!(parse_hex_color(""), None);
    }

    #[test]
    fn e11_02_format_hex_color_is_upper_case() {
        assert_eq!(format_hex_color(0x0F380F), "#0F380F");
        assert_eq!(format_hex_color(0x9BBC0F), "#9BBC0F");
    }

    #[test]
    fn e11_02_presets_table_has_four_entries() {
        assert_eq!(PRESETS.len(), 4);
        assert_eq!(PRESETS[0].name, "Grayscale");
        assert_eq!(PRESETS[0].colors, [0xFFFFFF, 0xAAAAAA, 0x555555, 0x000000]);
        assert_eq!(PRESETS[1].colors, [0x9BBC0F, 0x8BAC0F, 0x306230, 0x0F380F]);
    }

    #[test]
    fn e11_02_preset_index_ignores_case_and_spaces() {
        assert_eq!(preset_index(" classic GREEN "), Some(1));
        assert_eq!(preset_index("POCKET"), Some(2));
        assert_eq!(preset_index("nope"), None);
    }

    #[test]
    fn e11_02_palette_text_round_trip() {
        let text = format_palette(PRESETS[3].colors);
        assert_eq!(text, "#E0F8D0,#88C070,#346856,#081820");
        assert_eq!(parse_palette(&text), Some(PRESETS[3].colors));
    }

    #[test]
    fn e11_02_parse_palette_needs_exactly_four_colors() {
        assert_eq!(parse_palette("#FFFFFF,#AAAAAA,#555555"), None);
        assert_eq!(
            parse_palette("#FFFFFF,#AAAAAA,#555555,#000000,#111111"),
            None
        );
        assert_eq!(parse_palette("#FFFFFF,#AAAAAA,#555555,zzzzzz"), None);
    }
}
