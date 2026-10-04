use egui::{Color32, RichText, SidePanel, TextureHandle, TopBottomPanel};
use egui_plot::{HLine, Line, Plot};

use crate::components::{apu::Apu, dma::FifoChannel, gba::GBA};
use shared::debug::create_game_screen;

#[derive(Clone, Copy)]
enum AudioChannel {
    Channel1,
    Channel2,
    Channel3,
    Channel4,
    FifoA,
    FifoB,
}

#[derive(Clone, Copy)]
enum PanDirection {
    Left,
    Right,
}

const CHANNELS: [AudioChannel; 6] = [
    AudioChannel::Channel1,
    AudioChannel::Channel2,
    AudioChannel::Channel3,
    AudioChannel::Channel4,
    AudioChannel::FifoA,
    AudioChannel::FifoB,
];

struct AudioRegisters {
    channel1: [u16; 3],
    channel2: [u16; 2],
    channel3: [u16; 3],
    channel4: [u16; 2],
}

impl AudioRegisters {
    fn new() -> Self {
        Self {
            channel1: [0; 3],
            channel2: [0; 2],
            channel3: [0; 3],
            channel4: [0; 2],
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

struct PanSettings {
    channel1_left: bool,
    channel1_right: bool,
    channel2_left: bool,
    channel2_right: bool,
    channel3_left: bool,
    channel3_right: bool,
    channel4_left: bool,
    channel4_right: bool,
    fifo_a_left: bool,
    fifo_a_right: bool,
    fifo_b_left: bool,
    fifo_b_right: bool,
}

impl PanSettings {
    fn new() -> Self {
        Self {
            channel1_left: false,
            channel1_right: false,
            channel2_left: false,
            channel2_right: false,
            channel3_left: false,
            channel3_right: false,
            channel4_left: false,
            channel4_right: false,
            fifo_a_left: false,
            fifo_a_right: false,
            fifo_b_left: false,
            fifo_b_right: false,
        }
    }

    fn update(&mut self, channel_id: AudioChannel, panning: PanDirection, on: bool) {
        match panning {
            PanDirection::Left => match channel_id {
                AudioChannel::Channel1 => self.channel1_left = on,
                AudioChannel::Channel2 => self.channel2_left = on,
                AudioChannel::Channel3 => self.channel3_left = on,
                AudioChannel::Channel4 => self.channel4_left = on,
                AudioChannel::FifoA => self.fifo_a_left = on,
                AudioChannel::FifoB => self.fifo_b_left = on,
            },
            PanDirection::Right => match channel_id {
                AudioChannel::Channel1 => self.channel1_right = on,
                AudioChannel::Channel2 => self.channel2_right = on,
                AudioChannel::Channel3 => self.channel3_right = on,
                AudioChannel::Channel4 => self.channel4_right = on,
                AudioChannel::FifoA => self.fifo_a_right = on,
                AudioChannel::FifoB => self.fifo_b_right = on,
            },
        }
    }

    fn status(&mut self, channel_id: AudioChannel, panning: PanDirection) -> bool {
        match panning {
            PanDirection::Left => match channel_id {
                AudioChannel::Channel1 => self.channel1_left,
                AudioChannel::Channel2 => self.channel2_left,
                AudioChannel::Channel3 => self.channel3_left,
                AudioChannel::Channel4 => self.channel4_left,
                AudioChannel::FifoA => self.fifo_a_left,
                AudioChannel::FifoB => self.fifo_b_left,
            },
            PanDirection::Right => match channel_id {
                AudioChannel::Channel1 => self.channel1_right,
                AudioChannel::Channel2 => self.channel2_right,
                AudioChannel::Channel3 => self.channel3_right,
                AudioChannel::Channel4 => self.channel4_right,
                AudioChannel::FifoA => self.fifo_a_right,
                AudioChannel::FifoB => self.fifo_b_right,
            },
        }
    }
}

fn to_percent(volume: f32) -> String {
    format!("{}%", (volume * 100.0) as u32)
}

fn register(ui: &mut egui::Ui, name: &str, value: u16) {
    let size = (ui.available_width() / 18.0).clamp(8.0, 24.0);

    ui.horizontal(|ui| {
        ui.label(RichText::new(name).strong().size(size));
        ui.label(
            RichText::new(format!("{:016b}", value))
                .monospace()
                .size(size),
        );
    });
}

pub struct AudioDebugger {
    pub frozen: bool,
    mute: [bool; 6],
    texture: Option<TextureHandle>,
    registers: AudioRegisters,
    volume: VolumeSettings,
    pan_settings: PanSettings,
}

impl AudioDebugger {
    pub fn new() -> Self {
        Self {
            frozen: false,
            mute: [false; 6],
            texture: None,
            registers: AudioRegisters::new(),
            volume: VolumeSettings::new(),
            pan_settings: PanSettings::new(),
        }
    }

    pub fn close(&mut self, gba: &mut GBA) {
        self.frozen = false;
        self.mute = [false; 6];
        self.mute_channels(gba);
        gba.bus.apu.debugger_status(false);
    }

    pub fn freeze(&mut self) {
        self.frozen = match self.frozen {
            true => false,
            false => true,
        }
    }

    pub fn show_ui(&mut self, egui_ctx: &egui::Context, gba: &mut GBA) {
        if !self.frozen {
            let registers = &gba.bus.apu.psg_registers;
            self.registers.channel1 = [
                registers.read_u16(0x4000060),
                registers.read_u16(0x4000062),
                registers.read_u16(0x4000064),
            ];
            self.registers.channel2 =
                [registers.read_u16(0x4000068), registers.read_u16(0x400006C)];
            self.registers.channel3 = [
                registers.read_u16(0x4000070),
                registers.read_u16(0x4000072),
                registers.read_u16(0x4000074),
            ];
            self.registers.channel4 =
                [registers.read_u16(0x4000078), registers.read_u16(0x400007C)];

            self.volume.fifo_a = gba
                .bus
                .apu
                .global_control
                .volume_control_fifo(FifoChannel::A);
            self.volume.fifo_b = gba
                .bus
                .apu
                .global_control
                .volume_control_fifo(FifoChannel::B);
            self.volume.psg = gba.bus.apu.global_control.psg_volume();
        }

        let apu = &gba.bus.apu;
        SidePanel::right("FIFO Audio").show(egui_ctx, |ui| {
            ui.heading("FIFO Audio").highlight();
            ui.separator();

            let fifo_a_samples = Line::new(
                "FIFO A Samples",
                apu.fifo_a
                    .history
                    .iter()
                    .enumerate()
                    .map(|(index, &sample)| [index as f64, sample as f64])
                    .collect::<Vec<[f64; 2]>>(),
            );

            let fifo_a_occupancy = Line::new(
                "FIFO A Occupancy",
                apu.fifo_a
                    .occupancy
                    .iter()
                    .enumerate()
                    .map(|(index, &size)| [index as f64, size as f64])
                    .collect::<Vec<[f64; 2]>>(),
            );

            let text = "Silences channel contribution to sound; graphs still show";
            ui.horizontal(|ui| {
                ui.strong("FIFO A");
                ui.checkbox(&mut self.mute[4], "mute").on_hover_text(text);
            });
            Plot::new("FIFO A Samples")
                .view_aspect(3.0)
                .include_y(-128.0)
                .include_y(127.0)
                .show(ui, |plot_ui| {
                    plot_ui.line(fifo_a_samples);
                });

            ui.horizontal(|ui| {
                ui.strong("FIFO A Occupancy");
            });
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
                apu.fifo_b
                    .history
                    .iter()
                    .enumerate()
                    .map(|(index, &sample)| [index as f64, sample as f64])
                    .collect::<Vec<[f64; 2]>>(),
            );

            let fifo_b_occupancy = Line::new(
                "FIFO B Occupancy",
                apu.fifo_b
                    .occupancy
                    .iter()
                    .enumerate()
                    .map(|(index, &size)| [index as f64, size as f64])
                    .collect::<Vec<[f64; 2]>>(),
            );

            ui.horizontal(|ui| {
                ui.strong("FIFO B");
                ui.checkbox(&mut self.mute[5], "mute").on_hover_text(text);
            });
            Plot::new("FIFO B Samples")
                .view_aspect(3.0)
                .include_y(-128.0)
                .include_y(120.0)
                .show(ui, |plot_ui| {
                    plot_ui.line(fifo_b_samples);
                });

            ui.horizontal(|ui| {
                ui.strong("FIFO B Occupancy");
            });
            Plot::new("FIFO B Occupancy")
                .view_aspect(3.0)
                .include_y(0.0)
                .include_y(32.0)
                .show(ui, |plot_ui| {
                    plot_ui.line(fifo_b_occupancy);
                    plot_ui.hline(HLine::new("FIFO B Occupancy", 16.0));
                });
        });

        SidePanel::left("PSG").show(egui_ctx, |ui| {
            ui.heading("PSG Channels").highlight();
            ui.separator();

            let channel1_samples = Line::new(
                "Channel 1 Samples",
                apu.psg_history[0]
                    .iter()
                    .enumerate()
                    .map(|(index, &sample)| [index as f64, sample as f64])
                    .collect::<Vec<[f64; 2]>>(),
            );

            let text = "Silences channel contribution to sound; graphs still show";
            ui.horizontal(|ui| {
                ui.strong("Channel 1");
                ui.checkbox(&mut self.mute[0], "mute").on_hover_text(text);
            });
            Plot::new("Channel 1 Samples")
                .view_aspect(3.0)
                .include_y(0.0)
                .include_y(16.0)
                .show(ui, |plot_ui| {
                    plot_ui.line(channel1_samples);
                });

            let channel2_samples = Line::new(
                "Channel 2 Samples",
                apu.psg_history[1]
                    .iter()
                    .enumerate()
                    .map(|(index, &sample)| [index as f64, sample as f64])
                    .collect::<Vec<[f64; 2]>>(),
            );

            ui.horizontal(|ui| {
                ui.strong("Channel 2");
                ui.checkbox(&mut self.mute[1], "mute").on_hover_text(text);
            });
            Plot::new("Channel 2 Samples")
                .view_aspect(3.0)
                .include_y(0.0)
                .include_y(16.0)
                .show(ui, |plot_ui| {
                    plot_ui.line(channel2_samples);
                });

            let channel3_samples = Line::new(
                "Channel 3 Samples",
                apu.psg_history[2]
                    .iter()
                    .enumerate()
                    .map(|(index, &sample)| [index as f64, sample as f64])
                    .collect::<Vec<[f64; 2]>>(),
            );

            ui.horizontal(|ui| {
                ui.strong("Channel 3");
                ui.checkbox(&mut self.mute[2], "mute").on_hover_text(text);
            });
            Plot::new("Channel 3 Samples")
                .view_aspect(3.0)
                .include_y(0.0)
                .include_y(16.0)
                .show(ui, |plot_ui| {
                    plot_ui.line(channel3_samples);
                });

            let channel4_samples = Line::new(
                "Channel 4 Samples",
                apu.psg_history[3]
                    .iter()
                    .enumerate()
                    .map(|(index, &sample)| [index as f64, sample as f64])
                    .collect::<Vec<[f64; 2]>>(),
            );

            ui.horizontal(|ui| {
                ui.strong("Channel 4");
                ui.checkbox(&mut self.mute[3], "mute").on_hover_text(text);
            });
            Plot::new("Channel 4 Samples")
                .view_aspect(3.0)
                .include_y(0.0)
                .include_y(16.0)
                .show(ui, |plot_ui| {
                    plot_ui.line(channel4_samples);
                });
        });

        TopBottomPanel::top("Global Controls").show(egui_ctx, |ui| {
            ui.horizontal(|ui| {
                ui.heading("Global Control Register Settings").highlight();

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
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

                    ui.add_space(12.0);

                    if ui
                        .add(egui::Label::new(text).sense(egui::Sense::click()))
                        .on_hover_text(hover)
                        .on_hover_cursor(egui::CursorIcon::PointingHand)
                        .clicked()
                    {
                        self.freeze();
                    }

                    ui.label(RichText::new("Debugger Status:").strong());
                });
            });

            ui.separator();

            egui::Grid::new("Global Control Register Settings")
                .striped(true)
                .show(ui, |ui| {
                    for channel_id in [
                        "",
                        "Channel 1",
                        "Channel 2",
                        "Channel 3",
                        "Channel 4",
                        "FIFO A",
                        "FIFO B",
                    ] {
                        ui.strong(channel_id);
                    }

                    ui.end_row();

                    for (direction, string) in
                        [(PanDirection::Left, "Left"), (PanDirection::Right, "Right")]
                    {
                        ui.label(string);
                        for channel_id in CHANNELS {
                            let on = if !self.frozen {
                                self.pan_settings.update(
                                    channel_id,
                                    direction,
                                    sound_on(&gba.bus.apu, channel_id, direction),
                                );
                                self.pan_settings.status(channel_id, direction)
                            } else {
                                self.pan_settings.status(channel_id, direction)
                            };

                            ui.label(if on {
                                RichText::new("on").color(Color32::LIGHT_GREEN)
                            } else {
                                RichText::new("off").weak()
                            });
                        }

                        ui.end_row();
                    }

                    ui.label("Volume");
                    ui.label("");
                    ui.label("");
                    ui.label(to_percent(self.volume.psg));

                    ui.label("");
                    ui.label(to_percent(self.volume.fifo_a));
                    ui.label(to_percent(self.volume.fifo_b));
                    ui.end_row();
                })
        });

        TopBottomPanel::bottom("Registers").show(egui_ctx, |ui| {
            ui.heading("PSG Registers").highlight();
            ui.separator();

            ui.columns(4, |cols| {
                register(&mut cols[0], "SOUND1CNT_L:", self.registers.channel1[0]);
                register(&mut cols[0], "SOUND1CNT_H:", self.registers.channel1[1]);
                register(&mut cols[0], "SOUND1CNT_X:", self.registers.channel1[2]);

                register(&mut cols[1], "SOUND2CNT_L:", self.registers.channel2[0]);
                register(&mut cols[1], "SOUND2CNT_H:", self.registers.channel2[1]);

                register(&mut cols[2], "SOUND3CNT_L:", self.registers.channel3[0]);
                register(&mut cols[2], "SOUND3CNT_H:", self.registers.channel3[1]);
                register(&mut cols[2], "SOUND3CNT_X:", self.registers.channel3[2]);

                register(&mut cols[3], "SOUND4CNT_L:", self.registers.channel4[0]);
                register(&mut cols[3], "SOUND4CNT_H:", self.registers.channel4[1]);
            });
        });

        create_game_screen(
            &mut self.texture,
            egui_ctx,
            &mut gba.bus.ppu.frontend,
            "Game Screen".to_string(),
        );

        self.mute_channels(gba);
        gba.bus.apu.debugger_status(!self.frozen);
    }

    fn mute_channels(&self, gba: &mut GBA) {
        gba.bus.apu.psg_mute.copy_from_slice(&self.mute[0..4]);
        gba.bus.apu.fifo_a.mute = self.mute[4];
        gba.bus.apu.fifo_b.mute = self.mute[5];
    }
}

fn sound_on(apu: &Apu, channel_id: AudioChannel, direction: PanDirection) -> bool {
    match (channel_id, direction) {
        (AudioChannel::Channel1, PanDirection::Left) => apu.psg_mixer.panned_left(0),
        (AudioChannel::Channel1, PanDirection::Right) => apu.psg_mixer.panned_right(0),
        (AudioChannel::Channel2, PanDirection::Left) => apu.psg_mixer.panned_left(1),
        (AudioChannel::Channel2, PanDirection::Right) => apu.psg_mixer.panned_right(1),
        (AudioChannel::Channel3, PanDirection::Left) => apu.psg_mixer.panned_left(2),
        (AudioChannel::Channel3, PanDirection::Right) => apu.psg_mixer.panned_right(2),
        (AudioChannel::Channel4, PanDirection::Left) => apu.psg_mixer.panned_left(3),
        (AudioChannel::Channel4, PanDirection::Right) => apu.psg_mixer.panned_right(3),
        (AudioChannel::FifoA, PanDirection::Left) => {
            apu.global_control.panned_left_fifo(FifoChannel::A)
        }
        (AudioChannel::FifoA, PanDirection::Right) => {
            apu.global_control.panned_right_fifo(FifoChannel::A)
        }
        (AudioChannel::FifoB, PanDirection::Left) => {
            apu.global_control.panned_left_fifo(FifoChannel::B)
        }
        (AudioChannel::FifoB, PanDirection::Right) => {
            apu.global_control.panned_right_fifo(FifoChannel::B)
        }
    }
}
