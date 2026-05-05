use std::path::Path;

#[derive(Debug, thiserror::Error)]
pub enum WavError {
    #[error("failed to open wav file")]
    Open(#[from] hound::Error),

    #[error("unsupported wav format: {0}")]
    Unsupported(String),
}

pub struct WavData {
    pub samples: Vec<f32>,
    pub sample_rate: u32,
}

pub fn load_mono<P: AsRef<Path>>(path: P) -> Result<WavData, WavError> {
    let mut reader = hound::WavReader::open(path)?;
    let spec = reader.spec();
    let sample_rate = spec.sample_rate;
    let channels = spec.channels as usize;

    let mono = match (spec.sample_format, spec.bits_per_sample) {
        (hound::SampleFormat::Int, 16) => read_i16_mono(&mut reader, channels)?,
        (hound::SampleFormat::Int, 24) => read_int_mono(&mut reader, channels, 1 << 23)?,
        (hound::SampleFormat::Int, 32) => read_int_mono(&mut reader, channels, 1 << 31)?,
        (hound::SampleFormat::Float, 32) => read_f32_mono(&mut reader, channels)?,
        (fmt, bits) => {
            return Err(WavError::Unsupported(format!(
                "format {fmt:?} with {bits} bits/sample"
            )));
        }
    };

    Ok(WavData {
        samples: mono,
        sample_rate,
    })
}

fn read_i16_mono(
    reader: &mut hound::WavReader<std::io::BufReader<std::fs::File>>,
    channels: usize,
) -> Result<Vec<f32>, WavError> {
    let raw: Result<Vec<i16>, _> = reader.samples::<i16>().collect();
    let raw = raw?;
    Ok(downmix(&raw, channels, |s| s as f32 / i16::MAX as f32))
}

fn read_int_mono(
    reader: &mut hound::WavReader<std::io::BufReader<std::fs::File>>,
    channels: usize,
    scale: i32,
) -> Result<Vec<f32>, WavError> {
    let raw: Result<Vec<i32>, _> = reader.samples::<i32>().collect();
    let raw = raw?;
    let scale_f = scale as f32;
    Ok(downmix(&raw, channels, |s| s as f32 / scale_f))
}

fn read_f32_mono(
    reader: &mut hound::WavReader<std::io::BufReader<std::fs::File>>,
    channels: usize,
) -> Result<Vec<f32>, WavError> {
    let raw: Result<Vec<f32>, _> = reader.samples::<f32>().collect();
    let raw = raw?;
    Ok(downmix(&raw, channels, |s| s))
}

fn downmix<T, F>(samples: &[T], channels: usize, to_f32: F) -> Vec<f32>
where
    T: Copy,
    F: Fn(T) -> f32,
{
    if channels <= 1 {
        return samples.iter().map(|s| to_f32(*s)).collect();
    }
    samples
        .chunks_exact(channels)
        .map(|frame| {
            let sum: f32 = frame.iter().map(|s| to_f32(*s)).sum();
            sum / channels as f32
        })
        .collect()
}
