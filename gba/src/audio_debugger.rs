use egui::{CentralPanel, SidePanel, TextureHandle, TextureOptions, TopBottomPanel};
use egui_plot::{HLine, Line, Plot};
use macroquad::input::{KeyCode, get_keys_pressed};
use std::collections::VecDeque;

use crate::components::{apu::AudioChannel, gba::GBA};
use shared::render::to_rgba;

struct AudioSamples {
    channel1: VecDeque<i8>,
    channel2: VecDeque<i8>,
    fifo_a: VecDeque<i8>,
    fifo_b: VecDeque<i8>,
}

impl AudioSamples {
    fn new() -> Self {
        Self {
            channel1: VecDeque::new(),
            channel2: VecDeque::new(),
            fifo_a: VecDeque::new(),
            fifo_b: VecDeque::new(),
        }
    }
}

struct AudioOccupancy {
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

struct AudioRegisters {
    channel1: [u16; 3],
    channel2: [u16; 2],
}

impl AudioRegisters {
    fn new() -> Self {
        Self {
            channel1: [0; 3],
            channel2: [0; 2],
        }
    }
}

struct VolumeSettings {
    fifo_a: f32,
    fifo_b: f32,
    psg: f32,
}

impl VolumeSettings {
    fn new() -> Self {
        Self {
            fifo_a: 0.0,
            fifo_b: 0.0,
            psg: 0.0,
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
    registers: AudioRegisters,
    volume: VolumeSettings,
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
            registers: AudioRegisters::new(),
            volume: VolumeSettings::new(),
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

            for sample in gba.bus.apu.channel1.history.drain(..) {
                if self.samples.channel1.len() == 2048 {
                    self.samples.channel1.pop_front();
                }

                self.samples.channel1.push_back(sample as i8);
            }

            for sample in gba.bus.apu.channel2.history.drain(..) {
                if self.samples.channel2.len() == 2048 {
                    self.samples.channel2.pop_front();
                }

                self.samples.channel2.push_back(sample as i8);
            }

            self.registers.channel1 = [
                gba.bus.apu.channel1.soundcnt.from_index(0),
                gba.bus.apu.channel1.soundcnt.from_index(1),
                gba.bus.apu.channel1.soundcnt.from_index(2),
            ];

            self.registers.channel2 = [
                gba.bus.apu.channel2.soundcnt.from_index(0),
                gba.bus.apu.channel2.soundcnt.from_index(2),
            ];

            self.volume.fifo_a = gba.bus.apu.volume_control(AudioChannel::FifoA);
            self.volume.fifo_b = gba.bus.apu.volume_control(AudioChannel::FifoB);
            self.volume.psg = gba.bus.apu.volume_control(AudioChannel::Channel1);
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
            });

            let frame = &gba.bus.ppu.frontend;
            let image = egui::ColorImage::from_rgba_unmultiplied(
                [frame.width, frame.height],
                &to_rgba(&frame),
            );
            let texture = self.texture.get_or_insert_with(|| {
                egui_ctx.load_texture("GBA", image.clone(), TextureOptions::NEAREST)
            });
            texture.set(image, TextureOptions::NEAREST);

            SidePanel::left("PSG").show(egui_ctx, |ui| {
                ui.heading("PSG Channels").highlight();
                ui.separator();

                let channel1_samples = Line::new(
                    "Channel 1 Samples",
                    self.samples
                        .channel1
                        .iter()
                        .enumerate()
                        .map(|(index, &sample)| [index as f64, sample as f64])
                        .collect::<Vec<[f64; 2]>>(),
                );

                ui.monospace(format!("Channel 1 Samples"));
                Plot::new("Channel 1 Samples")
                    .view_aspect(3.0)
                    .include_y(0.0)
                    .include_y(16.0)
                    .show(ui, |plot_ui| {
                        plot_ui.line(channel1_samples);
                    });

                let channel2_samples = Line::new(
                    "Channel 2 Samples",
                    self.samples
                        .channel2
                        .iter()
                        .enumerate()
                        .map(|(index, &sample)| [index as f64, sample as f64])
                        .collect::<Vec<[f64; 2]>>(),
                );

                ui.monospace(format!("Channel 2 Samples"));
                Plot::new("Channel 2 Samples")
                    .view_aspect(3.0)
                    .include_y(0.0)
                    .include_y(16.0)
                    .show(ui, |plot_ui| {
                        plot_ui.line(channel2_samples);
                    });

                ui.heading("Mute PSG Channels").highlight();
                ui.separator();
                let text = "Silences channel contribution to sound; graphs still show";
                ui.checkbox(&mut self.mute[AudioChannel::Channel1 as usize], "Channel 1")
                    .on_hover_text(text);
                ui.checkbox(&mut self.mute[AudioChannel::Channel2 as usize], "Channel 2")
                    .on_hover_text(text);
            });

            TopBottomPanel::top("Global Controls").show(egui_ctx, |ui| {
                ui.heading("Global Control Register Settings").highlight();
                ui.separator();

                ui.horizontal(|ui| {
                    egui::Grid::new("First")
                        .num_columns(1)
                        .spacing([20.0, 4.0])
                        .show(ui, |ui| {
                            ui.label(format!("Fifo A Volume: {}", self.volume.fifo_a));
                            ui.end_row();

                            ui.label(format!("Fifo B Volume: {}", self.volume.fifo_b));
                            ui.end_row();

                            ui.label(format!("PSG Volume: {}", self.volume.psg));
                            ui.end_row();
                        });
                });
            });

            TopBottomPanel::bottom("Registers").show(egui_ctx, |ui| {
                ui.heading("PSG Registers").highlight();
                ui.separator();

                ui.horizontal(|ui| {
                    egui::Grid::new("First")
                        .num_columns(1)
                        .spacing([20.0, 4.0])
                        .show(ui, |ui| {
                            ui.label(format!("SOUND1CNT_L: {:16b}", self.registers.channel1[0]));
                            ui.end_row();

                            ui.label(format!("SOUND1CNT_L: {:16b}", self.registers.channel1[1]));
                            ui.end_row();

                            ui.label(format!("SOUND1CNT_X: {:16b}", self.registers.channel1[2]));
                            ui.end_row();
                        });

                    ui.add_space(30.0);
                    egui::Grid::new("Second")
                        .num_columns(1)
                        .spacing([20.0, 4.0])
                        .show(ui, |ui| {
                            ui.label(format!("SOUND2CNT_L: {:16b}", self.registers.channel2[0]));
                            ui.end_row();

                            ui.label(format!("SOUND2CNT_X: {:16b}", self.registers.channel2[1]));
                            ui.end_row();
                        });
                });
            });

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

        self.mute_channels(gba);
        egui_macroquad::draw()
    }

    fn mute_channels(&self, gba: &mut GBA) {
        gba.bus.apu.fifo_a.mute = self.mute[4];
        gba.bus.apu.fifo_b.mute = self.mute[5];
    }
}
