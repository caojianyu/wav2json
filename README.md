# wave2json

A library that decodes wav audio files into json waveform data

## Examples

Generate a json waveform data file:

```rust
use std::{fs::File, io::Write, time::Instant};
use wav2json::Wav;

fn main() {
    // Support for local and network files.
    // let mut wav = Wav::new("http://[host]/sample-15s.wav").set_json_width(1920);
    let mut wav = Wav::new("examples/sample-15s.wav").set_json_width(1920);
    let result_data = wav.decode();

    println!("duration: {}", Instant::now().elapsed().as_millis());

    // Generate json data files that can be rendered.
    let mut json_file = File::create("examples/data.json").expect("create failed");
    json_file.write(b"[").unwrap();
    for v in result_data.iter() {
        json_file.write(v.to_string().as_bytes()).unwrap();
        json_file.write(b",").unwrap();
    }
    json_file.write(b"]").unwrap();
}
```