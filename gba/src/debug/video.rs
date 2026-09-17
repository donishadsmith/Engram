use crate::components::{gba::GBA, ppu::sprites::SpriteAttributes};
use egui::{Color32, RichText, Sense, TextureHandle, Window, vec2};
use shared::{
    debug::{compute_size, create_game_screen, get_texture_id},
    render::{Frame, PixelFormat, rgb555_to_rgb888},
    traits::BitOps,
};
use std::{
    array::from_fn,
    fmt::Display,
    mem::{swap, take},
};

const GRID_COLUMNS: f32 = 16.0;
const GRID_SPACING: f32 = 2.0;

#[derive(Clone, Copy, PartialEq)]
enum RenderTab {
    Background,
    Sprite,
    Information,
    Palette,
}

#[derive(Clone, Copy, PartialEq)]
enum PaletteType {
    Background = 0,
    Sprite = 256,
}

fn compute_cell_width(available_width: f32) -> f32 {
    ((available_width - GRID_SPACING * (GRID_COLUMNS - 1.0)) / GRID_COLUMNS).max(4.0)
}

fn palette_grid(ui: &mut egui::Ui, id: &str, palette: &[Color32]) {
    let size = compute_cell_width(ui.available_width());

    egui::Grid::new(id)
        .spacing([GRID_SPACING, GRID_SPACING])
        .min_col_width(0.0)
        .min_row_height(0.0)
        .show(ui, |ui| {
            for (index, &color) in palette.iter().enumerate() {
                let (rect, response) = ui.allocate_exact_size(vec2(size, size), Sense::hover());
                ui.painter().rect_filled(rect, 1.0, color);

                response.on_hover_text(format!(
                    "Palette Index={:3x}\n#{:2x}{:2x}{:2x}",
                    index,
                    color.r(),
                    color.g(),
                    color.b()
                ));

                if (index + 1) % GRID_COLUMNS as usize == 0 {
                    ui.end_row();
                }
            }
        });
}

fn horizontal_text<T: Display>(ui: &mut egui::Ui, title: &'static str, state: T) {
    ui.horizontal(|ui| {
        ui.label(format!("{}: ", title));
        ui.label(state.to_string());
    });
}

pub struct PpuDebugger {
    frozen: bool,
    texture: Option<TextureHandle>,
    palette: [u8; 0x400],
    palette_tab: PaletteType,
    background_frames: [Frame; 4],
    background_textures: Vec<Option<TextureHandle>>,
    sprites: Vec<SpriteAttributes>,
    sprite_textures: Vec<Option<TextureHandle>>,
    render_tab: RenderTab,
    current_mode: u8,
    bg_on: [bool; 4],
    interrupt_flag: u16,
    vcount: u8,
    dispstat: u16,
    dispcnt: u16,
}

impl PpuDebugger {
    pub fn new() -> Self {
        Self {
            frozen: false,
            texture: None,
            palette: [0; 0x400],
            palette_tab: PaletteType::Background,
            background_frames: from_fn(|_| Frame {
                pixels: Box::new([0; 240 * 160]),
                width: 240,
                height: 160,
                pixel_format: PixelFormat::Rgb555,
            }),
            background_textures: vec![None; 4],
            sprites: Vec::with_capacity(128),
            sprite_textures: vec![None; 128],
            render_tab: RenderTab::Background,
            current_mode: 0,
            bg_on: [false; 4],
            interrupt_flag: 0,
            vcount: 0,
            dispcnt: 0,
            dispstat: 0,
        }
    }

    pub fn close(&mut self, gba: &mut GBA) {
        self.frozen = false;
        self.palette_tab = PaletteType::Background;
        gba.bus.ppu.transparant_sprite_background = true;
        gba.bus.ppu.transparant_background = false;
        self.current_mode = 0;
        self.bg_on = [false; 4];
        self.interrupt_flag = 0;
        self.vcount = 0;
        self.dispcnt = 0;
        self.dispstat = 0;
    }

    pub fn show_ui(&mut self, egui_ctx: &egui::Context, gba: &mut GBA) {
        if !self.frozen {
            self.palette = *gba.bus.ppu.palette_ram.clone();

            if take(&mut gba.bus.ppu.debug_frame_ready) {
                swap(&mut gba.bus.ppu.debug_frontend, &mut self.background_frames);
            }

            if take(&mut gba.bus.ppu.sprites_ready) {
                swap(&mut gba.bus.ppu.sprites_data, &mut self.sprites);
            }

            self.current_mode = gba.bus.ppu.current_mode();

            for bg_id in 0..4 {
                self.bg_on[bg_id] = gba.bus.ppu.dispcnt.is_set(8 + bg_id);
            }

            self.interrupt_flag = gba.bus.interrupt_flag_copy;
            self.vcount = gba.bus.ppu.vcount;
            self.dispstat = gba.bus.ppu.dispstat;
            self.dispcnt = gba.bus.ppu.dispcnt;
        }

        let background_palettes = self.get_pallete(PaletteType::Background);
        let sprite_palettes = self.get_pallete(PaletteType::Sprite);

        // TODO: figure out the organization for this + add registers
        Window::new("Video Debugger")
            .resizable(true)
            .collapsible(true)
            .default_size([360.0, 820.0])
            .min_width(120.0)
            .default_pos(egui_ctx.screen_rect().right_top() + vec2(-370.0, 10.0))
            .show(egui_ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                        ui.label(RichText::new("Debugger Status:").strong());

                        let (text, hover) = if self.frozen {
                            (
                                RichText::new("FROZEN").strong().color(Color32::YELLOW),
                                "Click to resume debugger",
                            )
                        } else {
                            (
                                RichText::new("LIVE").strong().color(Color32::LIGHT_GREEN),
                                "Click to pause debugger",
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

                    ui.separator();

                    ui.selectable_value(
                        &mut self.render_tab,
                        RenderTab::Information,
                        "Information",
                    );
                    ui.selectable_value(&mut self.render_tab, RenderTab::Background, "Background");
                    ui.selectable_value(&mut self.render_tab, RenderTab::Sprite, "Sprite");
                    ui.selectable_value(&mut self.render_tab, RenderTab::Palette, "Palette");
                });

                ui.separator();

                egui::ScrollArea::vertical().show(ui, |ui| match self.render_tab {
                    RenderTab::Background => {
                        ui.horizontal(|ui| {
                            ui.checkbox(
                                &mut gba.bus.ppu.transparant_background,
                                "Transparent Background",
                            );
                        });

                        self.show_backgrounds(ui)
                    }
                    RenderTab::Sprite => {
                        ui.horizontal(|ui| {
                            ui.checkbox(
                                &mut gba.bus.ppu.transparant_sprite_background,
                                "Transparent Sprite Background",
                            );
                        });

                        self.show_sprites(ui)
                    }
                    RenderTab::Information => {
                        // TODO: do the rest of this later
                        ui.vertical(|ui| {
                            horizontal_text(ui, "Current Mode", self.current_mode);
                            self.interrupt_text(ui, "VBlank Interrupt", 0);
                            self.interrupt_text(ui, "HBlank Interrupt", 1);
                            self.interrupt_text(ui, "VCounter Interrupt", 2);
                            horizontal_text(ui, "Vcount Line", self.vcount);
                            horizontal_text(ui, "Dispstat", format!("{:016b}", self.dispstat));
                            horizontal_text(ui, "Dispcnt", format!("{:016b}", self.dispcnt));
                        });
                    }
                    RenderTab::Palette => {
                        self.show_palettes(ui, &background_palettes, &sprite_palettes)
                    }
                });
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

    fn show_palettes(
        &mut self,
        ui: &mut egui::Ui,
        background_palettes: &[Color32],
        sprite_palettes: &[Color32],
    ) {
        ui.horizontal(|ui| {
            ui.selectable_value(&mut self.palette_tab, PaletteType::Background, "Background");
            ui.selectable_value(&mut self.palette_tab, PaletteType::Sprite, "Sprite");
        });

        ui.separator();

        match self.palette_tab {
            PaletteType::Background => palette_grid(ui, "Background Palette", &background_palettes),
            PaletteType::Sprite => palette_grid(ui, "Sprite Palette", &sprite_palettes),
        }
    }

    fn show_backgrounds(&mut self, ui: &mut egui::Ui) {
        let width = ui.available_width();
        let size_vec = vec2(width, width * 160.0 / 240.0);

        for index in 0..4 {
            let title = format!("Background {}:", index);
            let state = if self.bg_on[index] {
                RichText::new("on").color(Color32::LIGHT_GREEN)
            } else {
                RichText::new("off").color(Color32::RED)
            };

            ui.horizontal(|ui| {
                ui.label(&title);
                ui.label(state);
            });

            let size = compute_size(size_vec, &self.background_frames[index]);
            let texture_id = get_texture_id(
                &mut self.background_textures[index],
                ui.ctx(),
                &self.background_frames[index],
                title,
            );

            ui.image((texture_id, size));
            ui.separator();
        }
    }

    fn show_sprites(&mut self, ui: &mut egui::Ui) {
        let cell_side = compute_cell_width(ui.available_width());
        let cell = vec2(cell_side, cell_side);

        egui::Grid::new("Sprite Grid")
            .spacing([GRID_SPACING, GRID_SPACING])
            .min_col_width(0.0)
            .min_row_height(0.0)
            .show(ui, |ui| {
                for index in 0..128 {
                    let (rect, response) = ui.allocate_exact_size(cell, Sense::hover());

                    if let Some(frame) = &self.sprites[index].frame {
                        if frame.width > 0 && frame.height > 0 {
                        let scale = (cell.x / frame.width as f32).min(cell.y / frame.height as f32);
                            let size = vec2(frame.width as f32 * scale, frame.height as f32 * scale);
                            let texture_id = get_texture_id(
                                &mut self.sprite_textures[index],
                                ui.ctx(),
                                frame,
                                format!("Sprite {index}"),
                            );

                            let image_rect = egui::Rect::from_center_size(rect.center(), size);

                            egui::Image::new((texture_id, size)).paint_at(ui, image_rect);
                        }
                    }

                    let sprite = &self.sprites[index];
                    let mode = if sprite.matrix.is_some() {
                        "affine"
                    } else {
                        "normal"
                    };
                    let double = sprite.matrix.is_some() && sprite.double_size;

                    response.on_hover_text(format!(
                        "Sprite #{index}\ndimension: {}x{}\ncoordinate: ({}, {})\nbounding box: {}x{}\ntile: {}\npriority: {}\npalette: {}\nhorizontal flip: {}\nvertical flip: {}\ndisabled: {}\nmode: {}\ndouble size: {}",
                        sprite.dimension.width,
                        sprite.dimension.height,
                        sprite.coordinate.x,
                        sprite.coordinate.y,
                        sprite.bounding_box.width,
                        sprite.bounding_box.height,
                        sprite.tile,
                        sprite.priority,
                        sprite.palette_bank,
                        sprite.horizontal_flip,
                        sprite.vertical_flip,
                        sprite.disabled,
                        mode,
                        double,
                    ));

                if (index + 1) % GRID_COLUMNS as usize == 0 {
                        ui.end_row();
                    }
                }
            });
    }

    fn interrupt_text(&self, ui: &mut egui::Ui, text: &'static str, bit: usize) {
        let state = if self.interrupt_flag.is_set(bit) {
            RichText::new("pending").color(Color32::YELLOW)
        } else {
            RichText::new("idle").weak()
        };

        ui.horizontal(|ui| {
            ui.label(format!("{}: ", text));
            ui.label(state);
        });
    }

    pub fn freeze(&mut self) {
        self.frozen = !self.frozen;
    }
}
