mod dts;
mod uff;

use anyhow::Result;
use clap::Parser;
use indicatif::{ProgressBar, ProgressStyle};
use std::fs;
use std::path::PathBuf;

/// A Rust utility to convert a DTS Test Folder to a UFF (Universal File Format) Type 58 file.
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Path to the input directory containing DTS files (.dts, .chn)
    #[arg(short, long)]
    input_dir: PathBuf,

    /// Path to the .txt file containing track names, one per line or comma-separated
    #[arg(short, long)]
    tracks: PathBuf,

    /// Output UFF file path
    #[arg(short, long)]
    output: PathBuf,
}

fn main() -> Result<()> {
    let args = Args::parse();

    // 1. Read track names from the specified text file
    let track_names_str = fs::read_to_string(&args.tracks)?;
    let track_names: Vec<String> = track_names_str
        .split(|c| c == ',' || c == '\n' || c == '\r')
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .collect();

    println!("Found {} track names.", track_names.len());

    // 2. Initialize the DTS reader, which parses all metadata from the folder.
    println!("Reading metadata from folder: {:?}", &args.input_dir);
    let dts_reader = dts::DtsReader::new(&args.input_dir)?;
    let num_channels = dts_reader.channel_count();
    println!("Found {} channels in the DTS folder.", num_channels);

    if track_names.len() != num_channels {
        println!(
            "Warning: Number of track names ({}) does not match number of channels ({}).",
            track_names.len(),
            num_channels
        );
    }

    // 3. Set up progress bar
    let bar = ProgressBar::new(num_channels as u64);
    bar.set_style(
        ProgressStyle::default_bar()
            .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} ({percent}%) - Processing {msg}")?
            .progress_chars("#>-"),
    );

    // 4. Loop through each channel, read its data, and append to the UFF file.
    let mut append_request = args.output.exists();
    for i in 0..num_channels {
        let track_name = track_names.get(i).cloned().unwrap_or_else(|| format!("Channel_{}", i + 1));
        bar.set_message(track_name.clone());

        // Read data for one track only
        let channel_data = dts_reader.read_track(i)?;

        // Write the data to the UFF file
        uff::write_uff58_file(&args.output, &channel_data, &track_name, append_request)?;
        append_request = true;

        bar.inc(1);
    }

    bar.finish_with_message("Conversion complete!");
    Ok(())
}
