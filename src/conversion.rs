use crate::{dts, uff};
use anyhow::{anyhow, Context, Result};
use clap::ValueEnum;
use rayon::prelude::*;
use std::fs::{self, OpenOptions};
use std::io::{BufWriter, Write};
use std::ops::Range;
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

/// A slice of samples to export for every processed track.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SampleSlice {
    /// Inclusive starting index of the slice.
    pub start: usize,
    /// Exclusive ending index of the slice.
    pub end: usize,
}

impl SampleSlice {
    /// Validate the slice for the provided vector length and return it as a range.
    pub fn as_range(&self, len: usize) -> Result<Range<usize>> {
        if self.start >= self.end {
            return Err(anyhow!(
                "Invalid slice: start index ({}) must be less than end index ({}).",
                self.start,
                self.end
            ));
        }

        if self.end > len {
            return Err(anyhow!(
                "Invalid slice: end index ({}) exceeds available samples ({}).",
                self.end,
                len
            ));
        }

        Ok(self.start..self.end)
    }
}

impl std::str::FromStr for SampleSlice {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (start, end) = s
            .split_once(':')
            .ok_or_else(|| "Slice must be provided in the format start:end".to_string())?;

        let start = start
            .trim()
            .parse::<usize>()
            .map_err(|err| format!("Failed to parse slice start '{start}': {err}"))?;
        let end = end
            .trim()
            .parse::<usize>()
            .map_err(|err| format!("Failed to parse slice end '{end}': {err}"))?;

        Ok(SampleSlice { start, end })
    }
}

/// Convert a DTS directory to a UFF Type 58 file while reporting progress.
pub fn convert_with_progress<F>(
    input_dir: &Path,
    tracks_path: &Path,
    output_path: &Path,
    format: OutputFormat,
    slice: Option<SampleSlice>,
    track_list_filter: Option<&[String]>,
    mut progress: F,
) -> Result<ConversionReport>
where
    F: FnMut(ConversionProgress<'_>),
{
    // 1. Read track names
    let track_names_raw = fs::read_to_string(tracks_path)
        .with_context(|| format!("Failed to read track names from {}", tracks_path.display()))?;

    let track_names: Vec<String> = track_names_raw
        .split([',', '\n', '\r'])
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(ToOwned::to_owned)
        .collect();

    // 2. Parse DTS metadata
    let dts_reader = dts::DtsReader::new(input_dir)
        .with_context(|| format!("Failed to read DTS metadata from {}", input_dir.display()))?;
    let num_channels = dts_reader.channel_count();

    let mut warnings = Vec::new();

    let channel_plan: Vec<(usize, usize)> = if let Some(filter) = track_list_filter {
        let mut plan = Vec::new();
        let mut used = vec![false; track_names.len()];

        for (order, requested_name) in filter.iter().enumerate() {
            if let Some((channel_index, _)) = track_names
                .iter()
                .enumerate()
                .find(|(idx, name)| !used[*idx] && *name == requested_name)
            {
                used[channel_index] = true;
                plan.push((order, channel_index));
            } else {
                warnings.push(format!(
                    "Requested track '{requested_name}' was not found in the provided track list."
                ));
            }
        }

        plan
    } else {
        (0..num_channels).map(|index| (index, index)).collect()
    };

    progress(ConversionProgress::Started {
        track_name_count: track_names.len(),
        channel_count: channel_plan.len(),
    });

    // 3. Read channel data in parallel
    let track_names_ref = &track_names;
    let dts_reader_ref = &dts_reader;
    let mut processed_channels = channel_plan
        .into_par_iter()
        .map(
            |(order, channel_index)| -> Result<(usize, String, dts::ChannelData)> {
                let track_name = track_names_ref
                    .get(channel_index)
                    .cloned()
                    .unwrap_or_else(|| format!("Channel_{}", channel_index + 1));

                let mut channel_data = dts_reader_ref
                    .read_track(channel_index)
                    .with_context(|| format!("Failed to read channel {}", channel_index + 1))?;

                if let Some(slice) = slice {
                    let len = channel_data.time_series.len();
                    let range = slice.as_range(len)?;
                    channel_data.time_series = channel_data.time_series[range].to_vec();
                }

                Ok((order, track_name, channel_data))
            },
        )
        .collect::<Result<Vec<_>>>()?;

    processed_channels.sort_by_key(|(order, _, _)| *order);

    // 4. Stream channel data into the output file
    let file = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(output_path)
        .with_context(|| format!("Failed to open {} for writing", output_path.display()))?;

    let mut writer = BufWriter::with_capacity(8 * 1024 * 1024, file);
    let total_channels = processed_channels.len();
    let mut processed_names = Vec::with_capacity(total_channels);

    for (position, (_order, track_name, channel_data)) in processed_channels.into_iter().enumerate()
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
            total: total_channels,
            track_name: &track_name,
        });
        processed_names.push(track_name);
    }

    writer
        .flush()
        .with_context(|| format!("Failed to flush writer for {}", output_path.display()))?;

    progress(ConversionProgress::Finished);

    let processed_channel_count = processed_names.len();

    if track_names.len() != num_channels {
        warnings.push(format!(
            "Number of track names ({}) does not match number of channels ({})",
            track_names.len(),
            num_channels
        ));
    }

    if track_list_filter.is_none() && processed_channel_count != num_channels {
        warnings.push(format!(
            "Channel count ({num_channels}) did not match processed channel count ({processed_channel_count})."
        ));
    }

    Ok(ConversionReport {
        channel_count: processed_channel_count,
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
    convert_with_progress(
        input_dir,
        tracks_path,
        output_path,
        format,
        None,
        None,
        |_| {},
    )
}

/// Convert with optional sample slicing and track list extraction without reporting progress.
pub fn convert_with_options(
    input_dir: &Path,
    tracks_path: &Path,
    output_path: &Path,
    format: OutputFormat,
    slice: Option<SampleSlice>,
    track_list_filter: Option<&[String]>,
) -> Result<ConversionReport> {
    convert_with_progress(
        input_dir,
        tracks_path,
        output_path,
        format,
        slice,
        track_list_filter,
        |_| {},
    )
}
