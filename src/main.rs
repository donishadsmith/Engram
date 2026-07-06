use gameboy_emulator::components::{gameboy::GameBoy, rom::cartridge::Cartridge};
use std::path::PathBuf;

/* References:
   - https://gbdev.io/pandocs
   - https://aquova.net/emudev/gb
   - https://github.com/mvdnes/rboy
   - https://github.com/smparsons/retroboy
*/

fn check_cartridge() -> Result<(), std::io::Error> {
    let rom_names = [
        r".\roms\test1.gb",
        r".\roms\test2.gb",
        r".\roms\test3.gbc",
        r".\roms\test4.gbc",
        r".\roms\test5.gb",
    ];
    for rom_name in rom_names {
        let filename = Some(std::path::PathBuf::from(rom_name));
        let cartridge = Cartridge::load(filename)?;

        println!("------------------\n{}", cartridge.header.title);
        println!("{}", cartridge.header.mbc_type.to_str());
        println!("{}", cartridge.header.cgb_flag.to_str());
        println!("{:0x?}", cartridge.mbc.get_rom()[0x143]);
        println!("Cartridge Type: {}", cartridge.mbc.get_rom()[0x147]);
        println!("Checksum: {}", cartridge.header.checksum);
        println!("Has Battery: {}", cartridge.header.has_battery);
        println!("Has Timer: {}", cartridge.header.has_timer);
        println!("Has Rumble: {}\n", cartridge.header.has_rumble);
        println!("RAM length: {}\n", cartridge.header.ram_size);
        println!("RAM length: {}\n", cartridge.mbc.get_ram().len());
    }

    Ok(())
}

fn main() -> Result<(), std::io::Error> {
    let rom_path = PathBuf::from(r".\roms\test5.gb"); // no mbc; rom only
    let cartridge = Cartridge::load(Some(rom_path))?;
    let mut gameboy = GameBoy::boot(cartridge);
    loop {
        gameboy.run();
    }

    Ok(())
}
