use anyhow::Context as _;
use dts_to_uff_converter::conversion::{self, OutputFormat, SampleSlice};
use dts_to_uff_converter::dts;
use rust_mcp_sdk::schema::{schema_utils::CallToolError, CallToolResult, TextContent};
use rust_mcp_sdk::{macros::mcp_tool, macros::JsonSchema, tool_box};
use serde::{Deserialize, Serialize};
use serde_json::{Map as JsonMap, Value as JsonValue};
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

        let track_count = track_metadata.len();
        let mut missing_description_count = 0usize;
        let mut missing_unit_count = 0usize;
        let mut unsupported_unit_counts: BTreeMap<String, usize> = BTreeMap::new();

        let tracks: Vec<ListDtsTrack> = track_metadata
            .iter()
            .enumerate()
            .map(|(index, track)| {
                let name = resolved_names
                    .get(index)
                    .cloned()
                    .unwrap_or_else(|| format!("Track {}", index + 1));

                let description = track.description.trim();
                if description.is_empty() {
                    missing_description_count += 1;
                }
                let description = description.to_string();

                let serial = track.serial_number.trim();
                let serial = if serial.is_empty() {
                    None
                } else {
                    Some(serial.to_string())
                };

                let sensitivity = track.sensitivity;
                let sensitivity_m_v_per_g = if sensitivity.is_finite() {
                    Some(sensitivity)
                } else {
                    None
                };

                let raw_unit = track.eu.trim();
                let mut extras: Option<JsonMap<String, JsonValue>> = None;
                let unit = if raw_unit.is_empty() {
                    missing_unit_count += 1;
                    let mut extras_map = JsonMap::new();
                    extras_map.insert("unitDefaultedToG".to_string(), JsonValue::Bool(true));
                    extras = Some(extras_map);
                    "g".to_string()
                } else if raw_unit.eq_ignore_ascii_case("g") {
                    "g".to_string()
                } else {
                    *unsupported_unit_counts
                        .entry(raw_unit.to_string())
                        .or_default() += 1;
                    let mut extras_map = JsonMap::new();
                    extras_map.insert(
                        "rawUnit".to_string(),
                        JsonValue::String(raw_unit.to_string()),
                    );
                    extras_map.insert("unitDefaultedToG".to_string(), JsonValue::Bool(true));
                    extras = Some(extras_map);
                    "g".to_string()
                };

                if let Some(ref mut extras_map) = extras {
                    if description.is_empty() {
                        extras_map.insert("descriptionPresent".to_string(), JsonValue::Bool(false));
                    }
                } else if description.is_empty() {
                    let mut extras_map = JsonMap::new();
                    extras_map.insert("descriptionPresent".to_string(), JsonValue::Bool(false));
                    extras = Some(extras_map);
                }

                ListDtsTrack {
                    channel: (index + 1) as u32,
                    name,
                    description,
                    sampling_rate_hz: track.sampling_rate.round() as u64,
                    sensitivity_m_v_per_g,
                    serial,
                    unit,
                    extras,
                }
            })
            .collect();

        if missing_description_count > 0 {
            warnings.push(format!(
                "{} track{} missing descriptions.",
                missing_description_count,
                if missing_description_count == 1 {
                    ""
                } else {
                    "s"
                }
            ));
        }
        if missing_unit_count > 0 {
            warnings.push(format!(
                "{} track{} missing units; defaulted to 'g'.",
                missing_unit_count,
                if missing_unit_count == 1 { "" } else { "s" }
            ));
        }
        if !unsupported_unit_counts.is_empty() {
            let total: usize = unsupported_unit_counts.values().sum();
            let mut details: Vec<String> = unsupported_unit_counts
                .into_iter()
                .map(|(unit, count)| format!("{count}×{unit}"))
                .collect();
            details.sort();
            warnings.push(format!(
                "{} track{} used unsupported units: {} (reported as 'g').",
                total,
                if total == 1 { "" } else { "s" },
                details.join(", ")
            ));
        }

        warnings.sort();
        warnings.dedup();

        let mut summary = format!(
            "Track metadata for '{}' — {} track{}.",
            input_display,
            track_count,
            if track_count == 1 { "" } else { "s" }
        );

        if let Some(ref path) = tracks_path {
            summary.push_str(&format!(
                " Track names loaded from '{}'.",
                path.to_string_lossy()
            ));
        }

        if !warnings.is_empty() {
            summary.push(' ');
            summary.push_str("Warnings: ");
            summary.push_str(&warnings.join("; "));
        }

        let structured = ListDtsTracksStructuredContent {
            source: input_display,
            count: track_count as u32,
            page: Some(0),
            page_size: if track_count > 0 {
                Some(track_count as u32)
            } else {
                None
            },
            tracks,
        };

        let structured_value = serde_json::to_value(&structured).map_err(|err| {
            CallToolError::from_message(format!(
                "Failed to serialize track metadata response: {err}"
            ))
        })?;

        let structured_map = structured_value.as_object().cloned().ok_or_else(|| {
            CallToolError::from_message(
                "Structured content was not serialized as an object".to_string(),
            )
        })?;

        Ok(
            CallToolResult::text_content(vec![TextContent::from(summary)])
                .with_structured_content(structured_map),
        )
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

#[derive(Debug, Serialize, JsonSchema)]
struct ListDtsTracksStructuredContent {
    source: String,
    count: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    page: Option<u32>,
    #[serde(rename = "pageSize", skip_serializing_if = "Option::is_none")]
    page_size: Option<u32>,
    tracks: Vec<ListDtsTrack>,
}

#[derive(Debug, Serialize, JsonSchema)]
struct ListDtsTrack {
    channel: u32,
    name: String,
    description: String,
    #[serde(rename = "samplingRateHz")]
    sampling_rate_hz: u64,
    #[serde(
        rename = "sensitivity_mV_per_g",
        skip_serializing_if = "Option::is_none"
    )]
    sensitivity_m_v_per_g: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    serial: Option<String>,
    unit: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    extras: Option<JsonMap<String, JsonValue>>,
}
