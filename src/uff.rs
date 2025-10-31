use crate::dts::ChannelData;
use anyhow::Result;
use std::fmt::Write as FmtWrite;
use std::fs::OpenOptions;
use std::io::{BufWriter, Write as IoWrite};
use std::path::Path;

const UFF_SEPARATOR: &str = "    -1";

fn truncate_to_width(text: &str, width: usize) -> String {
    text.chars().take(width).collect()
}

fn write_line<W: IoWrite>(writer: &mut W, buffer: &mut String) -> Result<()> {
    let original_len = buffer.len();
    if original_len < 80 {
        let pad_len = 80 - original_len;
        buffer.reserve(pad_len);
        for _ in 0..pad_len {
            buffer.push(' ');
        }
    }

    writer.write_all(buffer.as_bytes())?;
    writer.write_all(b"\n")?;

    if buffer.len() > original_len {
        buffer.truncate(original_len);
    }
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

fn format_data_line(buffer: &mut String, values: &[f64]) {
    buffer.clear();
    for &value in values {
        buffer.push_str(&format_scientific(value, 20, 11));
    }
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

    let mut line_buffer = String::new();

    // --- Block 1: UFF Type 58 Header (ASCII layout) ---
    line_buffer.clear();
    line_buffer.push_str(UFF_SEPARATOR);
    write_line(&mut writer, &mut line_buffer)?;

    line_buffer.clear();
    line_buffer.push_str("    58");
    write_line(&mut writer, &mut line_buffer)?;

    line_buffer.clear();
    write_line(&mut writer, &mut line_buffer)?;

    let pt_label = truncate_to_width(track_name, 64);
    line_buffer.clear();
    line_buffer.push_str("Pt=");
    line_buffer.push_str(&pt_label);
    line_buffer.push(';');
    write_line(&mut writer, &mut line_buffer)?;

    line_buffer.clear();
    write_line(&mut writer, &mut line_buffer)?;

    line_buffer.clear();
    line_buffer.push_str("NONE");
    write_line(&mut writer, &mut line_buffer)?;

    line_buffer.clear();
    line_buffer.push_str("NONE");
    write_line(&mut writer, &mut line_buffer)?;

    let channel_label = truncate_to_width(track_name, 19);
    let channel_field = format!("{:<19}", channel_label);
    line_buffer.clear();
    FmtWrite::write_fmt(
        &mut line_buffer,
        format_args!(
            "    1         0    0         0 {}{}{}{}{}",
            channel_field,
            0,
            format!("{:>4}", 0),
            format!(" {:<19}", "NONE"),
            format!("{:<4}{}", 1, 0)
        ),
    )
    .unwrap();
    write_line(&mut writer, &mut line_buffer)?;

    line_buffer.clear();
    FmtWrite::write_fmt(
        &mut line_buffer,
        format_args!(
            "{:>10}{:>10}{:>10}  {}  {}  {}",
            4,
            data.time_series.len(),
            1,
            format_scientific(0.0, 11, 5),
            format_scientific(1.0 / data.sample_rate, 11, 5),
            format_scientific(0.0, 11, 5)
        ),
    )
    .unwrap();
    write_line(&mut writer, &mut line_buffer)?;

    let abscissa_name = format!(" {:<19}", "Time");
    let abscissa_units = format!("{: <48}", format!("  {}", truncate_to_width("s", 46)));
    line_buffer.clear();
    FmtWrite::write_fmt(
        &mut line_buffer,
        format_args!(
            "{:>10}{:>5}{:>5}{:>5}{}{}",
            17, 0, 0, 0, abscissa_name, abscissa_units
        ),
    )
    .unwrap();
    write_line(&mut writer, &mut line_buffer)?;

    let ordinate_name_field = format!(" {:<19}", channel_label);
    let ordinate_units_field = format!(
        "{: <35}",
        format!("  {}", truncate_to_width(&data.units, 33))
    );
    line_buffer.clear();
    FmtWrite::write_fmt(
        &mut line_buffer,
        format_args!(
            "{:>10}{:>5}{:>5}{:>5}{}{}",
            8, 0, 0, 0, ordinate_name_field, ordinate_units_field
        ),
    )
    .unwrap();
    write_line(&mut writer, &mut line_buffer)?;

    let none_name_field = format!(" {:<19}", "NONE");
    let none_units_field = format!("{: <35}", format!("  {}", "NONE"));
    line_buffer.clear();
    FmtWrite::write_fmt(
        &mut line_buffer,
        format_args!(
            "{:>10}{:>5}{:>5}{:>5}{}{}",
            0, 0, 0, 0, none_name_field, none_units_field
        ),
    )
    .unwrap();
    write_line(&mut writer, &mut line_buffer)?;
    write_line(&mut writer, &mut line_buffer)?;

    // --- ASCII Data Section ---
    for chunk in data.time_series.chunks(4) {
        format_data_line(&mut line_buffer, chunk);
        write_line(&mut writer, &mut line_buffer)?;
    }

    // --- End of Block ---
    line_buffer.clear();
    line_buffer.push_str(UFF_SEPARATOR);
    write_line(&mut writer, &mut line_buffer)?;

    writer.flush()?;
    Ok(())
}
