use crate::components::gba::GBA;
use egui::{Color32, RichText, Sense, TextureHandle, TopBottomPanel, Window, vec2};
use shared::{debug::create_game_screen, render::rgb555_to_rgb888};

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
}

impl PpuDebugger {
    pub fn new() -> Self {
        Self {
            frozen: false,
            texture: None,
            palette: [0; 0x400],
            show_palettes: true,
            palette_tab: PaletteType::Background,
        }
    }

    pub fn close(&mut self) {
        self.frozen = false;
        self.show_palettes = true;
    }

    pub fn show_ui(&mut self, egui_ctx: &egui::Context, gba: &GBA) {
        if !self.frozen {
            self.palette = *gba.bus.ppu.palette_ram.clone();
        }

        let background_palettes = self.get_pallete(PaletteType::Background);
        let sprite_palettes = self.get_pallete(PaletteType::Sprite);

        // TODO: figure out the organization for this
        TopBottomPanel::top("Registers").show(egui_ctx, |ui| {
            ui.heading("Registers").highlight();
            ui.separator();

            egui::Grid::new("Registers").show(ui, |ui| {
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
                                .add(egui::Label::new(text.clone()).sense(egui::Sense::click()))
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
            .open(&mut self.show_palettes)
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

        create_game_screen(&mut self.texture, egui_ctx, &gba.bus.ppu.frontend);
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
