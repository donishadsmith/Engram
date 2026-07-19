use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use rtrb::{Producer, RingBuffer};

// Audibly tested constants that don't result in popping
const BASE_AUDIO_TARGET_OCCUPANCY: usize = 4096;
pub const AUDIO_TARGET_OCCUPANCY: usize = BASE_AUDIO_TARGET_OCCUPANCY * 2;
pub const AUDIO_BUFFER_CAPACITY: usize = AUDIO_TARGET_OCCUPANCY * 2;

pub struct AudioOutput {
    pub producer: Producer<f32>,
    _stream: cpal::Stream,
    pub sample_rate: u32,
}

impl AudioOutput {
    pub fn new() -> Self {
        let host = cpal::default_host();
        let device = host
            .default_output_device()
            .expect("Output device unavailable");
        let config = device.default_output_config().unwrap();
        let channels = config.channels() as usize;

        let (producer, mut consumer) = RingBuffer::<f32>::new(AUDIO_BUFFER_CAPACITY);

        let stream = device
            .build_output_stream(
                config.into(),
                move |data: &mut [f32], _| {
                    for frame in data.chunks_mut(channels) {
                        let left = consumer.pop().unwrap_or(0.0);
                        let right = consumer.pop().unwrap_or(0.0);
                        for (i, out) in frame.iter_mut().enumerate() {
                            *out = match i {
                                0 => {
                                    if channels == 1 {
                                        (left + right) * 0.5
                                    } else {
                                        left
                                    }
                                }
                                1 => right,
                                _ => 0.0,
                            };
                        }
                    }
                },
                |err| eprintln!("Some audio-related error occured: {err}"),
                None,
            )
            .unwrap();

        stream.play().unwrap();

        Self {
            producer,
            _stream: stream,
            sample_rate: config.sample_rate(),
        }
    }
}
