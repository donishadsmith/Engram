use egui_plot::{Line, Plot};
use macroquad::input::{KeyCode, get_keys_pressed};
use std::collections::VecDeque;

use crate::components::gba::GBA;

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

pub struct AudioDebugger {
    pub visible: bool,
    pub frozen: bool,
    samples: AudioSamples,
}

impl AudioDebugger {
    pub fn new() -> Self {
        Self {
            visible: false,
            frozen: false,
            samples: AudioSamples::new(),
        }
    }

    pub fn turn_on(&mut self) {
        if get_keys_pressed().contains(&KeyCode::F12) {
            self.visible = match self.visible {
                true => {
                    self.frozen = false;
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
            for sample in gba.bus.apu.fifo_a.history.drain(..) {
                if self.samples.fifo_a.len() == 2048 {
                    self.samples.fifo_a.pop_front();
                }

                self.samples.fifo_a.push_back(sample as i8);
            }

            for sample in gba.bus.apu.fifo_b.history.drain(..) {
                if self.samples.fifo_b.len() == 2048 {
                    self.samples.fifo_b.pop_front();
                }

                self.samples.fifo_b.push_back(sample as i8);
            }
        }

        egui_macroquad::ui(|egui_ctx| {
            egui::Window::new("Audio").show(egui_ctx, |ui| {
                let fifo_a = Line::new(
                    "FIFO A",
                    self.samples
                        .fifo_a
                        .iter()
                        .enumerate()
                        .map(|(index, &sample)| [index as f64, sample as f64])
                        .collect::<Vec<[f64; 2]>>(),
                );

                ui.monospace(format!("FIFO A"));
                Plot::new("FIFO A")
                    .view_aspect(2.0)
                    .include_y(-128.0)
                    .include_y(127.0)
                    .show(ui, |plot_ui| {
                        plot_ui.line(fifo_a);
                    });

                let fifo_b = Line::new(
                    "FIFO B",
                    self.samples
                        .fifo_a
                        .iter()
                        .enumerate()
                        .map(|(index, &sample)| [index as f64, sample as f64])
                        .collect::<Vec<[f64; 2]>>(),
                );

                ui.monospace(format!("FIFO B"));
                Plot::new("FIFO B")
                    .view_aspect(2.0)
                    .include_y(-128.0)
                    .include_y(120.0)
                    .show(ui, |plot_ui| {
                        plot_ui.line(fifo_b);
                    });
            });
        });

        egui_macroquad::draw()
    }
}
