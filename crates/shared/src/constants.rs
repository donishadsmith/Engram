pub const ALLOWED_AUDIO_PITCH_DEVIATION: f64 = 0.005;

// TODO: while this method is better than the previous one, there are still issues with frame pacing, lowering the value worsens audio but unsuprisingly improves the frmae skipping issue
// need to find a balance that doesnt sacrifice one over the other - issue specific to gba and most noticeable in emerald
// also reducing the swap interval to 0/vsync helps so frame isnt being delayed further - investigate more for alternative methods
pub const MAX_FRAMES_PER_CALL: u32 = 4;

pub const SAVE_STATE_SLOTS: usize = 12;
