// Seiko 3511 datasheet: https://www.datasheet.live/pdfviewer?url=https%3A%2F%2Fpdf.datasheet.live%2Fd3941c26%2Fsii.co.jp%2FS-3511AEFS-TB.pdf
// https://github.com/mgba-emu/mgba/blob/master/src/gba/cart/gpio.c
// https://problemkaputt.de/gbatek-gba-cart-real-time-clock-rtc.htm
use chrono::{Datelike, Local, Timelike};

use shared::traits::BitOps;

// code is lowkey a bit jank but at least emerald no longer reports a dry battery, maybe refactor this later
#[derive(Clone, Copy, PartialEq, Eq)]
enum RtcMode {
    Idle,
    Command {
        bits: u8,
        bits_received: u8,
    },
    Data {
        code: u8,
        read: bool,
        byte_index: usize,
        n_bits: u8,
    },
}

pub struct Rtc {
    mode: RtcMode,
    sck_high: bool,
    status_register: u8,
    buffer: [u8; 7],
    sio_out_bit: bool,
}

impl Rtc {
    pub fn new() -> Self {
        Self {
            mode: RtcMode::Idle,
            sck_high: false,
            status_register: 0x40,
            buffer: [0; 7],
            sio_out_bit: true,
        }
    }

    pub fn set_pins(&mut self, pins: u8) {
        let sck_is_high = pins.is_set(0);
        let sio_is_high = pins.is_set(1);
        let cs_is_high = pins.is_set(2);

        let sck_previously_low = !self.sck_high;
        self.sck_high = sck_is_high;

        if !cs_is_high {
            self.mode = RtcMode::Idle;

            return;
        };

        if self.mode == RtcMode::Idle {
            self.mode = RtcMode::Command {
                bits: 0,
                bits_received: 0,
            };
        }

        if !(self.sck_high && sck_previously_low) {
            return;
        }

        match &mut self.mode {
            RtcMode::Idle => unreachable!(),
            RtcMode::Command {
                bits,
                bits_received,
            } => {
                *bits = (*bits << 1) | sio_is_high as u8;
                *bits_received += 1;
                if *bits_received == 8 {
                    if bits.get_bit_range(4..8) != 0b0110 {
                        self.mode = RtcMode::Idle;
                    } else {
                        let code = bits.get_bit_range(1..4);
                        match code {
                            0 => {
                                self.reset();
                                self.mode = RtcMode::Idle
                            }
                            6 | 7 => {
                                self.mode = RtcMode::Idle;
                            }
                            _ => {
                                let code = bits.get_bit_range(1..4);
                                let read = bits.is_set(0);

                                if read {
                                    self.load_buffer(code);
                                }

                                self.mode = RtcMode::Data {
                                    code,
                                    read,
                                    byte_index: 0,
                                    n_bits: 0,
                                }
                            }
                        }
                    }
                }
            }
            RtcMode::Data {
                code,
                read,
                n_bits,
                byte_index,
            } => {
                if *read {
                    self.sio_out_bit = (self.buffer[*byte_index] >> *n_bits).is_set(0);
                } else {
                    self.buffer[*byte_index] |= (sio_is_high as u8) << *n_bits;
                }

                *n_bits += 1;
                if *n_bits == 8 {
                    *n_bits = 0;
                    *byte_index += 1;

                    if *byte_index == data_len(*code) {
                        if !*read && *code == 1 {
                            self.status_register = self.buffer[0];
                        }

                        self.mode = RtcMode::Idle;
                    }
                }
            }
        }
    }

    pub fn sio_out(&self) -> bool {
        self.sio_out_bit
    }

    pub fn military_time(&self) -> bool {
        self.status_register.is_set(6)
    }

    pub fn reset(&mut self) {
        self.status_register = 0;
    }

    fn current_time(&mut self, code: u8) {
        let timestamp = Local::now();
        let mut buffer = [0; 7];
        buffer[0] = to_bcd((timestamp.year() - 2000) as u8);
        buffer[1] = to_bcd(timestamp.month() as u8);
        buffer[2] = to_bcd(timestamp.day() as u8);
        buffer[3] = to_bcd(timestamp.weekday().num_days_from_sunday() as u8);
        let hour = timestamp.hour() as u8;
        buffer[4] = if self.military_time() {
            to_bcd(hour)
        } else {
            to_bcd(hour % 12)
        };
        if hour >= 12 {
            buffer[4] |= 0x80;
        }
        buffer[5] = to_bcd(timestamp.minute() as u8);
        buffer[6] = to_bcd(timestamp.second() as u8);

        if code == 2 {
            self.buffer = buffer;
        } else {
            self.buffer[..3].copy_from_slice(&buffer[4..]);
        }
    }

    fn load_buffer(&mut self, code: u8) {
        self.clear_buffer();

        match code {
            1 => self.buffer[0] = self.status_register,
            2 | 3 => self.current_time(code),
            4 | 5 => self.buffer[0] = 0xFF,
            _ => {}
        }
    }

    fn clear_buffer(&mut self) {
        self.buffer = [0; 7];
    }
}

fn to_bcd(value: u8) -> u8 {
    (value / 10) << 4 | (value % 10)
}

fn data_len(code: u8) -> usize {
    match code {
        2 => 7,
        3 => 3,
        _ => 1,
    }
}
