use std::{error::Error, fs::File, time::Instant};
use wav2json::Wav;

fn main() -> Result<(), Box<dyn Error>> {
    let start = Instant::now();

    // Support for local and network files.
    // let mut wav = Wav::new("http://[host]/sample-15s.wav")?.set_json_width(1920);
    let mut wav = Wav::new("examples/sample-15s.wav")?.set_json_width(1920);
    let result_data = wav.decode()?;

    println!("duration: {}", start.elapsed().as_millis());

    // Generate json data files that can be rendered.
    let json_file = File::create("examples/data.json")?;
    serde_json::to_writer(json_file, &result_data)?;

    Ok(())
}
