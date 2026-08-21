// https://gbadev.net/gbadoc/audio/introduction.html

pub struct APU {}

impl APU {
    pub fn new() -> Self {
        Self {}
    }

    pub fn on_timer_overflow(&mut self, timer: u8) {}
}
