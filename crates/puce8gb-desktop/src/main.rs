//! Desktop frontend: an eframe window showing the core framebuffer (E07_01).
mod scale;

use eframe::egui;
use puce8gb_core::{Dmg, Machine};

const SCREEN_W: usize = 160;
const SCREEN_H: usize = 144;

struct App {
    machine: Option<Dmg>,
    texture: Option<egui::TextureHandle>,
    status: String,
}

impl App {
    fn new(rom_path: Option<String>) -> Self {
        let mut app = App {
            machine: None,
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
            Ok(bytes) => match Dmg::new(&bytes) {
                Ok(dmg) => {
                    self.machine = Some(dmg);
                    self.status = format!("Loaded {}", path.display());
                }
                Err(err) => self.status = format!("ROM rejected: {err:?}"),
            },
            Err(err) => self.status = format!("Cannot read {}: {err}", path.display()),
        }
    }

    /// Current framebuffer as an egui image (black when no ROM is loaded).
    fn current_image(&self) -> egui::ColorImage {
        let mut image = egui::ColorImage::filled([SCREEN_W, SCREEN_H], egui::Color32::BLACK);
        if let Some(machine) = &self.machine {
            for (dst, &argb) in image.pixels.iter_mut().zip(machine.framebuffer()) {
                let r = ((argb >> 16) & 0xFF) as u8;
                let g = ((argb >> 8) & 0xFF) as u8;
                let b = (argb & 0xFF) as u8;
                *dst = egui::Color32::from_rgb(r, g, b);
            }
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
        if let Some(machine) = self.machine.as_mut() {
            machine.run_frame(); // temporary: the core runs on the UI thread, replaced in E07_02
        }
        let image = self.current_image();
        if let Some(texture) = &mut self.texture {
            texture.set(image, egui::TextureOptions::NEAREST);
        } else {
            self.texture = Some(ctx.load_texture("screen", image, egui::TextureOptions::NEAREST));
        }
        egui::Panel::bottom("status").show(ui, |ui| {
            ui.label(&self.status);
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
