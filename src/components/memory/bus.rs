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

    // TODO: Map out all IO registers
    // Eventually, each IO component will have its own
    // read and write registers function to route to
    fn read_register<T: From<u16>>(&self, address: u32) -> T {
        match address {
            0x4000000 => {}, // LCD Control (DISPCNT), 16 bit register (read + write)
            0x4000002 => {}, // Undocumented 16 bit register (read + write)
            0x4000004 => {}, // Stat & LYC, 16 bit register (read + write)
            0x4000006 => {}, // LY, 16 bit, (VCOUNT), read only
            0x4000008 => {}, // LCD Control
        }
    }

    pub fn write<T: Into<u32>>(&mut self, address: u32, value: T) {

    }

    fn write_register() {}
}
}