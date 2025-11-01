use anyhow::Result;
use clap::Parser;
use dts_to_uff_converter::conversion::{
    convert_with_progress, ConversionProgress, OutputFormat, SampleSlice,
};
use indicatif::{ProgressBar, ProgressStyle};
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Path to the input directory containing DTS files (.dts, .chn)
    #[arg(short, long)]
    input_dir: PathBuf,

    /// Output format for the generated UFF file (`ascii` or `binary`)
    #[arg(short, long, value_enum, default_value_t = OutputFormat::Ascii)]
    format: OutputFormat,

    /// Path to the .txt file containing track names, one per line or comma-separated
    #[arg(short, long)]
    tracks: PathBuf,

    /// Output UFF file path
    #[arg(short, long)]
    output: PathBuf,

    /// Sample range to export for every track, in the form `start:end` using zero-based indices.
    /// The start index is inclusive, the end index is exclusive, and values must be non-negative.
    /// The same slice is applied to each track in its native sample units. Omit the flag to export every sample.
    #[arg(long, value_parser = parse_sample_slice)]
    slice: Option<SampleSlice>,

    /// Comma-separated list of track names to write into the output file.
    #[arg(long = "track-list-output", value_parser = parse_track_selection)]
    track_list_output: Option<Vec<String>>,
}

fn parse_sample_slice(value: &str) -> Result<SampleSlice, String> {
    value.parse()
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

fn main() -> Result<()> {
    let args = Args::parse();

    let bar = ProgressBar::new(0);
    bar.set_style(
        ProgressStyle::default_bar()
            .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} ({percent}%) - Processing {msg}")?
            .progress_chars("#>-"),
    );

    let _report = convert_with_progress(
        &args.input_dir,
        &args.tracks,
        &args.output,
        args.format,
        args.slice,
        args.track_list_output.as_deref(),
        |update| match update {
            ConversionProgress::Started {
                track_name_count,
                channel_count,
            } => {
                println!("Found {} track names.", track_name_count);
                println!("Reading metadata from folder: {:?}", &args.input_dir);
                println!("Will export {} channel(s).", channel_count);
                if args.track_list_output.is_none() && track_name_count != channel_count {
                    println!(
                        "Warning: Number of track names ({}) does not match number of channels ({}).",
                        track_name_count, channel_count
                    );
                }
                if let Some(selection) = args.track_list_output.as_ref() {
                    println!("Requested tracks: {}.", selection.join(", "));
                }
                bar.set_length(channel_count as u64);
            }
            ConversionProgress::Advanced {
                completed,
                total: _,
                track_name,
            } => {
                bar.set_message(track_name.to_string());
                bar.set_position(completed as u64);
            }
            ConversionProgress::Finished => {
                bar.finish_with_message("Conversion complete!");
            }
        },
    )?;

    Ok(())
}
