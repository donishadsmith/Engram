// https://problemkaputt.de/gbatek.htm#gbamemorymap

pub struct Bus {

}

impl Bus {
    pub fn read<T: From<u8>>(&self, address: u32) -> T {
        match address {
            // General Internal Memory
            0x00000000..=0x00003FFF => {}, // BIOS
            0x00004000..=0x01FFFFFF => {}, // Not Used
            0x02000000..=0x0203FFFF => {}, // On-board WRAM
            0x03000000..=0x03007FFF => {}, // On-chip WRAM
            0x03008000..=0x03FFFFFF => {}, // Not used
            0x04000000..=0x040003FE => self.read_register(address), // I/O register
            0x04000400..=0x04FFFFFF => {}, // Not Used

            // Internal Display Memory
            0x05000000..=0x050003FF => {}, // BG/OBJ Palette RAM
            0x05000400..=0x05FFFFFF => {}, // Not Used
            0x06000000..=0x06017FFF => {}, // VRAM
            0x06018000..=0x06FFFFFF => {}, // Not Used
            0x07000000..=0x070003FF => {}, // OAM
            0x07000400..=0x07FFFFFF => {}, // Not Used

            // External Memory (Game Pak/ROM)
            0x08000000..=0x09FFFFFF => {}, // Game Pak ROM/FlashROM (max 32MB) - Wait State 0
            0x0A000000..=0x0BFFFFFF => {}, // Game Pak ROM/FlashROM (max 32MB) - Wait State 1
            0x0C000000..=0x0DFFFFFF => {}, // Game Pak ROM/FlashROM (max 32MB) - Wait State 2
            0x0E000000..=0x0E00FFFF => {}, // Game Pak SRAM (max 64 KBytes) - 8bit Bus width
            0x0E010000..=0x0FFFFFFF => {}, // Not used  
            
            // Unused Memory Area
            0x10000000..=0xFFFFFFFF => {} // Not used (upper 4bits of address bus unused)
        }
    }

    // TODO: Map out all IO registers
    // Eventually, each IO component will have its own
    // read and write registers function to route to
    fn read_register<T: From<u16>>(&self, address: u32) -> T {
        match address {
            // LCD I/O Registers
            0x4000000 => {}, // LCD Control (DISPCNT), 16 bit register (read + write)
            0x4000002 => {}, // Undocumented 16 bit register (read + write)
            0x4000004 => {}, // Stat & LYC, 16 bit register (read + write)
            0x4000006 => {}, // LY, 16 bit, (VCOUNT), read only
            0x4000008 => {}, // BG0 Control (BG0CNT) 16 bit register (read + write)
            0x400000A => {}, // BG1 Control (BG1CNT) 16 bit register (read + write)
            0x400000C => {}, // BG2 Control (BG2CNT) 16 bit register (read + write)
            0x400000E => {}, // BG3 Control (BG3CNT) 16 bit register (read + write)
            0x4000048 => {}, // Inside of Window 0 and 1 (WININ), 16 bit register (read + write)
            0x400004A => {}, // Inside of OBJ Window & Outside of Windows 2 (WINOUT) (read + write)
            0x400004E => {}, // Not Used
            0x4000050 => {}, // Color Special Effects Selection (BLDCNT), 16 bit register (read + write)
            0x4000052 => {}, // Alpha Blending Coefficients (BLDALPHA), 16 bit register (read + write)
            0x4000056 => {}, // Not Used

            // Sound Registers
            0x4000060 => {}, // Channel 1 Sweep register (NR10) (SOUND1CNT_L), 16 bit register (read + write)
            0x4000062 => {}, // Channel 1 Duty/Length/Envelope (NR11, NR12) (SOUND1CNT_H), 16 bit register (read + write)
            0x4000064 => {}, // Channel 1 Frequency/Control (NR13, NR14) (SOUND1CNT_X), 16 bit register (read + write)
            0x4000066 => {}, // Not Used
            0x4000068 => {}, // Channel 2 Duty/Length/Envelope (NR21, NR22) (SOUND2CNT_L), 16 bit register (read + write)
            0x400006A => {}, // Not Used
            0x400006C => {}, // Channel 2 Frequency/Control (NR23, NR24) (SOUND2CNT_H), 16 bit register (read + write)
            0x400006E => {}, // Not Used
            0x4000070 => {}, // Channel 3 Stop/Wave RAM select (NR30) (SOUND3CNT_L), 16 bit register (read + write)
            0x4000072 => {}, // Channel 3 Length/Volume (NR31, NR32), 16 bit register (read + write)
            0x4000074 => {}, // Channel 3 Frequency/Control (NR33, NR34) (SOUND3CNT_X), 16 bit register (read + write)
            0x4000076 => {}, // Not Used
            0x4000078 => {}, // Channel 4 Length/Envelope (NR41, NR42) (SOUND4CNT_L), 16 bit register (read + write)
            0x400007A => {}, // Not Used
            0x400007C => {}, // Channel 4 Frequency/Control (NR43, NR44) (SOUND4CNT_H), 16 bit register (read + write)
            0x400007E => {}, // Not Used
            0x4000080 => {}, // Control Stereo/Volume/Enable (NR50, NR51) (SOUNDCNT_L), 16 bit register (read + write)
            0x4000082 => {}, // Control Mixing/DMA Control (SOUNDCNT_H), 16 bit register (read + write)
            0x4000084 => {}, // Control Sound on/off (NR52) (SOUNDCNT_X), 16 bit register (read + write)
            0x4000086 => {}, // Not Used
            0x4000088 => {}, // BIOS/Sound PWM Control (SOUNDBIAS), 16 bit register (read + write)
            0x400008A => {}, // Not Used
            0x4000090 => {}, // Channel 3 Wave Pattern RAM (2 banks) (WAVE_RAM) 2x10h in size, (read + write)
            0x40000A8 => {}, // Not Used

            // DMA Transfer Channels
            0x40000BA => {}, // DMA 0 Control (DMA0CNT_H), 16 bit register (read + write)
            // Start back at 40000C6h
        }
    }

    pub fn write<T: Into<u32>>(&mut self, address: u32, value: T) {
        match address {
            // General Internal Memory
            0x00004000..=0x01FFFFFF => {}, // Not Used
            0x02000000..=0x0203FFFF => {}, // On-board WRAM
            0x03000000..=0x03007FFF => {}, // On-chip WRAM
            0x03008000..=0x03FFFFFF => {}, // Not used
            0x04000000..=0x040003FE => self.write_register(address, value), // I/O register
            0x04000400..=0x04FFFFFF => {}, // Not Used

            // Internal Display Memory
            0x05000000..=0x050003FF => {}, // BG/OBJ Palette RAM
            0x05000400..=0x05FFFFFF => {}, // Not Used
            0x06000000..=0x06017FFF => {}, // VRAM
            0x06018000..=0x06FFFFFF => {}, // Not Used
            0x07000000..=0x070003FF => {}, // OAM
            0x07000400..=0x07FFFFFF => {}, // Not Used

            // External Memory (Game Pak/ROM)
            0x08000000..=0x09FFFFFF => {}, // Game Pak ROM/FlashROM (max 32MB) - Wait State 0
            0x0A000000..=0x0BFFFFFF => {}, // Game Pak ROM/FlashROM (max 32MB) - Wait State 1
            0x0C000000..=0x0DFFFFFF => {}, // Game Pak ROM/FlashROM (max 32MB) - Wait State 2
            0x0E000000..=0x0E00FFFF => {}, // Game Pak SRAM (max 64 KBytes) - 8bit Bus width
            0x0E010000..=0x0FFFFFFF => {}, // Not used  
            
            // Unused Memory Area
            0x10000000..=0xFFFFFFFF => {} // Not used (upper 4bits of address bus unused)
        }
    }

    fn write_register<T: Into<u32>>(&mut self, address: u32, value: T) {
        match address {
            // LCD I/O Registers
            0x4000000 => {}, // LCD Control (DISPCNT), 16 bit register (read + write)
            0x4000002 => {}, // Undocumented 16 bit register (read + write)
            0x4000004 => {}, // Stat & LYC, 16 bit register (read + write)
            0x4000008 => {}, // BG0 Control (BG0CNT) 16 bit register (read + write)
            0x400000A => {}, // BG1 Control (BG1CNT) 16 bit register (read + write)
            0x400000C => {}, // BG2 Control (BG2CNT) 16 bit register (read + write)
            0x400000E => {}, // BG3 Control (BG3CNT) 16 bit register (read + write)
            0x4000010 => {}, // BG0 X-Offset (BG0HOFS) 16 bit register (write only)
            0x4000012 => {}, // BG0 Y-Offset (BG0VOFS) 16 bit register (write only)
            0x4000014 => {}, // BG1 X-Offset (BG1HOFS) 16 bit register (write only)
            0x4000016 => {}, // BG1 Y-Offset (BG1VOFS) 16 bit register (write only)
            0x4000018 => {}, // BG2 X-Offset (BG2HOFS) 16 bit register (write only)
            0x400001A => {}, // BG2 Y-Offset (BG2VOFS) 16 bit register (write only)
            0x400001C => {}, // BG3 X-Offset (BG3HOFS) 16 bit register (write only)
            0x400001E => {}, // BG3 Y-Offset (BG3VOFS) 16 bit register (write only)
            0x4000020 => {}, // BG2 Rotation/Scaling Parameter A (dx) (BG2PA), 16 bit register (write only)
            0x4000022 => {}, // BG2 Rotation/Scaling Parameter B (dmx) (BG2PB), 16 bit register (write only)
            0x4000024 => {}, // BG2 Rotation/Scaling Parameter C (dy) (BG2PC), 16 bit register (write only)
            0x4000026 => {}, // BG2 Rotation/Scaling Parameter D (dmy) (BG2PD), 16 bit register (write only)
            0x4000028 => {}, // BG2 Reference Point X-Coordinate (BG2X), 32 bit register (write only)
            0x400002C => {}, // BG2 Reference Point Y-Coordinate (BG2Y), 32 bit register (write only)
            0x4000030 => {}, // BG3 Rotation/Scaling Parameter A (dx) (BG3PA), 16 bit register (write only)
            0x4000032 => {}, // BG3 Rotation/Scaling Parameter B (dmx) (BG3PB), 16 bit register (write only)
            0x4000034 => {}, // BG3 Rotation/Scaling Parameter C (dy) (BG3PC), 16 bit register (write only)
            0x4000036 => {}, // BG3 Rotation/Scaling Parameter D (dmy) (BG3PD), 16 bit register (write only)
            0x4000038 => {}, // BG3 Reference Point X-Coordinate (BG3X), 32 bit register (write only)
            0x400003C => {}, // BG3 Reference Point Y-Coordinate (BG3Y), 32 bit register (write only)
            0x4000040 => {}, // Window 0 Horizontal Dimensions (WIN0H), 16 bit register (write only)
            0x4000042 => {}, // Window 1 Horizontal Dimensions (WIN1H), 16 bit register (write only)
            0x4000044 => {}, // Window 0 Vertical Dimensions (WIN0V), 16 bit register (write only)
            0x4000046 => {}, // Window 1 Vertical Dimensions (WIN1V), 16 bit register (write only)
            0x4000048 => {}, // Inside of Window 0 and 1 (WININ), 16 bit register (read + write)
            0x400004A => {}, // Inside of OBJ Window & Outside of Windows 2 (WINOUT), 16 bit register (read + write)
            0x400004C => {}, // Mosaic Size (MOSAIC), 16 bit register (write only)
            0x400004E => {}, // Not Used
            0x4000050 => {}, // Color Special Effects Selection (BLDCNT), 16 bit register (read + write)
            0x4000052 => {}, // Alpha Blending Coefficients (BLDALPHA), 16 bit register (read + write)
            0x4000054 => {}, // Brightness (Fade-In/Out) Coefficient (BLDY), 16 bit register (write only)
            0x4000056 => {}, // Not Used

            // Sound Registers
            0x4000060 => {}, // Channel 1 Sweep register (NR10) (SOUND1CNT_L), 16 bit register (read + write)
            0x4000062 => {}, // Channel 1 Duty/Length/Envelope (NR11, NR12) (SOUND1CNT_H), 16 bit register (read + write)
            0x4000064 => {}, // Channel 1 Frequency/Control (NR13, NR14) (SOUND1CNT_X), 16 bit register (read + write)
            0x4000066 => {}, // Not Used
            0x4000068 => {}, // Channel 2 Duty/Length/Envelope (NR21, NR22) (SOUND2CNT_L), 16 bit register (read + write)
            0x400006A => {}, // Not Used
            0x400006C => {}, // Channel 2 Frequency/Control (NR23, NR24) (SOUND2CNT_H), 16 bit register (read + write)
            0x400006E => {}, // Not Used
            0x4000070 => {}, // Channel 3 Stop/Wave RAM select (NR30) (SOUND3CNT_L), 16 bit register (read + write)
            0x4000072 => {}, // Channel 3 Length/Volume (NR31, NR32), 16 bit register (read + write)
            0x4000074 => {}, // Channel 3 Frequency/Control (NR33, NR34) (SOUND3CNT_X), 16 bit register (read + write)
            0x4000076 => {}, // Not Used
            0x4000078 => {}, // Channel 4 Length/Envelope (NR41, NR42) (SOUND4CNT_L), 16 bit register (read + write)
            0x400007A => {}, // Not Used
            0x400007C => {}, // Channel 4 Frequency/Control (NR43, NR44) (SOUND4CNT_H), 16 bit register (read + write)
            0x400007E => {}, // Not Used
            0x4000080 => {}, // Control Stereo/Volume/Enable (NR50, NR51) (SOUNDCNT_L), 16 bit register (read + write)
            0x4000082 => {}, // Control Mixing/DMA Control (SOUNDCNT_H), 16 bit register (read + write)
            0x4000084 => {}, // Control Sound on/off (NR52) (SOUNDCNT_X), 16 bit register (read + write)
            0x4000086 => {}, // Not Used
            0x4000088 => {}, // BIOS/Sound PWM Control (SOUNDBIAS), 16 bit register (read + write)
            0x400008A => {}, // Not Used
            0x4000090 => {}, // Channel 3 Wave Pattern RAM (2 banks) (WAVE_RAM) 2x10h in size, (read + write)
            0x40000A0 => {}, // Channel A FIFO, Data 0-3, (FIFO_A) (write only), 32 bit register
            0x40000A4 => {}, // Channel B FIFO, Data 0-3, (FIFO_B) (write only), 32 bit register
            0x40000A8 => {}, // Not Used

            // DMA Transfer Channels
            0x40000B0 => {}, // DMA 0 Source Address (DMA0SAD), 32 bit register (write only)
            0x40000B4 => {}, // DMA 0 Destination Address (DMA0DAD), 32 bit register (write only)
            0x40000B8 => {}, // DMA 0 Word Count (DMA0CNT_L), 16 bit register (write only)
            0x40000BA => {}, // DMA 0 Control (DMA0CNT_H), 16 bit register (read + write)
            0x40000BC => {}, // DMA 1 Source Address (DMA1SAD), 32 bit register (write only)
            0x40000C0 => {}, // DMA 1 Destination Address (DMA1DAD), 32 bit register (write only)
            // Start back at 40000C6h
        }
    }
}
