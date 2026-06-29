pub struct Cartridge {
    pub buffer: Vec<u8>,
}

impl Cartridge {
    pub fn load(filename: Option<std::path::PathBuf>) -> Result<Self, std::io::Error> {
        let Some(rom_path) = filename else {
            let file_error_msg = "Issue occured with file selection".to_string();

            return Err(Self::error_message(file_error_msg));
        };

        Ok(Self {
            buffer: std::fs::read(rom_path)?,
        })
    }

    pub fn error_message(message: String) -> std::io::Error {
        std::io::Error::new(std::io::ErrorKind::InvalidData, message)
    }
}
