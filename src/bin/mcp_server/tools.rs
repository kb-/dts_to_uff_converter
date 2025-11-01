use dts_to_uff_converter::conversion::{self, OutputFormat, SampleSlice};
use rust_mcp_sdk::schema::{schema_utils::CallToolError, CallToolResult, TextContent};
use rust_mcp_sdk::{macros::mcp_tool, macros::JsonSchema, tool_box};
use serde::{Deserialize, Serialize};
use std::fmt::Write as _;
use std::path::PathBuf;
use std::str::FromStr;

tool_box!(ConverterTools, [ConvertDtsToUff]);

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
    /// Absolute path to the folder containing .dts/.chn files.
    input_dir: String,
    /// Path to the text file with track names (newline or comma separated).
    tracks_file: String,
    /// Location where the generated .uff file should be written.
    output_path: String,
    /// Output format (`ascii` or `binary`). Defaults to `ascii`.
    #[serde(default)]
    format: Option<String>,
    /// Optional comma-separated list of track names to write.
    #[serde(default)]
    track_list_output: Option<String>,
    /// Optional slice (start:end) of samples to export for each track.
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

        let mut summary = format!(
            "Converted {} channel(s) from '{}' to '{}' using {} format.",
            report.channel_count, input_display, output_display, format_display
        );

        if let Some(selection) = track_selection.as_ref() {
            let _ = write!(&mut summary, " Requested tracks: {}.", selection.join(", "));
        }

        if report.track_name_count != report.channel_count {
            let _ = write!(
                &mut summary,
                " Track name count ({}) differed from channel count ({}).",
                report.track_name_count, report.channel_count
            );
        }

        if !report.processed_track_names.is_empty() {
            let preview: Vec<_> = report
                .processed_track_names
                .iter()
                .take(5)
                .map(String::as_str)
                .collect();
            let ellipsis = if report.processed_track_names.len() > preview.len() {
                " …"
            } else {
                ""
            };
            let _ = write!(
                &mut summary,
                " First tracks: {}{}.",
                preview.join(", "),
                ellipsis
            );
        }

        Ok(CallToolResult::text_content(vec![TextContent::from(
            summary,
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
