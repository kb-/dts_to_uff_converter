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

struct LineBuffer {
    text: String,
}

impl LineBuffer {
    fn with_capacity(capacity: usize) -> Self {
        let mut text = String::with_capacity(capacity.max(80));
        if text.capacity() < 80 {
            text.reserve(80 - text.capacity());
        }
        Self { text }
    }

    fn clear(&mut self) {
        self.text.clear();
    }

    fn push_str(&mut self, s: &str) {
        self.text.push_str(s);
    }

    fn push_char(&mut self, ch: char) {
        self.text.push(ch);
    }

    fn write_fmt(&mut self, args: fmt::Arguments<'_>) {
        FmtWrite::write_fmt(self, args).expect("writing formatted text into line buffer");
    }

    fn ensure_minimum_capacity(&mut self, capacity: usize) {
        if self.text.capacity() < capacity {
            self.text.reserve(capacity - self.text.capacity());
        }
    }

    fn write_line<W: IoWrite>(&mut self, writer: &mut W) -> Result<()> {
        let original_len = self.text.len();
        if original_len < 80 {
            let pad_len = 80 - original_len;
            self.text.extend(std::iter::repeat_n(' ', pad_len));
        }

        writer.write_all(self.text.as_bytes())?;
        writer.write_all(b"\n")?;

        self.text.truncate(original_len);
        Ok(())
    }
}

impl fmt::Write for LineBuffer {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        self.text.push_str(s);
        Ok(())
    }

    fn write_char(&mut self, c: char) -> fmt::Result {
        self.text.push(c);
        Ok(())
    }
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

fn format_data_line(buffer: &mut LineBuffer, values: &[f64]) {
    buffer.clear();
    buffer.ensure_minimum_capacity(80);

    for &value in values {
        write_scientific(buffer, value, 20, 11).expect("writing scientific value to buffer");
    }
}

/// Writes a single channel's data to a UFF Type 58 file using the ASCII layout emitted by MATLAB.
fn write_uff58_impl<W: IoWrite>(
    writer: &mut W,
    data: &ChannelData,
    track_name: &str,
) -> Result<()> {
    let mut line_buffer = LineBuffer::with_capacity(256);

    // --- Block 1: UFF Type 58 Header (ASCII layout) ---
    line_buffer.clear();
    line_buffer.push_str(UFF_SEPARATOR);
    line_buffer.write_line(writer)?;

    line_buffer.clear();
    line_buffer.push_str("    58");
    line_buffer.write_line(writer)?;

    line_buffer.clear();
    line_buffer.write_line(writer)?;

    let pt_label = truncate_to_width(track_name, 64);
    line_buffer.clear();
    line_buffer.push_str("Pt=");
    line_buffer.push_str(&pt_label);
    line_buffer.push_char(';');
    line_buffer.write_line(writer)?;

    line_buffer.clear();
    line_buffer.write_line(writer)?;

    line_buffer.clear();
    line_buffer.push_str("NONE");
    line_buffer.write_line(writer)?;

    line_buffer.clear();
    line_buffer.push_str("NONE");
    line_buffer.write_line(writer)?;

    let channel_label = truncate_to_width(track_name, 19);
    let channel_field = format!("{:<19}", channel_label);
    let zero_field = format!("{:>4}", 0);
    let none_field = format!(" {:<19}", "NONE");
    let flag_field = format!("{:<4}{}", 1, 0);
    line_buffer.clear();
    line_buffer.write_fmt(format_args!(
        "    1         0    0         0 {}{}{}{}{}",
        channel_field, 0, zero_field, none_field, flag_field
    ));
    line_buffer.write_line(writer)?;

    line_buffer.clear();
    line_buffer.write_fmt(format_args!(
        "{:>10}{:>10}{:>10}  ",
        4,
        data.time_series.len(),
        1
    ));
    write_scientific(&mut line_buffer, 0.0, 11, 5).expect("writing record2 start time");
    line_buffer.push_str("  ");
    write_scientific(&mut line_buffer, 1.0 / data.sample_rate, 11, 5)
        .expect("writing record2 time step");
    line_buffer.push_str("  ");
    write_scientific(&mut line_buffer, 0.0, 11, 5).expect("writing record2 abscissa start");
    line_buffer.write_line(writer)?;

    let abscissa_name = format!(" {:<19}", "Time");
    let abscissa_units = format!("{: <48}", format!("  {}", truncate_to_width("s", 46)));
    line_buffer.clear();
    line_buffer.write_fmt(format_args!(
        "{:>10}{:>5}{:>5}{:>5}{}{}",
        17, 0, 0, 0, abscissa_name, abscissa_units
    ));
    line_buffer.write_line(writer)?;

    let ordinate_name_field = format!(" {:<19}", channel_label);
    let ordinate_units_field = format!(
        "{: <35}",
        format!("  {}", truncate_to_width(&data.units, 33))
    );
    line_buffer.clear();
    line_buffer.write_fmt(format_args!(
        "{:>10}{:>5}{:>5}{:>5}{}{}",
        8, 0, 0, 0, ordinate_name_field, ordinate_units_field
    ));
    line_buffer.write_line(writer)?;

    let none_name_field = format!(" {:<19}", "NONE");
    let none_units_field = format!("{: <35}", format!("  {}", "NONE"));
    line_buffer.clear();
    line_buffer.write_fmt(format_args!(
        "{:>10}{:>5}{:>5}{:>5}{}{}",
        0, 0, 0, 0, none_name_field, none_units_field
    ));
    line_buffer.write_line(writer)?;
    line_buffer.write_line(writer)?;

    // --- ASCII Data Section ---
    for chunk in data.time_series.chunks(4) {
        format_data_line(&mut line_buffer, chunk);
        line_buffer.write_line(writer)?;
    }

    // --- End of Block ---
    line_buffer.clear();
    line_buffer.push_str(UFF_SEPARATOR);
    line_buffer.write_line(writer)?;

    Ok(())
}

/// Writes a single channel to a UFF Type 58 writer without managing the underlying file handle.
pub fn write_uff58<W: IoWrite>(writer: &mut W, data: &ChannelData, track_name: &str) -> Result<()> {
    write_uff58_impl(writer, data, track_name)
}

#[derive(Clone, Copy, Debug)]
pub enum Uff58Format {
    Ascii,
    Binary58b,
}

pub fn write_uff58_ascii<W: IoWrite>(writer: &mut W, data: &ChannelData, track_name: &str) -> Result<()> {
    write_uff58_impl(writer, data, track_name)
}

/// Convenience helper that opens a file handle and writes a single channel.

/// Write UFF 58 in "58b" (binary) variant:
/// - ASCII header with a "b" line (byte order, fp format, n_ascii_lines, n_bytes, ...)
/// - Records mirror our ASCII writer for fields
/// - Data payload written as f32 in selected endian (like reference Python code)
pub fn write_uff58b<W: IoWrite>(writer: &mut W, data: &ChannelData, track_name: &str) -> Result<()> {
    let mut line_buffer = LineBuffer::with_capacity(256);

    // Separator and type
    line_buffer.clear();
    line_buffer.push_str(UFF_SEPARATOR);
    line_buffer.write_line(writer)?;

    // "58b" header line with binary info
    // bo: 1 little-endian, 2 big-endian
    let bo: i32 = if cfg!(target_endian = "little") { 1 } else { 2 };
    // fp_format: 2 (IEEE floating-point)
    let fp_format: i32 = 2;
    // number of ASCII lines before binary payload (like reference: 11)
    let n_ascii_lines: i32 = 11;

    // Even abscissa (1) – we always write evenly sampled time series
    let num_pts = data.time_series.len();
    // We'll write real-even data payload as f32 values (num_pts entries)
    let n_bytes: i32 = (num_pts as i32) * 4;

    // Type line with trailing "b"
    {
        let header_line = format!("    58b{:>6}{:>6}{:>12}{:>12}{:>6}{:>6}{:>12}{:>12}",
            bo, fp_format, n_ascii_lines, n_bytes, 0, 0, 0, 0);
        line_buffer.clear();
        line_buffer.push_str(&header_line);
        line_buffer.write_line(writer)?;
    }

    // Record 1..5 (IDs) – mirror ASCII writer layout
    line_buffer.clear();
    line_buffer.write_line(writer)?; // blank ID1

    let pt_label = truncate_to_width(track_name, 64);
    line_buffer.clear();
    line_buffer.push_str("Pt=");
    line_buffer.push_str(&pt_label);
    line_buffer.push_char(';');
    line_buffer.write_line(writer)?;

    line_buffer.clear();
    line_buffer.write_line(writer)?; // blank ID3

    line_buffer.clear();
    line_buffer.push_str("NONE");
    line_buffer.write_line(writer)?;

    line_buffer.clear();
    line_buffer.push_str("NONE");
    line_buffer.write_line(writer)?;

    // Record 6
    // Real data at nodal DOF, response entity name is the channel label (first 19 chars)
    let channel_label = truncate_to_width(track_name, 19);
    let channel_field = format!("{:<19}", channel_label);
    line_buffer.clear();
    line_buffer.write_fmt(format_args!(
        "    1         0    0         0 {}{}{}{}{}",
        channel_field,
        0,
        format!("{:>4}", 0),
        format!(" {:<19}", "NONE"),
        format!("{:<4}{}", 1, 0)
    ));
    line_buffer.write_line(writer)?;

    // Record 7
    line_buffer.clear();
    line_buffer.write_fmt(format_args!(
        "{:>10}{:>10}{:>10}  ",
        2,          // ord_data_type (single precision for binary payload)
        num_pts,
        1           // abscissa_spacing (even)
    ));
    write_scientific(&mut line_buffer, 0.0, 11, 5).expect("writing abscissa min");
    line_buffer.push_str("  ");
    write_scientific(&mut line_buffer, 1.0 / data.sample_rate, 11, 5).expect("writing abscissa inc");
    line_buffer.push_str("  ");
    write_scientific(&mut line_buffer, 0.0, 11, 5).expect("writing z-axis value");
    line_buffer.write_line(writer)?;

    // Record 8 (Abscissa characteristics)
    let abscissa_name = format!(" {:<19}", "Time");
    let abscissa_units = format!("{: <48}", format!("  {}", truncate_to_width("s", 46)));
    line_buffer.clear();
    line_buffer.write_fmt(format_args!(
        "{:>10}{:>5}{:>5}{:>5}{}{}",
        17, 0, 0, 0, abscissa_name, abscissa_units
    ));
    line_buffer.write_line(writer)?;

    // Record 9 (Ordinate characteristics)
    let ordinate_name_field = format!(" {:<19}", channel_label);
    let ordinate_units_field = format!("{: <35}", format!("  {}", truncate_to_width(&data.units, 33)));
    line_buffer.clear();
    line_buffer.write_fmt(format_args!(
        "{:>10}{:>5}{:>5}{:>5}{}{}",
        8, 0, 0, 0, ordinate_name_field, ordinate_units_field
    ));
    line_buffer.write_line(writer)?;

    // Record 10 (Ordinate denom)
    let none_name_field = format!(" {:<19}", "NONE");
    let none_units_field = format!("{: <35}", format!("  {}", "NONE"));
    line_buffer.clear();
    line_buffer.write_fmt(format_args!(
        "{:>10}{:>5}{:>5}{:>5}{}{}",
        0, 0, 0, 0, none_name_field, none_units_field
    ));
    line_buffer.write_line(writer)?;

    // Record 11 (Z-axis characteristics)
    line_buffer.clear();
    line_buffer.write_fmt(format_args!(
        "{:>10}{:>5}{:>5}{:>5}{}{}",
        0, 0, 0, 0, none_name_field, none_units_field
    ));
    line_buffer.write_line(writer)?;

    // Record 12: binary data payload (real, even) as f32
    // We'll write in the byte order indicated by `bo`
    if cfg!(target_endian = "little") {
        for &v in &data.time_series {
            let b = (v as f32).to_le_bytes();
            writer.write_all(&b)?;
        }
    } else {
        for &v in &data.time_series {
            let b = (v as f32).to_be_bytes();
            writer.write_all(&b)?;
        }
    }

    // End of block
    line_buffer.clear();
    line_buffer.push_str(UFF_SEPARATOR);
    line_buffer.write_line(writer)?;

    Ok(())
}

/// Dispatcher that writes either ASCII (58) or Binary (58b).
pub fn write_uff58_with_format<W: IoWrite>(writer: &mut W, data: &ChannelData, track_name: &str, fmt: Uff58Format) -> Result<()> {
    match fmt {
        Uff58Format::Ascii => write_uff58_ascii(writer, data, track_name),
        Uff58Format::Binary58b => write_uff58b(writer, data, track_name),
    }
}
pub fn write_uff58_file_with_format<P: AsRef<Path>>(
    path: P,
    data: &ChannelData,
    track_name: &str,
    append_request: bool,
    fmt: Uff58Format,
) -> Result<()> {
    let path_ref = path.as_ref();
    let file = OpenOptions::new()
        .write(true)
        .create(true)
        .append(append_request)
        .truncate(!append_request)
        .open(path_ref)?;

    let mut writer = BufWriter::with_capacity(8 * 1024 * 1024, file);
    write_uff58_with_format(&mut writer, data, track_name, fmt)?;
    writer.flush()?;
    Ok(())
}

/// Backward-compatibility wrapper; now defaults to Binary 58b.
pub fn write_uff58_file<P: AsRef<Path>>(
    path: P,
    data: &ChannelData,
    track_name: &str,
    append_request: bool,
) -> Result<()> {
    write_uff58_file_with_format(path, data, track_name, append_request, Uff58Format::Binary58b)
}
