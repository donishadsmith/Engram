use crate::render::{Frame, to_rgba};
use egui::{CentralPanel, TextureHandle, TextureOptions};

pub const DEBUG_PAGES: [DebugPage; 2] = [DebugPage::Audio, DebugPage::Video];

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

pub fn create_game_screen(
    texture_handle: &mut Option<TextureHandle>,
    egui_ctx: &egui::Context,
    frame: &Frame,
) {
    let image =
        egui::ColorImage::from_rgba_unmultiplied([frame.width, frame.height], &to_rgba(&frame));
    let texture = texture_handle.get_or_insert_with(|| {
        egui_ctx.load_texture("Game Screen", image.clone(), TextureOptions::NEAREST)
    });

    texture.set(image, TextureOptions::NEAREST);
    let texture_id = texture.id();

    CentralPanel::default().show(egui_ctx, |ui| {
        let size = ui.available_size();
        let scale = (size.x / frame.width as f32)
            .min(size.y / frame.height as f32)
            .floor()
            .max(1.0);

        let size = egui::vec2(frame.width as f32 * scale, frame.height as f32 * scale);
        ui.centered_and_justified(|ui| {
            ui.image((texture_id, size));
        });
    });
}
