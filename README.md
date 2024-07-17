# wav2json

A library that decodes wav audio files into json waveform data

[![Crates.io](https://img.shields.io/crates/v/wav2json.svg)](https://crates.io/crates/wav2json)
[![Documentation](https://docs.rs/wav2json/badge.svg)][dox]
[![MIT licensed](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

More information about this crate can be found in the [crate documentation][dox].

[dox]: https://docs.rs/wav2json

## Usage

To use `wav2json`, first add this to your `Cargo.toml`:

```toml
[dependencies]
wav2json = "0.1.0"
```

Next, add this to your crate:

```rust
use wav2json::Wav;

fn main() {
    // ...
}
```

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

Copy the resulting json data into the index.html file, and then open the web page to see the effect as shown in the following image

![Waveform picture](/examples/example.png)

## License

wav2json is provided under the MIT license. See [LICENSE](LICENSE).
