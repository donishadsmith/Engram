pub mod mbc;

/*
    https://tonisagrista.com/blog/2026/playkid/

    https://gbdev.io/pandocs/The_Cartridge_Header.html
    0147-Cartridge type, indicates the memory bank controller based on some 8 bit value

    https://gbdev.io/pandocs/MBCs.html
    Gameboy can only see 64 KB but some Roms can be up to 1 MB, bank switching required
*/

use std::{
    fs::{read, write},
    io::Error,
    path::PathBuf,
};

use crate::components::gamepak::mbc::prelude::*;
use serde::{Deserialize, Serialize};
use shared::utils::{error_message, zero_arr};

// "RTC" in ASCII
const MAGIC_NUMBERS: [u8; 4] = [0x52, 0x54, 0x43, 0x31];
// 4 magic numebers for Mbc3 with timer enabled + 18 RTC states = 22 bytes before the RAM save data
const SAV_HEADER_SIZE: usize = MAGIC_NUMBERS.len() + RtcSaveState::BYTE_SIZE;

macro_rules! trait_functions {
    ($self:expr, $mbc:ident, $func:expr) => {
        match $self {
            MbcType::RomOnly($mbc) => $func,
            MbcType::Mbc1($mbc) => $func,
            MbcType::Mbc2($mbc) => $func,
            MbcType::Mbc3($mbc) => $func,
            MbcType::Mbc5($mbc) => $func,
            MbcType::Huc1($mbc) => $func,
        }
    };
}

#[derive(Deserialize, Serialize, PartialEq, Eq)]
pub enum MbcType {
    RomOnly(RomOnly),
    Mbc1(Mbc1),
    Mbc2(Mbc2),
    Mbc3(Mbc3),
    Mbc5(Mbc5),
    Huc1(Huc1),
}

impl MbcType {
    fn byte_to_struct(
        rom: Box<[u8]>,
        ram: Box<[u8]>,
        rtc_save_state: Option<RtcSaveState>,
        has_rumble: bool,
    ) -> Self {
        match rom[0x0147] {
            0x00 | 0x08 | 0x09 => MbcType::RomOnly(RomOnly::new(rom, ram)),
            0x01..=0x03 => MbcType::Mbc1(Mbc1::new(rom, ram)),
            0x05 | 0x06 => MbcType::Mbc2(Mbc2::new(rom, ram)),
            0x0F..=0x13 => MbcType::Mbc3(Mbc3::new(rom, ram, rtc_save_state)),
            0x19..=0x1E => MbcType::Mbc5(Mbc5::new(rom, ram, has_rumble)),
            0xFF => MbcType::Huc1(Huc1::new(rom, ram)),
            _ => panic!("Only Mbc1, Mbc2, Mbc3, Mbc5, and RomOnly are supported."),
        }
    }
}

impl Mbc for MbcType {
    fn read(&self, address: u16) -> u8 {
        trait_functions!(self, mbc, mbc.read(address))
    }

    fn write(&mut self, address: u16, value: u8) {
        trait_functions!(self, mbc, mbc.write(address, value))
    }

    fn get_rom(&self) -> &[u8] {
        trait_functions!(self, mbc, mbc.get_rom())
    }

    fn get_ram(&self) -> &[u8] {
        trait_functions!(self, mbc, mbc.get_ram())
    }

    fn get_ram_mut(&mut self) -> &mut [u8] {
        trait_functions!(self, mbc, mbc.get_ram_mut())
    }

    fn get_rom_mut(&mut self) -> &mut [u8] {
        trait_functions!(self, mbc, mbc.get_rom_mut())
    }

    fn rom_bank(&self) -> usize {
        trait_functions!(self, mbc, mbc.rom_bank())
    }

    fn ram_bank(&self) -> usize {
        trait_functions!(self, mbc, mbc.ram_bank())
    }

    fn ram_changed(&mut self) -> &mut bool {
        trait_functions!(self, mbc, mbc.ram_changed())
    }

    fn is_timer_enabled(&self) -> bool {
        trait_functions!(self, mbc, mbc.is_timer_enabled())
    }

    fn n_ram_banks(&self) -> usize {
        trait_functions!(self, mbc, mbc.n_ram_banks())
    }

    fn n_rom_banks(&self) -> usize {
        trait_functions!(self, mbc, mbc.n_rom_banks())
    }

    fn ram_size(&self) -> usize {
        trait_functions!(self, mbc, mbc.ram_size())
    }

    fn rom_size(&self) -> usize {
        trait_functions!(self, mbc, mbc.rom_size())
    }

    fn rtc_save_state(&self) -> Option<RtcSaveState> {
        trait_functions!(self, mbc, mbc.rtc_save_state())
    }

    fn tick(&mut self) {
        trait_functions!(self, mbc, mbc.tick())
    }

    fn set_rom(&mut self, rom: Box<[u8]>) {
        trait_functions!(self, mbc, mbc.set_rom(rom))
    }
}

#[derive(Clone, Copy, PartialEq, Debug, Deserialize, Serialize)]
pub enum CgbFlag {
    Cgb,
    Dmg,
}

impl CgbFlag {
    fn byte_to_id(rom: &[u8]) -> Self {
        match rom[0x0143] {
            0x80 | 0xC0 => CgbFlag::Cgb,
            _ => CgbFlag::Dmg,
        }
    }
}

#[derive(Deserialize, Serialize)]
pub struct Header {
    pub title: String,
    pub rom_size: usize,
    pub ram_size: usize,
    pub cgb_flag: CgbFlag,
    pub has_battery: bool,
    pub has_rumble: bool,
    pub has_timer: bool,
    pub checksum: u8,
}

impl Header {
    pub fn new(rom: &[u8]) -> Self {
        let title = Self::title(&rom);
        let rom_size = Self::rom_size(&rom);
        let ram_size = Self::ram_size(&rom);
        let cgb_flag = Self::mode(&rom);
        let has_battery = Self::has_battery(&rom);
        let has_rumble = Self::has_rumble(&rom);
        let has_timer = Self::has_timer(&rom);
        let checksum = Self::checksum(&rom);

        Self {
            title,
            rom_size,
            ram_size,
            cgb_flag,
            has_battery,
            has_rumble,
            has_timer,
            checksum,
        }
    }

    /*
        https://gbdev.io/pandocs/The_Cartridge_Header.html#footnote-mbc30
        0104-0133-Nintendo logo; valid rom contains this
        0134-0143-Title- in uppercase ASCII, if the title is less than 16 characters, it gets zero padded, which is NULL in ASCII
    */
    fn title(rom: &[u8]) -> String {
        rom[0x0134..=0x0143]
            .iter()
            .take_while(|&&byte| byte != 0)
            .map(|&byte| byte as char)
            .collect()
    }
    /*
        uint8_t- wraps
        uint8_t checksum = 0;
        for (uint16_t address = 0x0134; address <= 0x014C; address++) {
            checksum = checksum- rom[address]- 1;
        }
    */
    fn checksum(rom: &[u8]) -> u8 {
        let mut checksum: u8 = 0;
        for address in 0x0134..=0x014C {
            checksum = checksum.wrapping_sub(rom[address]).wrapping_sub(1);
        }

        checksum
    }

    fn mode(rom: &[u8]) -> CgbFlag {
        CgbFlag::byte_to_id(rom)
    }

    fn has_battery(rom: &[u8]) -> bool {
        match rom[0x0147] {
            0x03 | 0x06 | 0x09 | 0x0D..=0x10 | 0x13 | 0x1B | 0x1E | 0x22 | 0xFF => true,
            _ => false,
        }
    }

    fn has_rumble(rom: &[u8]) -> bool {
        match rom[0x0147] {
            0x1C..=0x1E => true,
            _ => false,
        }
    }

    fn has_timer(rom: &[u8]) -> bool {
        match rom[0x0147] {
            0x0F | 0x10 => true,
            _ => false,
        }
    }

    // 0148-ROM size: 32 KiB * (1 << <value>)
    fn rom_size(rom: &[u8]) -> usize {
        32 * 1024 * (1usize << rom[0x0148] as usize)
    }

    // 0149-RAM size
    fn ram_size(rom: &[u8]) -> usize {
        match rom[0x0149] {
            0x02 => 8 * 1024,   // 1 bank; bank size is multiple of 8
            0x03 => 32 * 1024,  // 4 banks of 8 KiB each
            0x04 => 128 * 1024, // 16 banks of 8 KiB each
            0x05 => 64 * 1024,  // 8 banks of 8 KiB each
            _ => 0,
        }
    }

    fn fake() -> Self {
        Self {
            title: String::from("Test"),
            rom_size: 0,
            ram_size: 0,
            cgb_flag: CgbFlag::Dmg,
            has_battery: false,
            has_rumble: false,
            has_timer: false,
            checksum: 0,
        }
    }
}

#[derive(Deserialize, Serialize)]
pub struct GamePak {
    pub header: Header,
    #[serde(skip)]
    pub sav_path: PathBuf,
    pub mbc: MbcType,
}

impl GamePak {
    pub fn load(rom_path: PathBuf) -> Result<Self, Error> {
        let rom = read(&rom_path)?;
        let rom = rom.into_boxed_slice();
        if rom.len() < 0x150 {
            return Err(error_message(
                "File too small to be a valid ROM".to_string(),
            ));
        }

        let header = Header::new(&rom);
        let sav_path = rom_path.with_extension("sav");
        let (ram, rtc_save_state) = Self::read_sav(&sav_path, &header)?;
        let mbc = MbcType::byte_to_struct(rom, ram, rtc_save_state, header.has_rumble);

        Ok(Self {
            header,
            sav_path,
            mbc,
        })
    }

    pub fn read_sav(
        sav_path: &PathBuf,
        header: &Header,
    ) -> Result<(Box<[u8]>, Option<RtcSaveState>), Error> {
        let mut ram = zero_arr::<u8>(header.ram_size);
        let mut rtc_save_state = None;

        if ram.is_empty() || !sav_path.exists() {
            return Ok((ram, rtc_save_state));
        }

        let mut sav_buffer = read(sav_path)?;
        if sav_buffer.len() >= SAV_HEADER_SIZE {
            let magic_start = sav_buffer.len() - SAV_HEADER_SIZE;
            let magic_end = magic_start + MAGIC_NUMBERS.len();
            if sav_buffer[magic_start..magic_end] == MAGIC_NUMBERS {
                rtc_save_state = Some(RtcSaveState::from_bytes(&sav_buffer[magic_end..]));
                sav_buffer.truncate(magic_start);
            }
        }

        let n = ram.len().min(sav_buffer.len());
        ram[..n].copy_from_slice(&sav_buffer[..n]);

        Ok((ram, rtc_save_state))
    }

    pub fn ram_changed(&mut self) -> bool {
        let updated_ram = self.mbc.ram_changed().clone();
        *self.mbc.ram_changed() = false;

        updated_ram
    }

    pub fn write_sav(&mut self) -> Result<(), Error> {
        if !self.header.has_battery || self.mbc.get_ram().is_empty() {
            return Ok(());
        }

        if !self.ram_changed() {
            return Ok(());
        }

        match self.mbc.rtc_save_state() {
            Some(state) => {
                let ram = self.mbc.get_ram();
                let mut buffer = Vec::with_capacity(SAV_HEADER_SIZE + ram.len());
                buffer.extend_from_slice(ram);
                buffer.extend_from_slice(&MAGIC_NUMBERS);
                buffer.extend_from_slice(&state.to_bytes());

                write(&self.sav_path, buffer)?;
            }
            None => write(&self.sav_path, self.mbc.get_ram())?,
        }

        Ok(())
    }

    // Just for testing purposes
    pub fn fake() -> Self {
        let rom = Box::new([0]);
        let ram = Box::new([0]);
        let header = Header::fake();
        let mbc = MbcType::Mbc2(Mbc2::new(rom, ram));

        Self {
            header: header,
            sav_path: PathBuf::new(),
            mbc,
        }
    }
}
