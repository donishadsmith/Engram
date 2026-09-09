// https://www.datasheet.live/pdfviewer?url=https%3A%2F%2Fpdf.datasheet.live%2Fd3941c26%2Fsii.co.jp%2FS-3511AEFS-TB.pdf
// page 5

pub struct Gpio {
    data: u8,
    direction: u8,
    readable: bool,
}

impl Gpio {
    pub fn new() -> Self {
        Self {
            data: 0,
            direction: 0,
            readable: false,
        }
    }

    pub fn is_readable(&self) {}

    pub fn read_u16(&self, address: u32, sio_in: bool) {}

    pub fn write_u16(&mut self, address: u32, value: u16) {}

    pub fn pins() {}
}
