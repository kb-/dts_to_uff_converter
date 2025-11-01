use dts_to_uff_converter::conversion::{self, OutputFormat};
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
        if self.output_path.trim().is_empty() {
            return Err(CallToolError::invalid_arguments(
                "convert_dts_to_uff",
                Some("`output_path` cannot be empty".to_string()),
            ));
        }

        let input_dir = PathBuf::from(&self.input_dir);
        let tracks_file = PathBuf::from(&self.tracks_file);
        let output_path = PathBuf::from(&self.output_path);

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

        let report = tokio::task::spawn_blocking(move || {
            conversion::convert(&input_dir, &tracks_file, &output_path, format)
        })
        .await
        .map_err(|err| CallToolError::from_message(format!("Background task failed: {err}")))?
        .map_err(|err| CallToolError::from_message(err.to_string()))?;

        let mut summary = format!(
            "Converted {} channel(s) from '{}' to '{}' using {} format.",
            report.channel_count, input_display, output_display, format_display
        );

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
