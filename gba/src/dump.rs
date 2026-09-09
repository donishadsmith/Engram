use std::{
    fmt::Write,
    fs::{read, write},
    io::Error,
    path::PathBuf,
};

#[allow(dead_code)]
pub enum DumpWidth {
    Byte,
    HalfWord,
    Word,
}

#[allow(dead_code)]
pub fn read_rom(source: PathBuf) -> Result<Vec<u8>, Error> {
    let buffer = read(source)?;

    Ok(buffer)
}

#[allow(dead_code)]
fn dump_lines<const N: usize>(buffer: &[u8], per_row: usize) -> String {
    let mut output = String::new();
    let (units, _) = buffer.as_chunks::<N>();
    for (i, row) in units.chunks(per_row).enumerate() {
        write!(output, "{:#010x}: ", i * per_row * N).unwrap();
        for &unit in row {
            match N {
                2 => write!(output, "{:04x} ", u16::from_le_bytes([unit[0], unit[1]])).unwrap(),
                4 => write!(
                    output,
                    "{:08x} ",
                    u32::from_le_bytes([unit[0], unit[1], unit[2], unit[3]])
                )
                .unwrap(),
                _ => {
                    for b in unit {
                        write!(output, "{b:02x} ").unwrap();
                    }
                }
            }
        }

        output.push('\n');
    }

    output
}

#[allow(dead_code)]
pub fn hexdump(buffer: &[u8], width: DumpWidth) -> Result<(), Error> {
    let output = match width {
        DumpWidth::Byte => dump_lines::<1>(buffer, 16),
        DumpWidth::HalfWord => dump_lines::<2>(buffer, 8),
        DumpWidth::Word => dump_lines::<4>(buffer, 4),
    };

    write("hexdump.txt", output)
}
