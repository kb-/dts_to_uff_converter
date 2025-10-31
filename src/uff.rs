use crate::dts::ChannelData;
use anyhow::Result;
use std::fmt::{self, Write as FmtWrite};
use std::fs::OpenOptions;
use std::io::{BufWriter, Write as IoWrite};
use std::path::Path;

const UFF_SEPARATOR: &str = "    -1";

fn truncate_to_width(text: &str, width: usize) -> String {
    text.chars().take(width).collect()
}

fn write_line<W: IoWrite>(writer: &mut W, line: &str) -> Result<()> {
    let mut padded = String::from(line);
    if padded.len() < 80 {
        padded.push_str(&" ".repeat(80 - padded.len()));
    }
    writer.write_all(padded.as_bytes())?;
    writer.write_all(b"\n")?;
    Ok(())
}

struct ScientificComponents {
    mantissa: [u8; 32],
    mantissa_len: usize,
    exponent_value: i32,
    exponent_negative: bool,
    after_exponent_marker: bool,
    exponent_sign_consumed: bool,
}

impl ScientificComponents {
    fn new() -> Self {
        Self {
            mantissa: [0; 32],
            mantissa_len: 0,
            exponent_value: 0,
            exponent_negative: false,
            after_exponent_marker: false,
            exponent_sign_consumed: false,
        }
    }
}

impl fmt::Write for ScientificComponents {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for &byte in s.as_bytes() {
            if !self.after_exponent_marker {
                if self.mantissa_len >= self.mantissa.len() {
                    return Err(fmt::Error);
                }
                self.mantissa[self.mantissa_len] = byte;
                self.mantissa_len += 1;
                if byte == b'e' || byte == b'E' {
                    self.after_exponent_marker = true;
                }
            } else if !self.exponent_sign_consumed {
                match byte {
                    b'+' => {
                        self.exponent_sign_consumed = true;
                    }
                    b'-' => {
                        self.exponent_negative = true;
                        self.exponent_sign_consumed = true;
                    }
                    b'0'..=b'9' => {
                        self.exponent_sign_consumed = true;
                        self.exponent_value = (byte - b'0') as i32;
                    }
                    _ => return Err(fmt::Error),
                }
            } else {
                match byte {
                    b'0'..=b'9' => {
                        self.exponent_value = self.exponent_value * 10 + (byte - b'0') as i32;
                    }
                    _ => return Err(fmt::Error),
                }
            }
        }
        Ok(())
    }
}

fn write_scientific<W: FmtWrite>(
    buffer: &mut W,
    value: f64,
    width: usize,
    precision: usize,
) -> fmt::Result {
    let mut components = ScientificComponents::new();
    fmt::write(&mut components, format_args!("{:.*e}", precision, value))?;

    let mantissa = &components.mantissa[..components.mantissa_len];
    let mantissa_str = std::str::from_utf8(mantissa).map_err(|_| fmt::Error)?;

    let exponent_value = if components.exponent_negative {
        -components.exponent_value
    } else {
        components.exponent_value
    };
    let abs_exponent = exponent_value.abs();

    let mut digits = [b'0'; 3];
    let digits_slice = if abs_exponent >= 100 {
        digits[0] = b'0' + ((abs_exponent / 100) as u8);
        digits[1] = b'0' + (((abs_exponent / 10) % 10) as u8);
        digits[2] = b'0' + ((abs_exponent % 10) as u8);
        &digits[..3]
    } else {
        digits[0] = b'0' + ((abs_exponent / 10) as u8);
        digits[1] = b'0' + ((abs_exponent % 10) as u8);
        &digits[..2]
    };
    let digits_str = std::str::from_utf8(digits_slice).map_err(|_| fmt::Error)?;

    let exponent_len = 1 + digits_str.len();
    let total_len = mantissa_str.len() + exponent_len;
    if width > total_len {
        for _ in 0..(width - total_len) {
            buffer.write_char(' ')?;
        }
    }

    buffer.write_str(mantissa_str)?;
    buffer.write_char(if exponent_value < 0 { '-' } else { '+' })?;
    buffer.write_str(digits_str)
}

fn format_data_line(buffer: &mut String, values: &[f64]) {
    buffer.clear();
    if buffer.capacity() < 80 {
        buffer.reserve(80 - buffer.capacity());
    }

    for &value in values {
        write_scientific(buffer, value, 20, 11).expect("writing scientific value to buffer");
    }

    if buffer.len() < 80 {
        buffer.extend(std::iter::repeat(' ').take(80 - buffer.len()));
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

    let mut record2 = String::new();
    write!(
        record2,
        "{:>10}{:>10}{:>10}  ",
        4,
        data.time_series.len(),
        1
    )
    .expect("writing record2 prefix");
    write_scientific(&mut record2, 0.0, 11, 5).expect("writing record2 start time");
    record2.push_str("  ");
    write_scientific(&mut record2, 1.0 / data.sample_rate, 11, 5)
        .expect("writing record2 time step");
    record2.push_str("  ");
    write_scientific(&mut record2, 0.0, 11, 5).expect("writing record2 abscissa start");
    write_line(&mut writer, &record2)?;

    let abscissa_name = format!(" {:<19}", "Time");
    let abscissa_units = format!("{: <48}", format!("  {}", truncate_to_width("s", 46)));
    let record3 = format!(
        "{:>10}{:>5}{:>5}{:>5}{}{}",
        17, 0, 0, 0, abscissa_name, abscissa_units
    );
    write_line(&mut writer, &record3)?;

    let ordinate_name_field = format!(" {:<19}", channel_label);
    let ordinate_units_field = format!(
        "{: <35}",
        format!("  {}", truncate_to_width(&data.units, 33))
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
    let mut data_line = String::with_capacity(80);
    for chunk in data.time_series.chunks(4) {
        format_data_line(&mut data_line, chunk);
        writer.write_all(data_line.as_bytes())?;
        writer.write_all(b"\n")?;
    }

    // --- End of Block ---
    write_line(&mut writer, UFF_SEPARATOR)?;

    writer.flush()?;
    Ok(())
}
