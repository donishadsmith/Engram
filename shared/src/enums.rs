#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Width {
    Byte,
    Halfword,
    Word,
}

impl Width {
    pub fn from_string(string: String) -> Result<Width, mlua::Error> {
        match string.to_lowercase().as_str() {
            "byte" => Ok(Width::Byte),
            "halfword" => Ok(Width::Halfword),
            "word" => Ok(Width::Word),
            _ => Err(mlua::Error::runtime(format!(
                "invalid option for `access`: {string}; valid options are 'byte', 'halfword', and 'word'."
            ))),
        }
    }

    pub fn to_string(self) -> &'static str {
        match self {
            Width::Byte => "byte",
            Width::Halfword => "halfword",
            Width::Word => "word",
        }
    }

    pub fn align(self, address: u32) -> u32 {
        match self {
            Width::Byte => address,
            Width::Halfword => address & !1,
            Width::Word => address & !3,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum FetchSource {
    Hit,
    Miss,
    Uncached,
}
