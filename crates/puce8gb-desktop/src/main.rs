//! Desktop frontend: an eframe window showing the core framebuffer (E07_01).
mod audio;
mod battery;
mod disasm;
mod emu;
mod fps;
mod input_map;
mod inspect;
mod tracewin;
#[allow(dead_code)]
#[rustfmt::skip]
mod opcodes;
mod display;
mod pacing;
mod pads;
mod palettes;
mod resample;
mod rom_info;
mod scale;
mod settings;
mod slots;
mod viewer;
mod volume;

use eframe::egui;
use emu::{Command, EmuHandle};
use fps::FpsCounter;
use input_map::{merge, state_from, Button, KeyMap, BUTTONS};
use pads::Pads;
use puce8gb_core::Input;
use settings::Settings;
use std::path::PathBuf;
use std::time::Instant;

const SCREEN_W: usize = 160;
const SCREEN_H: usize = 144;

struct App {
    emu: EmuHandle,
    frame: Vec<u32>,
    paused: bool,
    last_input: Input,
    texture: Option<egui::TextureHandle>,
    status: String,
    pads: Pads,
    key_map: KeyMap,
    volume: u8,
    fullscreen: bool,
    path_input: String,
    fps: FpsCounter,
    started: Instant,
    title: String,
    sent_title: String,
    settings_file: PathBuf,
    capture: Option<Button>,
    viewer_open: bool,
    snapshot: Option<viewer::Snapshot>,
    snapshot_dirty: bool,
    sheet_tex: Option<egui::TextureHandle>,
    map_tex: [Option<egui::TextureHandle>; 2],
    debug_open: bool,
    trace_open: bool,
    inspect: inspect::InspectState,
    window_scale: u32,
    show_fps: bool,
    chrome_h: f32,
    resize_pending: bool,
}

impl App {
    fn new(rom_path: Option<String>) -> Self {
        let settings_file = settings::settings_path(&std::env::current_exe().unwrap_or_default());
        let loaded = std::fs::read_to_string(&settings_file)
            .map(|text| Settings::from_text(&text))
            .unwrap_or_default();
        let mut app = App {
            emu: EmuHandle::spawn(loaded.volume),
            frame: Vec::new(),
            paused: false,
            last_input: Input::default(),
            texture: None,
            status: String::from("Drop a ROM file on the window"),
            pads: Pads::default(),
            key_map: loaded.keys,
            volume: loaded.volume,
            fullscreen: false,
            path_input: String::new(),
            fps: FpsCounter::default(),
            started: Instant::now(),
            title: rom_info::window_title(""),
            sent_title: rom_info::window_title(""),
            settings_file,
            capture: None,
            viewer_open: false,
            snapshot: None,
            snapshot_dirty: false,
            sheet_tex: None,
            map_tex: [None, None],
            debug_open: false,
            trace_open: false,
            inspect: inspect::InspectState::default(),
            window_scale: loaded.window_scale,
            show_fps: loaded.show_fps,
            chrome_h: 60.0,
            resize_pending: true,
        };
        if let Some(path) = rom_path {
            app.load_rom(std::path::Path::new(&path));
        }
        app
    }

    fn save_settings(&self) {
        let settings = Settings {
            volume: self.volume,
            keys: self.key_map,
            window_scale: self.window_scale,
            show_fps: self.show_fps,
        };
        let _ = std::fs::write(&self.settings_file, settings.to_text());
    }

    /// Asks the window manager for the inner size matching the chosen scale (all values in points).
    fn resize_window(&self, ctx: &egui::Context) {
        let ppp = ctx.pixels_per_point();
        let (w, h) = display::window_size(self.window_scale, self.chrome_h * ppp, 480.0 * ppp);
        ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(egui::vec2(
            display::points_for(w.round() as u32, ppp),
            display::points_for(h.round() as u32, ppp),
        )));
    }

    fn load_rom(&mut self, path: &std::path::Path) {
        match std::fs::read(path) {
            Ok(bytes) => {
                self.title = rom_info::window_title(&rom_info::rom_title(&bytes));
                self.path_input = path.display().to_string();
                self.emu
                    .send(Command::SavePath(Some(rom_info::save_path(path))));
                self.emu.send(Command::Load(bytes));
                self.frame.clear();
                self.last_input = Input::default();
                self.status = format!("Loading {}", path.display());
            }
            Err(err) => self.status = format!("Cannot read {}: {err}", path.display()),
        }
    }

    fn toggle_pause(&mut self) {
        self.paused = !self.paused;
        self.emu.send(Command::Pause(self.paused));
    }

    /// Current framebuffer as an egui image (black when no ROM is loaded).
    fn current_image(&self) -> egui::ColorImage {
        let mut image = egui::ColorImage::filled([SCREEN_W, SCREEN_H], egui::Color32::BLACK);
        for (dst, &argb) in image.pixels.iter_mut().zip(self.frame.iter()) {
            let r = ((argb >> 16) & 0xFF) as u8;
            let g = ((argb >> 8) & 0xFF) as u8;
            let b = (argb & 0xFF) as u8;
            *dst = egui::Color32::from_rgb(r, g, b);
        }
        image
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        let dropped = ctx.input(|i| i.raw.dropped_files.clone());
        if let Some(path) = dropped.iter().map(|f| f.path().to_path_buf()).next() {
            self.load_rom(&path);
        }
        while let Ok(frame) = self.emu.frames.try_recv() {
            self.frame = frame;
            let now_ms = u64::try_from(self.started.elapsed().as_millis()).unwrap_or(u64::MAX);
            self.fps.frame(now_ms);
        }
        while let Ok(message) = self.emu.status.try_recv() {
            self.status = message;
        }
        while let Ok(snap) = self.emu.snapshots.try_recv() {
            self.snapshot = Some(snap);
            self.snapshot_dirty = true;
        }
        // Key remapping: the next key pressed is bound to the waiting button.
        if let Some(button) = self.capture {
            let pressed = ctx.input(|i| {
                i.events.iter().find_map(|event| match event {
                    egui::Event::Key {
                        key, pressed: true, ..
                    } => Some(*key),
                    _ => None,
                })
            });
            if let Some(key) = pressed {
                if key != egui::Key::Escape {
                    self.key_map = self.key_map.rebind(button, key);
                    self.save_settings();
                }
                self.capture = None;
            }
        }
        let typing = ctx.text_edit_focused() || self.capture.is_some();
        let pad_state = self.pads.poll();
        // Keyboard drives the buttons only when no text widget has focus (E07_05).
        let keyboard = if typing {
            Input::default()
        } else {
            ctx.input(|i| state_from(|button| i.key_down(self.key_map.key_for(button))))
        };
        let input = merge(keyboard, pad_state);
        if input != self.last_input {
            self.emu.send(Command::Input(input));
            self.last_input = input;
        }
        if !typing {
            if ctx.input(|i| i.key_pressed(egui::Key::P)) {
                self.toggle_pause();
            }
            if ctx.input(|i| i.key_pressed(egui::Key::R)) {
                self.emu.send(Command::Reset);
            }
            if ctx.input(|i| i.key_pressed(egui::Key::N)) {
                self.emu.send(Command::FrameStep);
            }
            if ctx.input(|i| i.key_pressed(egui::Key::F11)) {
                self.fullscreen = !self.fullscreen;
                ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(self.fullscreen));
            }
            let slot_key = [egui::Key::F5, egui::Key::F6, egui::Key::F7, egui::Key::F8]
                .into_iter()
                .find(|key| ctx.input(|i| i.key_pressed(*key)));
            if let Some(key) = slot_key {
                let slot = match key {
                    egui::Key::F5 => 1,
                    egui::Key::F6 => 2,
                    egui::Key::F7 => 3,
                    _ => 4,
                };
                if ctx.input(|i| i.modifiers.shift) {
                    self.emu.send(Command::LoadSlot(slot));
                } else {
                    self.emu.send(Command::SaveSlot(slot));
                }
            }
        }
        if self.title != self.sent_title {
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(self.title.clone()));
            self.sent_title = self.title.clone();
        }
        let image = self.current_image();
        if let Some(texture) = &mut self.texture {
            texture.set(image, egui::TextureOptions::NEAREST);
        } else {
            self.texture = Some(ctx.load_texture("screen", image, egui::TextureOptions::NEAREST));
        }
        let top_panel = egui::Panel::top("menu").show(ui, |ui| {
            egui::MenuBar::new().ui(ui, |ui| {
                ui.menu_button("Emulation", |ui| {
                    let pause_label = if self.paused {
                        "Resume (P)"
                    } else {
                        "Pause (P)"
                    };
                    if ui.button(pause_label).clicked() {
                        self.toggle_pause();
                        ui.close();
                    }
                    if ui.button("Reset (R)").clicked() {
                        self.emu.send(Command::Reset);
                        ui.close();
                    }
                    if ui.button("Frame step (N)").clicked() {
                        self.emu.send(Command::FrameStep);
                        ui.close();
                    }
                });
                ui.menu_button("Audio", |ui| {
                    let slider = egui::Slider::new(&mut self.volume, 0..=100).text("Volume");
                    let response = ui.add(slider);
                    if response.changed() {
                        self.emu.set_volume(self.volume);
                    }
                    if response.drag_stopped() || (response.changed() && !response.dragged()) {
                        self.save_settings();
                    }
                });
                ui.menu_button("Save states", |ui| {
                    for slot in 1..=slots::SLOT_COUNT {
                        if ui
                            .button(format!("Save slot {slot} (F{})", 4 + slot))
                            .clicked()
                        {
                            self.emu.send(Command::SaveSlot(slot));
                            ui.close();
                        }
                        if ui
                            .button(format!("Load slot {slot} (Shift+F{})", 4 + slot))
                            .clicked()
                        {
                            self.emu.send(Command::LoadSlot(slot));
                            ui.close();
                        }
                    }
                });
                ui.menu_button("View", |ui| {
                    if ui
                        .checkbox(&mut self.fullscreen, "Fullscreen (F11)")
                        .changed()
                    {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(self.fullscreen));
                    }
                    ui.menu_button("Window size", |ui| {
                        for scale in display::all_scales() {
                            let selected = self.window_scale == scale;
                            if ui.radio(selected, display::scale_label(scale)).clicked() {
                                self.window_scale = scale;
                                self.save_settings();
                                self.resize_window(&ctx);
                                ui.close();
                            }
                        }
                    });
                    if ui.checkbox(&mut self.show_fps, "Show frame rate").changed() {
                        self.save_settings();
                    }
                    if ui.checkbox(&mut self.trace_open, "CPU trace").changed() {
                        self.emu.send(Command::Trace(self.trace_open));
                        self.emu.send(Command::Viewer(
                            self.viewer_open || self.debug_open || self.trace_open,
                        ));
                    }
                    if ui.checkbox(&mut self.viewer_open, "Tile viewer").changed() {
                        self.emu.send(Command::Viewer(
                            self.viewer_open || self.debug_open || self.trace_open,
                        ));
                    }
                    if ui
                        .checkbox(&mut self.debug_open, "Memory, OAM and palettes")
                        .changed()
                    {
                        self.emu.send(Command::Viewer(
                            self.viewer_open || self.debug_open || self.trace_open,
                        ));
                    }
                });
                ui.menu_button("Keys", |ui| {
                    for button in BUTTONS.iter().copied() {
                        ui.horizontal(|ui| {
                            ui.label(format!("{button:?}"));
                            let text = if self.capture == Some(button) {
                                String::from("press a key...")
                            } else {
                                self.key_map.key_for(button).name().to_string()
                            };
                            if ui.button(text).clicked() {
                                self.capture = Some(button);
                            }
                        });
                    }
                    if ui.button("Defaults").clicked() {
                        self.key_map = KeyMap::default();
                        self.save_settings();
                    }
                });
                ui.separator();
                ui.label("ROM:");
                let field =
                    ui.add(egui::TextEdit::singleline(&mut self.path_input).desired_width(320.0));
                let enter = field.lost_focus() && ctx.input(|i| i.key_pressed(egui::Key::Enter));
                if ui.button("Open").clicked() || enter {
                    let text = self.path_input.trim().trim_matches('"').to_string();
                    self.load_rom(std::path::Path::new(&text));
                }
            });
        });
        let top_height = top_panel.response.rect.height();
        let bottom_panel = egui::Panel::bottom("status").show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(&self.status);
                if let Some(text) = display::overlay_text(self.show_fps, &self.fps.label()) {
                    ui.separator();
                    ui.label(text);
                }
                if self.paused {
                    ui.separator();
                    ui.label("PAUSED");
                }
                if let Some(button) = self.capture {
                    ui.separator();
                    ui.label(format!("Press a key for {button:?} (Escape cancels)"));
                }
            });
            ui.label(
                "Default keys: Arrows, W=A, X=B, Enter=Start, Backspace=Select (Keys menu to change) | P pause, N frame step, R reset, F11 fullscreen | F5-F8 save slots 1-4, Shift+F5-F8 load",
            );
        });
        self.chrome_h = top_height + bottom_panel.response.rect.height();
        egui::CentralPanel::default().show(ui, |ui| {
            let avail = ui.available_size();
            let ppp = ctx.pixels_per_point();
            let fit = scale::integer_scale(
                (avail.x * ppp) as u32,
                (avail.y * ppp) as u32,
                SCREEN_W as u32,
                SCREEN_H as u32,
            );
            let scale = display::effective_scale(self.window_scale, fit);
            let (px_w, px_h) = display::image_size(scale);
            let size = egui::vec2(
                display::points_for(px_w, ppp),
                display::points_for(px_h, ppp),
            );
            if let Some(texture) = &self.texture {
                ui.vertical_centered(|ui| {
                    let response = ui.add(egui::Image::new(egui::load::SizedTexture::new(
                        texture.id(),
                        size,
                    )));
                    if let Some(text) = display::overlay_text(self.show_fps, &self.fps.label()) {
                        let pos = response.rect.left_top() + egui::vec2(4.0, 4.0);
                        let font = egui::FontId::monospace(14.0);
                        let painter = ui.painter();
                        painter.text(
                            pos + egui::vec2(1.0, 1.0),
                            egui::Align2::LEFT_TOP,
                            &text,
                            font.clone(),
                            egui::Color32::BLACK,
                        );
                        painter.text(
                            pos,
                            egui::Align2::LEFT_TOP,
                            &text,
                            font,
                            egui::Color32::WHITE,
                        );
                    }
                });
            }
        });
        if self.resize_pending {
            self.resize_pending = false;
            self.resize_window(&ctx);
        }
        if self.viewer_open {
            if self.snapshot_dirty {
                if let Some(snap) = &self.snapshot {
                    let sheet = viewer::to_color_image(
                        &viewer::sheet_shades(snap),
                        viewer::SHEET_W,
                        viewer::SHEET_H,
                    );
                    viewer::upload(&ctx, &mut self.sheet_tex, "tile_sheet", sheet);
                    for (which, slot) in self.map_tex.iter_mut().enumerate() {
                        let map = viewer::to_color_image(
                            &viewer::map_shades(snap, which),
                            viewer::MAP_PX,
                            viewer::MAP_PX,
                        );
                        viewer::upload(&ctx, slot, &format!("tile_map_{which}"), map);
                    }
                }
                self.snapshot_dirty = false;
            }
            let mut open = true;
            egui::Window::new("Tile viewer")
                .open(&mut open)
                .show(&ctx, |ui| {
                    egui::ScrollArea::both().show(ui, |ui| {
                        ui.horizontal_top(|ui| {
                            if let Some(tex) = &self.sheet_tex {
                                ui.vertical(|ui| {
                                    ui.label("Tiles (current BGP)");
                                    let size = egui::vec2(
                                        (viewer::SHEET_W * viewer::SHEET_SCALE) as f32,
                                        (viewer::SHEET_H * viewer::SHEET_SCALE) as f32,
                                    );
                                    ui.add(egui::Image::new(egui::load::SizedTexture::new(
                                        tex.id(),
                                        size,
                                    )));
                                });
                            }
                            ui.vertical(|ui| {
                                for (which, slot) in self.map_tex.iter().enumerate() {
                                    if let Some(tex) = slot {
                                        let base = if which == 0 { "$9800" } else { "$9C00" };
                                        ui.label(format!("Tile map {base}"));
                                        let size = egui::vec2(
                                            (viewer::MAP_PX * viewer::MAP_SCALE) as f32,
                                            (viewer::MAP_PX * viewer::MAP_SCALE) as f32,
                                        );
                                        ui.add(egui::Image::new(egui::load::SizedTexture::new(
                                            tex.id(),
                                            size,
                                        )));
                                    }
                                }
                            });
                        });
                    });
                });
            if !open {
                self.viewer_open = false;
                self.emu
                    .send(Command::Viewer(self.debug_open || self.trace_open));
            }
        }
        if self.debug_open && !inspect::show_window(&ctx, &mut self.inspect, self.snapshot.as_ref())
        {
            self.debug_open = false;
            self.emu
                .send(Command::Viewer(self.viewer_open || self.trace_open));
        }
        if self.trace_open && !tracewin::show_window(&ctx, self.snapshot.as_ref()) {
            self.trace_open = false;
            self.emu.send(Command::Trace(false));
            self.emu
                .send(Command::Viewer(self.viewer_open || self.debug_open));
        }
        ctx.request_repaint();
    }
}

fn main() -> eframe::Result<()> {
    let rom_path = std::env::args().nth(1);
    let options = eframe::NativeOptions {
        renderer: eframe::Renderer::Wgpu,
        ..Default::default()
    };
    eframe::run_native(
        "Puce8 GameBoy",
        options,
        Box::new(move |_cc| Ok(Box::new(App::new(rom_path)))),
    )
}
