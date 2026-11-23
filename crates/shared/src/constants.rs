pub const ALLOWED_AUDIO_PITCH_DEVIATION: f64 = 0.005;

// TODO: while this method is better than the previous one, there are still
// issues with frame pacing, lowering the value worsens audio but unsuprisingly
// improves the frmae skipping issue need to find a balance that doesnt
// sacrifice one over the other - issue specific to gba and most noticeable in
// emerald also reducing the swap interval to 0/vsync helps so frame isnt being
// delayed further - investigate more for alternative methods
pub const MAX_FRAMES_PER_CALL: u32 = 4;

pub const SAVE_STATE_SLOTS: usize = 12;

pub const GB_REFRESH_RATE: f64 = 59.727;

pub const VALID_EXTENSIONS: [&'static str; 3] = [".gba", ".gb", ".gbc"];

// TODO: Optimize gba ppu, dying a little inside that my gba ppu does a lot of
// work per pixel and kneecaped the gba fps to be anywhere from 240 fos to 450
// for my cpu, but still increasing since gameboy can technically reach
// 1000-1200 max but still capping to 500 fps
pub const ALLOWED_SPEED: [f32; 13] = [0.25, 0.50, 0.75, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0];

pub const ALLOWED_REWIND_CAPACITY: [usize; 8] = [0, 15, 30, 60, 120, 180, 240, 300];
