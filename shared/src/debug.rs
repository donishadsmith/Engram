use crate::render::{Frame, to_rgba};
use egui::{CentralPanel, TextureHandle, TextureId, TextureOptions, Vec2};
use std::{
    fmt::Arguments,
    fs::File,
    io::{BufWriter, Write},
};

pub struct Trace {
    pub file: BufWriter<File>,
    pub limit: u64,
    pub count: u64,
}

impl Trace {
    pub fn record(&mut self, write: impl FnOnce(&mut dyn Write)) {
        if self.count <= self.limit {
            write(&mut self.file);
        }
    }

    pub fn dump(&mut self, arguments: Arguments) {
        self.record(|write| {
            let _ = write.write_fmt(arguments);
            let _ = writeln!(write);
        });
    }
}

impl Drop for Trace {
    fn drop(&mut self) {
        let _ = self.file.flush();
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum DebugPage {
    Audio,
    Video,
}

impl DebugPage {
    pub fn to_str(self) -> &'static str {
        match self {
            DebugPage::Audio => "Audio",
            DebugPage::Video => "Video",
        }
    }
}

pub fn get_texture_id(
    texture_handle: &mut Option<TextureHandle>,
    egui_ctx: &egui::Context,
    frame: &mut Frame,
    id: String,
) -> TextureId {
    if texture_handle.is_none() || frame.dimensions_changed || frame.buffer_changed {
        let image =
            egui::ColorImage::from_rgba_unmultiplied([frame.width, frame.height], &to_rgba(frame));

        match texture_handle {
            Some(texture) => texture.set(image, TextureOptions::NEAREST),
            None => {
                *texture_handle = Some(egui_ctx.load_texture(id, image, TextureOptions::NEAREST))
            }
        }

        frame.buffer_changed = false;
        frame.dimensions_changed = false;
    }

    texture_handle.as_ref().unwrap().id()
}

pub fn compute_size(size: Vec2, frame: &Frame) -> Vec2 {
    let scale = (size.x / frame.width as f32)
        .min(size.y / frame.height as f32)
        .floor()
        .max(1.0);

    egui::vec2(frame.width as f32 * scale, frame.height as f32 * scale)
}

pub fn create_game_screen(
    texture_handle: &mut Option<TextureHandle>,
    egui_ctx: &egui::Context,
    frame: &mut Frame,
    id: String,
) {
    let texture_id = get_texture_id(texture_handle, egui_ctx, frame, id);

    CentralPanel::default()
        .frame(egui::Frame::NONE)
        .show(egui_ctx, |ui| {
            let size = compute_size(ui.available_size(), frame);
            ui.centered_and_justified(|ui| {
                ui.image((texture_id, size));
            });
        });
}
