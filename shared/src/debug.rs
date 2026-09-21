use crate::render::{Frame, to_rgba};
use egui::{CentralPanel, TextureHandle, TextureId, TextureOptions, Vec2};

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
    frame: &Frame,
    id: String,
) -> TextureId {
    let image =
        egui::ColorImage::from_rgba_unmultiplied([frame.width, frame.height], &to_rgba(&frame));
    let texture = texture_handle
        .get_or_insert_with(|| egui_ctx.load_texture(id, image.clone(), TextureOptions::NEAREST));

    texture.set(image, TextureOptions::NEAREST);

    texture.id()
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
    frame: &Frame,
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
