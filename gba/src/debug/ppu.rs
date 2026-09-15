use crate::components::gba::GBA;
use egui::{Color32, RichText, Sense, TextureHandle, TopBottomPanel, Window, vec2};
use shared::{
    debug::{compute_size, create_game_screen, get_texture_id},
    render::{Frame, PixelFormat, rgb555_to_rgb888},
};
use std::{
    array::from_fn,
    mem::{swap, take},
};

#[derive(Clone, Copy, PartialEq)]
enum PaletteType {
    Background = 0,
    Sprite = 256,
}

fn palette_grid(ui: &mut egui::Ui, id: &str, palette: &[Color32]) {
    egui::Grid::new(id)
        .spacing([2.0, 2.0])
        .min_col_width(0.0)
        .min_row_height(0.0)
        .show(ui, |ui| {
            for (index, &color) in palette.iter().enumerate() {
                let (rect, response) = ui.allocate_exact_size(vec2(20.0, 20.0), Sense::hover());
                ui.painter().rect_filled(rect, 1.0, color);

                response.on_hover_text(format!(
                    "Palette Index={:3x}\n#{:2x}{:2x}{:2x}",
                    index,
                    color.r(),
                    color.g(),
                    color.b()
                ));

                if (index + 1) % 16 == 0 {
                    ui.end_row();
                }
            }
        });
}

pub struct PpuDebugger {
    frozen: bool,
    texture: Option<TextureHandle>,
    palette: [u8; 0x400],
    palette_tab: PaletteType,
    show_palettes: bool,
    background_frames: [Frame; 4],
    background_textures: Vec<Option<TextureHandle>>,
}

impl PpuDebugger {
    pub fn new() -> Self {
        Self {
            frozen: false,
            texture: None,
            palette: [0; 0x400],
            show_palettes: true,
            palette_tab: PaletteType::Background,
            background_frames: from_fn(|_| Frame {
                pixels: Box::new([0; 240 * 160]),
                width: 240,
                height: 160,
                pixel_format: PixelFormat::Rgb555,
            }),
            background_textures: vec![None; 4],
        }
    }

    pub fn close(&mut self) {
        self.frozen = false;
        self.show_palettes = true;
    }

    pub fn show_ui(&mut self, egui_ctx: &egui::Context, gba: &mut GBA) {
        if !self.frozen {
            self.palette = *gba.bus.ppu.palette_ram.clone();

            if take(&mut gba.bus.ppu.debug_frame_ready) {
                swap(&mut gba.bus.ppu.debug_frontend, &mut self.background_frames);
            }
        }

        let background_palettes = self.get_pallete(PaletteType::Background);
        let sprite_palettes = self.get_pallete(PaletteType::Sprite);

        // TODO: figure out the organization for this
        TopBottomPanel::top("Header").show(egui_ctx, |ui| {
            egui::Grid::new("Header").show(ui, |ui| {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let (text, hover) = if self.frozen {
                        (
                            RichText::new("PAUSED").strong().color(Color32::YELLOW),
                            "Click to resume",
                        )
                    } else {
                        (
                            RichText::new("LIVE").strong().color(Color32::LIGHT_GREEN),
                            "Click to pause",
                        )
                    };

                    ui.allocate_ui_with_layout(
                        egui::vec2(50.0, ui.spacing().interact_size.y),
                        egui::Layout::left_to_right(egui::Align::Center),
                        |ui| {
                            if ui
                                .add(egui::Label::new(text).sense(egui::Sense::click()))
                                .on_hover_text(hover)
                                .on_hover_cursor(egui::CursorIcon::PointingHand)
                                .clicked()
                            {
                                self.freeze();
                            }
                        },
                    );
                });

                ui.end_row();
            })
        });

        Window::new("Palettes")
            .default_open(false)
            .open(&mut self.show_palettes)
            .collapsible(true)
            .default_size([260.0, 250.0])
            .resizable(false)
            .show(egui_ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.selectable_value(
                        &mut self.palette_tab,
                        PaletteType::Background,
                        "Background",
                    );
                    ui.selectable_value(&mut self.palette_tab, PaletteType::Sprite, "Sprite");
                });

                ui.separator();

                match self.palette_tab {
                    PaletteType::Background => {
                        palette_grid(ui, "Background Palette", &background_palettes)
                    }
                    PaletteType::Sprite => palette_grid(ui, "Sprite Palette", &sprite_palettes),
                }
            });

        Window::new("Background")
            .collapsible(true)
            .anchor(egui::Align2::RIGHT_TOP, egui::vec2(0.0, 10.0))
            .default_size([260.0, 250.0])
            .resizable(false)
            .show(egui_ctx, |ui| {
                let size_vec = egui::vec2(260.0, 250.0);
                ui.separator();

                for index in 0..4 {
                    let title = format!("Background {}", index);
                    ui.label(&title);

                    let size = compute_size(size_vec, &self.background_frames[index]);
                    let texture_id = get_texture_id(
                        &mut self.background_textures[index],
                        egui_ctx,
                        &self.background_frames[index],
                        title,
                    );

                    ui.image((texture_id, size));
                    ui.separator();
                }
            });

        create_game_screen(
            &mut self.texture,
            egui_ctx,
            &gba.bus.ppu.frontend,
            "Game Screen".to_string(),
        );
    }

    fn get_pallete(&self, palette_type: PaletteType) -> Vec<Color32> {
        let start_index = palette_type as usize;
        let end_index = start_index + 256;
        let mut palette = Vec::with_capacity(256);

        for index in start_index..end_index {
            let rg555 = rgb555_to_rgb888(u16::from_le_bytes([
                self.palette[index * 2],
                self.palette[index * 2 + 1],
            ]));

            palette.push(Color32::from_rgb(rg555[0], rg555[1], rg555[2]));
        }

        palette
    }

    pub fn freeze(&mut self) {
        self.frozen = match self.frozen {
            true => false,
            false => true,
        }
    }
}
