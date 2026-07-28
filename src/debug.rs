use std::{error::Error, fmt::Write as _, fs::{read, write}, path::PathBuf};

pub enum DumpWidth {
    Byte,
    HalfWord,
    Word
}


pub fn read_rom(source: PathBuf) -> Result<Vec<u8>, std::io::Error>{
    let buffer = read(source)?;

    Ok(buffer)
}

pub fn hexdump(buffer: &[u8], width: DumpWidth) -> Result<(), Box<dyn Error>> {
    let mut output = String::new();
    match width {
        DumpWidth::Byte => dump_lines::<1>(buffer, &mut output, 16)?,
        DumpWidth::HalfWord => dump_lines::<2>(buffer, &mut output, 8)?,
        DumpWidth::Word => dump_lines::<4>(buffer, &mut output, 4)?,
    }

    Ok(())
}

fn dump_lines<const N: usize>(buffer: &[u8], output: &mut String, per_row: usize) -> Result<(), Box<dyn Error>> {
    let (units, _) = buffer.as_chunks::<N>();
    for (i, row) in units.chunks(per_row).enumerate() {
        write!(output, "{:#010X}: ", i * per_row * N)?;
        for &unit in row {
            match N {
                2 => write!(output, "{:04X} ", u16::from_le_bytes([unit[0], unit[1]]))?,
                4 => write!(output, "{:08X} ", u32::from_le_bytes([unit[0], unit[1], unit[2], unit[3]]))?,
                _ => { for b in unit { write!(output, "{b:02X} ")?; } }
            }
        }
        writeln!(output)?;
    }

    write("hexdump.txt", output)?;

    Ok(())
}