use anyhow::{Context, Result, anyhow};
use cpal::SampleFormat;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

pub struct CaptureConfig {
    pub wav_path: Option<String>,
}

pub struct CaptureStats {
    pub callbacks: AtomicU64,
    pub samples: AtomicU64,
    pub max_callback_gap_us: AtomicU64,
}

impl CaptureStats {
    pub fn new() -> Self {
        Self {
            callbacks: AtomicU64::new(0),
            samples: AtomicU64::new(0),
            max_callback_gap_us: AtomicU64::new(0),
        }
    }
}

pub fn run<F>(config: CaptureConfig, mut on_samples: F) -> Result<Arc<CaptureStats>>
where
    F: FnMut(&[f32], u32) + Send + 'static,
{
    let host = cpal::default_host();
    let device = host
        .default_input_device()
        .ok_or_else(|| anyhow!("기본 입력 장치를 찾을 수 없음"))?;

    let device_name = device.name().unwrap_or_else(|_| "<unknown>".into());
    let supported = device
        .default_input_config()
        .context("기본 입력 설정을 가져오지 못함")?;

    let sample_rate = supported.sample_rate().0;
    let channels = supported.channels();
    let sample_format = supported.sample_format();

    println!(
        "[device] {device_name}  | {sample_rate} Hz  | {channels} ch  | {sample_format:?}"
    );

    let stream_config: cpal::StreamConfig = supported.clone().into();
    let stats = Arc::new(CaptureStats::new());

    let writer = if let Some(path) = config.wav_path.as_ref() {
        let spec = hound::WavSpec {
            channels,
            sample_rate,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let w = hound::WavWriter::create(path, spec)
            .with_context(|| format!("wav 파일 생성 실패: {path}"))?;
        println!("[wav] writing to {path}");
        Some(Arc::new(Mutex::new(w)))
    } else {
        None
    };

    let stats_for_cb = stats.clone();
    let writer_for_cb = writer.clone();
    let last_callback: Arc<Mutex<Option<Instant>>> = Arc::new(Mutex::new(None));

    let err_fn = |err| eprintln!("[stream error] {err}");

    let stream = match sample_format {
        SampleFormat::F32 => device.build_input_stream(
            &stream_config,
            move |data: &[f32], _info: &cpal::InputCallbackInfo| {
                handle_callback(
                    data,
                    channels,
                    sample_rate,
                    &stats_for_cb,
                    &last_callback,
                    writer_for_cb.as_ref(),
                    &mut on_samples,
                );
            },
            err_fn,
            None,
        )?,
        SampleFormat::I16 => {
            let mut buf = Vec::<f32>::new();
            device.build_input_stream(
                &stream_config,
                move |data: &[i16], _info: &cpal::InputCallbackInfo| {
                    buf.clear();
                    buf.extend(data.iter().map(|s| *s as f32 / i16::MAX as f32));
                    handle_callback(
                        &buf,
                        channels,
                        sample_rate,
                        &stats_for_cb,
                        &last_callback,
                        writer_for_cb.as_ref(),
                        &mut on_samples,
                    );
                },
                err_fn,
                None,
            )?
        }
        SampleFormat::U16 => {
            let mut buf = Vec::<f32>::new();
            device.build_input_stream(
                &stream_config,
                move |data: &[u16], _info: &cpal::InputCallbackInfo| {
                    buf.clear();
                    buf.extend(data.iter().map(|s| {
                        (*s as f32 - i16::MAX as f32) / i16::MAX as f32
                    }));
                    handle_callback(
                        &buf,
                        channels,
                        sample_rate,
                        &stats_for_cb,
                        &last_callback,
                        writer_for_cb.as_ref(),
                        &mut on_samples,
                    );
                },
                err_fn,
                None,
            )?
        }
        other => return Err(anyhow!("지원하지 않는 sample format: {other:?}")),
    };

    stream.play().context("입력 스트림 시작 실패")?;
    println!("[stream] started — Ctrl+C 로 종료");

    std::mem::forget(stream);

    Ok(stats)
}

fn handle_callback<F>(
    samples: &[f32],
    channels: u16,
    sample_rate: u32,
    stats: &Arc<CaptureStats>,
    last_callback: &Arc<Mutex<Option<Instant>>>,
    writer: Option<&Arc<Mutex<hound::WavWriter<std::io::BufWriter<std::fs::File>>>>>,
    on_samples: &mut F,
) where
    F: FnMut(&[f32], u32) + Send,
{
    let now = Instant::now();
    {
        let mut guard = last_callback.lock().unwrap();
        if let Some(prev) = *guard {
            let gap = now.duration_since(prev).as_micros() as u64;
            let cur_max = stats.max_callback_gap_us.load(Ordering::Relaxed);
            if gap > cur_max {
                stats.max_callback_gap_us.store(gap, Ordering::Relaxed);
            }
        }
        *guard = Some(now);
    }

    stats.callbacks.fetch_add(1, Ordering::Relaxed);
    stats.samples.fetch_add(samples.len() as u64, Ordering::Relaxed);

    let mono = downmix_to_mono(samples, channels);
    on_samples(&mono, sample_rate);

    if let Some(w) = writer {
        if let Ok(mut guard) = w.lock() {
            for s in samples {
                let clamped = s.clamp(-1.0, 1.0);
                let v = (clamped * i16::MAX as f32) as i16;
                let _ = guard.write_sample(v);
            }
        }
    }
}

fn downmix_to_mono(samples: &[f32], channels: u16) -> Vec<f32> {
    if channels <= 1 {
        return samples.to_vec();
    }
    let ch = channels as usize;
    let frames = samples.len() / ch;
    let mut out = Vec::with_capacity(frames);
    for i in 0..frames {
        let mut acc = 0.0f32;
        for c in 0..ch {
            acc += samples[i * ch + c];
        }
        out.push(acc / ch as f32);
    }
    out
}
