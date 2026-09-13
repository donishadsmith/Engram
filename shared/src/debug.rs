pub const DEBUG_PAGES: [DebugPage; 1] = [DebugPage::Audio];

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum DebugPage {
    Audio,
}

impl DebugPage {
    pub fn to_str(self) -> &'static str {
        match self {
            DebugPage::Audio => "Audio",
        }
    }
}
