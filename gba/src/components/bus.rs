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
    dma::DmaChannels,
    gamepak::{BackupChip, GamePak},
    ppu::PPU,
    scheduler::EventScheduler,
    timer::Timers,
    utils::{BitOps, zero_arr},
};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum AccessType {
    Sequential, // Memory address related to previous address, incremented by + 2 (half word) or +4 (word)
    Nonsequential, // Memory address is fetched and has nothing to do with the previous instruction
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
    interrupt_master_enable: u32,
    pub interrupt_enable: u16,
    pub interrupt_flag: u16,
    postflg: u8,
    haltcnt: Option<u8>,
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
            interrupt_master_enable: 0,
            interrupt_flag: 0,
            interrupt_enable: 0,
            postflg: 0,
            haltcnt: None,
        }
    }

    #[inline]
    pub fn ewram_index(address: u32) -> usize {
        let address = address.get_bit_range(0..18);

        address as usize
    }

    #[inline]
    pub fn iwram_index(address: u32) -> usize {
        let address = address.get_bit_range(0..15);

        address as usize
    }

    #[inline]
    pub fn palette_index(address: u32) -> usize {
        let address = address.get_bit_range(0..10);

        address as usize
    }

    #[inline]
    pub fn oam_index(address: u32) -> usize {
        let address = address.get_bit_range(0..10);

        address as usize
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

    // Maybe reconsider design of the backup functions
    // just fix enough for code to compile
    fn backup_byte(&self, address: u32) -> Option<usize> {
        if matches!(self.gamepak.backup_chip, BackupChip::None) {
            return None;
        }

        let base_address = 0x0E000000;
        let mask = match &self.gamepak.backup_chip {
            BackupChip::Eeprom(eeprom) => (eeprom.memory.len() - 1) as u32,
            BackupChip::Sram(sram) => (sram.memory.len() - 1) as u32,
            BackupChip::Flash(flash) => (flash.memory.len() - 1) as u32,
            _ => unreachable!(),
        };

        let index = ((address - base_address) & mask) as usize;

        Some(index)
    }

    #[inline]
    pub fn read_backup_byte(&self, address: u32) -> u8 {
        match self.backup_byte(address) {
            Some(index) => match &self.gamepak.backup_chip {
                BackupChip::Eeprom(eeprom) => eeprom.memory[index],
                BackupChip::Sram(sram) => sram.memory[index],
                BackupChip::Flash(flash) => flash.memory[index],
                _ => unreachable!(),
            },
            None => 0,
        }
    }

    #[inline]
    pub fn write_backup_byte(&mut self, address: u32, value: u8) {
        match self.backup_byte(address) {
            Some(index) => match &mut self.gamepak.backup_chip {
                BackupChip::Eeprom(eeprom) => eeprom.write(index, value),
                BackupChip::Sram(sram) => sram.write(index, value),
                BackupChip::Flash(flash) => flash.write(index, value),
                _ => unreachable!(),
            },
            None => {}
        }
    }

    pub fn read_u8(&mut self, address: u32, access_type: AccessType) -> u8 {
        self.cost(address, 8, access_type);

        match address >> 24 {
            0x00 => (self.last_bios_fetch >> (8 * (address.get_bit_range(0..2)))) as u8,
            0x02 => self.ewram[Bus::ewram_index(address)],
            0x03 => self.iwram[Bus::iwram_index(address)],
            0x04 => {
                let half_word = self.read_register_16(address & !1);
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
            ands r1, r2, #0x80 = r1 = r2 & 0x80 checking bit 7 (serial), which i will never implement - s updates condition flag
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
                self.postflg = value & 1; // postflag is touched in boot sequence, consider if want to support bios
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
                let mut half_word = self.read_register_16(address & !1);
                let new_half_word = if address.is_clear(0) {
                    half_word.clear_bit_range(0..8);
                    half_word | value as u16
                } else {
                    half_word.clear_bit_range(8..16);
                    half_word | (value as u16) << 8
                };

                self.write_register_16(address & !1, new_half_word);
            }
            0x05 => {
                let index = Bus::palette_index(address) & !1;
                self.ppu.palette_ram[index] = value;
                self.ppu.palette_ram[index + 1] = value;
            }
            0x06 => {
                // https://gbadev.net/tonc/bitmaps.html
                // https://www.patater.com/gbaguy/gba/ch5.htm
                let index = Bus::vram_index(address) & !1;
                self.ppu.vram[index] = value;
                self.ppu.vram[index + 1] = value;
            }
            0x07 => {}
            0x08..=0x0D => {}
            0x0E | 0x0F => self.write_backup_byte(address, value),
            _ => {}
        }
    }

    pub fn read_u16(&mut self, mut address: u32, access_type: AccessType) -> u16 {
        self.cost(address, 16, access_type);
        address.clear_bit(0); //ensure even
        let little_endian =
            |arr: &[u8], index: usize| u16::from_le_bytes([arr[index], arr[index + 1]]);

        match address >> 24 {
            0x00 => (self.last_bios_fetch >> (8 * (address.get_bit_range(0..2)))) as u16,
            0x02 => little_endian(&*self.ewram, Bus::ewram_index(address)),
            0x03 => little_endian(&*self.iwram, Bus::iwram_index(address)),
            0x04 => self.read_register_16(address),
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
        let bytes = value.to_le_bytes();
        address.clear_bit(0); //ensure even

        match address >> 24 {
            0x00 => {} // BIOS no write,
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
            0x04 => self.write_register_16(address, value),
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
            0x0E | 0x0F => self.write_backup_byte(address, bytes[0]),
            _ => {}
        }
    }

    pub fn read_u32(&mut self, mut address: u32, access_type: AccessType) -> u32 {
        self.cost(address, 32, access_type);
        address.clear_bit_range(0..2); // every 4th address

        let little_endian = |arr: &[u8], index: usize| {
            u32::from_le_bytes([arr[index], arr[index + 1], arr[index + 2], arr[index + 3]])
        };

        match address >> 24 {
            0x00 => self.last_bios_fetch,
            0x02 => little_endian(&*self.ewram, Bus::ewram_index(address)),
            0x03 => little_endian(&*self.iwram, Bus::iwram_index(address)),
            0x04 => {
                let low_half_word = self.read_register_16(address);
                let high_half_word = self.read_register_16(address + 2);

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
        address.clear_bit_range(0..2); // every 4th address

        match address >> 24 {
            0x00 => {} // BIOS no write,
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
                self.write_register_16(address, u16::from_le_bytes([bytes[0], bytes[1]]));
                self.write_register_16(address + 2, u16::from_le_bytes([bytes[2], bytes[3]]));
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
            0x0E | 0x0F => self.write_backup_byte(address, bytes[0]),
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
            0x08..=0x0D => self.rom_cost(width, access_type),
            0x0E | 0x0F => 5,
            _ => 1,
        };

        self.scheduler.current += cycles as u64;
    }

    pub fn rom_cost(&mut self, width: u32, access_type: AccessType) -> usize {
        let first = match access_type {
            AccessType::Nonsequential => 5,
            AccessType::Sequential => 3,
        };

        if width == 32 { first + 3 } else { first }
    }

    // Take regiisters from an early commit and just map them back for now
    // GBATEK lists 32-bit registers at TWO halfword addresses (e.g. "40000D4h,0D6h").
    // Both halves are independently addressable, so each gets its own table entry (0x40000D4 = bits 0-15, 0x40000D6 = bits 16-31)
    fn read_register_16(&mut self, address: u32) -> u16 {
        match address {
            // LCD I/O Registers
            // 0x4000000 => {} // LCD Control (DISPCNT), 16 bit register (read + write)
            // 0x4000002 => {} // Undocumented 16 bit register (read + write)
            // 0x4000004 => {} // Stat & LYC, 16 bit register (read + write)
            // 0x4000006 => {} // LY, 16 bit, (VCOUNT), read only
            // 0x4000008 => {} // BG0 Control (BG0CNT) 16 bit register (read + write)
            // 0x400000A => {} // BG1 Control (BG1CNT) 16 bit register (read + write)
            // 0x400000C => {} // BG2 Control (BG2CNT) 16 bit register (read + write)
            // 0x400000E => {} // BG3 Control (BG3CNT) 16 bit register (read + write)
            // 0x4000048 => {} // Inside of Window 0 and 1 (WININ), 16 bit register (read + write)
            // 0x400004A => {} // Inside of OBJ Window & Outside of Windows 2 (WINOUT) (read + write)
            // 0x400004E => {} // Not Used
            // 0x4000050 => {} // Color Special Effects Selection (BLDCNT), 16 bit register (read + write)
            // 0x4000052 => {} // Alpha Blending Coefficients (BLDALPHA), 16 bit register (read + write)
            // 0x4000056 => {} // Not Used

            // Sound Registers
            // 0x4000060 => {} // Channel 1 Sweep register (NR10) (SOUND1CNT_L), 16 bit register (read + write)
            // 0x4000062 => {} // Channel 1 Duty/Length/Envelope (NR11, NR12) (SOUND1CNT_H), 16 bit register (read + write)
            // 0x4000064 => {} // Channel 1 Frequency/Control (NR13, NR14) (SOUND1CNT_X), 16 bit register (read + write)
            // 0x4000066 => {} // Not Used
            // 0x4000068 => {} // Channel 2 Duty/Length/Envelope (NR21, NR22) (SOUND2CNT_L), 16 bit register (read + write)
            // 0x400006A => {} // Not Used
            // 0x400006C => {} // Channel 2 Frequency/Control (NR23, NR24) (SOUND2CNT_H), 16 bit register (read + write)
            // 0x400006E => {} // Not Used
            // 0x4000070 => {} // Channel 3 Stop/Wave RAM select (NR30) (SOUND3CNT_L), 16 bit register (read + write)
            // 0x4000072 => {} // Channel 3 Length/Volume (NR31, NR32), 16 bit register (read + write)
            // 0x4000074 => {} // Channel 3 Frequency/Control (NR33, NR34) (SOUND3CNT_X), 16 bit register (read + write)
            // 0x4000076 => {} // Not Used
            // 0x4000078 => {} // Channel 4 Length/Envelope (NR41, NR42) (SOUND4CNT_L), 16 bit register (read + write)
            // 0x400007A => {} // Not Used
            // 0x400007C => {} // Channel 4 Frequency/Control (NR43, NR44) (SOUND4CNT_H), 16 bit register (read + write)
            // 0x400007E => {} // Not Used
            // 0x4000080 => {} // Control Stereo/Volume/Enable (NR50, NR51) (SOUNDCNT_L), 16 bit register (read + write)
            // 0x4000082 => {} // Control Mixing/DMA Control (SOUNDCNT_H), 16 bit register (read + write)
            // 0x4000084 => {} // Control Sound on/off (NR52) (SOUNDCNT_X), 16 bit register (read + write)
            // 0x4000086 => {} // Not Used
            // 0x4000088 => {} // BIOS/Sound PWM Control (SOUNDBIAS), 16 bit register (read + write)
            // 0x400008A => {} // Not Used
            // 0x4000090 => {} // Channel 3 Wave Pattern RAM (2 banks) (WAVE_RAM) 2x10h in size, (read + write)
            // 0x40000A8 => {} // Not Used

            // DMA Transfer Channels
            // 0x40000BA => {} // DMA 0 Control (DMA0CNT_H), 16 bit register (read + write)
            // 0x40000C6 => {} // DMA 1 Control (DMA1CNT_H), 16 bit register (read + write)
            // 0x40000D2 => {} // DMA 2 Control (DMA2CNT_H), 16 bit register (read + write)
            // 0x40000DE => {} // DMA 3 Control (DMA3CNT_H), 16 bit register (read + write)
            // 0x40000E0 => {} // Not Used

            // Timer Registers
            0x4000100 => self.timers.timers[0].current_counter(self.scheduler.current), // Timer 0 Counter/Reload (TM0CNT_L), 16 bit register (read + write)
            0x4000102 => self.timers.timers[0].read_control_register(), // Timer 0 Control (TM0CNT_H), 16 bit register (read + write)
            0x4000104 => self.timers.timers[1].current_counter(self.scheduler.current), // Timer 1 Counter/Reload (TM1CNT_L), 16 bit register (read + write)
            0x4000106 => self.timers.timers[1].read_control_register(), // Timer 1 Control (TM1CNT_H), 16 bit register (read + write)
            0x4000108 => self.timers.timers[2].current_counter(self.scheduler.current), // Timer 2 Counter/Reload (TM2CNT_L), 16 bit register (read + write)
            0x400010A => self.timers.timers[2].read_control_register(), // Timer 2 Control (TM2CNT_H), 16 bit register (read + write)
            0x400010C => self.timers.timers[3].current_counter(self.scheduler.current), // Timer 3 Counter/Reload (TM3CNT_L), 16 bit register (read + write)
            0x400010E => self.timers.timers[3].read_control_register(), // Timer 3 Control (TM3CNT_H), 16 bit register (read + write)
            // 0x4000110 => {} // Not Used

            // Serial Communication (1)
            // 0x4000120 => {} // SIO Data (Normal-32bit Mode; shared with SIO Data 0 (Parent) (SIODATA32). SIO Data is a 32 bit register and SIO Data 0 (Parent) (Multi-Player Mode) is a 16 bit register (read + write) (SIOMULTI0)
            // 0x4000122 => {} // SIO Data 1 (1st Child) (Multi-Player Mode) (SIOMULTI1), 16 bit register (read + write)
            // 0x4000124 => {} // SIO Data 2 (2nd Child) (Multi-Player Mode) (SIOMULTI2), 16 bit register (read + write)
            // 0x4000126 => {} // SIO Data 3 (3rd Child) (Multi-Player Mode) (SIOMULTI3), 16 bit register (read + write)
            // 0x4000128 => {} // SIO Control Register (SIOCNT), 16 bit register (read + write)
            // 0x400012A => {} // SIO Data (Local of MultiPlayer; shared with SIODATA8) (SIOMLT_SEND), 16 bit register (read + write); SIO Data (Normal-8bit and UART Mode) (SIODATA8), 16 bit register (read + write)
            // 0x400012C => {} // Not Used

            // Keypad Input
            // 0x4000130 => {} // Key Status (KEYINPUT), 16 bit register read only
            // 0x4000132 => {} // Key Interrupt Control (KEYCNT), 16 bit register (read + write)

            // Serial Communication (2)
            // 0x4000134 => {} // SIO Mode Select/General Purpose Data (RCNT), 16 bit register (read + write)
            // 0x4000136 => {} // Ancient - Infrared Register (Prototypes only) (IR)
            // 0x4000138 => {} // Not Used
            // 0x4000140 => {} // SIO JOY Bus Control (JOYCNT), 16 bit register (read + write)
            // 0x4000142 => {} // Not Used
            // 0x4000150 => {} // SIO JOY Bus Receive Data (JOY_RECV), 32 bit register (read + write)
            // 0x4000154 => {} // SIO JOY Bus Transmit Data (JOY_TRANS), 32 bit register (read + write)
            // 0x4000158 => {} // SIO JOY Bus Receive Status (JOYSTAT), 16 bit register (read + maybe write?)
            // 0x400015A => {} // Not Used

            //Interrupt, Waitstate, and Power-Down Control
            0x4000200 => self.interrupt_enable, // Interrupt Enable Register (IE), 16 bit register (read + write)
            0x4000202 => self.interrupt_flag, // Interrupt Request Flags / IRQ Acknowledge (IF), 16 bit register (read + write)
            // 0x4000204 => {} // Game Pak Waitstate Control (AITCNT), 16 bit register (read + write)
            // 0x4000206 => {} // Not used
            0x4000208 => self.interrupt_master_enable as u16, // Interrupt Master Enable Register (IME), 16 bit register (read + write)
            // 0x400020A => {} // Not used
            0x4000300 => self.postflg as u16, // Undocumented - Post Boot Flag (POSTFLG), 8 bit register (read + write)
            // 0x4000301 => {} // Undocumented - Power Down Control (HALTCNT), 8 bit register (write only)
            // 0x4000302 => {} // Not used
            // 0x4000410 => {} // Undocumented - Purpose Unknown / Bug ??? 0FFh
            // 0x4000411 => {} // Not used
            // 0x4000800 => {} // Undocumented - Internal Memory Control, 32 bit register (read + write)
            // 0x4000804 => {} // Not used
            // address if (address & 0xFF00FFFF) == 0x04000800 => {} // Mirrors of 4000800h (repeated each 64K), 32 bit (read + write)
            _ => 0 as u16, // ***CHANGE***
        }
    }

    fn write_register_16(&mut self, address: u32, mut value: u16) {
        match address {
            // LCD I/O Registers
            0x4000000 => {} // LCD Control (DISPCNT), 16 bit register (read + write)
            0x4000002 => {} // Undocumented 16 bit register (read + write)
            0x4000004 => {} // Stat & LYC, 16 bit register (read + write)
            0x4000008 => {} // BG0 Control (BG0CNT) 16 bit register (read + write)
            0x400000A => {} // BG1 Control (BG1CNT) 16 bit register (read + write)
            0x400000C => {} // BG2 Control (BG2CNT) 16 bit register (read + write)
            0x400000E => {} // BG3 Control (BG3CNT) 16 bit register (read + write)
            0x4000010 => {} // BG0 X-Offset (BG0HOFS) 16 bit register (write only)
            0x4000012 => {} // BG0 Y-Offset (BG0VOFS) 16 bit register (write only)
            0x4000014 => {} // BG1 X-Offset (BG1HOFS) 16 bit register (write only)
            0x4000016 => {} // BG1 Y-Offset (BG1VOFS) 16 bit register (write only)
            0x4000018 => {} // BG2 X-Offset (BG2HOFS) 16 bit register (write only)
            0x400001A => {} // BG2 Y-Offset (BG2VOFS) 16 bit register (write only)
            0x400001C => {} // BG3 X-Offset (BG3HOFS) 16 bit register (write only)
            0x400001E => {} // BG3 Y-Offset (BG3VOFS) 16 bit register (write only)
            0x4000020 => {} // BG2 Rotation/Scaling Parameter A (dx) (BG2PA), 16 bit register (write only)
            0x4000022 => {} // BG2 Rotation/Scaling Parameter B (dmx) (BG2PB), 16 bit register (write only)
            0x4000024 => {} // BG2 Rotation/Scaling Parameter C (dy) (BG2PC), 16 bit register (write only)
            0x4000026 => {} // BG2 Rotation/Scaling Parameter D (dmy) (BG2PD), 16 bit register (write only)
            0x4000028 => {} // BG2 Reference Point X-Coordinate (BG2X), 32 bit register (write only)
            0x400002C => {} // BG2 Reference Point Y-Coordinate (BG2Y), 32 bit register (write only)
            0x4000030 => {} // BG3 Rotation/Scaling Parameter A (dx) (BG3PA), 16 bit register (write only)
            0x4000032 => {} // BG3 Rotation/Scaling Parameter B (dmx) (BG3PB), 16 bit register (write only)
            0x4000034 => {} // BG3 Rotation/Scaling Parameter C (dy) (BG3PC), 16 bit register (write only)
            0x4000036 => {} // BG3 Rotation/Scaling Parameter D (dmy) (BG3PD), 16 bit register (write only)
            0x4000038 => {} // BG3 Reference Point X-Coordinate (BG3X), 32 bit register (write only)
            0x400003C => {} // BG3 Reference Point Y-Coordinate (BG3Y), 32 bit register (write only)
            0x4000040 => {} // Window 0 Horizontal Dimensions (WIN0H), 16 bit register (write only)
            0x4000042 => {} // Window 1 Horizontal Dimensions (WIN1H), 16 bit register (write only)
            0x4000044 => {} // Window 0 Vertical Dimensions (WIN0V), 16 bit register (write only)
            0x4000046 => {} // Window 1 Vertical Dimensions (WIN1V), 16 bit register (write only)
            0x4000048 => {} // Inside of Window 0 and 1 (WININ), 16 bit register (read + write)
            0x400004A => {} // Inside of OBJ Window & Outside of Windows 2 (WINOUT), 16 bit register (read + write)
            0x400004C => {} // Mosaic Size (MOSAIC), 16 bit register (write only)
            0x400004E => {} // Not Used
            0x4000050 => {} // Color Special Effects Selection (BLDCNT), 16 bit register (read + write)
            0x4000052 => {} // Alpha Blending Coefficients (BLDALPHA), 16 bit register (read + write)
            0x4000054 => {} // Brightness (Fade-In/Out) Coefficient (BLDY), 16 bit register (write only)
            0x4000056 => {} // Not Used

            // Sound Registers
            0x4000060 => {} // Channel 1 Sweep register (NR10) (SOUND1CNT_L), 16 bit register (read + write)
            0x4000062 => {} // Channel 1 Duty/Length/Envelope (NR11, NR12) (SOUND1CNT_H), 16 bit register (read + write)
            0x4000064 => {} // Channel 1 Frequency/Control (NR13, NR14) (SOUND1CNT_X), 16 bit register (read + write)
            0x4000066 => {} // Not Used
            0x4000068 => {} // Channel 2 Duty/Length/Envelope (NR21, NR22) (SOUND2CNT_L), 16 bit register (read + write)
            0x400006A => {} // Not Used
            0x400006C => {} // Channel 2 Frequency/Control (NR23, NR24) (SOUND2CNT_H), 16 bit register (read + write)
            0x400006E => {} // Not Used
            0x4000070 => {} // Channel 3 Stop/Wave RAM select (NR30) (SOUND3CNT_L), 16 bit register (read + write)
            0x4000072 => {} // Channel 3 Length/Volume (NR31, NR32), 16 bit register (read + write)
            0x4000074 => {} // Channel 3 Frequency/Control (NR33, NR34) (SOUND3CNT_X), 16 bit register (read + write)
            0x4000076 => {} // Not Used
            0x4000078 => {} // Channel 4 Length/Envelope (NR41, NR42) (SOUND4CNT_L), 16 bit register (read + write)
            0x400007A => {} // Not Used
            0x400007C => {} // Channel 4 Frequency/Control (NR43, NR44) (SOUND4CNT_H), 16 bit register (read + write)
            0x400007E => {} // Not Used
            0x4000080 => {} // Control Stereo/Volume/Enable (NR50, NR51) (SOUNDCNT_L), 16 bit register (read + write)
            0x4000082 => {} // Control Mixing/DMA Control (SOUNDCNT_H), 16 bit register (read + write)
            0x4000084 => {} // Control Sound on/off (NR52) (SOUNDCNT_X), 16 bit register (read + write)
            0x4000086 => {} // Not Used
            0x4000088 => {} // BIOS/Sound PWM Control (SOUNDBIAS), 16 bit register (read + write)
            0x400008A => {} // Not Used
            0x4000090 => {} // Channel 3 Wave Pattern RAM (2 banks) (WAVE_RAM) 2x10h in size, (read + write)
            0x40000A0 => {} // Channel A FIFO, Data 0-3, (FIFO_A) (write only), 32 bit register
            0x40000A4 => {} // Channel B FIFO, Data 0-3, (FIFO_B) (write only), 32 bit register
            0x40000A8 => {} // Not Used

            // DMA Transfer Channels
            0x40000B0 => {} // DMA 0 Source Address (DMA0SAD), 32 bit register (write only)
            0x40000B4 => {} // DMA 0 Destination Address (DMA0DAD), 32 bit register (write only)
            0x40000B8 => {} // DMA 0 Word Count (DMA0CNT_L), 16 bit register (write only)
            0x40000BA => {} // DMA 0 Control (DMA0CNT_H), 16 bit register (read + write)
            0x40000BC => {} // DMA 1 Source Address (DMA1SAD), 32 bit register (write only)
            0x40000C0 => {} // DMA 1 Destination Address (DMA1DAD), 32 bit register (write only)
            0x40000C6 => {} // DMA 1 Control (DMA1CNT_H), 16 bit register (read + write)
            0x40000C8 => {} // DMA 2 Source Address (DMA2SAD), 32 bit register (write only)
            0x40000CC => {} // DMA 2 Destination Address (DMA2DAD), 32 bit register (write only)
            0x40000D0 => {} // DMA 2 Word Count (DMA2CNT_L), 16 bit register (write only)
            0x40000D2 => {} // DMA 2 Control (DMA2CNT_H), 16 bit register (read + write)
            0x40000D4 => {} // DMA 3 Source Address (DMA3SAD), 32 bit register (write only)
            0x40000D8 => {} // DMA 3 Destination Address (DMA3DAD), 32 bit register (write only)
            0x40000DC => {} // DMA 3 Word Count (DMA3CNT_L), 16 bit register (write only)
            0x40000DE => {} // DMA 3 Control (DMA3CNT_H), 16 bit register (read + write)
            0x40000E0 => {} // Not Used

            // Timer Registers
            0x4000100 => self.timers.timers[0].write_counter_register(value), // Timer 0 Counter/Reload (TM0CNT_L), 16 bit register (read + write)
            0x4000102 => self.timers.timers[0].write_control_register(value, &mut self.scheduler), // Timer 0 Control (TM0CNT_H), 16 bit register (read + write)
            0x4000104 => self.timers.timers[1].write_counter_register(value), // Timer 1 Counter/Reload (TM1CNT_L), 16 bit register (read + write)
            0x4000106 => self.timers.timers[1].write_control_register(value, &mut self.scheduler), // Timer 1 Control (TM1CNT_H), 16 bit register (read + write)
            0x4000108 => self.timers.timers[2].write_counter_register(value), // Timer 2 Counter/Reload (TM2CNT_L), 16 bit register (read + write)
            0x400010A => self.timers.timers[2].write_control_register(value, &mut self.scheduler), // Timer 2 Control (TM2CNT_H), 16 bit register (read + write)
            0x400010C => self.timers.timers[3].write_counter_register(value), // Timer 3 Counter/Reload (TM3CNT_L), 16 bit register (read + write)
            0x400010E => self.timers.timers[3].write_control_register(value, &mut self.scheduler), // Timer 3 Control (TM3CNT_H), 16 bit register (read + write)
            0x4000110 => {} // Not Used
            0x4000112 => {} // Not Used

            // Serial Communication (1)
            0x4000120 => {} // SIO Data (Normal-32bit Mode; shared with SIO Data 0 (Parent) (SIODATA32). SIO Data is a 32 bit register and SIO Data 0 (Parent) (Multi-Player Mode) is a 16 bit register (read + write) (SIOMULTI0)
            0x4000122 => {} // SIO Data 1 (1st Child) (Multi-Player Mode) (SIOMULTI1), 16 bit register (read + write)
            0x4000124 => {} // SIO Data 2 (2nd Child) (Multi-Player Mode) (SIOMULTI2), 16 bit register (read + write)
            0x4000126 => {} // SIO Data 3 (3rd Child) (Multi-Player Mode) (SIOMULTI3), 16 bit register (read + write)
            0x4000128 => {} // SIO Control Register (SIOCNT), 16 bit register (read + write)
            0x400012A => {} // SIO Data (Local of MultiPlayer; shared with SIODATA8) (SIOMLT_SEND), 16 bit register (read + write); SIO Data (Normal-8bit and UART Mode) (SIODATA8), 16 bit register (read + write)
            0x400012C => {} // Not Used

            // Keypad Input
            0x4000132 => {} // Key Interrupt Control (KEYCNT), 16 bit register (read + write)

            // Serial Communication (2)
            0x4000134 => {} // SIO Mode Select/General Purpose Data (RCNT), 16 bit register (read + write)
            0x4000136 => {} // Ancient - Infrared Register (Prototypes only) (IR)
            0x4000138 => {} // Not Used
            0x4000140 => {} // SIO JOY Bus Control (JOYCNT), 16 bit register (read + write)
            0x4000142 => {} // Not Used
            0x4000150 => {} // SIO JOY Bus Receive Data (JOY_RECV), 32 bit register (read + write)
            0x4000154 => {} // SIO JOY Bus Transmit Data (JOY_TRANS), 32 bit register (read + write)
            0x4000158 => {} // SIO JOY Bus Receive Status (JOYSTAT), 16 bit register (read + maybe write?)
            0x400015A => {} // Not Used

            //Interrupt, Waitstate, and Power-Down Control
            0x4000200 => {
                self.interrupt_enable = {
                    value.clear_bit_range(14..16);

                    value
                }
            } // Interrupt Enable Register (IE), 16 bit register (read + write)
            0x4000202 => self.interrupt_flag &= !value, // Interrupt Request Flags / IRQ Acknowledge (IF), 16 bit register (read + write), 1's erase bits, write to clear
            0x4000204 => {} // Game Pak Waitstate Control (AITCNT), 16 bit register (read + write)
            0x4000206 => {} // Not used
            0x4000208 => self.interrupt_master_enable = value.get_bit(0) as u32, // Interrupt Master Enable Register (IME), 16 bit register (read + write)
            0x400020A => {}                                                      // Not used
            0x4000300 => {
                self.postflg = value.get_bit(0) as u8;
                self.haltcnt = Some((value >> 8) as u8);
            } // Undocumented - Post Boot Flag (POSTFLG), 8 bit register (read + write)
            0x4000302 => {}                                                      // Not used
            0x4000410 => {} // Undocumented - Purpose Unknown / Bug ??? 0FFh
            0x4000411 => {} // Not used
            0x4000800 => {} // Undocumented - Internal Memory Control, 32 bit register (read + write)
            0x4000804 => {} // Not used
            address if (address & 0xFF00FFFF) == 0x04000800 => {} // Mirrors of 4000800h (repeated each 64K), 32 bit (read + write)
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
    }

    pub fn take_halt_request(&mut self) -> Option<u8> {
        self.haltcnt.take()
    }

    pub fn run_dma(&mut self, channel: usize) {}
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::gamepak::GamePak;

    #[test]
    fn test_bus_write_u32() {
        let gamepak = GamePak::mock();
        let mut bus = Bus::new(gamepak);

        let address = 0x05000001;
        let value = 0b100000001 as u32;
        let access_type = AccessType::Sequential;
        bus.write_u32(address, value, access_type);

        assert_eq!(bus.scheduler.current, 2);
        assert_eq!(&bus.ppu.palette_ram[..4], [1, 1, 0, 0]);

        let gamepak = GamePak::mock();
        let mut bus = Bus::new(gamepak);
        bus.write_u32(address, value, access_type);

        assert_eq!(bus.scheduler.current, 2);
        assert_eq!(&bus.ppu.palette_ram[..4], [1, 1, 0, 0]);

        let gamepak = GamePak::mock();
        let mut bus = Bus::new(gamepak);
        bus.write_u32(address, value, access_type);

        assert_eq!(bus.scheduler.current, 2);
        assert_eq!(&bus.ppu.palette_ram[..4], [1, 1, 0, 0]);
    }

    #[test]
    fn test_interrupt_write_flag() {
        let gamepak = GamePak::mock();
        let mut bus = Bus::new(gamepak);

        bus.interrupt_flag = 0b0101;
        bus.write_u16(0x4000200, !0, AccessType::Sequential);
        assert_eq!(bus.pending_interrupt(), 0b0101);

        bus.write_u16(0x4000202, 0b0001, AccessType::Sequential);
        assert_eq!(bus.pending_interrupt(), 0b0100);

        bus.write_u16(0x4000202, 0b0100, AccessType::Sequential);
        assert_eq!(bus.pending_interrupt(), 0);
    }

    #[test]
    fn test_if_byte_write_preserves_other_byte() {
        let gamepak = GamePak::mock();
        let mut bus = Bus::new(gamepak);

        bus.interrupt_flag = 0x0101;
        bus.write_u8(0x4000202, 0x01, AccessType::Sequential);
        assert_eq!(bus.interrupt_flag, 0x0100);

        bus.write_u8(0x4000203, 0x01, AccessType::Sequential);
        assert_eq!(bus.interrupt_flag, 0);
    }
}
