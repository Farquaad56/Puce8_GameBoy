//! CPU trace window: the last traced instructions with their disassembly (E08_04).

use crate::disasm::trace_row;
use crate::viewer::Snapshot;
use eframe::egui;

/// Number of trace rows shown, newest first.
pub const TRACE_ROWS: usize = 64;

/// Text lines of the window: at most TRACE_ROWS, newest instruction first.
pub fn trace_lines(snap: &Snapshot) -> Vec<String> {
    snap.trace
        .iter()
        .rev()
        .take(TRACE_ROWS)
        .map(trace_row)
        .collect()
}

/// Shows the window. Returns false when the user closed it.
pub fn show_window(ctx: &egui::Context, snap: Option<&Snapshot>) -> bool {
    let mut open = true;
    egui::Window::new("CPU trace")
        .open(&mut open)
        .default_width(640.0)
        .show(ctx, |ui| {
            let Some(snap) = snap else {
                ui.label("No data yet (load a ROM)");
                return;
            };
            if snap.trace.is_empty() {
                ui.label("Waiting for instructions...");
                return;
            }
            ui.label("Newest first. Registers are shown before the instruction runs.");
            egui::ScrollArea::vertical().show(ui, |ui| {
                for line in trace_lines(snap) {
                    ui.label(egui::RichText::new(line).monospace());
                }
            });
        });
    open
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::viewer::capture;
    use puce8gb_core::{Dmg, Machine};

    fn trace_dmg() -> Dmg {
        let mut rom = vec![0u8; 0x8000];
        rom[0x100..0x10B].copy_from_slice(&[
            0x3E, 0x01, 0x06, 0x00, 0x04, 0x80, 0xEA, 0x00, 0xC0, 0x18, 0xF9,
        ]);
        Dmg::new(&rom).expect("valid rom")
    }

    fn pc_of(line: &str) -> u16 {
        u16::from_str_radix(&line[..4], 16).expect("hex pc")
    }

    #[test]
    fn e08_04_window_lines_follow_the_program() {
        let mut dmg = trace_dmg();
        dmg.set_trace_enabled(true);
        dmg.run_frame();
        let lines = trace_lines(&capture(&dmg));
        assert_eq!(lines.len(), TRACE_ROWS);
        for line in &lines {
            let expected = match pc_of(line) {
                0x0104 => "INC B",
                0x0105 => "ADD A,B",
                0x0106 => "LD ($C000),A",
                0x0109 => "JR $0104",
                other => panic!("unexpected pc {other:04X}"),
            };
            assert!(line.contains(expected), "{line}");
        }
        for pair in lines.windows(2) {
            let older = match pc_of(&pair[0]) {
                0x0104 => 0x0109,
                0x0105 => 0x0104,
                0x0106 => 0x0105,
                _ => 0x0106,
            };
            assert_eq!(pc_of(&pair[1]), older, "{} / {}", pair[0], pair[1]);
        }
    }

    #[test]
    fn e08_04_window_lines_empty_when_trace_disabled() {
        let mut dmg = trace_dmg();
        dmg.run_frame();
        assert!(trace_lines(&capture(&dmg)).is_empty());
    }

    #[test]
    fn e08_04_capture_keeps_all_ring_entries() {
        let mut dmg = trace_dmg();
        dmg.set_trace_enabled(true);
        dmg.run_frame();
        assert_eq!(capture(&dmg).trace.len(), 256);
    }
}
