use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::SyncSender;
use std::sync::Arc;
use std::time::Duration;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::SampleFormat;

#[derive(Debug, thiserror::Error)]
pub enum CaptureError {
    #[error("no default input device available")]
    NoInputDevice,
    #[error("failed to read default input config: {0}")]
    DefaultConfig(#[from] cpal::DefaultStreamConfigError),
    #[error("failed to build input stream: {0}")]
    BuildStream(#[from] cpal::BuildStreamError),
    #[error("failed to start input stream: {0}")]
    PlayStream(#[from] cpal::PlayStreamError),
    #[error("unsupported sample format: {0:?}")]
    UnsupportedFormat(SampleFormat),
}

/// One microphone callback worth of mono samples.
pub struct AudioChunk {
    pub samples: Vec<f32>,
    pub sample_rate: u32,
}

/// Open the default input device and stream mono chunks into `tx` until
/// `stop` is set. Blocks the calling thread for the lifetime of the stream
/// (cpal streams are not `Send`, so the stream must live on one thread).
///
/// The audio callback only downmixes and hands off through the channel;
/// `try_send` drops chunks instead of blocking if the consumer stalls.
pub fn run_capture(tx: SyncSender<AudioChunk>, stop: Arc<AtomicBool>) -> Result<(), CaptureError> {
    let host = cpal::default_host();
    let device = host
        .default_input_device()
        .ok_or(CaptureError::NoInputDevice)?;

    let device_name = device.name().unwrap_or_else(|_| "<unknown>".into());
    let supported = device.default_input_config()?;

    let sample_rate = supported.sample_rate().0;
    let channels = supported.channels();
    let sample_format = supported.sample_format();
    println!("[audio] device={device_name} rate={sample_rate} channels={channels} format={sample_format:?}");

    let stream_config: cpal::StreamConfig = supported.into();
    let err_fn = |err| eprintln!("[audio] stream error: {err}");

    let stream = match sample_format {
        SampleFormat::F32 => {
            let tx = tx.clone();
            device.build_input_stream(
                &stream_config,
                move |data: &[f32], _info: &cpal::InputCallbackInfo| {
                    forward_chunk(data, channels, sample_rate, &tx);
                },
                err_fn,
                None,
            )?
        }
        SampleFormat::I16 => {
            let tx = tx.clone();
            let mut scratch = Vec::<f32>::new();
            device.build_input_stream(
                &stream_config,
                move |data: &[i16], _info: &cpal::InputCallbackInfo| {
                    scratch.clear();
                    scratch.extend(data.iter().map(|s| *s as f32 / i16::MAX as f32));
                    forward_chunk(&scratch, channels, sample_rate, &tx);
                },
                err_fn,
                None,
            )?
        }
        SampleFormat::U16 => {
            let tx = tx.clone();
            let mut scratch = Vec::<f32>::new();
            device.build_input_stream(
                &stream_config,
                move |data: &[u16], _info: &cpal::InputCallbackInfo| {
                    scratch.clear();
                    scratch.extend(
                        data.iter()
                            .map(|s| (*s as f32 - i16::MAX as f32) / i16::MAX as f32),
                    );
                    forward_chunk(&scratch, channels, sample_rate, &tx);
                },
                err_fn,
                None,
            )?
        }
        other => return Err(CaptureError::UnsupportedFormat(other)),
    };

    stream.play()?;
    println!("[audio] capture started");

    while !stop.load(Ordering::Relaxed) {
        std::thread::sleep(Duration::from_millis(100));
    }

    drop(stream);
    println!("[audio] capture stopped");
    Ok(())
}

fn forward_chunk(samples: &[f32], channels: u16, sample_rate: u32, tx: &SyncSender<AudioChunk>) {
    let mono = downmix_to_mono(samples, channels);
    // Drop the chunk if the detection thread is behind; the audio callback
    // must never block.
    let _ = tx.try_send(AudioChunk {
        samples: mono,
        sample_rate,
    });
}

fn downmix_to_mono(samples: &[f32], channels: u16) -> Vec<f32> {
    if channels <= 1 {
        return samples.to_vec();
    }
    let ch = channels as usize;
    samples
        .chunks_exact(ch)
        .map(|frame| frame.iter().sum::<f32>() / ch as f32)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn downmix_mono_passthrough() {
        let samples = vec![0.1, 0.2, 0.3];
        assert_eq!(downmix_to_mono(&samples, 1), samples);
    }

    #[test]
    fn downmix_stereo_averages_pairs() {
        let samples = vec![0.2, 0.4, -0.2, -0.4];
        let mono = downmix_to_mono(&samples, 2);
        assert_eq!(mono.len(), 2);
        assert!((mono[0] - 0.3).abs() < 1e-6);
        assert!((mono[1] - (-0.3)).abs() < 1e-6);
    }
}
