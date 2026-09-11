// https://www.datasheet.live/pdfviewer?url=https%3A%2F%2Fpdf.datasheet.live%2Fd3941c26%2Fsii.co.jp%2FS-3511AEFS-TB.pdf
// page 5

use crate::components::{
    gamepak::{rtc::Rtc, solar::SolarSensor},
    utils::BitOps,
};

pub struct Gpio {
    data: u16, // sck, sio, cs
    direction: u16,
    pub readable: bool,
    pub rtc: Option<Rtc>,
    pub solar_sensor: Option<SolarSensor>,
}

impl Gpio {
    pub fn new() -> Self {
        Self {
            data: 0,
            direction: 0,
            readable: false,
            rtc: None,
            solar_sensor: None,
        }
    }

    fn has_device(&self, address: u32) -> bool {
        (0x80000C4..=0x80000C9).contains(&address)
            && (self.rtc.is_some() || self.solar_sensor.is_some())
    }

    pub fn read_device(&self, address: u32) -> bool {
        self.readable && self.has_device(address)
    }

    pub fn write_device(&self, address: u32) -> bool {
        self.has_device(address)
    }

    pub fn read_u16(&self, address: u32) -> u16 {
        match address {
            0x80000C4 | 0x80000C5 => {
                let gba_side = self.data & self.direction;
                let rtc_side = match self.rtc.as_ref() {
                    Some(rtc) => ((rtc.sio_out() as u16) << 1) & !self.direction,
                    None => 0,
                };
                let solar_side = match self.solar_sensor.as_ref() {
                    Some(solar_sensor) => ((solar_sensor.flag() as u16) << 3) & !self.direction,
                    None => 0,
                };

                gba_side | rtc_side | solar_side
            }
            0x80000C6 | 0x80000C7 => self.direction,
            0x80000C8 | 0x80000C9 => self.readable as u16,
            _ => 0,
        }
    }

    pub fn write_u16(&mut self, address: u32, value: u16) {
        match address {
            0x80000C4 => {
                self.data = value.get_bit_range(0..4);
                self.notify_rtc();
                self.notify_solar();
            }
            0x80000C6 => {
                self.direction = value.get_bit_range(0..4);
                self.notify_rtc();
                self.notify_solar();
            }
            0x80000C8 => self.readable = value.is_set(0),
            _ => {}
        }
    }

    fn notify_rtc(&mut self) {
        if let Some(rtc) = self.rtc.as_mut() {
            rtc.set_pins((self.data & self.direction) as u8)
        }
    }

    fn notify_solar(&mut self) {
        if let Some(solar_sensor) = self.solar_sensor.as_mut() {
            solar_sensor.set_pins((self.data & self.direction) as u8)
        }
    }
}
