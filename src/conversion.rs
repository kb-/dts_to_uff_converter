use crate::{dts, uff};
use anyhow::{Context, Result};
use clap::ValueEnum;
use rayon::prelude::*;
use std::fs::{self, OpenOptions};
use std::io::{BufWriter, Write};
use std::path::Path;

/// Output format options for generating the UFF file.
#[derive(ValueEnum, Clone, Copy, Debug, Eq, PartialEq)]
pub enum OutputFormat {
    /// Generate an ASCII UFF file.
    Ascii,
    /// Generate a binary UFF file.
    Binary,
}

impl OutputFormat {
    /// Returns the human readable name of the format.
    pub fn as_str(&self) -> &'static str {
        match self {
            OutputFormat::Ascii => "ascii",
            OutputFormat::Binary => "binary",
        }
    }
}

impl std::fmt::Display for OutputFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl std::str::FromStr for OutputFormat {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_ascii_lowercase().as_str() {
            "ascii" => Ok(OutputFormat::Ascii),
            "binary" => Ok(OutputFormat::Binary),
            other => Err(format!(
                "Unsupported output format '{other}'. Expected 'ascii' or 'binary'."
            )),
        }
    }
}

/// High-level progress updates emitted during conversion.
#[derive(Debug)]
pub enum ConversionProgress<'a> {
    /// Conversion is starting with the provided counts.
    Started {
        /// Number of track names provided by the caller.
        track_name_count: usize,
        /// Number of channels detected in the DTS directory.
        channel_count: usize,
    },
    /// A channel finished processing.
    Advanced {
        /// Number of channels that have been processed so far.
        completed: usize,
        /// Total number of channels that will be processed.
        total: usize,
        /// The track name associated with the completed channel.
        track_name: &'a str,
    },
    /// Conversion finished successfully.
    Finished,
}

/// Summary returned after a conversion succeeds.
#[derive(Debug)]
pub struct ConversionReport {
    /// Number of channels written to the UFF file.
    pub channel_count: usize,
    /// Number of track names supplied by the caller.
    pub track_name_count: usize,
    /// Track names written to the UFF file, in order.
    pub processed_track_names: Vec<String>,
    /// Any warnings generated during conversion.
    pub warnings: Vec<String>,
}

/// Convert a DTS directory to a UFF Type 58 file while reporting progress.
pub fn convert_with_progress<F>(
    input_dir: &Path,
    tracks_path: &Path,
    output_path: &Path,
    format: OutputFormat,
    mut progress: F,
) -> Result<ConversionReport>
where
    F: FnMut(ConversionProgress<'_>),
{
    // 1. Read track names
    let track_names_raw = fs::read_to_string(tracks_path)
        .with_context(|| format!("Failed to read track names from {}", tracks_path.display()))?;

    let track_names: Vec<String> = track_names_raw
        .split(|c| matches!(c, ',' | '\n' | '\r'))
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(ToOwned::to_owned)
        .collect();

    // 2. Parse DTS metadata
    let dts_reader = dts::DtsReader::new(input_dir)
        .with_context(|| format!("Failed to read DTS metadata from {}", input_dir.display()))?;
    let num_channels = dts_reader.channel_count();

    progress(ConversionProgress::Started {
        track_name_count: track_names.len(),
        channel_count: num_channels,
    });

    // 3. Read channel data in parallel
    let track_names_ref = &track_names;
    let dts_reader_ref = &dts_reader;
    let mut processed_channels = (0..num_channels)
        .into_par_iter()
        .map(|i| -> Result<(usize, String, dts::ChannelData)> {
            let track_name = track_names_ref
                .get(i)
                .cloned()
                .unwrap_or_else(|| format!("Channel_{}", i + 1));

            let channel_data = dts_reader_ref
                .read_track(i)
                .with_context(|| format!("Failed to read channel {}", i + 1))?;

            Ok((i, track_name, channel_data))
        })
        .collect::<Result<Vec<_>>>()?;

    processed_channels.sort_by_key(|(index, _, _)| *index);

    // 4. Stream channel data into the output file
    let file = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(output_path)
        .with_context(|| format!("Failed to open {} for writing", output_path.display()))?;

    let mut writer = BufWriter::with_capacity(8 * 1024 * 1024, file);
    let mut processed_names = Vec::with_capacity(num_channels);

    for (position, (_index, track_name, channel_data)) in processed_channels.into_iter().enumerate()
    {
        match format {
            OutputFormat::Ascii => uff::write_uff58_ascii(&mut writer, &channel_data, &track_name)
                .with_context(|| {
                    format!(
                        "Failed to write ASCII UFF data for channel '{}'",
                        track_name
                    )
                })?,
            OutputFormat::Binary => uff::write_uff58b(&mut writer, &channel_data, &track_name)
                .with_context(|| {
                    format!(
                        "Failed to write binary UFF data for channel '{}'",
                        track_name
                    )
                })?,
        };

        let completed = position + 1;
        progress(ConversionProgress::Advanced {
            completed,
            total: num_channels,
            track_name: &track_name,
        });
        processed_names.push(track_name);
    }

    writer
        .flush()
        .with_context(|| format!("Failed to flush writer for {}", output_path.display()))?;

    progress(ConversionProgress::Finished);

    let mut warnings = Vec::new();
    if track_names.len() != num_channels {
        warnings.push(format!(
            "Number of track names ({}) does not match number of channels ({})",
            track_names.len(),
            num_channels
        ));
    }

    Ok(ConversionReport {
        channel_count: num_channels,
        track_name_count: track_names.len(),
        processed_track_names: processed_names,
        warnings,
    })
}

/// Convert a DTS directory to a UFF file without reporting progress.
pub fn convert(
    input_dir: &Path,
    tracks_path: &Path,
    output_path: &Path,
    format: OutputFormat,
) -> Result<ConversionReport> {
    convert_with_progress(input_dir, tracks_path, output_path, format, |_| {})
}
