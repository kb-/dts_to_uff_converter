use anyhow::Result;
use clap::Parser;
use dts_to_uff_converter::{dts, uff};
use indicatif::{ProgressBar, ProgressStyle};
use rayon::prelude::*;
use std::fs::{self, OpenOptions};
use std::io::{BufWriter, Write};
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
    let track_names_ref = &track_names;
    let dts_reader_ref = &dts_reader;

    let mut processed_channels = (0..num_channels)
        .into_par_iter()
        .map(|i| -> Result<(usize, String, dts::ChannelData)> {
            let track_name = track_names_ref
                .get(i)
                .cloned()
                .unwrap_or_else(|| format!("Channel_{}", i + 1));

            let channel_data = dts_reader_ref.read_track(i)?;
            Ok((i, track_name, channel_data))
        })
        .collect::<Result<Vec<_>>>()?;

    processed_channels.sort_by_key(|(index, _, _)| *index);

    let file = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(&args.output)?;
    let mut writer = BufWriter::with_capacity(8 * 1024 * 1024, file);

    for (_index, track_name, channel_data) in processed_channels {
        bar.set_message(track_name.clone());
        uff::write_uff58(&mut writer, &channel_data, &track_name)?;
        bar.inc(1);
    }

    writer.flush()?;

    bar.finish_with_message("Conversion complete!");
    Ok(())
}
