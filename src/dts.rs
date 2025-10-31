use anyhow::{anyhow, Result};
use byteorder::{LittleEndian, ReadBytesExt};
use natord::compare;
use quick_xml::de::from_str;
use serde::Deserialize;
use std::fs::{self, File};
use std::io::{BufReader, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

// --- Structs to deserialize the .dts XML file ---
#[derive(Debug, Deserialize)]
#[serde(rename = "DTS_Setup")]
struct DtsSetup {
    #[serde(rename = "Module", default)]
    modules: Vec<Module>,
}

#[derive(Debug, Deserialize)]
struct Module {
    #[serde(rename = "@StartRecordSampleNumber")]
    start_record_sample_number: f64,
    #[serde(rename = "AnalogInputChanel", default)]
    channels: Vec<AnalogInputChannel>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct AnalogInputChannel {
    #[serde(rename = "@ProportionalToExcitation")]
    pub proportional_to_excitation: String,
    #[serde(rename = "@IsInverted")]
    pub is_inverted: String,
    #[serde(rename = "@MeasuredExcitationVoltage")]
    pub measured_excitation_voltage: f64,
    #[serde(rename = "@FactoryExcitationVoltage")]
    pub factory_excitation_voltage: f64,
    #[serde(rename = "@InitialEu")]
    pub initial_eu: f64,
    #[serde(rename = "@ZeroMethod")]
    pub zero_method: String,
    #[serde(rename = "@Eu")]
    pub eu: String,
    #[serde(rename = "@Description")]
    pub description: String,
    #[serde(rename = "@AbsoluteDisplayOrder")]
    pub display_order: u32,
}

// --- Structs for data from .chn files ---
#[derive(Debug, Clone)]
struct ChnHeader {
    channel_start: u64,
    npts: u64,
    sample_rate: f64,
    trigger_sample_number: i64,
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
    base_path: PathBuf,
    // Metadata is stored per-channel, in the correct, sorted order.
    chn_files: Vec<PathBuf>,
    xml_metadata: Vec<(AnalogInputChannel, f64)>, // (ChannelInfo, StartRecordSampleNumber)
    chn_headers: Vec<ChnHeader>,
    min_npts: u64,
}

impl DtsReader {
    /// Creates a new DtsReader by analyzing a test folder.
    pub fn new<P: AsRef<Path>>(path: P) -> Result<Self> {
        let base_path = path.as_ref().to_path_buf();

        // 1. Find and parse the .dts XML file
        let dts_file_path = find_file_by_extension(&base_path, "dts")?;
        let xml_string = fs::read_to_string(dts_file_path)?;
        let parsed_xml: DtsSetup = from_str(&xml_string)
            .map_err(|e| anyhow!("Failed to parse .dts XML file: {}", e))?;

        // Flatten the XML structure into a single list of channels with their parent module's info
        let mut all_channels: Vec<(AnalogInputChannel, f64)> = parsed_xml
            .modules
            .into_iter()
            .flat_map(|module| {
                let start_sample = module.start_record_sample_number;
                module
                    .channels
                    .into_iter()
                    .map(move |channel| (channel, start_sample))
            })
            .collect();

        // Sort channels by their absolute display order
        all_channels.sort_by_key(|(ch, _)| ch.display_order);

        // 2. Find and sort all .chn files
        let mut chn_files: Vec<PathBuf> = fs::read_dir(&base_path)?
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
            base_path,
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
        let trigger_sample_number = reader.read_i64::<LittleEndian>()?;

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
            trigger_sample_number,
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
        if xml_meta.is_inverted == "True" {
            scale_factor_mv = -scale_factor_mv;
        }

        let excitation = if xml_meta.proportional_to_excitation == "False" {
            1.0
        } else if xml_meta.factory_excitation_voltage.is_nan() {
            xml_meta.measured_excitation_voltage
        } else {
            xml_meta.factory_excitation_voltage
        };

        let offset = match xml_meta.zero_method.as_str() {
            "UsePreCalZero" => {
                (-f64::from(chn_header.pre_test_zero_level_adc) * scale_factor_mv
                    / chn_header.scale_factor_eu
                    / excitation)
                    + xml_meta.initial_eu
            }
            "AverageOverTime" => {
                (-f64::from(chn_header.data_zero_level_adc) * scale_factor_mv
                    / chn_header.scale_factor_eu
                    / excitation)
                    + xml_meta.initial_eu
            }
            _ => xml_meta.initial_eu, // "None"
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
            entry.path().extension().map_or(false, |ext| ext == extension)
        })
        .map(|entry| entry.path())
        .ok_or_else(|| anyhow!("No '.{}' file found in directory {:?}", extension, dir))
}