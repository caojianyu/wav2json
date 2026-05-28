use std::{
    error::Error as StdError,
    fmt,
    fs::File,
    io::{self, BufReader, Read},
};

use reqwest::blocking::Response;

/// Bytes used by the wav file header.
const LEN_CHUNK_DESCRIPTOR: usize = 4;
const LEN_WAVE_FLAG: usize = 4;
const LEN_CHUNK_ID: usize = 4;
const MIN_FMT_CHUNK_SIZE: u32 = 16;
const DEFAULT_JSON_WIDTH: u32 = 1000;

/// Wav file header information.
const RIFF: &str = "RIFF";
const WAVE: &str = "WAVE";
const FMT_: &str = "fmt ";
const DATA: &str = "data";

#[derive(Debug)]
enum WavReader {
    FileReader(BufReader<File>),
    ResponseReader(BufReader<Response>),
}

impl Read for WavReader {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        match self {
            WavReader::FileReader(reader) => reader.read(buf),
            WavReader::ResponseReader(reader) => reader.read(buf),
        }
    }
}

/// Error type returned while loading or decoding a wav file.
#[derive(Debug)]
pub enum WavError {
    Io(io::Error),
    Http(reqwest::Error),
    InvalidHeader {
        field: &'static str,
        expected: &'static str,
        actual: String,
    },
    InvalidData(String),
    InvalidJsonWidth,
    UnsupportedAudioFormat(u16),
    UnsupportedBitsPerSample(u32),
}

impl fmt::Display for WavError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            WavError::Io(error) => write!(f, "I/O error: {error}"),
            WavError::Http(error) => write!(f, "HTTP error: {error}"),
            WavError::InvalidHeader {
                field,
                expected,
                actual,
            } => write!(
                f,
                "invalid wav {field}: expected {expected:?}, found {actual:?}"
            ),
            WavError::InvalidData(message) => write!(f, "invalid wav data: {message}"),
            WavError::InvalidJsonWidth => write!(f, "json width must be greater than 0"),
            WavError::UnsupportedAudioFormat(format) => {
                write!(f, "unsupported wav audio format: {format}")
            }
            WavError::UnsupportedBitsPerSample(bits) => {
                write!(f, "unsupported bits per sample: {bits}")
            }
        }
    }
}

impl StdError for WavError {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        match self {
            WavError::Io(error) => Some(error),
            WavError::Http(error) => Some(error),
            _ => None,
        }
    }
}

impl From<io::Error> for WavError {
    fn from(error: io::Error) -> Self {
        WavError::Io(error)
    }
}

impl From<reqwest::Error> for WavError {
    fn from(error: reqwest::Error) -> Self {
        WavError::Http(error)
    }
}

/// Result type returned by this crate.
pub type WavResult<T> = Result<T, WavError>;

#[derive(Debug)]
pub struct Wav {
    chunk_descriptor: String,
    chunk_size: u64,
    wave_flag: String,
    fmt_sub_chunk: String,
    sub_chunk1_size: u64,
    audio_format: u16,
    num_channels: u32,
    sample_rate: u64,
    byte_rate: u64,
    block_align: u16,
    bits_per_sample: u32,
    sub_chunk2_size: u32,
    data: String,
    length: u32,
    reader: Option<WavReader>,
    sample_data: Vec<Vec<f64>>,
    json_width: u32,
}

impl Default for Wav {
    fn default() -> Self {
        Self {
            chunk_descriptor: String::new(),
            chunk_size: 0,
            wave_flag: String::new(),
            fmt_sub_chunk: String::new(),
            sub_chunk1_size: 0,
            audio_format: 0,
            num_channels: 0,
            sample_rate: 0,
            byte_rate: 0,
            block_align: 0,
            bits_per_sample: 0,
            sub_chunk2_size: 0,
            data: String::new(),
            length: 0,
            reader: None,
            sample_data: Vec::new(),
            json_width: DEFAULT_JSON_WIDTH,
        }
    }
}

impl Wav {
    /// Creates a wav decoder from a local path or an HTTP(S) URL.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use std::time::Instant;
    /// use wav2json::Wav;
    ///
    /// fn main() -> Result<(), Box<dyn std::error::Error>> {
    ///     // Support for local and network files.
    ///     // let mut wav = Wav::new("http://[host]/sample-15s.wav")?.set_json_width(1920);
    ///     let start = Instant::now();
    ///     let mut wav = Wav::new("examples/sample-15s.wav")?.set_json_width(1920);
    ///     let result_data = wav.decode()?;
    ///
    ///     println!("duration: {}", start.elapsed().as_millis());
    ///     println!("points: {}", result_data.len());
    ///
    ///     Ok(())
    /// }
    /// ```
    pub fn new(path: &str) -> WavResult<Self> {
        let reader = if path.starts_with("http://") || path.starts_with("https://") {
            let resp = reqwest::blocking::get(path)?.error_for_status()?;
            Some(WavReader::ResponseReader(BufReader::new(resp)))
        } else {
            let file = File::open(path)?;
            Some(WavReader::FileReader(BufReader::new(file)))
        };

        Ok(Self {
            reader,
            ..Default::default()
        })
    }

    /// Sets the number of json points generated by decoding.
    ///
    /// The recommendation is determined by the horizontal pixels of the screen or browser.
    pub fn set_json_width(mut self, width: u32) -> Self {
        self.json_width = width;

        self
    }

    /// Start decoding.
    pub fn decode(&mut self) -> WavResult<Vec<f64>> {
        if self.json_width == 0 {
            return Err(WavError::InvalidJsonWidth);
        }

        self.sample_data.clear();
        self.chunk_descriptor = self.read_string(LEN_CHUNK_DESCRIPTOR)?;
        self.expect_header("chunk descriptor", RIFF, &self.chunk_descriptor)?;
        self.chunk_size = self.read_u32()? as u64;
        self.wave_flag = self.read_string(LEN_WAVE_FLAG)?;
        self.expect_header("wave flag", WAVE, &self.wave_flag)?;

        self.read_chunks()?;
        self.read_data(self.length)?;

        let len = self.sample_data.len();
        if len == 1 {
            return Ok(self.sample_data[0].clone());
        }

        let max_len = self
            .sample_data
            .iter()
            .map(Vec::len)
            .max()
            .unwrap_or_default();
        let mut result_data = Vec::with_capacity(max_len * len);
        for i in 0..max_len {
            for channel_data in &self.sample_data {
                if let Some(value) = channel_data.get(i) {
                    result_data.push(round2(*value));
                }
            }
        }

        Ok(result_data)
    }

    fn read_chunks(&mut self) -> WavResult<()> {
        let mut found_fmt = false;

        loop {
            let chunk_id = self.read_string(LEN_CHUNK_ID)?;
            let chunk_size = self.read_u32()?;

            match chunk_id.as_str() {
                FMT_ => {
                    self.fmt_sub_chunk = chunk_id;
                    self.sub_chunk1_size = chunk_size as u64;
                    self.read_fmt_chunk(chunk_size)?;
                    found_fmt = true;
                }
                DATA => {
                    if !found_fmt {
                        return Err(WavError::InvalidData(
                            "data chunk appeared before fmt chunk".to_string(),
                        ));
                    }

                    self.data = chunk_id;
                    self.sub_chunk2_size = chunk_size;
                    self.length =
                        self.sub_chunk2_size / (self.bits_per_sample / 8) / self.num_channels;
                    return Ok(());
                }
                _ => self.skip_chunk(chunk_size)?,
            }
        }
    }

    fn read_fmt_chunk(&mut self, chunk_size: u32) -> WavResult<()> {
        if chunk_size < MIN_FMT_CHUNK_SIZE {
            return Err(WavError::InvalidData(format!(
                "fmt chunk is too small: {chunk_size}"
            )));
        }

        self.audio_format = self.read_u16()?;
        if self.audio_format != 1 {
            return Err(WavError::UnsupportedAudioFormat(self.audio_format));
        }

        self.num_channels = self.read_u16()? as u32;
        if self.num_channels == 0 {
            return Err(WavError::InvalidData(
                "channel count must be greater than 0".to_string(),
            ));
        }

        self.sample_rate = self.read_u32()? as u64;
        self.byte_rate = self.read_u32()? as u64;
        self.block_align = self.read_u16()?;
        self.bits_per_sample = self.read_u16()? as u32;

        match self.bits_per_sample {
            8 | 16 => {}
            bits => return Err(WavError::UnsupportedBitsPerSample(bits)),
        }

        self.skip_bytes((chunk_size - MIN_FMT_CHUNK_SIZE) as usize)?;
        if chunk_size % 2 == 1 {
            self.skip_bytes(1)?;
        }

        Ok(())
    }

    fn skip_chunk(&mut self, chunk_size: u32) -> WavResult<()> {
        let padded_size = chunk_size + (chunk_size % 2);
        self.skip_bytes(padded_size as usize)
    }

    /// Parsing audio data.
    fn read_data(&mut self, length: u32) -> WavResult<()> {
        if length == 0 {
            return Ok(());
        }

        let size = if length <= self.json_width {
            1
        } else {
            length / self.json_width
        };
        let channels = self.num_channels as usize;
        let mut sample_sums = vec![0i64; channels];
        let mut samples_in_bucket = 0;
        self.sample_data = vec![Vec::new(); channels];

        for i in 0..length {
            for sample_sum in &mut sample_sums {
                *sample_sum += self.read_sample_square()?;
            }

            samples_in_bucket += 1;
            if samples_in_bucket == size || i == length - 1 {
                for (channel, sample_sum) in sample_sums.iter_mut().enumerate() {
                    self.handle_bit(channel, *sample_sum, samples_in_bucket);
                    *sample_sum = 0;
                }
                samples_in_bucket = 0;
            }
        }

        Ok(())
    }

    fn read_sample_square(&mut self) -> WavResult<i64> {
        match self.bits_per_sample {
            8 => {
                let mut buf = [0u8; 1];
                self.read_exact(&mut buf)?;
                let sample = buf[0] as i16 - 128;
                Ok((sample as i64).pow(2))
            }
            16 => Ok((self.read_i16()? as i64).pow(2)),
            bits => Err(WavError::UnsupportedBitsPerSample(bits)),
        }
    }

    fn handle_bit(&mut self, key: usize, sample_sum: i64, size: u32) {
        let scope = match self.bits_per_sample {
            8 => 128.0,
            16 => 32768.0,
            _ => unreachable!("bits per sample is validated before decoding"),
        };
        let data = cal_rms(sample_sum, size, scope);
        self.sample_data[key].push(data);
    }

    fn expect_header(
        &self,
        field: &'static str,
        expected: &'static str,
        actual: &str,
    ) -> WavResult<()> {
        if actual == expected {
            Ok(())
        } else {
            Err(WavError::InvalidHeader {
                field,
                expected,
                actual: actual.to_string(),
            })
        }
    }

    fn read_string(&mut self, len: usize) -> WavResult<String> {
        let mut buf = vec![0u8; len];
        self.read_exact(&mut buf)?;
        Ok(String::from_utf8_lossy(&buf).into_owned())
    }

    fn read_u32(&mut self) -> WavResult<u32> {
        let mut buf = [0u8; 4];
        self.read_exact(&mut buf)?;
        Ok(u32::from_le_bytes(buf))
    }

    fn read_u16(&mut self) -> WavResult<u16> {
        let mut buf = [0u8; 2];
        self.read_exact(&mut buf)?;
        Ok(u16::from_le_bytes(buf))
    }

    fn read_i16(&mut self) -> WavResult<i16> {
        let mut buf = [0u8; 2];
        self.read_exact(&mut buf)?;
        Ok(i16::from_le_bytes(buf))
    }

    fn read_exact(&mut self, buf: &mut [u8]) -> WavResult<()> {
        self.reader
            .as_mut()
            .ok_or_else(|| WavError::InvalidData("missing wav reader".to_string()))?
            .read_exact(buf)?;
        Ok(())
    }

    fn skip_bytes(&mut self, mut len: usize) -> WavResult<()> {
        let mut buf = [0u8; 1024];
        while len > 0 {
            let bytes_to_read = len.min(buf.len());
            self.read_exact(&mut buf[..bytes_to_read])?;
            len -= bytes_to_read;
        }
        Ok(())
    }
}

/// rms algorithm.
fn cal_rms(sample_sum: i64, size: u32, scope: f64) -> f64 {
    (sample_sum as f64 / size as f64).sqrt() / scope
}

fn round2(value: f64) -> f64 {
    (value * 100.0).round() / 100.0
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{env, fs, path::PathBuf, process};

    #[test]
    fn decode_with_configured_width() {
        let mut wav = Wav::new("examples/sample-15s.wav")
            .unwrap()
            .set_json_width(3000);
        let result_data = wav.decode().unwrap();
        assert!(!result_data.is_empty());
    }

    #[test]
    fn decode_with_default_width() {
        let mut wav = Wav::new("examples/sample-15s.wav").unwrap();
        let result_data = wav.decode().unwrap();
        assert!(!result_data.is_empty());
    }

    #[test]
    fn zero_json_width_is_invalid() {
        let mut wav = Wav::new("examples/sample-15s.wav")
            .unwrap()
            .set_json_width(0);
        assert!(matches!(wav.decode(), Err(WavError::InvalidJsonWidth)));
    }

    #[test]
    fn eight_bit_pcm_silence_is_centered_at_zero() {
        let path = write_test_wav("eight_bit_pcm_silence_is_centered_at_zero", 1, 8, &[128]);
        let mut wav = Wav::new(path.to_str().unwrap()).unwrap();
        let result_data = wav.decode().unwrap();

        assert_eq!(result_data, vec![0.0]);

        fs::remove_file(path).unwrap();
    }

    #[test]
    fn sixteen_bit_rms_is_normalized_after_averaging() {
        let mut samples = Vec::new();
        samples.extend_from_slice(&16384i16.to_le_bytes());
        samples.extend_from_slice(&(-16384i16).to_le_bytes());

        let path = write_test_wav(
            "sixteen_bit_rms_is_normalized_after_averaging",
            1,
            16,
            &samples,
        );
        let mut wav = Wav::new(path.to_str().unwrap()).unwrap().set_json_width(1);
        let result_data = wav.decode().unwrap();

        assert_eq!(result_data.len(), 1);
        assert!((result_data[0] - 0.5).abs() < f64::EPSILON);

        fs::remove_file(path).unwrap();
    }

    fn write_test_wav(
        test_name: &str,
        channels: u16,
        bits_per_sample: u16,
        samples: &[u8],
    ) -> PathBuf {
        let mut path = env::temp_dir();
        path.push(format!("wav2json-{test_name}-{}.wav", process::id()));

        fs::write(&path, wav_bytes(channels, bits_per_sample, samples)).unwrap();
        path
    }

    fn wav_bytes(channels: u16, bits_per_sample: u16, samples: &[u8]) -> Vec<u8> {
        let sample_rate = 44_100u32;
        let byte_rate = sample_rate * channels as u32 * bits_per_sample as u32 / 8;
        let block_align = channels * bits_per_sample / 8;
        let data_len = samples.len() as u32;

        let mut bytes = Vec::new();
        bytes.extend_from_slice(b"RIFF");
        bytes.extend_from_slice(&(36 + data_len).to_le_bytes());
        bytes.extend_from_slice(b"WAVE");
        bytes.extend_from_slice(b"fmt ");
        bytes.extend_from_slice(&16u32.to_le_bytes());
        bytes.extend_from_slice(&1u16.to_le_bytes());
        bytes.extend_from_slice(&channels.to_le_bytes());
        bytes.extend_from_slice(&sample_rate.to_le_bytes());
        bytes.extend_from_slice(&byte_rate.to_le_bytes());
        bytes.extend_from_slice(&block_align.to_le_bytes());
        bytes.extend_from_slice(&bits_per_sample.to_le_bytes());
        bytes.extend_from_slice(b"data");
        bytes.extend_from_slice(&data_len.to_le_bytes());
        bytes.extend_from_slice(samples);
        bytes
    }
}
