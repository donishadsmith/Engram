// https://mgba.io/2015/06/27/cycle-counting-prefetch/
// https://github.com/nba-emu/NanoBoyAdvance/blob/master/src/nba/src/bus/bus.cc
// https://developer.arm.com/documentation/ddi0084/f/memory-interface/bus-cycle-types/sequential-cycles
// https://corrupt.wiki/systems/gameboy-advance/bizhawk-memory-domains
// https://medium.com/@michelheily/hello-gba-journey-of-making-an-emulator-part-1-8793000e8606
// https://www.cs.rit.edu/~tjh8300/CowBite/CowBiteSpec.htm#Memory%20Map
// https://www.nesdev.org/wiki/Open_bus_behavior
// https://www.cs.rit.edu/~tjh8300/CowBite/CowBiteSpec.htm#Memory%20Map
// https://www.chibiakumas.com/arm/gba.php
// https://gbadev.net/gbadoc/interrupts.html
// https://mgba.io/2017/05/29/holy-grail-bugs/

// https://blog.asie.pl/2025/09/wonderful-update-september-2025/
// https://github.com/michelhe/rustboyadvance-ng/blob/master/arm7tdmi/src/memory.rs
// Just do an afterboot startup

// https://problemkaputt.de/gbatek.htm#GBAUnpredictableThings

use crate::components::{
    apu::APU,
    dma::{DmaChannels, TransferType, Trigger},
    gamepak::{BackupChip, GamePak},
    keypad::Keypad,
    ppu::PPU,
    scheduler::EventScheduler,
    serial::Serial,
    timer::Timers,
    utils::{BitOps, zero_arr},
};

const WAIT_STATE_NONSEQUENTIAL: [u8; 4] = [4, 3, 2, 8];
const WAIT_STATE0_SEQUENTIAL: [u8; 2] = [2, 1];
const WAIT_STATE1_SEQUENTIAL: [u8; 2] = [4, 1];
const WAIT_STATE2_SEQUENTIAL: [u8; 2] = [8, 1];

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum AccessType {
    Sequential, // Memory address related to previous address, incremented by + 2 (half word) or +4 (word)
    Nonsequential, // Memory address is fetched and has nothing to do with the previous instruction
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum WaitState {
    WaitState0,
    WaitState1,
    WaitState2,
    SramWaitControl,
}

impl WaitState {
    fn from_address(address: u32) -> WaitState {
        match address {
            0x08000000..=0x09FFFFFF => WaitState::WaitState0,
            0x0A000000..=0x0BFFFFFF => WaitState::WaitState1,
            0x0C000000..=0x0DFFFFFF => WaitState::WaitState2,
            0x0E000000..=0x0FFFFFFF => WaitState::SramWaitControl,
            _ => unreachable!(),
        }
    }

    fn cycles(self, waitcnt: u16, access_type: AccessType) -> u8 {
        match self {
            WaitState::SramWaitControl => {
                WAIT_STATE_NONSEQUENTIAL[waitcnt.get_bit_range(0..2) as usize]
            }
            WaitState::WaitState0 if access_type == AccessType::Nonsequential => {
                WAIT_STATE_NONSEQUENTIAL[waitcnt.get_bit_range(2..4) as usize]
            }
            WaitState::WaitState0 => WAIT_STATE0_SEQUENTIAL[waitcnt.get_bit(4) as usize],
            WaitState::WaitState1 if access_type == AccessType::Nonsequential => {
                WAIT_STATE_NONSEQUENTIAL[waitcnt.get_bit_range(5..7) as usize]
            }
            WaitState::WaitState1 => WAIT_STATE1_SEQUENTIAL[waitcnt.get_bit(7) as usize],
            WaitState::WaitState2 if access_type == AccessType::Nonsequential => {
                WAIT_STATE_NONSEQUENTIAL[waitcnt.get_bit_range(8..10) as usize]
            }
            WaitState::WaitState2 => WAIT_STATE2_SEQUENTIAL[waitcnt.get_bit(10) as usize],
        }
    }
}

pub struct Bus {
    pub scheduler: EventScheduler,
    _bios: Box<[u8; 0x4000]>,
    pub ewram: Box<[u8; 0x40000]>,
    pub iwram: Box<[u8; 0x8000]>,
    pub last_instruction_read: u32,
    pub last_bios_fetch: u32, // According to medium article, MMBN6 has an email bug due to null pointer dereference in the BIOS
    // region [00DCh+8] in bios is 0xE129F000; https://problemkaputt.de/gbatek.htm#GBAUnpredictableThings
    pub apu: APU,
    pub ppu: PPU,
    pub dma: DmaChannels,
    pub timers: Timers,
    pub gamepak: GamePak,
    pub serial: Serial,
    pub keypad: Keypad,
    interrupt_master_enable: u32,
    pub interrupt_enable: u16,
    pub interrupt_flag: u16,
    postflg: u8,
    pub waitcnt: u16,
    haltcnt: Option<u8>,
    internal_memory_control: u32,
}

impl Bus {
    pub fn new(gamepak: GamePak) -> Self {
        Self {
            scheduler: EventScheduler::new(),
            _bios: zero_arr(),
            ewram: zero_arr(),
            iwram: zero_arr(),
            last_instruction_read: 0,
            last_bios_fetch: 0xE129F000,
            ppu: PPU::new(),
            gamepak,
            apu: APU::new(),
            dma: DmaChannels::new(),
            timers: Timers::new(),
            serial: Serial::new(),
            keypad: Keypad::new(),
            interrupt_master_enable: 0,
            interrupt_flag: 0,
            interrupt_enable: 0,
            postflg: 0,
            waitcnt: 0,
            haltcnt: None,
            internal_memory_control: 0x0D000020,
        }
    }

    #[inline]
    pub fn ewram_index(address: u32) -> usize {
        address.get_bit_range(0..18) as usize
    }

    #[inline]
    pub fn iwram_index(address: u32) -> usize {
        address.get_bit_range(0..15) as usize
    }

    #[inline]
    pub fn palette_index(address: u32) -> usize {
        address.get_bit_range(0..10) as usize
    }

    #[inline]
    pub fn oam_index(address: u32) -> usize {
        address.get_bit_range(0..10) as usize
    }

    #[inline]
    pub fn vram_index(address: u32) -> usize {
        let index = address.get_bit_range(0..17) as usize;
        let index = if index >= 0x18000 {
            index - 0x8000
        } else {
            index
        };

        index
    }

    #[inline]
    pub fn read_backup_byte(&self, address: u32) -> u8 {
        match &self.gamepak.backup_chip {
            BackupChip::Sram(sram) => sram.read((address & 0x7FFF) as usize),
            BackupChip::Flash(flash) => flash.read(address),
            BackupChip::Eeprom(_) | BackupChip::None => 0xFF,
        }
    }

    #[inline]
    pub fn write_backup_byte(&mut self, address: u32, value: u8) {
        match &mut self.gamepak.backup_chip {
            BackupChip::Sram(sram) => sram.write((address & 0x7FFF) as usize, value),
            BackupChip::Flash(flash) => flash.write(address, value),
            BackupChip::Eeprom(_) | BackupChip::None => {}
        }
    }

    pub fn read_u8(&mut self, address: u32, access_type: AccessType) -> u8 {
        self.cost(address, 8, access_type);

        if address & !1 == 0x4000300 {
            if address.is_clear(0) {
                return self.postflg;
            } else {
                // 0x4000301 => {} // Undocumented - Power Down Control (HALTCNT), 8 bit register (write only)
                // technically not read but just in case
                return 0;
            }
        }

        match address >> 24 {
            0x00 => (self.last_bios_fetch >> (8 * (address.get_bit_range(0..2)))) as u8,
            0x02 => self.ewram[Bus::ewram_index(address)],
            0x03 => self.iwram[Bus::iwram_index(address)],
            0x04 => {
                let half_word = self.read_register(address & !1);
                if address.is_clear(0) {
                    half_word as u8
                } else {
                    (half_word >> 8) as u8
                }
            }
            0x05 => self.ppu.palette_ram[Bus::palette_index(address)],
            0x06 => self.ppu.vram[Bus::vram_index(address)],
            0x07 => self.ppu.oam[Bus::oam_index(address)],
            0x08..=0x0D => self.gamepak.read_rom_region(address),
            0x0E | 0x0F => self.read_backup_byte(address),
            _ => (self.last_instruction_read >> (8 * (address.get_bit_range(0..2)))) as u8,
        }
    }

    pub fn write_u8(&mut self, address: u32, value: u8, access_type: AccessType) {
        self.cost(address, 8, access_type);
        // https://github.com/camthesaxman/gba_bios/blob/master/asm/bios.s

        /*
        _00000300:
            mov  r3, #0x4000000 = base 0x4000000 in r3
            ldr  r2, [r3, #0x200] = 32 bit read of x4000200 stored in r2
            and  r2, r2, r2, lsr #16 = bitwise and; r2 & (r2 >> 16) - IE & IF flags
            ands r1, r2, #0x80 = r1 = r2 & 0x80 checking bit 7 (serial) - s updates condition flag
            ldrne r0, _00000AB8 = if (ne = Z flag is clear = !0) load value at _00000AB8 to r0
            andeq r1, r2, #1 = r1 = r2 & 1, if (eq = Z flag is set = 0) - bit 1 is vblank
            ldreq r0, _00000ABC - load address from _00000ABC into r0 if Z flag set
            strheq r2, [r3, #-8] - if z is set, take lowest half word of r2 and store at r3 + - 8 = 0x03FFFFF8
            strb r1, [r3, #0x202] - store lowest byte in to 0x4000202
            bx r0
        */

        // Some game could right to 203
        if address & !1 == 0x4000202 {
            let mask = (value as u16) << (address.get_bit(0) * 8);
            self.interrupt_flag &= !mask;

            return;
        }

        /*
           _000001AC:
               moves lowest 8 bits of the 32 bit value in r2 to address 0x40000301 - HALTCNT register
               mov r12, #0x4000000
               strb r2, [r12, #0x301]
        */

        if address & !1 == 0x4000300 {
            if address.is_clear(0) {
                self.postflg = value & 1;
            } else {
                self.haltcnt = Some(value)
            }

            return;
        }

        match address >> 24 {
            0x00 => {}
            0x02 => self.ewram[Bus::ewram_index(address)] = value,
            0x03 => self.iwram[Bus::iwram_index(address)] = value,
            0x04 => {
                let mut half_word = self.read_register(address & !1);
                let new_half_word = if address.is_clear(0) {
                    half_word.clear_bit_range(0..8);
                    half_word | value as u16
                } else {
                    half_word.clear_bit_range(8..16);
                    half_word | (value as u16) << 8
                };

                self.write_register(address & !1, new_half_word);
            }
            0x05 => {
                let index = Bus::palette_index(address) & !1;
                self.ppu.palette_ram[index] = value;
                self.ppu.palette_ram[index + 1] = value;
            }
            0x06 => {
                // https://gbadev.net/tonc/bitmaps.html
                // https://www.patater.com/gbaguy/gba/ch5.htm
                // https://problemkaputt.de/gbatek-gba-unpredictable-things.htm
                let index = Bus::vram_index(address) & !1;
                let obj_start = if self.ppu.current_mode() >= 3 {
                    0x14000
                } else {
                    0x10000
                };
                if index + 1 < obj_start {
                    self.ppu.vram[index] = value;
                    self.ppu.vram[index + 1] = value;
                }
            }
            0x07 => {}
            0x08..=0x0D => {}
            0x0E | 0x0F => self.write_backup_byte(address, value),
            _ => {}
        }
    }

    pub fn read_u16(&mut self, mut address: u32, access_type: AccessType) -> u16 {
        self.cost(address, 16, access_type);

        if self.is_eeprom_address(address) {
            return self.eeprom_read_u16();
        }

        if address & !1 == 0x4000300 {
            if address.is_clear(0) {
                return self.postflg as u16;
            } else {
                return 0;
            }
        }

        let shifted_address = address >> 24;
        if !matches!(shifted_address, 0x0E | 0x0F) {
            address.clear_bit(0);
        }

        let little_endian =
            |arr: &[u8], index: usize| u16::from_le_bytes([arr[index], arr[index + 1]]);

        match shifted_address {
            0x00 => (self.last_bios_fetch >> (8 * (address.get_bit_range(0..2)))) as u16,
            0x02 => little_endian(&*self.ewram, Bus::ewram_index(address)),
            0x03 => little_endian(&*self.iwram, Bus::iwram_index(address)),
            0x04 => self.read_register(address),
            0x05 => little_endian(&*self.ppu.palette_ram, Bus::palette_index(address)),
            0x06 => little_endian(&*self.ppu.vram, Bus::vram_index(address)),
            0x07 => little_endian(&*self.ppu.oam, Bus::oam_index(address)),
            0x08..=0x0D => u16::from_le_bytes([
                self.gamepak.read_rom_region(address),
                self.gamepak.read_rom_region(address + 1),
            ]),
            0x0E | 0x0F => {
                let byte = self.read_backup_byte(address) as u16;
                (byte << 8) | byte
            }
            _ => (self.last_instruction_read >> (8 * (address.get_bit_range(0..2)))) as u16,
        }
    }

    pub fn write_u16(&mut self, mut address: u32, value: u16, access_type: AccessType) {
        self.cost(address, 16, access_type);

        if self.is_eeprom_address(address) {
            self.eeprom_write_u16(value);

            return;
        }

        let bytes = value.to_le_bytes();

        if address & !1 == 0x4000300 {
            if address.is_clear(0) {
                self.postflg = value.get_bit_range(0..8) as u8;
            } else {
                self.haltcnt = Some(value.get_bit_range(8..16) as u8);
            }

            return;
        }

        let shifted_address = address >> 24;
        if !matches!(shifted_address, 0x0E | 0x0F) {
            address.clear_bit(0);
        }

        match shifted_address {
            0x00 => {}
            0x02 => {
                let index = Bus::ewram_index(address);
                self.ewram[index] = bytes[0];
                self.ewram[index + 1] = bytes[1];
            }
            0x03 => {
                let index = Bus::iwram_index(address);
                self.iwram[index] = bytes[0];
                self.iwram[index + 1] = bytes[1];
            }
            0x04 => self.write_register(address, value),
            0x05 => {
                let index = Bus::palette_index(address);
                self.ppu.palette_ram[index] = bytes[0];
                self.ppu.palette_ram[index + 1] = bytes[1];
            }
            0x06 => {
                let index = Bus::vram_index(address);
                self.ppu.vram[index] = bytes[0];
                self.ppu.vram[index + 1] = bytes[1];
            }
            0x07 => {
                let index = Bus::oam_index(address);
                self.ppu.oam[index] = bytes[0];
                self.ppu.oam[index + 1] = bytes[1];
            }

            0x08..=0x0D => {}
            0x0E | 0x0F => self.write_backup_byte(address, bytes[(address & 1) as usize]),
            _ => {}
        }
    }

    pub fn read_u32(&mut self, mut address: u32, access_type: AccessType) -> u32 {
        self.cost(address, 32, access_type);

        if address & !1 == 0x4000300 {
            if address.is_clear(0) {
                return self.postflg as u32;
            } else {
                return 0;
            }
        }

        let shifted_address = address >> 24;
        if !matches!(shifted_address, 0x0E | 0x0F) {
            address.clear_bit_range(0..2);
        }

        let little_endian = |arr: &[u8], index: usize| {
            u32::from_le_bytes([arr[index], arr[index + 1], arr[index + 2], arr[index + 3]])
        };

        match shifted_address {
            0x00 => self.last_bios_fetch,
            0x02 => little_endian(&*self.ewram, Bus::ewram_index(address)),
            0x03 => little_endian(&*self.iwram, Bus::iwram_index(address)),
            0x04 => {
                let low_half_word = self.read_register(address);
                let high_half_word = self.read_register(address + 2);

                (high_half_word as u32) << 16 | low_half_word as u32
            }
            0x05 => little_endian(&*self.ppu.palette_ram, Bus::palette_index(address)),
            0x06 => little_endian(&*self.ppu.vram, Bus::vram_index(address)),
            0x07 => little_endian(&*self.ppu.oam, Bus::oam_index(address)),
            0x08..=0x0D => u32::from_le_bytes([
                self.gamepak.read_rom_region(address),
                self.gamepak.read_rom_region(address + 1),
                self.gamepak.read_rom_region(address + 2),
                self.gamepak.read_rom_region(address + 3),
            ]),
            0x0E | 0x0F => {
                let byte = self.read_backup_byte(address) as u32;
                (byte << 24) | (byte << 16) | (byte << 8) | byte
            }
            _ => self.last_instruction_read,
        }
    }

    pub fn write_u32(&mut self, mut address: u32, value: u32, access_type: AccessType) {
        self.cost(address, 32, access_type);
        let bytes = value.to_le_bytes();
        if address & !1 == 0x4000300 {
            if address.is_clear(0) {
                self.postflg = value.get_bit_range(0..8) as u8;
            } else {
                self.haltcnt = Some(value.get_bit_range(8..16) as u8);
            }

            return;
        }

        let shifted_address = address >> 24;
        if !matches!(shifted_address, 0x0E | 0x0F) {
            address.clear_bit_range(0..2);
        }

        match shifted_address {
            0x00 => {}
            0x02 => {
                let index = Bus::ewram_index(address);
                self.ewram[index] = bytes[0];
                self.ewram[index + 1] = bytes[1];
                self.ewram[index + 2] = bytes[2];
                self.ewram[index + 3] = bytes[3];
            }
            0x03 => {
                let index = Bus::iwram_index(address);
                self.iwram[index] = bytes[0];
                self.iwram[index + 1] = bytes[1];
                self.iwram[index + 2] = bytes[2];
                self.iwram[index + 3] = bytes[3];
            }
            0x04 => {
                self.write_register(address, u16::from_le_bytes([bytes[0], bytes[1]]));
                self.write_register(address + 2, u16::from_le_bytes([bytes[2], bytes[3]]));
            }
            0x05 => {
                let index = Bus::palette_index(address);
                self.ppu.palette_ram[index] = bytes[0];
                self.ppu.palette_ram[index + 1] = bytes[1];
                self.ppu.palette_ram[index + 2] = bytes[2];
                self.ppu.palette_ram[index + 3] = bytes[3];
            }
            0x06 => {
                let index = Bus::vram_index(address);
                self.ppu.vram[index] = bytes[0];
                self.ppu.vram[index + 1] = bytes[1];
                self.ppu.vram[index + 2] = bytes[2];
                self.ppu.vram[index + 3] = bytes[3];
            }
            0x07 => {
                let index = Bus::oam_index(address);
                self.ppu.oam[index] = bytes[0];
                self.ppu.oam[index + 1] = bytes[1];
                self.ppu.oam[index + 2] = bytes[2];
                self.ppu.oam[index + 3] = bytes[3];
            }

            0x08..=0x0D => {}
            0x0E | 0x0F => self.write_backup_byte(address, bytes[(address & 3) as usize]),
            _ => {}
        }
    }

    pub fn idle(&mut self, cycles: u64) {
        self.scheduler.current += cycles;
    }

    pub fn cost(&mut self, address: u32, width: u32, access_type: AccessType) {
        let region = (address >> 24) as usize;
        let cycles = match region {
            0x00 | 0x03 | 0x04 | 0x07 => 1,
            0x02 => {
                if width == 32 {
                    6
                } else {
                    3
                }
            }
            0x05 | 0x06 => {
                if width == 32 {
                    2
                } else {
                    1
                }
            }
            0x08..=0x0D => {
                let wait_state = WaitState::from_address(address);
                let mut cycles = 1 + wait_state.cycles(self.waitcnt, access_type);
                if width == 32 {
                    cycles += 1 + wait_state.cycles(self.waitcnt, AccessType::Sequential);
                }

                cycles
            }
            0x0E | 0x0F => 1 + WaitState::SramWaitControl.cycles(self.waitcnt, access_type),
            _ => 1,
        };

        self.scheduler.current += cycles as u64;
    }

    fn read_register(&mut self, address: u32) -> u16 {
        //eprintln!("READ REGISTER: address={:08x}", address);
        match address {
            // LCD I/O Registers
            0x4000000 => self.ppu.dispcnt,
            // 0x4000002 => {} // Undocumented 16 bit register (read + write)
            0x4000004 => self.ppu.dispstat,
            0x4000006 => self.ppu.vcount as u16,
            0x4000008 | 0x400000A | 0x400000C | 0x400000E => self.ppu.bg_control.read_u16(address),
            0x4000048 | 0x400004A => self.ppu.window_features.read_u16(address),
            0x4000050 | 0x4000052 => self.ppu.color_special_effects.read_u16(address),

            // Sound Registers
            // 0x4000060 => {} // Channel 1 Sweep register (NR10) (SOUND1CNT_L), 16 bit register (read + write)
            // 0x4000062 => {} // Channel 1 Duty/Length/Envelope (NR11, NR12) (SOUND1CNT_H), 16 bit register (read + write)
            // 0x4000064 => {} // Channel 1 Frequency/Control (NR13, NR14) (SOUND1CNT_X), 16 bit register (read + write)
            // 0x4000068 => {} // Channel 2 Duty/Length/Envelope (NR21, NR22) (SOUND2CNT_L), 16 bit register (read + write)
            // 0x400006C => {} // Channel 2 Frequency/Control (NR23, NR24) (SOUND2CNT_H), 16 bit register (read + write)
            // 0x4000070 => {} // Channel 3 Stop/Wave RAM select (NR30) (SOUND3CNT_L), 16 bit register (read + write)
            // 0x4000072 => {} // Channel 3 Length/Volume (NR31, NR32), 16 bit register (read + write)
            // 0x4000074 => {} // Channel 3 Frequency/Control (NR33, NR34) (SOUND3CNT_X), 16 bit register (read + write)
            // 0x4000078 => {} // Channel 4 Length/Envelope (NR41, NR42) (SOUND4CNT_L), 16 bit register (read + write)
            // 0x400007C => {} // Channel 4 Frequency/Control (NR43, NR44) (SOUND4CNT_H), 16 bit register (read + write)
            // 0x4000080 => {} // Control Stereo/Volume/Enable (NR50, NR51) (SOUNDCNT_L), 16 bit register (read + write)
            // 0x4000082 => {} // Control Mixing/DMA Control (SOUNDCNT_H), 16 bit register (read + write)
            // 0x4000084 => {} // Control Sound on/off (NR52) (SOUNDCNT_X), 16 bit register (read + write)
            // 0x4000088 => {} // BIOS/Sound PWM Control (SOUNDBIAS), 16 bit register (read + write)
            // 0x4000090 => {} // Channel 3 Wave Pattern RAM (2 banks) (WAVE_RAM) 2x10h in size, (read + write)

            // DMA Transfer Channels
            0x40000BA => self.dma.channels[0].control_register,
            0x40000C6 => self.dma.channels[1].control_register,
            0x40000D2 => self.dma.channels[2].control_register,
            0x40000DE => self.dma.channels[3].control_register,

            // Timer Registers
            0x4000100 => self.timers.timers[0].current_counter(self.scheduler.current),
            0x4000102 => self.timers.timers[0].control_register,
            0x4000104 => self.timers.timers[1].current_counter(self.scheduler.current),
            0x4000106 => self.timers.timers[1].control_register,
            0x4000108 => self.timers.timers[2].current_counter(self.scheduler.current),
            0x400010A => self.timers.timers[2].control_register,
            0x400010C => self.timers.timers[3].current_counter(self.scheduler.current),
            0x400010E => self.timers.timers[3].control_register,

            // Serial Communication (1)
            // https://problemkaputt.de/gbatek-sio-multi-player-mode.htm
            0x4000120 => self.serial.sio_data[0],
            0x4000122 => self.serial.sio_data[1],
            0x4000124 => self.serial.sio_data[2],
            0x4000126 => self.serial.sio_data[3],
            0x4000128 => self.serial.siocnt,
            0x400012A => self.serial.siomlt_send,

            // Keypad Input
            0x4000130 => self.keypad.keyinput,
            0x4000132 => self.keypad.keycnt,

            // Serial Communication (2)
            0x4000134 => self.serial.rcnt,
            0x4000140 => self.serial.joycnt,
            0x4000150 => self.serial.joy_recv_l,
            0x4000152 => self.serial.joy_recv_h,
            0x4000154 => self.serial.joy_trans_l,
            0x4000156 => self.serial.joy_trans_h,
            0x4000158 => self.serial.joystat,

            //Interrupt, Waitstate, and Power-Down Control
            0x4000200 => self.interrupt_enable,
            0x4000202 => self.interrupt_flag,
            0x4000204 => self.waitcnt,
            0x4000208 => self.interrupt_master_enable as u16,
            0x4000300 => self.postflg as u16,
            0x4000410 => 0x0FF as u16,

            // https://problemkaputt.de/gbatek-gba-system-control.htm
            address if (address & 0xFF00FFFF) == 0x04000800 => {
                (self.internal_memory_control >> (8 * (address & 2))) as u16
            }

            // https://github.com/mgba-emu/mgba/blob/master/src/gba/io.c
            // https://codeberg.org/nba-emu/NanoBoyAdvance/src/branch/master/src/nba/src/bus/io.cc
            0x4000066 | 0x400006A | 0x400006E | 0x4000076 | 0x400007A | 0x400007E | 0x4000086
            | 0x400008A | 0x4000136 | 0x4000142 | 0x400015A | 0x4000206 | 0x4000302 => 0,
            0x40000B8 | 0x40000C4 | 0x40000D0 | 0x40000DC => 0,
            0x400020A => 0,

            _ => (self.last_instruction_read >> (8 * (address & 2))) as u16,
        }
    }

    fn write_register(&mut self, address: u32, mut value: u16) {
        //eprintln!("WRITE REGISTER: address={:08x}, value={:16b}", address, value);
        match address {
            // LCD I/O Registers
            0x4000000 => self.ppu.write_dispcnt(value),
            0x4000002 => {} // Undocumented 16 bit register (read + write)
            0x4000004 => self.ppu.write_dispstat(value),
            0x4000008 | 0x400000A | 0x400000C | 0x400000E => {
                self.ppu.bg_control.write_u16(address, value);
            }
            0x4000010 | 0x4000012 | 0x4000014 | 0x4000016 | 0x4000018 | 0x400001A | 0x400001C
            | 0x400001E => self.ppu.bg_text_offset.write_u16(address, value),
            0x4000020 | 0x4000022 | 0x4000024 | 0x4000026 => {
                self.ppu.bg2_affine_parameters.write_u16(address, value);
            }
            0x4000028 | 0x400002A | 0x400002C | 0x400002E => {
                self.ppu.bg2_affine_reference.write_u16(address, value);
                match address {
                    0x4000028 | 0x400002A => self
                        .ppu
                        .bg2_affine_state
                        .write_x(self.ppu.bg2_affine_reference.from_index(0)),
                    _ => self
                        .ppu
                        .bg2_affine_state
                        .write_y(self.ppu.bg2_affine_reference.from_index(1)),
                }
            }
            0x4000030 | 0x4000032 | 0x4000034 | 0x4000036 => {
                self.ppu.bg3_affine_parameters.write_u16(address, value);
            }
            0x4000038 | 0x400003A | 0x400003C | 0x400003E => {
                self.ppu.bg3_affine_reference.write_u16(address, value);
                match address {
                    0x4000038 | 0x400003A => self
                        .ppu
                        .bg3_affine_state
                        .write_x(self.ppu.bg3_affine_reference.from_index(0)),
                    _ => self
                        .ppu
                        .bg3_affine_state
                        .write_y(self.ppu.bg3_affine_reference.from_index(1)),
                }
            }
            0x4000040 | 0x4000042 | 0x4000044 | 0x4000046 | 0x4000048 | 0x400004A => {
                self.ppu.window_features.write_u16(address, value)
            }
            0x400004C => self.ppu.mosaic = value,
            0x4000050 | 0x4000052 | 0x4000054 => {
                self.ppu.color_special_effects.write_u16(address, value)
            }

            // Sound Registers
            0x4000060 => {} // Channel 1 Sweep register (NR10) (SOUND1CNT_L), 16 bit register (read + write)
            0x4000062 => {} // Channel 1 Duty/Length/Envelope (NR11, NR12) (SOUND1CNT_H), 16 bit register (read + write)
            0x4000064 => {} // Channel 1 Frequency/Control (NR13, NR14) (SOUND1CNT_X), 16 bit register (read + write)
            0x4000068 => {} // Channel 2 Duty/Length/Envelope (NR21, NR22) (SOUND2CNT_L), 16 bit register (read + write)
            0x400006C => {} // Channel 2 Frequency/Control (NR23, NR24) (SOUND2CNT_H), 16 bit register (read + write)
            0x4000070 => {} // Channel 3 Stop/Wave RAM select (NR30) (SOUND3CNT_L), 16 bit register (read + write)
            0x4000072 => {} // Channel 3 Length/Volume (NR31, NR32), 16 bit register (read + write)
            0x4000074 => {} // Channel 3 Frequency/Control (NR33, NR34) (SOUND3CNT_X), 16 bit register (read + write)
            0x4000078 => {} // Channel 4 Length/Envelope (NR41, NR42) (SOUND4CNT_L), 16 bit register (read + write)
            0x400007C => {} // Channel 4 Frequency/Control (NR43, NR44) (SOUND4CNT_H), 16 bit register (read + write)
            0x4000080 => {} // Control Stereo/Volume/Enable (NR50, NR51) (SOUNDCNT_L), 16 bit register (read + write)
            0x4000082 => {} // Control Mixing/DMA Control (SOUNDCNT_H), 16 bit register (read + write)
            0x4000084 => {} // Control Sound on/off (NR52) (SOUNDCNT_X), 16 bit register (read + write)
            0x4000088 => {} // BIOS/Sound PWM Control (SOUNDBIAS), 16 bit register (read + write)
            0x4000090 => {} // Channel 3 Wave Pattern RAM (2 banks) (WAVE_RAM) 2x10h in size, (read + write)
            0x40000A0 | 0x40000A2 => {} // Channel A FIFO, Data 0-3, (FIFO_A) (write only), 32 bit register
            0x40000A4 | 0x40000A6 => {} // Channel B FIFO, Data 0-3, (FIFO_B) (write only), 32 bit register

            // DMA Transfer Channels
            0x40000B0 | 0x40000B2 => self.dma.channels[0].write_source_address(address, value),
            0x40000B4 | 0x40000B6 => self.dma.channels[0].write_destination_address(address, value),
            0x40000B8 => self.dma.channels[0].word_count_register = value,
            0x40000BA => {
                self.dma.channels[0].write_control_register(value);
                self.run_dma(0, None);
            }
            0x40000BC | 0x40000BE => self.dma.channels[1].write_source_address(address, value),
            0x40000C0 | 0x40000C2 => self.dma.channels[1].write_destination_address(address, value),
            0x40000C4 => self.dma.channels[1].word_count_register = value,
            0x40000C6 => {
                self.dma.channels[1].write_control_register(value);
                self.run_dma(1, None);
            }
            0x40000C8 | 0x40000CA => self.dma.channels[2].write_source_address(address, value),
            0x40000CC | 0x40000CE => self.dma.channels[2].write_destination_address(address, value),
            0x40000D0 => self.dma.channels[2].word_count_register = value,
            0x40000D2 => {
                self.dma.channels[2].write_control_register(value);
                self.run_dma(2, None);
            }
            0x40000D4 | 0x40000D6 => self.dma.channels[3].write_source_address(address, value),
            0x40000D8 | 0x40000DA => self.dma.channels[3].write_destination_address(address, value),
            0x40000DC => self.dma.channels[3].word_count_register = value,
            0x40000DE => {
                self.dma.channels[3].write_control_register(value);
                self.run_dma(3, None);
            }

            // Timer Registers
            0x4000100 => self.timers.timers[0].counter_register = value,
            0x4000102 => self.timers.timers[0].write_control_register(value, &mut self.scheduler),
            0x4000104 => self.timers.timers[1].counter_register = value,
            0x4000106 => self.timers.timers[1].write_control_register(value, &mut self.scheduler),
            0x4000108 => self.timers.timers[2].counter_register = value,
            0x400010A => self.timers.timers[2].write_control_register(value, &mut self.scheduler),
            0x400010C => self.timers.timers[3].counter_register = value,
            0x400010E => self.timers.timers[3].write_control_register(value, &mut self.scheduler),

            // Serial Communication (1)
            0x4000120 => self.serial.sio_data[0] = value,
            0x4000122 => self.serial.sio_data[1] = value,
            0x4000124 => self.serial.sio_data[2] = value,
            0x4000126 => self.serial.sio_data[3] = value,
            0x4000128 => self.serial.siocnt = value,
            0x400012A => self.serial.siomlt_send = value,

            // Keypad Input
            0x4000132 => self.keypad.keycnt = value,

            // Serial Communication (2)
            0x4000134 => self.serial.rcnt = value,
            0x4000140 => self.serial.joycnt = value,
            0x4000150 => self.serial.joy_recv_l = value,
            0x4000152 => self.serial.joy_recv_h = value,
            0x4000154 => self.serial.joy_trans_l = value,
            0x4000156 => self.serial.joy_trans_h = value,
            0x4000158 => self.serial.joystat = value,

            //Interrupt, Waitstate, and Power-Down Control
            0x4000200 => {
                self.interrupt_enable = {
                    value.clear_bit_range(14..16);

                    value
                }
            }
            0x4000202 => self.interrupt_flag &= !value,
            0x4000204 => self.waitcnt = value,
            0x4000208 => self.interrupt_master_enable = value.get_bit(0) as u32,
            address if (address & 0xFF00FFFF) == 0x04000800 => {
                if address.get_bit(1) == 0 {
                    self.internal_memory_control.clear_bit_range(0..16);
                    self.internal_memory_control |= value as u32
                } else {
                    self.internal_memory_control.clear_bit_range(16..32);
                    self.internal_memory_control |= (value as u32) << 16
                }
            }
            _ => {}
        }
    }

    pub fn pending_interrupt(&self) -> usize {
        let mut pending = self.interrupt_enable & self.interrupt_flag;
        pending.clear_bit_range(14..16);

        return pending as usize;
    }

    pub fn ime_enabled(&self) -> bool {
        self.interrupt_master_enable.is_set(0)
    }

    pub fn skip_boot(&mut self) {
        self.postflg = 1;
        self.ppu.skip_boot();
    }

    pub fn take_halt_request(&mut self) -> Option<u8> {
        self.haltcnt.take()
    }

    pub fn run_dma(&mut self, channel: usize, trigger: Option<Trigger>) {
        if !self.dma.channels[channel].start_transfer(trigger) {
            return;
        }

        if (0x08..=0x0F).contains(&(self.dma.channels[channel].current_source_address >> 24))
            && (0x08..=0x0F)
                .contains(&(self.dma.channels[channel].current_destination_address >> 24))
        {
            self.idle(4);
        } else {
            self.idle(2);
        }

        if channel == 3 {
            if self.is_eeprom_address(self.dma.channels[channel].current_source_address)
                || self.is_eeprom_address(self.dma.channels[channel].current_destination_address)
            {
                if let BackupChip::Eeprom(eeprom) = &mut self.gamepak.backup_chip {
                    if !eeprom.size_known {
                        match self.dma.channels[channel].current_word_count {
                            17 | 81 => eeprom.increase_capacity(),
                            _ => {}
                        }

                        // can there be wierd counts?
                        if matches!(
                            self.dma.channels[channel].current_word_count,
                            9 | 17 | 73 | 81
                        ) {
                            eeprom.size_known = true;
                        }
                    }
                }
            }
        }

        let mut access_type = AccessType::Nonsequential;
        while self.dma.channels[channel].current_word_count != 0 {
            let source_address = self.dma.channels[channel].current_source_address;
            let destination_address = self.dma.channels[channel].current_destination_address;

            match self.dma.channels[channel].transfer_type {
                TransferType::Halfword => {
                    let halfword = self.read_u16(source_address, access_type);
                    self.write_u16(destination_address, halfword, access_type);
                }
                TransferType::Word => {
                    let word = self.read_u32(source_address, access_type);
                    self.write_u32(destination_address, word, access_type);
                }
            }

            access_type = AccessType::Sequential;
            self.dma.channels[channel].update_address_pointers();

            self.dma.channels[channel].current_word_count -= 1;
        }

        self.dma.channels[channel].reload_destination_address();
        self.dma.channels[channel].transfer_complete(&mut self.interrupt_flag);
    }

    fn eeprom_read_u16(&mut self) -> u16 {
        match &mut self.gamepak.backup_chip {
            BackupChip::Eeprom(eeprom) => eeprom.read_bit(),
            _ => unreachable!(),
        }
    }

    fn eeprom_write_u16(&mut self, value: u16) {
        match &mut self.gamepak.backup_chip {
            BackupChip::Eeprom(eeprom) => eeprom.write_bit(value),
            _ => unreachable!(),
        }
    }

    fn is_eeprom_address(&self, address: u32) -> bool {
        if !matches!(self.gamepak.backup_chip, BackupChip::Eeprom(_)) {
            return false;
        }

        if self.gamepak.rom.len() > 0x1000000 {
            (0x0DFFFF00..=0x0DFFFFFF).contains(&address)
        } else {
            address >> 24 == 0x0D
        }
    }

    pub fn sound_fifo(&mut self, timer_id: u8) {}
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::{gamepak::BackupType, utils::create_bus};

    #[test]
    fn test_bus_write_u32() {
        let mut bus = create_bus(BackupType::Flash);

        let address = 0x05000001;
        let value = 0b100000001 as u32;
        let access_type = AccessType::Sequential;
        bus.write_u32(address, value, access_type);

        assert_eq!(bus.scheduler.current, 2);
        assert_eq!(&bus.ppu.palette_ram[..4], [1, 1, 0, 0]);
    }

    #[test]
    fn test_interrupt_write_flag() {
        let mut bus = create_bus(BackupType::Flash);

        bus.interrupt_flag = 0b0101;
        bus.write_u16(0x4000200, !0, AccessType::Sequential);
        assert_eq!(bus.pending_interrupt(), 0b0101);

        bus.write_u16(0x4000202, 0b0001, AccessType::Sequential);
        assert_eq!(bus.pending_interrupt(), 0b0100);

        bus.write_u16(0x4000202, 0b0100, AccessType::Sequential);
        assert_eq!(bus.pending_interrupt(), 0);
    }

    #[test]
    fn test_preservation_other_byte() {
        let mut bus = create_bus(BackupType::Flash);

        bus.interrupt_flag = 0x0101;
        bus.write_u8(0x4000202, 0x01, AccessType::Sequential);
        assert_eq!(bus.interrupt_flag, 0x0100);

        bus.write_u8(0x4000203, 0x01, AccessType::Sequential);
        assert_eq!(bus.interrupt_flag, 0);
    }
}
