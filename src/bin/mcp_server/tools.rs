use anyhow::Context as _;
use dts_to_uff_converter::conversion::{self, OutputFormat, SampleSlice};
use dts_to_uff_converter::dts;
use rust_mcp_sdk::schema::{schema_utils::CallToolError, CallToolResult, TextContent};
use rust_mcp_sdk::{macros::mcp_tool, macros::JsonSchema, tool_box};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::str::FromStr;

tool_box!(ConverterTools, [ConvertDtsToUff, ListDtsTracks]);

#[mcp_tool(
    name = "convert_dts_to_uff",
    description = "Convert a DTS test folder into a UFF Type 58 file.",
    title = "Convert DTS folder to UFF",
    idempotent_hint = false,
    destructive_hint = true,
    open_world_hint = false,
    read_only_hint = false,
    meta = r#"{"version": "0.1.0"}"#
)]
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct ConvertDtsToUff {
    /// Absolute path to the DTS export directory containing `.dts`/`.chn` files. Pass a
    /// directory path, not an individual file.
    input_dir: String,
    /// Absolute path to the text file with track names (newline or comma separated). Pass a
    /// file path, not a directory.
    tracks_file: String,
    /// Absolute path (including filename) where the generated `.uff` file should be written.
    /// Pass a file path; the parent directory must already exist.
    output_path: String,
    /// Output format (`ascii` or `binary`). Defaults to `ascii`.
    #[serde(default)]
    format: Option<String>,
    /// Optional comma-separated list of track names to write.
    #[serde(default)]
    track_list_output: Option<String>,
    /// Optional slice of samples to export for each track, written as `start:end`.
    /// Indices are zero-based, the start is inclusive, the end is exclusive, and step values
    /// are not supported. Values must be non-negative integers expressed in native sample
    /// units for every track. The same slice is applied to all tracks and requests that fall
    /// outside the available samples will return an error instead of clamping. Omit the field
    /// to export the full range.
    #[serde(default)]
    slice: Option<String>,
}

impl ConvertDtsToUff {
    pub async fn call_tool(&self) -> Result<CallToolResult, CallToolError> {
        if self.input_dir.trim().is_empty() {
            return Err(CallToolError::invalid_arguments(
                "convert_dts_to_uff",
                Some("`input_dir` cannot be empty".to_string()),
            ));
        }
        if self.tracks_file.trim().is_empty() {
            return Err(CallToolError::invalid_arguments(
                "convert_dts_to_uff",
                Some("`tracks_file` cannot be empty".to_string()),
            ));
        }

        let output_path_str = self.output_path.trim();
        if output_path_str.is_empty() {
            return Err(CallToolError::invalid_arguments(
                "convert_dts_to_uff",
                Some("`output_path` cannot be empty".to_string()),
            ));
        }

        let track_selection = self
            .track_list_output
            .as_ref()
            .map(|value| value.trim())
            .map(parse_track_selection)
            .transpose()
            .map_err(|err| CallToolError::invalid_arguments("convert_dts_to_uff", Some(err)))?;

        let slice = self
            .slice
            .as_deref()
            .map(SampleSlice::from_str)
            .transpose()
            .map_err(|err| CallToolError::invalid_arguments("convert_dts_to_uff", Some(err)))?;

        let input_dir = PathBuf::from(&self.input_dir);
        let tracks_file = PathBuf::from(&self.tracks_file);
        let output_path = PathBuf::from(output_path_str);

        let format = self
            .format
            .as_deref()
            .map(OutputFormat::from_str)
            .transpose()
            .map_err(|err| CallToolError::invalid_arguments("convert_dts_to_uff", Some(err)))?
            .unwrap_or(OutputFormat::Ascii);

        let input_display = input_dir.to_string_lossy().into_owned();
        let output_display = output_path.to_string_lossy().into_owned();
        let format_display = format.to_string();

        let report = tokio::task::spawn_blocking({
            let input_dir = input_dir.clone();
            let tracks_file = tracks_file.clone();
            let output_path = output_path.clone();
            let track_selection = track_selection.clone();
            move || {
                let track_filter = track_selection.as_deref();
                conversion::convert_with_options(
                    &input_dir,
                    &tracks_file,
                    &output_path,
                    format,
                    slice,
                    track_filter,
                )
            }
        })
        .await
        .map_err(|err| CallToolError::from_message(format!("Background task failed: {err}")))?
        .map_err(|err| CallToolError::from_message(err.to_string()))?;

        let tracks_display = tracks_file.to_string_lossy().into_owned();

        let mut summary = String::new();
        let _ = writeln!(&mut summary, "✅ **DTS to UFF conversion succeeded**");
        let _ = writeln!(&mut summary);
        let _ = writeln!(&mut summary, "- **Input directory:** `{}`", input_display);
        let _ = writeln!(&mut summary, "- **Track names file:** `{}`", tracks_display);
        let _ = writeln!(&mut summary, "- **Output file:** `{}`", output_display);
        let _ = writeln!(&mut summary, "- **Format:** `{}`", format_display);
        let _ = writeln!(
            &mut summary,
            "- **Channels written:** {}",
            report.channel_count
        );
        let _ = writeln!(
            &mut summary,
            "- **Track names provided:** {}",
            report.track_name_count
        );

        match track_selection.as_ref() {
            Some(selection) if !selection.is_empty() => {
                let _ = writeln!(
                    &mut summary,
                    "- **Requested tracks:** {}",
                    selection.join(", ")
                );
            }
            _ => {
                let _ = writeln!(&mut summary, "- **Requested tracks:** All");
            }
        }

        if let Some(range) = slice.map(|value| format!("{}:{}", value.start, value.end)) {
            let _ = writeln!(&mut summary, "- **Sample slice:** `{}`", range);
        } else {
            let _ = writeln!(&mut summary, "- **Sample slice:** full range");
        }

        if report.track_name_count != report.channel_count {
            let _ = writeln!(
                &mut summary,
                "\n⚠️ Track name count ({}) differed from channels processed ({}).",
                report.track_name_count, report.channel_count
            );
        }

        if !report.warnings.is_empty() {
            let _ = writeln!(&mut summary, "\n**Warnings:**");
            for warning in &report.warnings {
                let _ = writeln!(&mut summary, "- {warning}");
            }
        }

        if !report.processed_track_names.is_empty() {
            let _ = writeln!(&mut summary, "\n**Track preview:**");
            for (idx, name) in report.processed_track_names.iter().take(5).enumerate() {
                let _ = writeln!(&mut summary, "{}. {}", idx + 1, name);
            }
            if report.processed_track_names.len() > 5 {
                let remaining = report.processed_track_names.len() - 5;
                let _ = writeln!(&mut summary, "… and {remaining} more track(s).");
            }
        }

        Ok(CallToolResult::text_content(vec![TextContent::from(
            summary,
        )]))
    }
}

#[mcp_tool(
    name = "list_dts_tracks",
    description = "List metadata for each track inside a DTS export directory.",
    title = "List DTS track metadata",
    idempotent_hint = true,
    destructive_hint = false,
    open_world_hint = false,
    read_only_hint = true,
    meta = r#"{"version": "0.1.0"}"#
)]
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct ListDtsTracks {
    /// Absolute path to the DTS export directory containing `.dts`/`.chn` files. Pass a
    /// directory path, not an individual file.
    input_dir: String,
    /// Optional absolute path to the text file with track names used for UFF export ordering.
    #[serde(default)]
    tracks_file: Option<String>,
}

impl ListDtsTracks {
    pub async fn call_tool(&self) -> Result<CallToolResult, CallToolError> {
        if self.input_dir.trim().is_empty() {
            return Err(CallToolError::invalid_arguments(
                "list_dts_tracks",
                Some("`input_dir` cannot be empty".to_string()),
            ));
        }

        if let Some(path) = self.tracks_file.as_ref() {
            if path.trim().is_empty() {
                return Err(CallToolError::invalid_arguments(
                    "list_dts_tracks",
                    Some("`tracks_file` cannot be empty when provided".to_string()),
                ));
            }
        }

        let input_dir = PathBuf::from(&self.input_dir);
        let input_display = input_dir.to_string_lossy().into_owned();
        let tracks_path = self
            .tracks_file
            .as_ref()
            .map(|value| PathBuf::from(value.trim()));

        let (track_metadata, track_names) = tokio::task::spawn_blocking({
            let input_dir = input_dir.clone();
            let tracks_path = tracks_path.clone();
            move || -> anyhow::Result<(Vec<dts::TrackMetadata>, Option<Vec<String>>)> {
                let reader = dts::DtsReader::new(&input_dir)?;
                let metadata = reader.track_metadata();
                let track_names = match tracks_path {
                    Some(ref path) => Some(load_track_names(path)?),
                    None => None,
                };
                Ok((metadata, track_names))
            }
        })
        .await
        .map_err(|err| CallToolError::from_message(format!("Background task failed: {err}")))?
        .map_err(|err| CallToolError::from_message(err.to_string()))?;

        if track_metadata.is_empty() {
            return Ok(CallToolResult::text_content(vec![TextContent::from(
                format!("No tracks found in '{input_display}'."),
            )]));
        }

        let resolved_names: Vec<String> = track_metadata
            .iter()
            .enumerate()
            .map(|(index, track)| {
                track_names
                    .as_ref()
                    .and_then(|names| names.get(index))
                    .map(|name| name.trim())
                    .filter(|name| !name.is_empty())
                    .map(ToOwned::to_owned)
                    .unwrap_or_else(|| {
                        let fallback = track.name.trim();
                        if fallback.is_empty() {
                            format!("Track {}", index + 1)
                        } else {
                            fallback.to_string()
                        }
                    })
            })
            .collect();

        let mut warnings = Vec::new();
        if let Some(ref names) = track_names {
            if names.len() != track_metadata.len() {
                warnings.push(format!(
                    "Track name count ({}) differs from metadata entries ({}).",
                    names.len(),
                    track_metadata.len()
                ));
            }
        }

        let mut sampling_rate_counts: BTreeMap<String, usize> = BTreeMap::new();
        let mut unit_counts: BTreeMap<String, usize> = BTreeMap::new();
        for (index, track) in track_metadata.iter().enumerate() {
            *sampling_rate_counts
                .entry(format_sampling_rate(track.sampling_rate))
                .or_default() += 1;

            let unit_key = track.eu.trim();
            let unit_entry = if unit_key.is_empty() { "—" } else { unit_key };
            *unit_counts.entry(unit_entry.to_string()).or_default() += 1;

            if track.description.trim().is_empty() {
                warnings.push(format!(
                    "Track '{}' is missing a description.",
                    resolved_names
                        .get(index)
                        .map(|name| name.as_str())
                        .unwrap_or("")
                ));
            }
        }

        warnings.sort();
        warnings.dedup();

        let mut message = String::new();
        let track_count = track_metadata.len();
        let _ = writeln!(&mut message, "📊 **Track metadata overview**");
        let _ = writeln!(&mut message);
        let _ = writeln!(&mut message, "- **Input directory:** `{}`", input_display);
        match tracks_path {
            Some(ref path) => {
                let display = path.to_string_lossy();
                let names_loaded_display = track_names
                    .as_ref()
                    .map(|names| {
                        let count = names.len();
                        format!("{} name{}", count, if count == 1 { "" } else { "s" })
                    })
                    .unwrap_or_else(|| "no usable names".to_string());
                let _ = writeln!(
                    &mut message,
                    "- **Track names file:** `{}` ({})",
                    display, names_loaded_display
                );
            }
            None => {
                let _ = writeln!(&mut message, "- **Track names file:** not provided");
            }
        }
        let _ = writeln!(&mut message, "- **Tracks discovered:** {}", track_count);

        if !sampling_rate_counts.is_empty() {
            let sampling_rate_summary: Vec<String> = sampling_rate_counts
                .into_iter()
                .map(|(rate, count)| {
                    format!(
                        "{} Hz ({} track{})",
                        rate,
                        count,
                        if count == 1 { "" } else { "s" }
                    )
                })
                .collect();
            let _ = writeln!(
                &mut message,
                "- **Sampling rates:** {}",
                sampling_rate_summary.join(", ")
            );
        }

        if !unit_counts.is_empty() {
            let unit_summary: Vec<String> = unit_counts
                .into_iter()
                .map(|(unit, count)| {
                    format!(
                        "{} ({} track{})",
                        unit,
                        count,
                        if count == 1 { "" } else { "s" }
                    )
                })
                .collect();
            let _ = writeln!(
                &mut message,
                "- **Units present:** {}",
                unit_summary.join(", ")
            );
        }

        if !resolved_names.is_empty() {
            let preview_count = resolved_names.len().min(5);
            let preview = resolved_names
                .iter()
                .take(preview_count)
                .map(String::as_str)
                .collect::<Vec<_>>()
                .join(", ");
            if track_count > preview_count {
                let remaining = track_count - preview_count;
                let _ = writeln!(
                    &mut message,
                    "- **Track preview:** {} … (+{} more)",
                    preview, remaining
                );
            } else {
                let _ = writeln!(&mut message, "- **Track preview:** {}", preview);
            }
        }

        if !warnings.is_empty() {
            let _ = writeln!(&mut message, "\n**Warnings:**");
            for warning in warnings {
                let _ = writeln!(&mut message, "- {warning}");
            }
        }

        let _ = writeln!(&mut message, "\n**Track details**");
        let _ = writeln!(&mut message);

        let mut table = String::new();
        let _ = writeln!(
            &mut table,
            "| # | Name | Sampling Rate (Hz) | Description | Sensitivity | Serial Number | Units |"
        );
        let _ = writeln!(
            &mut table,
            "|---|------|---------------------|-------------|-------------|---------------|-------|"
        );

        for (index, track) in track_metadata.iter().enumerate() {
            let description = if track.description.trim().is_empty() {
                "—"
            } else {
                track.description.trim()
            };
            let serial = if track.serial_number.trim().is_empty() {
                "—"
            } else {
                track.serial_number.trim()
            };
            let units = if track.eu.trim().is_empty() {
                "—"
            } else {
                track.eu.trim()
            };
            let _ = writeln!(
                &mut table,
                "| {} | {} | {:.0} | {} | {:.6} | {} | {} |",
                index + 1,
                resolved_names
                    .get(index)
                    .map(|name| name.as_str())
                    .unwrap_or(""),
                track.sampling_rate,
                description,
                track.sensitivity,
                serial,
                units
            );
        }

        message.push_str(&table);

        Ok(CallToolResult::text_content(vec![TextContent::from(
            message,
        )]))
    }
}

fn parse_track_selection(value: &str) -> Result<Vec<String>, String> {
    let tracks: Vec<String> = value
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(ToOwned::to_owned)
        .collect();

    if tracks.is_empty() {
        Err("At least one track name must be provided".to_string())
    } else {
        Ok(tracks)
    }
}

fn load_track_names(path: &Path) -> anyhow::Result<Vec<String>> {
    let contents = std::fs::read_to_string(path)
        .with_context(|| format!("Failed to read track names from {}", path.display()))?;

    let names: Vec<String> = contents
        .split([',', '\n', '\r'])
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(ToOwned::to_owned)
        .collect();

    if names.is_empty() {
        anyhow::bail!(
            "Track name file '{}' did not contain any usable entries",
            path.display()
        );
    }

    Ok(names)
}

fn format_sampling_rate(value: f64) -> String {
    if (value.fract()).abs() <= f64::EPSILON {
        format!("{value:.0}")
    } else {
        format!("{value:.6}")
    }
}
