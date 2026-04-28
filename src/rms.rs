use std::time::{Duration, Instant};

pub struct RmsMeter {
    frame_size: usize,
    accumulator: Vec<f32>,
    last_log: Instant,
    log_interval: Duration,
    peak_db_window: f32,
    noise_floor_db: f32,
    event_threshold_db: f32,
    heartbeat_interval: Duration,
    last_heartbeat: Instant,
}

impl RmsMeter {
    pub fn new(sample_rate: u32, frame_ms: f32, log_interval_ms: u64) -> Self {
        let frame_size = ((sample_rate as f32) * (frame_ms / 1000.0)).round() as usize;
        let now = Instant::now();
        Self {
            frame_size: frame_size.max(1),
            accumulator: Vec::with_capacity(frame_size.max(1) * 2),
            last_log: now,
            log_interval: Duration::from_millis(log_interval_ms),
            peak_db_window: f32::NEG_INFINITY,
            noise_floor_db: -60.0,
            event_threshold_db: 8.0,
            heartbeat_interval: Duration::from_secs(10),
            last_heartbeat: now,
        }
    }

    pub fn push(&mut self, samples: &[f32]) {
        self.accumulator.extend_from_slice(samples);

        while self.accumulator.len() >= self.frame_size {
            let frame: Vec<f32> = self.accumulator.drain(..self.frame_size).collect();
            let db = rms_db(&frame);
            if db > self.peak_db_window {
                self.peak_db_window = db;
            }
        }

        if self.last_log.elapsed() >= self.log_interval {
            let peak = self.peak_db_window;
            let above_floor = peak - self.noise_floor_db;
            let is_event = peak.is_finite() && above_floor >= self.event_threshold_db;

            if is_event {
                let bar = render_bar(peak);
                println!(
                    "[event] peak {:>6.1} dBFS  (+{:>4.1} dB)  {}",
                    peak, above_floor, bar
                );
                self.last_heartbeat = Instant::now();
            } else if self.last_heartbeat.elapsed() >= self.heartbeat_interval {
                println!(
                    "[idle]  floor ~{:>6.1} dBFS",
                    self.noise_floor_db
                );
                self.last_heartbeat = Instant::now();
            }

            if !is_event && peak.is_finite() {
                self.noise_floor_db = self.noise_floor_db * 0.95 + peak * 0.05;
            }

            self.peak_db_window = f32::NEG_INFINITY;
            self.last_log = Instant::now();
        }
    }
}

fn rms_db(samples: &[f32]) -> f32 {
    if samples.is_empty() {
        return f32::NEG_INFINITY;
    }
    let sum_sq: f32 = samples.iter().map(|s| s * s).sum();
    let mean = sum_sq / samples.len() as f32;
    let rms = mean.sqrt();
    if rms <= 1e-9 {
        f32::NEG_INFINITY
    } else {
        20.0 * rms.log10()
    }
}

fn render_bar(db: f32) -> String {
    if !db.is_finite() {
        return String::from("·");
    }
    let clamped = db.clamp(-60.0, 0.0);
    let normalized = (clamped + 60.0) / 60.0;
    let width = 30usize;
    let filled = (normalized * width as f32).round() as usize;
    let mut s = String::with_capacity(width);
    for _ in 0..filled {
        s.push('#');
    }
    for _ in filled..width {
        s.push('·');
    }
    s
}
