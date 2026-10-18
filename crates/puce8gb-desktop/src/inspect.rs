//! OAM table, palette and hex memory helpers (E08_03). Pure functions: memory is read with peek only.

use crate::viewer::{shade_gray, Snapshot};
use eframe::egui;
use puce8gb_core::debug::{decode_palette, OamEntry};

/// Bytes shown per hex row.
pub const ROW_BYTES: usize = 16;

/// Formats `len` bytes of `memory` starting at `start` as rows of up to 16 bytes:
/// `AAAA: HH HH ... HH  ascii`. The hex part is padded to 47 columns so the ascii column
/// lines up on a short last row. The range is clamped to the end of `memory`.
pub fn hex_rows(memory: &[u8], start: u16, len: usize) -> Vec<String> {
    let begin = usize::from(start);
    let end = begin.saturating_add(len).min(memory.len());
    let mut rows = Vec::new();
    let mut addr = begin;
    while addr < end {
        let stop = (addr + ROW_BYTES).min(end);
        let bytes = &memory[addr..stop];
        let hex: Vec<String> = bytes.iter().map(|b| format!("{b:02X}")).collect();
        let ascii: String = bytes
            .iter()
            .map(|&b| {
                if (0x20..=0x7E).contains(&b) {
                    char::from(b)
                } else {
                    '.'
                }
            })
            .collect();
        rows.push(format!("{addr:04X}: {:<47}  {ascii}", hex.join(" ")));
        addr = stop;
    }
    rows
}

/// Parses a hexadecimal address with an optional `0x` or `$` prefix; None when invalid or above $FFFF.
pub fn parse_address(text: &str) -> Option<u16> {
    let t = text.trim();
    let t = t
        .strip_prefix("0x")
        .or_else(|| t.strip_prefix("0X"))
        .or_else(|| t.strip_prefix('$'))
        .unwrap_or(t);
    u16::from_str_radix(t, 16).ok()
}

/// One OAM table line with the raw bytes: `NN  Y=yyy  X=xxx  tile=TT  flags=FF BG YF XF P1`.
/// Flag bits: 7 BG priority, 6 Y flip, 5 X flip, 4 DMG palette (0 = OBP0, 1 = OBP1).
pub fn oam_row(index: usize, entry: &OamEntry) -> String {
    let flag = |bit: u8, on: &'static str, off: &'static str| {
        if entry.flags & (1 << bit) != 0 {
            on
        } else {
            off
        }
    };
    format!(
        "{index:02}  Y={:03}  X={:03}  tile={:02X}  flags={:02X} {} {} {} {}",
        entry.y,
        entry.x,
        entry.tile,
        entry.flags,
        flag(7, "BG", "--"),
        flag(6, "YF", "--"),
        flag(5, "XF", "--"),
        flag(4, "P1", "P0"),
    )
}

/// Palette register line: `NAME $ADDR = RR  shades a b c d` (shade 0 = lightest).
pub fn palette_label(name: &str, addr: u16, reg: u8) -> String {
    let s = decode_palette(reg);
    format!(
        "{name} ${addr:04X} = {reg:02X}  shades {} {} {} {}",
        s[0], s[1], s[2], s[3]
    )
}

/// Memory rows shown at once: one page of 256 bytes.
const PAGE: u16 = 256;
pub const MEM_VIEW_BYTES: usize = PAGE as usize;

/// State of the inspector window: first address of the memory page and the text typed.
pub struct InspectState {
    pub base: u16,
    pub input: String,
}

impl Default for InspectState {
    fn default() -> Self {
        InspectState {
            base: 0xC000,
            input: String::from("C000"),
        }
    }
}

fn palette_line(ui: &mut egui::Ui, name: &str, addr: u16, snap: &Snapshot) {
    let reg = snap.memory[usize::from(addr)];
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(palette_label(name, addr, reg)).monospace());
        for shade in decode_palette(reg) {
            let (rect, _) = ui.allocate_exact_size(egui::vec2(18.0, 18.0), egui::Sense::hover());
            ui.painter()
                .rect_filled(rect, 2.0, egui::Color32::from_gray(shade_gray(shade)));
        }
    });
}

/// Floating window with palettes, the OAM table and a hex page of memory (peek data only).
/// Returns false when the user closed the window.
pub fn show_window(ctx: &egui::Context, state: &mut InspectState, snap: Option<&Snapshot>) -> bool {
    let mut open = true;
    egui::Window::new("Memory, OAM and palettes")
        .open(&mut open)
        .show(ctx, |ui| {
            let Some(snap) = snap else {
                ui.label("No data yet (load a ROM)");
                return;
            };
            egui::ScrollArea::vertical().show(ui, |ui| {
                ui.heading("Palettes");
                palette_line(ui, "BGP ", 0xFF47, snap);
                palette_line(ui, "OBP0", 0xFF48, snap);
                palette_line(ui, "OBP1", 0xFF49, snap);
                ui.separator();
                ui.heading("OAM");
                for (index, entry) in snap.oam.iter().enumerate() {
                    ui.label(egui::RichText::new(oam_row(index, entry)).monospace());
                }
                ui.separator();
                ui.heading("Memory");
                ui.horizontal(|ui| {
                    ui.label("Address $");
                    let field =
                        ui.add(egui::TextEdit::singleline(&mut state.input).desired_width(60.0));
                    if field.changed() {
                        state.base = parse_address(&state.input).unwrap_or(state.base);
                    }
                    if ui.button("-256").clicked() {
                        state.base = state.base.wrapping_sub(PAGE);
                        state.input = format!("{:04X}", state.base);
                    }
                    if ui.button("+256").clicked() {
                        state.base = state.base.wrapping_add(PAGE);
                        state.input = format!("{:04X}", state.base);
                    }
                });
                for row in hex_rows(&snap.memory, state.base, MEM_VIEW_BYTES) {
                    ui.label(egui::RichText::new(row).monospace());
                }
            });
        });
    open
}

#[cfg(test)]
mod tests {
    use super::*;

    fn memory() -> Vec<u8> {
        let mut mem = vec![0u8; 0x10000];
        mem[0x100..0x110].copy_from_slice(&[
            0x00, 0xC3, 0x50, 0x01, 0x41, 0x42, 0x7E, 0x7F, 0x30, 0xFF, 0, 0, 0, 0, 0, 0,
        ]);
        mem[0x110..0x114].copy_from_slice(&[0x48, 0x49, 0x00, 0xFF]);
        mem
    }

    #[test]
    fn e08_03_hex_rows_full_row() {
        let rows = hex_rows(&memory(), 0x0100, 16);
        assert_eq!(rows.len(), 1);
        assert_eq!(
            rows[0],
            "0100: 00 C3 50 01 41 42 7E 7F 30 FF 00 00 00 00 00 00  ..P.AB~.0......."
        );
    }

    #[test]
    fn e08_03_hex_rows_partial_clamped_and_empty() {
        let rows = hex_rows(&memory(), 0x0100, 20);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[1], format!("0110: {:<47}  HI..", "48 49 00 FF"));
        let tail = hex_rows(&memory(), 0xFFF8, 100);
        assert_eq!(tail.len(), 1);
        assert!(tail[0].starts_with("FFF8: 00 00 00 00 00 00 00 00"));
        assert!(hex_rows(&memory(), 0x0100, 0).is_empty());
    }

    #[test]
    fn e08_03_parse_address_forms() {
        assert_eq!(parse_address("C000"), Some(0xC000));
        assert_eq!(parse_address("0x9800"), Some(0x9800));
        assert_eq!(parse_address("$FF40"), Some(0xFF40));
        assert_eq!(parse_address(" ff "), Some(0x00FF));
        assert_eq!(parse_address("10000"), None);
        assert_eq!(parse_address("zz"), None);
        assert_eq!(parse_address(""), None);
    }

    #[test]
    fn e08_03_oam_row_format_and_flags() {
        let a = OamEntry {
            y: 16,
            x: 8,
            tile: 0x42,
            flags: 0xA0,
        };
        assert_eq!(
            oam_row(3, &a),
            "03  Y=016  X=008  tile=42  flags=A0 BG -- XF P0"
        );
        let b = OamEntry {
            y: 0,
            x: 0,
            tile: 0,
            flags: 0x10,
        };
        assert_eq!(
            oam_row(0, &b),
            "00  Y=000  X=000  tile=00  flags=10 -- -- -- P1"
        );
    }

    #[test]
    fn e08_03_palette_label_format() {
        assert_eq!(
            palette_label("BGP", 0xFF47, 0xE4),
            "BGP $FF47 = E4  shades 0 1 2 3"
        );
        assert_eq!(
            palette_label("OBP0", 0xFF48, 0x1B),
            "OBP0 $FF48 = 1B  shades 3 2 1 0"
        );
    }
}
