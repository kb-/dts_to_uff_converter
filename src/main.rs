use anyhow::{anyhow, Result};
use clap::Parser;
use dts_to_uff_converter::{dts, uff};
use indicatif::{ProgressBar, ProgressStyle};
use rayon::current_num_threads;
use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;
use std::sync::mpsc::sync_channel;
use std::thread;

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
    let append_request = args.output.exists();

    let queue_bound = current_num_threads().max(1) * 2;
    let (tx, rx) = sync_channel::<Result<(usize, String, dts::ChannelData)>>(queue_bound);

    let writer_output = args.output.clone();
    let writer_handle = thread::spawn(move || -> Result<()> {
        let mut append_flag = append_request;
        let mut next_index = 0usize;
        let mut pending: BTreeMap<usize, (String, dts::ChannelData)> = BTreeMap::new();

        for received in rx {
            let (index, track_name, channel_data) = received?;
            pending.insert(index, (track_name, channel_data));

            while let Some((track_name, channel_data)) = pending.remove(&next_index) {
                uff::write_uff58_file(&writer_output, &channel_data, &track_name, append_flag)?;
                append_flag = true;
                next_index += 1;
            }
        }

        // Flush any remaining buffered channels once the sender closes.
        while let Some((track_name, channel_data)) = pending.remove(&next_index) {
            uff::write_uff58_file(&writer_output, &channel_data, &track_name, append_flag)?;
            append_flag = true;
            next_index += 1;
        }

        Ok(())
    });

    let track_names_ref = &track_names;
    let dts_reader_ref = &dts_reader;

    rayon::scope(|scope| {
        for i in 0..num_channels {
            let sender = tx.clone();
            let progress = bar.clone();
            let track_names = track_names_ref;
            let dts_reader = dts_reader_ref;
            scope.spawn(move |_| {
                let track_name = track_names
                    .get(i)
                    .cloned()
                    .unwrap_or_else(|| format!("Channel_{}", i + 1));

                progress.set_message(track_name.clone());

                let result = match dts_reader.read_track(i) {
                    Ok(channel_data) => {
                        progress.inc(1);
                        Ok((i, track_name, channel_data))
                    }
                    Err(err) => Err(err),
                };

                // Ignore failures when the receiver has already been dropped due to an earlier error.
                let _ = sender.send(result);
            });
        }
    });

    drop(tx);

    writer_handle
        .join()
        .map_err(|_| anyhow!("writer thread panicked"))??;

    bar.finish_with_message("Conversion complete!");
    Ok(())
}
