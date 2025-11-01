use anyhow::Result;
use dts_to_uff_converter::{
    dts,
    uff::{self, Uff58Format},
};
use std::fs;
use std::path::Path;
use tempfile::NamedTempFile;

#[test]
fn rust_output_matches_matlab_reference() -> Result<()> {
    let data_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("Bancairon_G1_training6_small");
    let tracks_path = data_dir.join("tracks.txt");
    let track_names: Vec<String> = fs::read_to_string(&tracks_path)?
        .split(|c| [',', '\n', '\r'].contains(&c))
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .collect();

    let reader = dts::DtsReader::new(&data_dir)?;
    assert_eq!(reader.channel_count(), track_names.len());

    let output = NamedTempFile::new()?;
    let output_path = output.path().to_path_buf();
    let mut append_request = false;

    for (i, track_name) in track_names.iter().enumerate() {
        let channel_data = reader.read_track(i)?;
        uff::write_uff58_file_with_format(
            &output_path,
            &channel_data,
            track_name,
            append_request,
            Uff58Format::Ascii,
        )?;
        append_request = true;
    }

    let expected = fs::read(data_dir.join("matlab_converted.uff"))?;
    let produced = fs::read(output_path)?;
    assert_eq!(produced, expected);

    Ok(())
}
