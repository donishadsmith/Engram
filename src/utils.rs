use macroquad::{
    input::{KeyCode, get_keys_down, get_keys_pressed},
    window::next_frame,
};
use rfd::FileDialog;
use std::{
    path::PathBuf,
    time::{Duration, Instant},
};

pub enum EmulatorState {
    Active,
    Quit,
    Start,
}

pub fn file_dialog() -> Option<PathBuf> {
    FileDialog::new()
        .set_title("Select a GameBoy ROM file")
        .add_filter("GameBoy Roms", &["gb", "gbc"])
        .pick_file()
}

pub async fn fps_lock(frame_start_time: Instant) {
    let frame_duration = Duration::from_secs_f64(1.0 / 60.0);

    let elapsed_time = frame_start_time.elapsed();
    if elapsed_time < frame_duration {
        spin_sleep::sleep(frame_duration - elapsed_time);
    }

    next_frame().await
}

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
        let cartridge = crate::components::rom::cartridge::Cartridge::load(filename)?;

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
