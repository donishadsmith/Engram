use egui::{CentralPanel, SidePanel, TextureHandle, TextureOptions};
use egui_plot::{HLine, Line, Plot};
use macroquad::input::{KeyCode, get_keys_pressed};
use std::collections::VecDeque;

use crate::components::gba::GBA;
use shared::render::to_rgba;
enum AudioChannel {
    Channel1 = 0,
    Channel2 = 1,
    Channel3 = 2,
    Channel4 = 3,
    FifoA = 4,
    FifoB = 5,
}

pub struct AudioSamples {
    fifo_a: VecDeque<i8>,
    fifo_b: VecDeque<i8>,
}

impl AudioSamples {
    fn new() -> Self {
        Self {
            fifo_a: VecDeque::new(),
            fifo_b: VecDeque::new(),
        }
    }
}

pub struct AudioOccupancy {
    fifo_a: VecDeque<u8>,
    fifo_b: VecDeque<u8>,
}

impl AudioOccupancy {
    fn new() -> Self {
        Self {
            fifo_a: VecDeque::new(),
            fifo_b: VecDeque::new(),
        }
    }
}

pub struct AudioDebugger {
    pub visible: bool,
    pub frozen: bool,
    samples: AudioSamples,
    occupancy: AudioOccupancy,
    mute: [bool; 6],
    texture: Option<TextureHandle>,
}

impl AudioDebugger {
    pub fn new() -> Self {
        Self {
            visible: false,
            frozen: false,
            samples: AudioSamples::new(),
            occupancy: AudioOccupancy::new(),
            mute: [false; 6],
            texture: None,
        }
    }

    pub fn turn_on(&mut self, gba: &mut GBA) {
        if get_keys_pressed().contains(&KeyCode::F12) {
            self.visible = match self.visible {
                true => {
                    self.frozen = false;
                    self.mute = [false; 6];
                    self.mute_channels(gba);
                    false
                }
                false => true,
            }
        }
    }

    pub fn freeze(&mut self) {
        if get_keys_pressed().contains(&KeyCode::Space) {
            self.frozen = match self.frozen {
                true => false,
                false => true,
            }
        }
    }

    // TODO: Add useful register states + psg channels when iplemented
    pub fn show_ui(&mut self, gba: &mut GBA) {
        if !self.visible {
            return;
        }

        if !self.frozen {
            for (sample, occupancy) in gba
                .bus
                .apu
                .fifo_a
                .history
                .drain(..)
                .zip(gba.bus.apu.fifo_a.occupancy.drain(..))
            {
                if self.samples.fifo_a.len() == 2048 {
                    self.samples.fifo_a.pop_front();
                    self.occupancy.fifo_a.pop_front();
                }

                self.samples.fifo_a.push_back(sample as i8);
                self.occupancy.fifo_a.push_back(occupancy as u8);
            }

            for (sample, occupancy) in gba
                .bus
                .apu
                .fifo_b
                .history
                .drain(..)
                .zip(gba.bus.apu.fifo_b.occupancy.drain(..))
            {
                if self.samples.fifo_b.len() == 2048 {
                    self.samples.fifo_b.pop_front();
                    self.occupancy.fifo_b.pop_front();
                }

                self.samples.fifo_b.push_back(sample as i8);
                self.occupancy.fifo_b.push_back(occupancy as u8);
            }
        }

        egui_macroquad::ui(|egui_ctx| {
            SidePanel::right("FIFO Audio").show(egui_ctx, |ui| {
                ui.heading("FIFO Audio").highlight();
                ui.separator();

                let fifo_a_samples = Line::new(
                    "FIFO A Samples",
                    self.samples
                        .fifo_a
                        .iter()
                        .enumerate()
                        .map(|(index, &sample)| [index as f64, sample as f64])
                        .collect::<Vec<[f64; 2]>>(),
                );

                let fifo_a_occupancy = Line::new(
                    "FIFO A Occupancy",
                    self.occupancy
                        .fifo_a
                        .iter()
                        .enumerate()
                        .map(|(index, &size)| [index as f64, size as f64])
                        .collect::<Vec<[f64; 2]>>(),
                );

                ui.monospace(format!("FIFO A Samples"));
                Plot::new("FIFO A Samples")
                    .view_aspect(3.0)
                    .include_y(-128.0)
                    .include_y(127.0)
                    .show(ui, |plot_ui| {
                        plot_ui.line(fifo_a_samples);
                    });

                ui.monospace(format!("FIFO A Occupancy"));
                Plot::new("FIFO A Occupancy")
                    .view_aspect(3.0)
                    .include_y(0.0)
                    .include_y(32.0)
                    .show(ui, |plot_ui| {
                        plot_ui.line(fifo_a_occupancy);
                        plot_ui.hline(HLine::new("FIFO A Occupancy", 16.0));
                    });

                let fifo_b_samples = Line::new(
                    "FIFO B Samples",
                    self.samples
                        .fifo_b
                        .iter()
                        .enumerate()
                        .map(|(index, &sample)| [index as f64, sample as f64])
                        .collect::<Vec<[f64; 2]>>(),
                );

                let fifo_b_occupancy = Line::new(
                    "FIFO B Occupancy",
                    self.occupancy
                        .fifo_b
                        .iter()
                        .enumerate()
                        .map(|(index, &size)| [index as f64, size as f64])
                        .collect::<Vec<[f64; 2]>>(),
                );

                ui.monospace(format!("FIFO B Samples"));
                Plot::new("FIFO B Samples")
                    .view_aspect(3.0)
                    .include_y(-128.0)
                    .include_y(120.0)
                    .show(ui, |plot_ui| {
                        plot_ui.line(fifo_b_samples);
                    });

                ui.monospace(format!("FIFO B Occupancy"));
                Plot::new("FIFO B Occupancy")
                    .view_aspect(3.0)
                    .include_y(0.0)
                    .include_y(32.0)
                    .show(ui, |plot_ui| {
                        plot_ui.line(fifo_b_occupancy);
                        plot_ui.hline(HLine::new("FIFO B Occupancy", 16.0));
                    });

                ui.heading("Mute FIFO Channels").highlight();
                ui.separator();
                let text = "Silences channel contribution to sound; graphs still show";
                ui.checkbox(&mut self.mute[AudioChannel::FifoA as usize], "FIFO A")
                    .on_hover_text(text);
                ui.checkbox(&mut self.mute[AudioChannel::FifoB as usize], "FIFO B")
                    .on_hover_text(text);

                self.mute_channels(gba);
            });

            let frame = &gba.bus.ppu.frame;
            let image = egui::ColorImage::from_rgba_unmultiplied(
                [frame.width, frame.height],
                &to_rgba(&frame),
            );
            let texture = self.texture.get_or_insert_with(|| {
                egui_ctx.load_texture("GBA", image.clone(), TextureOptions::NEAREST)
            });
            texture.set(image, TextureOptions::NEAREST);

            CentralPanel::default().show(egui_ctx, |ui| {
                let size = ui.available_size();
                let scale = (size.x / frame.width as f32)
                    .min(size.y / frame.height as f32)
                    .floor()
                    .max(1.0);

                let size = egui::vec2(frame.width as f32 * scale, frame.height as f32 * scale);
                ui.centered_and_justified(|ui| {
                    ui.image((texture.id(), size));
                });
            });
        });

        egui_macroquad::draw()
    }

    fn mute_channels(&self, gba: &mut GBA) {
        gba.bus.apu.fifo_a.mute = self.mute[4];
        gba.bus.apu.fifo_b.mute = self.mute[5];
    }
}
