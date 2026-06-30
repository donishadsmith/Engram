/* References:
   - https://gbdev.io/pandocs
   - https://aquova.net/emudev/gb
   - https://github.com/mvdnes/rboy
   - https://github.com/smparsons/retroboy
*/

fn main() -> Result<(), std::io::Error> {
    let rom_names = [
        r".\roms\test1.gb",
        r".\roms\test2.gb",
        r".\roms\test3.gbc",
        r".\roms\test4.gbc",
    ];
    for rom_name in rom_names {
        let filename = Some(std::path::PathBuf::from(rom_name));
        let cartridge = gameboy_emulator::components::cartridge::Cartridge::load(filename)?;

        println!("------------------\n{}", cartridge.header.title);
        println!("{}", cartridge.header.mbc_type.to_str());
        println!("{}", cartridge.header.cbc_flag.to_str());
        println!("{:0x?}", cartridge.rom[0x143]);
        println!("Cartridge Type: {}", cartridge.rom[0x147]);
        println!("Checksum: {}", cartridge.header.checksum);
        println!("Has Battery: {}", cartridge.header.has_battery);
        println!("Has Timer: {}", cartridge.header.has_timer);
        println!("Has Rumble: {}\n", cartridge.header.has_rumble);
    }

    Ok(())
}
