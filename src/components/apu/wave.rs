pub struct WaveChannel {
    pub ram: [u8; 16],
}

impl WaveChannel {
    pub fn new() -> Self {
        Self { ram: [0u8; 16] }
    }
}
