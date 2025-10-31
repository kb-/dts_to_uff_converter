use crate::dts::ChannelData;
use anyhow::Result;
use std::fs::OpenOptions;
use std::io::{BufWriter, Write};
use std::path::Path;

const UFF_SEPARATOR: &str = "    -1";

fn truncate_to_width(text: &str, width: usize) -> String {
    text.chars().take(width).collect()
}

fn write_line<W: Write>(writer: &mut W, line: &str) -> Result<()> {
    let mut padded = String::from(line);
    if padded.len() < 80 {
        padded.push_str(&" ".repeat(80 - padded.len()));
    }
    writer.write_all(padded.as_bytes())?;
    writer.write_all(b"\n")?;
    Ok(())
}

fn format_scientific(value: f64, width: usize, precision: usize) -> String {
    let raw = format!("{:.*e}", precision, value);
    let e_pos = raw.find('e').unwrap_or(raw.len() - 1);
    let mantissa = &raw[..=e_pos];
    let exp = &raw[e_pos + 1..];
    let exp_val: i32 = exp.parse().unwrap_or(0);
    let formatted = format!("{}{:>+03}", mantissa, exp_val);
    format!("{:>width$}", formatted, width = width)
}

fn format_data_line(values: &[f64]) -> String {
    let mut line = String::new();
    for &value in values {
        line.push_str(&format_scientific(value, 20, 11));
    }
    line
}

/// Writes a single channel's data to a UFF Type 58 file using the ASCII layout emitted by MATLAB.
pub fn write_uff58_file<P: AsRef<Path>>(
    path: P,
    data: &ChannelData,
    track_name: &str,
    append_request: bool,
) -> Result<()> {
    let path_ref = path.as_ref();
    let append = append_request || path_ref.exists();

    let file = OpenOptions::new()
        .write(true)
        .create(true)
        .append(append)
        .truncate(!append)
        .open(path_ref)?;

    let mut writer = BufWriter::new(file);

    // --- Block 1: UFF Type 58 Header (ASCII layout) ---
    write_line(&mut writer, UFF_SEPARATOR)?;
    write_line(&mut writer, "    58")?;
    write_line(&mut writer, "")?;

    let pt_label = truncate_to_width(track_name, 64);
    write_line(&mut writer, &format!("Pt={};", pt_label))?;
    write_line(&mut writer, "")?;
    write_line(&mut writer, "NONE")?;
    write_line(&mut writer, "NONE")?;

    let channel_label = truncate_to_width(track_name, 19);
    let channel_field = format!("{:<19}", channel_label);
    let record1 = format!(
        "    1         0    0         0 {}{}{}{}{}",
        channel_field,
        0,
        format!("{:>4}", 0),
        format!(" {:<19}", "NONE"),
        format!("{:<4}{}", 1, 0)
    );
    write_line(&mut writer, &record1)?;

    let record2 = format!(
        "{:>10}{:>10}{:>10}  {}  {}  {}",
        4,
        data.time_series.len(),
        1,
        format_scientific(0.0, 11, 5),
        format_scientific(1.0 / data.sample_rate, 11, 5),
        format_scientific(0.0, 11, 5)
    );
    write_line(&mut writer, &record2)?;

    let abscissa_name = format!(" {:<19}", "Time");
    let abscissa_units = format!(
        "{: <48}",
        format!("  {}", truncate_to_width("s", 46))
    );
    let record3 = format!(
        "{:>10}{:>5}{:>5}{:>5}{}{}",
        17, 0, 0, 0, abscissa_name, abscissa_units
    );
    write_line(&mut writer, &record3)?;

    let ordinate_name_field = format!(" {:<19}", channel_label);
    let ordinate_units_field = format!(
        "{: <35}",
        format!(
            "  {}",
            truncate_to_width(&data.units, 33)
        )
    );
    let record4 = format!(
        "{:>10}{:>5}{:>5}{:>5}{}{}",
        8, 0, 0, 0, ordinate_name_field, ordinate_units_field
    );
    write_line(&mut writer, &record4)?;

    let none_name_field = format!(" {:<19}", "NONE");
    let none_units_field = format!("{: <35}", format!("  {}", "NONE"));
    let record5 = format!(
        "{:>10}{:>5}{:>5}{:>5}{}{}",
        0, 0, 0, 0, none_name_field, none_units_field
    );
    write_line(&mut writer, &record5)?;

    let record6 = record5.clone();
    write_line(&mut writer, &record6)?;

    // --- ASCII Data Section ---
    for chunk in data.time_series.chunks(4) {
        let line = format_data_line(chunk);
        write_line(&mut writer, &line)?;
    }

    // --- End of Block ---
    write_line(&mut writer, UFF_SEPARATOR)?;

    writer.flush()?;
    Ok(())
}
