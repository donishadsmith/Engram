use hound::{SampleFormat, WavSpec, WavWriter};
use macroquad::audio::{Sound, play_sound, stop_sound};
use std::{f32::consts::PI, io::Cursor};

trait PulseChannel {}
pub struct Audio {
    pub beep: Option<Sound>,
    pub wav_bytes: Option<Vec<u8>>,
    pub sound_timer: u8,
}

impl Audio {
    pub fn start(sample_rate: u32, frequency: f32, duration: f32) -> Self {
        let wavspec = WavSpec {
            channels: 1,
            sample_rate,
            bits_per_sample: 16,
            sample_format: SampleFormat::Int,
        };

        let n_samples = (sample_rate as f32 * duration).round() as usize;

        let mut buffer = Vec::new();
        let mut writer = WavWriter::new(Cursor::new(&mut buffer), wavspec).unwrap();

        let amplitude = i16::MAX as f32;
        for i in 0..n_samples {
            let time = i as f32 / sample_rate as f32;
            let sample = (2.0 * PI * frequency * time).sin();
            writer.write_sample((sample * amplitude) as i16).unwrap();
        }

        writer.finalize().unwrap();

        Self {
            beep: None,
            wav_bytes: Some(buffer),
            sound_timer: 0,
        }
    }

    pub fn play(&self) {
        // /play_sound(self.beep.as_ref().unwrap());
    }
}
