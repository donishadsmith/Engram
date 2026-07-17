use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use rtrb::{Producer, RingBuffer};

// Audibly tested constants that don't result in popping
pub const AUDIO_BUFFER_CAPACITY: usize = 8192;
pub const AUDIO_TARGET_OCCUPANCY: usize = 4096;

pub struct AudioOutput {
    pub producer: Producer<f32>,
    stream: cpal::Stream,
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
                        let sample = consumer.pop().unwrap_or(0.0);
                        for out in frame.iter_mut() {
                            *out = sample;
                        }
                    }
                },
                |err| eprintln!("Some audio-related error occured: {err}"),
                None,
            )
            .unwrap();

        stream.play().unwrap();

        Self { producer, stream }
    }
}
