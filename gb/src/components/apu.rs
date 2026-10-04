use shared::{
    EmulatorId,
    psg::{PsgChannel, PsgMixer, PsgMixerRegister, convolve::LowPassFilter},
};

/*
    Crystal

    Pulse 1: Lead melody, sound effects (i.e., chimes, dings, thuds)
    Pulse 2: Lower melody
    Pulse 1 + 2: Cries, interestingly removing 1 channel barely changes cries
    Wave: Bass
    Noise: White noise effects (attack hits)
*/

// https://jsgroth.dev/blog/posts/gb-rewrite-Apu/
// https://nightshade256.github.io/2021/03/27/gb-sound-emulation.html
// https://gbdev.gg8.se/wiki/articles/Gameboy_sound_hardware
// https://gbdev.gg8.se/wiki/articles/Power_Up_Sequence
pub struct Apu {
    mixer: PsgMixer,
    pub psg: PsgChannel,
    sample_counter: u32,
    pub sample_buffer: Vec<f32>,
    low_pass_left: LowPassFilter,
    low_pass_right: LowPassFilter,
}

impl Apu {
    pub fn new() -> Self {
        let mut mixer = PsgMixer::new();
        mixer.write(PsgMixerRegister::Nr52, 0xF1);
        mixer.write(PsgMixerRegister::Nr50, 0x77);
        mixer.write(PsgMixerRegister::Nr51, 0xF3);

        let mut psg = PsgChannel::new(EmulatorId::Gb);
        psg.channel1.write_nrx1(0xBF);
        psg.channel1.write_nrx2(0xF3);
        psg.channel2.write_nrx1(0x3F);

        Self {
            mixer,
            psg,
            sample_counter: 0,
            sample_buffer: Vec::new(),
            low_pass_left: LowPassFilter::new(),
            low_pass_right: LowPassFilter::new(),
        }
    }

    pub fn tick(&mut self, t_cycles: u32, cycles_per_sample: u32, increase_apu_div_counter: bool) {
        if increase_apu_div_counter {
            self.psg.sequencer();
        };

        for _ in 0..t_cycles {
            self.psg.tick();
            let (sample_left, sample_right) = self.mixer.mix(self.psg.samples());
            self.low_pass_left
                .collect_sample((sample_left as f64) / 60.0);
            self.low_pass_right
                .collect_sample((sample_right as f64) / 60.0);

            self.sample_counter += 1;
            if self.sample_counter >= cycles_per_sample {
                self.sample_counter = 0;

                self.sample_buffer
                    .push(self.low_pass_left.convolve() as f32);
                self.sample_buffer
                    .push(self.low_pass_right.convolve() as f32);
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
