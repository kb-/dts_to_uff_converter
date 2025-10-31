use anyhow::{anyhow, Context, Result};
use byteorder::{LittleEndian, ReadBytesExt};
use encoding_rs::{Encoding, UTF_16BE, UTF_16LE};
use natord::compare;
use quick_xml::events::{BytesStart, Event};
use quick_xml::Reader;
use std::fs::{self, File};
use std::io::{BufReader, Seek, SeekFrom};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct AnalogInputChannel {
    pub proportional_to_excitation: bool,
    pub is_inverted: bool,
    pub measured_excitation_voltage: f64,
    pub factory_excitation_voltage: f64,
    pub initial_eu: f64,
    pub zero_method: ZeroMethod,
    pub eu: String,
    pub display_order: u32,
}

#[derive(Debug, Clone, Copy)]
pub enum ZeroMethod {
    UsePreCalZero,
    AverageOverTime,
    None,
}

// --- Structs for data from .chn files ---
#[derive(Debug, Clone)]
struct ChnHeader {
    channel_start: u64,
    npts: u64,
    sample_rate: f64,
    pre_test_zero_level_adc: i32,
    data_zero_level_adc: i32,
    scale_factor_mv: f64,
    scale_factor_eu: f64,
}

/// Holds all processed data for a single channel, ready for writing.
pub struct ChannelData {
    pub time_series: Vec<f32>,
    pub sample_rate: f64,
    pub units: String,
}

/// A reader that mimics the DTS.m class behavior.
/// It opens all files in a test folder and parses their metadata.
pub struct DtsReader {
    // Metadata is stored per-channel, in the correct, sorted order.
    chn_files: Vec<PathBuf>,
    xml_metadata: Vec<(AnalogInputChannel, f64)>, // (ChannelInfo, StartRecordSampleNumber)
    chn_headers: Vec<ChnHeader>,
    min_npts: u64,
}

impl DtsReader {
    /// Creates a new DtsReader by analyzing a test folder.
    pub fn new<P: AsRef<Path>>(path: P) -> Result<Self> {
        let base_path = path.as_ref();

        // 1. Find and parse the .dts XML file
        let dts_file_path = find_file_by_extension(base_path, "dts")?;
        let mut all_channels = parse_dts_metadata(&dts_file_path)?;

        // Sort channels by their absolute display order
        all_channels.sort_by_key(|(ch, _)| ch.display_order);

        // 2. Find and sort all .chn files
        let mut chn_files: Vec<PathBuf> = fs::read_dir(base_path)?
            .filter_map(|entry| {
                entry.ok().and_then(|e| {
                    let path = e.path();
                    if path.is_file() && path.extension().map_or(false, |ext| ext == "chn") {
                        Some(path)
                    } else {
                        None
                    }
                })
            })
            .collect();

        // Sort files using natural string comparison to match MATLAB's behavior
        chn_files.sort_by(|a, b| compare(a.to_str().unwrap_or(""), b.to_str().unwrap_or("")));

        if chn_files.len() != all_channels.len() {
            return Err(anyhow!(
                "Mismatch between channel count in .dts file ({}) and number of .chn files ({}).",
                all_channels.len(),
                chn_files.len()
            ));
        }

        // 3. Read headers from all .chn files to get metadata and min_npts
        let mut chn_headers = Vec::new();
        let mut min_npts = u64::MAX;
        for file_path in &chn_files {
            let header = Self::read_chn_header(file_path)?;
            if header.npts < min_npts {
                min_npts = header.npts;
            }
            chn_headers.push(header);
        }

        Ok(DtsReader {
            chn_files,
            xml_metadata: all_channels,
            chn_headers,
            min_npts,
        })
    }

    /// Reads the binary header of a single .chn file.
    fn read_chn_header<P: AsRef<Path>>(path: P) -> Result<ChnHeader> {
        let file = File::open(path)?;
        let mut reader = BufReader::new(file);

        reader.seek(SeekFrom::Start(0))?;
        let magic_key = reader.read_u32::<LittleEndian>()?;
        if magic_key != 0x2C36351F {
            return Err(anyhow!("Not a valid DTS .chn file (magic key mismatch)"));
        }

        reader.seek(SeekFrom::Start(8))?;
        let channel_start = reader.read_u64::<LittleEndian>()?;
        let npts = reader.read_u64::<LittleEndian>()?;
        reader.seek(SeekFrom::Start(32))?;
        let sample_rate = reader.read_f64::<LittleEndian>()?;
        let num_triggers = reader.read_u16::<LittleEndian>()?;
        let _trigger_sample_number = reader.read_i64::<LittleEndian>()?;

        let n = num_triggers as u64 * 8;
        reader.seek(SeekFrom::Start(n + 42))?;
        let pre_test_zero_level_adc = reader.read_i32::<LittleEndian>()?;
        reader.seek(SeekFrom::Start(n + 70))?;
        let data_zero_level_adc = reader.read_i32::<LittleEndian>()?;
        let scale_factor_mv = reader.read_f64::<LittleEndian>()?;
        let scale_factor_eu = reader.read_f64::<LittleEndian>()?;

        Ok(ChnHeader {
            channel_start,
            npts,
            sample_rate,
            pre_test_zero_level_adc,
            data_zero_level_adc,
            scale_factor_mv,
            scale_factor_eu,
        })
    }

    /// Reads and processes the data for a single track.
    pub fn read_track(&self, track_index: usize) -> Result<ChannelData> {
        if track_index >= self.channel_count() {
            return Err(anyhow!("Track index {} is out of bounds.", track_index));
        }

        let (xml_meta, _start_rec_sample) = &self.xml_metadata[track_index];
        let chn_header = &self.chn_headers[track_index];
        let chn_path = &self.chn_files[track_index];

        // --- Read raw ADC data ---
        let mut file = File::open(chn_path)?;
        file.seek(SeekFrom::Start(chn_header.channel_start))?;
        let num_samples_to_read = self.min_npts as usize;
        let mut adc_data = vec![0i16; num_samples_to_read];
        // We need to wrap the file in a BufReader to use read_i16_into
        let mut reader = BufReader::new(file);
        reader.read_i16_into::<LittleEndian>(&mut adc_data)?;

        // --- Perform scaling and offset calculations ---
        let mut scale_factor_mv = chn_header.scale_factor_mv;
        if xml_meta.is_inverted {
            scale_factor_mv = -scale_factor_mv;
        }

        let excitation = if !xml_meta.proportional_to_excitation {
            1.0
        } else if xml_meta.factory_excitation_voltage.is_nan() {
            xml_meta.measured_excitation_voltage
        } else {
            xml_meta.factory_excitation_voltage
        };

        let offset = match xml_meta.zero_method {
            ZeroMethod::UsePreCalZero => {
                (-f64::from(chn_header.pre_test_zero_level_adc) * scale_factor_mv
                    / chn_header.scale_factor_eu
                    / excitation)
                    + xml_meta.initial_eu
            }
            ZeroMethod::AverageOverTime => {
                (-f64::from(chn_header.data_zero_level_adc) * scale_factor_mv
                    / chn_header.scale_factor_eu
                    / excitation)
                    + xml_meta.initial_eu
            }
            ZeroMethod::None => xml_meta.initial_eu,
        };

        let scale = scale_factor_mv / chn_header.scale_factor_eu / excitation;

        let time_series: Vec<f32> = adc_data
            .into_iter()
            .map(|adc_val| ((f64::from(adc_val) * scale) + offset) as f32)
            .collect();

        Ok(ChannelData {
            time_series,
            sample_rate: chn_header.sample_rate,
            units: xml_meta.eu.clone(),
        })
    }

    pub fn channel_count(&self) -> usize {
        self.chn_files.len()
    }
}

/// Helper to find the first file with a given extension in a directory.
fn find_file_by_extension(dir: &Path, extension: &str) -> Result<PathBuf> {
    fs::read_dir(dir)?
        .filter_map(|entry| entry.ok())
        .find(|entry| {
            entry
                .path()
                .extension()
                .map_or(false, |ext| ext == extension)
        })
        .map(|entry| entry.path())
        .ok_or_else(|| anyhow!("No '.{}' file found in directory {:?}", extension, dir))
}

fn parse_dts_metadata(path: &Path) -> Result<Vec<(AnalogInputChannel, f64)>> {
    let mut xml = read_dts_xml(path)?;
    sanitize_duplicate_xml_headers(&mut xml);

    let mut reader = Reader::from_str(&xml);
    reader.trim_text(true);

    let mut buf = Vec::new();
    let mut module_stack: Vec<f64> = Vec::new();
    let mut channels = Vec::new();

    loop {
        match reader.read_event_into(&mut buf)? {
            Event::Start(ref e) => match e.name().as_ref() {
                b"Module" => {
                    let mut start_sample = 0.0;
                    for attr in e.attributes().with_checks(false) {
                        let attr = attr?;
                        let key = attr.key.as_ref();
                        if key == b"StartRecordSampleNumber" {
                            let value = attr.unescape_value().with_context(|| {
                                "Failed to decode StartRecordSampleNumber attribute"
                            })?;
                            start_sample = parse_f64(value.as_ref());
                        }
                    }
                    module_stack.push(start_sample);
                }
                b"AnalogInputChanel" => {
                    let start_sample = *module_stack.last().unwrap_or(&0.0);
                    collect_channel(e, start_sample, &mut channels)?;
                }
                _ => {}
            },
            Event::Empty(ref e) => match e.name().as_ref() {
                b"AnalogInputChanel" => {
                    let start_sample = *module_stack.last().unwrap_or(&0.0);
                    collect_channel(e, start_sample, &mut channels)?;
                }
                _ => {}
            },
            Event::End(ref e) => {
                if e.name().as_ref() == b"Module" {
                    module_stack.pop();
                }
            }
            Event::Eof => break,
            _ => {}
        }

        buf.clear();
    }

    Ok(channels)
}

fn collect_channel(
    event: &BytesStart,
    start_sample: f64,
    channels: &mut Vec<(AnalogInputChannel, f64)>,
) -> Result<()> {
    let mut proportional_to_excitation = false;
    let mut is_inverted = false;
    let mut measured_excitation_voltage = f64::NAN;
    let mut factory_excitation_voltage = f64::NAN;
    let mut initial_eu = 0.0;
    let mut zero_method = ZeroMethod::None;
    let mut eu = String::new();
    let mut display_order = 0u32;

    for attr in event.attributes().with_checks(false) {
        let attr = attr?;
        let key = attr.key.as_ref();
        let key_str = String::from_utf8_lossy(key);
        let value = attr
            .unescape_value()
            .with_context(|| format!("Failed to decode value for attribute '{}'.", key_str))?;

        match key {
            b"ProportionalToExcitation" => {
                proportional_to_excitation = value.as_ref().eq_ignore_ascii_case("True");
            }
            b"IsInverted" => {
                is_inverted = value.as_ref().eq_ignore_ascii_case("True");
            }
            b"MeasuredExcitationVoltage" => {
                measured_excitation_voltage = parse_f64(value.as_ref());
            }
            b"FactoryExcitationVoltage" => {
                factory_excitation_voltage = parse_f64(value.as_ref());
            }
            b"InitialEu" => {
                initial_eu = parse_f64(value.as_ref());
            }
            b"ZeroMethod" => {
                zero_method = match value.as_ref() {
                    "UsePreCalZero" => ZeroMethod::UsePreCalZero,
                    "AverageOverTime" => ZeroMethod::AverageOverTime,
                    _ => ZeroMethod::None,
                };
            }
            b"Eu" => {
                eu = value.into_owned();
            }
            b"AbsoluteDisplayOrder" => {
                display_order = value.as_ref().parse::<u32>().unwrap_or(0);
            }
            _ => {}
        }
    }

    channels.push((
        AnalogInputChannel {
            proportional_to_excitation,
            is_inverted,
            measured_excitation_voltage,
            factory_excitation_voltage,
            initial_eu,
            zero_method,
            eu,
            display_order,
        },
        start_sample,
    ));

    Ok(())
}

fn read_dts_xml(path: &Path) -> Result<String> {
    let bytes = fs::read(path).with_context(|| format!("Failed to read DTS file at {:?}", path))?;
    if bytes.is_empty() {
        return Ok(String::new());
    }

    if let Some((encoding, bom_len)) = Encoding::for_bom(&bytes) {
        let (decoded, had_errors) = encoding.decode_without_bom_handling(&bytes[bom_len..]);
        if had_errors {
            return Err(anyhow!(
                "Failed to decode DTS XML file due to invalid characters."
            ));
        }
        return Ok(decoded.into_owned());
    }

    if let Ok(text) = std::str::from_utf8(&bytes) {
        return Ok(text.to_string());
    }

    let looks_utf16le = bytes.len() > 1 && bytes[1] == 0;
    let looks_utf16be = bytes.len() > 1 && bytes[0] == 0;

    let encoding = if looks_utf16le {
        Some(UTF_16LE)
    } else if looks_utf16be {
        Some(UTF_16BE)
    } else {
        None
    };

    if let Some(enc) = encoding {
        let (decoded, had_errors) = enc.decode_without_bom_handling(&bytes);
        if !had_errors {
            return Ok(decoded.into_owned());
        }
    }

    Err(anyhow!(
        "Unable to determine encoding for DTS XML file at {:?}",
        path
    ))
}

fn sanitize_duplicate_xml_headers(xml: &mut String) {
    if let Some(first_idx) = xml.find("<?xml") {
        if let Some(second_rel) = xml[first_idx + 5..].find("<?xml") {
            let cutoff = first_idx + 5 + second_rel;
            xml.truncate(cutoff);
        }
    }
}

fn parse_f64(value: &str) -> f64 {
    value.trim().parse::<f64>().unwrap_or(f64::NAN)
}
