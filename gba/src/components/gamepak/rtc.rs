// https://www.datasheet.live/pdfviewer?url=https%3A%2F%2Fpdf.datasheet.live%2Fd3941c26%2Fsii.co.jp%2FS-3511AEFS-TB.pdf

enum RtcMode {
    Idle,
    Command {
        bits: u8,
        bits_received: u8,
    },
    Data {
        register: u8,
        read: bool,
        byte_index: u8,
        bits_received: u8,
    },
}

pub struct Rtc {
    mode: RtcMode,
}

impl Rtc {
    pub fn new() -> Self {
        Self {
            mode: RtcMode::Idle,
        }
    }

    pub fn set_pins(&mut self, pins: u8) {}

    pub fn sio_out(&self) -> bool {
        false
    }
}
