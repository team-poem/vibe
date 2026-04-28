mod capture;
mod rms;

use anyhow::Result;
use capture::{CaptureConfig, run};
use rms::RmsMeter;
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let wav_path = args
        .iter()
        .skip(1)
        .find(|a| !a.starts_with("--"))
        .cloned();

    let config = CaptureConfig { wav_path };

    let meter: Arc<Mutex<Option<RmsMeter>>> = Arc::new(Mutex::new(None));
    let meter_for_cb = meter.clone();

    let stats = run(config, move |samples, sample_rate| {
        let mut guard = meter_for_cb.lock().unwrap();
        let m = guard.get_or_insert_with(|| RmsMeter::new(sample_rate, 10.0, 200));
        m.push(samples);
    })?;

    let started = Instant::now();
    loop {
        std::thread::sleep(Duration::from_secs(5));
        let cb = stats.callbacks.load(Ordering::Relaxed);
        let samples = stats.samples.load(Ordering::Relaxed);
        let max_gap_us = stats.max_callback_gap_us.load(Ordering::Relaxed);
        let elapsed = started.elapsed().as_secs_f64();
        println!(
            "[stats] elapsed {:.1}s  callbacks {}  samples {}  max_gap {:.2} ms",
            elapsed,
            cb,
            samples,
            max_gap_us as f64 / 1000.0
        );
    }
}
