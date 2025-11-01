use anyhow::Result;
use dts_to_uff_converter::dts;
use std::path::Path;

#[test]
fn track_metadata_matches_dts_file() -> Result<()> {
    let data_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("Bancairon_G1_training6_small");
    let reader = dts::DtsReader::new(&data_dir)?;
    let metadata = reader.track_metadata();

    assert_eq!(metadata.len(), 2);

    let first = &metadata[0];
    assert_eq!(first.name, "IEPE 100 mV/g");
    assert_eq!(first.description, "IEPE 100 mV/g");
    assert!((first.sampling_rate - 200_000.0).abs() < f64::EPSILON);
    assert!((first.sensitivity - 98.5176059).abs() < 1e-6);
    assert_eq!(first.serial_number, "PCB_B34_xx");
    assert_eq!(first.eu, "g");

    Ok(())
}
