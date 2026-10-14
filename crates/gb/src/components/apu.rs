use serde::{Deserialize, Serialize};
use shared::{
    EmulatorId,
    psg::{PsgChannel, PsgMixer, PsgMixerRegister, PsgOutput},
};

/*
    Crystal

    Pulse 1: Lead melody, sound effects (i.e., chimes, dings, thuds)
    Pulse 2: Lower melody
    Pulse 1 + 2: Cries, interestingly removing 1 channel barely changes cries
    Wave: Bass
    Noise: White noise effects (attack hits)
*/

// https://jsgroth.dev/blog/posts/gb-rewrite-apu/
// https://nightshade256.github.io/2021/03/27/gb-sound-emulation.html
// https://gbdev.gg8.se/wiki/articles/Gameboy_sound_hardware
// https://gbdev.gg8.se/wiki/articles/Power_Up_Sequence
#[derive(Deserialize, Serialize)]
pub struct Apu {
    pub mixer: PsgMixer,
    pub psg: PsgChannel,
    #[serde(skip)]
    pub psg_output: PsgOutput,
    sample_counter: f64,
    pub sample_buffer: Vec<f32>,
    previous_samples: [u8; 4],
}

impl Apu {
    pub fn new(sample_rate: f64) -> Self {
        let mut mixer = PsgMixer::new();
        mixer.write(PsgMixerRegister::Nr50, 0x77);
        mixer.write(PsgMixerRegister::Nr51, 0xF3);
        mixer.write(PsgMixerRegister::Nr52, 0xF1);

        let mut psg = PsgChannel::new(EmulatorId::Gb);
        psg.channel1.write_nrx0(0x80);
        psg.channel1.write_nrx1(0xBF);
        psg.channel1.write_nrx2(0xF3);
        psg.channel1.write_nrx3(0xFF);
        psg.channel1.write_nrx4(0x3F);
        psg.channel2.write_nrx1(0x3F);
        psg.channel2.write_nrx3(0xFF);
        psg.channel2.write_nrx4(0xBF);
        psg.channel3.write_nrx0(0x7F);
        psg.channel3.write_nrx1(0xFF);
        psg.channel3.write_nrx2(0x9F);
        psg.channel3.write_nrx3(0xFF);
        psg.channel3.write_nrx4(0xBF);
        psg.channel4.write_nrx1(0xFF);
        psg.channel4.write_nrx4(0xBF);

        Self {
            mixer,
            psg,
            psg_output: PsgOutput::new(sample_rate),
            sample_counter: 0.0,
            sample_buffer: Vec::new(),
            previous_samples: [0; 4],
        }
    }

    pub fn tick(&mut self, t_cycles: u32, cycles_per_sample: f64, increase_apu_div_counter: bool) {
        if increase_apu_div_counter {
            self.psg.sequencer();
        };

        for _ in 0..t_cycles {
            self.psg.tick();
            let samples = self.psg.samples();

            if self.previous_samples != samples || self.mixer.changed {
                self.mixer.changed = false;
                self.previous_samples = samples;
                let (sample_left, sample_right) = self.mixer.mix(samples);
                self.psg_output.add_samples((sample_left, sample_right));
            }

            self.psg_output.cycle += 1;

            self.sample_counter += 1.0;
            if self.sample_counter >= cycles_per_sample {
                self.sample_counter -= cycles_per_sample;
                self.psg_output.read_samples(&mut self.sample_buffer);
            }
        }
    }

    pub fn read_register(&self, address: u16) -> u8 {
        match address {
            0xFF10 => self.psg.channel1.read_nrx0(),
            0xFF11 => self.psg.channel1.read_nrx1(),
            0xFF12 => self.psg.channel1.read_nrx2(),
            0xFF13 => self.psg.channel1.read_nrx3(),
            0xFF14 => self.psg.channel1.read_nrx4(),
            0xFF16 => self.psg.channel2.read_nrx1(),
            0xFF17 => self.psg.channel2.read_nrx2(),
            0xFF18 => self.psg.channel2.read_nrx3(),
            0xFF19 => self.psg.channel2.read_nrx4(),
            0xFF1A => self.psg.channel3.read_nrx0(),
            0xFF1B => self.psg.channel3.read_nrx1(),
            0xFF1C => self.psg.channel3.read_nrx2(),
            0xFF1D => self.psg.channel3.read_nrx3(),
            0xFF1E => self.psg.channel3.read_nrx4(),
            0xFF20 => self.psg.channel4.read_nrx1(),
            0xFF21 => self.psg.channel4.read_nrx2(),
            0xFF22 => self.psg.channel4.read_nrx3(),
            0xFF23 => self.psg.channel4.read_nrx4(),
            0xFF24 => self.mixer.nr50,
            0xFF25 => self.mixer.nr51,
            0xFF26 => 0x70 | self.mixer.status(self.psg.enabled()),
            0xFF30..=0xFF3F => self
                .psg
                .channel3
                .read_wave_ram(address as u32, self.mixer.on) as u8,
            _ => 0xFF,
        }
    }

    pub fn write_register(&mut self, address: u16, value: u8) {
        if !self.mixer.on && address != 0xFF26 {
            return;
        }

        match address {
            0xFF10 => self.psg.channel1.write_nrx0(value),
            0xFF11 => self.psg.channel1.write_nrx1(value),
            0xFF12 => self.psg.channel1.write_nrx2(value),
            0xFF13 => self.psg.channel1.write_nrx3(value),
            0xFF14 => self.psg.channel1.write_nrx4(value),
            0xFF15 => {}
            0xFF16 => self.psg.channel2.write_nrx1(value),
            0xFF17 => self.psg.channel2.write_nrx2(value),
            0xFF18 => self.psg.channel2.write_nrx3(value),
            0xFF19 => self.psg.channel2.write_nrx4(value),
            0xFF1A => self.psg.channel3.write_nrx0(value),
            0xFF1B => self.psg.channel3.write_nrx1(value),
            0xFF1C => self.psg.channel3.write_nrx2(value),
            0xFF1D => self.psg.channel3.write_nrx3(value),
            0xFF1E => self.psg.channel3.write_nrx4(value),
            0xFF20 => self.psg.channel4.write_nrx1(value),
            0xFF21 => self.psg.channel4.write_nrx2(value),
            0xFF22 => self.psg.channel4.write_nrx3(value),
            0xFF23 => self.psg.channel4.write_nrx4(value),
            0xFF24 => self.mixer.write(PsgMixerRegister::Nr50, value),
            0xFF25 => self.mixer.write(PsgMixerRegister::Nr51, value),
            0xFF26 => self.mixer.write(PsgMixerRegister::Nr52, value),
            0xFF30..=0xFF3F => {
                self.psg
                    .channel3
                    .write_wave_ram(address as u32, value as u16, self.mixer.on)
            }
            _ => {}
        }
    }
}
