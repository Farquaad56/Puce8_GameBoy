//! Desktop frontend: an eframe window showing the core framebuffer (E07_01).
mod audio;
mod emu;
mod pacing;
mod resample;
mod scale;

use eframe::egui;
use emu::{Command, EmuHandle};
use puce8gb_core::Input;

const SCREEN_W: usize = 160;
const SCREEN_H: usize = 144;

struct App {
    emu: EmuHandle,
    frame: Vec<u32>,
    paused: bool,
    last_input: Input,
    texture: Option<egui::TextureHandle>,
    status: String,
}

impl App {
    fn new(rom_path: Option<String>) -> Self {
        let mut app = App {
            emu: EmuHandle::spawn(),
            frame: Vec::new(),
            paused: false,
            last_input: Input::default(),
            texture: None,
            status: String::from("Drop a ROM file on the window"),
        };
        if let Some(path) = rom_path {
            app.load_rom(std::path::Path::new(&path));
        }
        app
    }

    fn load_rom(&mut self, path: &std::path::Path) {
        match std::fs::read(path) {
            Ok(bytes) => {
                self.emu.send(Command::Load(bytes));
                self.frame.clear();
                self.last_input = Input::default();
                self.status = format!("Loading {}", path.display());
            }
            Err(err) => self.status = format!("Cannot read {}: {err}", path.display()),
        }
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
        }
        while let Ok(message) = self.emu.status.try_recv() {
            self.status = message;
        }
        let input = ctx.input(|i| Input {
            up: i.key_down(egui::Key::ArrowUp),
            down: i.key_down(egui::Key::ArrowDown),
            left: i.key_down(egui::Key::ArrowLeft),
            right: i.key_down(egui::Key::ArrowRight),
            a: i.key_down(egui::Key::W),
            b: i.key_down(egui::Key::X),
            start: i.key_down(egui::Key::Enter),
            select: i.key_down(egui::Key::Backspace),
        });
        if input != self.last_input {
            self.emu.send(Command::Input(input));
            self.last_input = input;
        }
        if ctx.input(|i| i.key_pressed(egui::Key::P)) {
            self.paused = !self.paused;
            self.emu.send(Command::Pause(self.paused));
        }
        if ctx.input(|i| i.key_pressed(egui::Key::R)) {
            self.emu.send(Command::Reset);
        }
        if ctx.input(|i| i.key_pressed(egui::Key::N)) {
            self.emu.send(Command::FrameStep);
        }
        let image = self.current_image();
        if let Some(texture) = &mut self.texture {
            texture.set(image, egui::TextureOptions::NEAREST);
        } else {
            self.texture = Some(ctx.load_texture("screen", image, egui::TextureOptions::NEAREST));
        }
        egui::Panel::bottom("status").show(ui, |ui| {
            ui.label(&self.status);
            ui.label(
                "Arrows, W=A, X=B, Enter=Start, Backspace=Select | P pause, N frame step, R reset",
            );
        });
        egui::CentralPanel::default().show(ui, |ui| {
            let avail = ui.available_size();
            let scale = scale::integer_scale(
                avail.x as u32,
                avail.y as u32,
                SCREEN_W as u32,
                SCREEN_H as u32,
            );
            let size = egui::vec2(
                (SCREEN_W as u32 * scale) as f32,
                (SCREEN_H as u32 * scale) as f32,
            );
            if let Some(texture) = &self.texture {
                ui.vertical_centered(|ui| {
                    ui.add(egui::Image::new(egui::load::SizedTexture::new(
                        texture.id(),
                        size,
                    )));
                });
            }
        });
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
