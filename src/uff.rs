
### `src/uff.rs`

This file is updated to accept a track name and an `append` flag. It now correctly handles creating a new file or adding datasets to an existing one.

```rust src/uff.rs
use crate::dts::ChannelData;
use anyhow::Result;
use byteorder::{BigEndian, WriteBytesExt};
use std::fs::{File, OpenOptions};
use std::io::{BufWriter, Write};
use std::path::Path;

const UFF_SEPARATOR: &[u8] = b"    -1\r\n";

/// Writes a single channel's data to a UFF Type 58 file.
pub fn write_uff58_file<P: AsRef<Path>>(
    path: P,
    data: &ChannelData,
    track_name: &str,
    append: bool,
) -> Result<()> {
    let file = OpenOptions::new()
        .write(true)
        .create(true)
        .append(append)
        .truncate(!append)
        .open(path)?;

    let mut writer = BufWriter::new(file);

    // --- Block 1: UFF Type 58 Header ---
    writer.write_all(UFF_SEPARATOR)?;
    writer.write_all(b"    58\r\n")?;

    // Record 1: Function ID info
    let record1 = format!("{:<10}{:<10}", 1, 0); // Func type 1 (Time Response), load case 0
    writer.write_all(format!("{:<80}\r\n", record1).as_bytes())?;

    // Record 2: Title/Channel Name
    writer.write_all(format!("{:<80}\r\n", track_name).as_bytes())?;

    // Record 3 & 4: Date and other identifiers
    writer.write_all(format!("{:<80}\r\n", "").as_bytes())?;
    writer.write_all(format!("{:<80}\r\n", "").as_bytes())?;

    // Record 5: Abscissa (X-axis) info
    let x_axis_label = "Time (s)";
    let record5 = format!(
        "{:<10}{:<10}{:<20.10E}{:<20.10E}{:<10}",
        1,                      // Type: Time
        0,                      // Spacing: Even
        0.0,                    // Start time
        1.0 / data.sample_rate, // Time step (delta t)
        x_axis_label
    );
    writer.write_all(format!("{:<80}\r\n", record5).as_bytes())?;

    // Record 6: Ordinate (Y-axis) info
    let y_axis_label = &data.units;
    let record6 = format!("{:<10}{:<10}", 2, y_axis_label); // Type 2 = General
    writer.write_all(format!("{:<80}\r\n", record6).as_bytes())?;

    // --- Binary Data Section ---
    let num_points = data.time_series.len();
    let num_bytes = num_points * 4; // 4 bytes per f32
    writer.write_all(format!("{}\r\n", num_bytes).as_bytes())?;

    for &sample in &data.time_series {
        writer.write_f32::<BigEndian>(sample)?;
    }
    writer.write_all(b"\r\n")?;

    // --- End of Block ---
    writer.write_all(UFF_SEPARATOR)?;

    writer.flush()?;
    Ok(())
}