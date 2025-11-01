use anyhow::Result;
use clap::Parser;
use dts_to_uff_converter::conversion::{convert_with_progress, ConversionProgress, OutputFormat};
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
        |update| match update {
            ConversionProgress::Started {
                track_name_count,
                channel_count,
            } => {
                println!("Found {} track names.", track_name_count);
                println!("Reading metadata from folder: {:?}", &args.input_dir);
                println!("Found {} channels in the DTS folder.", channel_count);
                if track_name_count != channel_count {
                    println!(
                        "Warning: Number of track names ({}) does not match number of channels ({}).",
                        track_name_count, channel_count
                    );
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
