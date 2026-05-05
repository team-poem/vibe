use rustfft::{FftPlanner, num_complex::Complex};

pub fn rms_db(samples: &[f32]) -> f32 {
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

pub struct FlatnessAnalyzer {
    fft: std::sync::Arc<dyn rustfft::Fft<f32>>,
    fft_size: usize,
    scratch: Vec<Complex<f32>>,
    buffer: Vec<Complex<f32>>,
}

impl FlatnessAnalyzer {
    pub fn new(fft_size: usize) -> Self {
        let mut planner = FftPlanner::<f32>::new();
        let fft = planner.plan_fft_forward(fft_size);
        let scratch_len = fft.get_inplace_scratch_len();
        Self {
            fft,
            fft_size,
            scratch: vec![Complex { re: 0.0, im: 0.0 }; scratch_len],
            buffer: vec![Complex { re: 0.0, im: 0.0 }; fft_size],
        }
    }

    pub fn flatness(&mut self, frame: &[f32]) -> f32 {
        for slot in self.buffer.iter_mut() {
            *slot = Complex { re: 0.0, im: 0.0 };
        }
        let copy_len = frame.len().min(self.fft_size);
        for (slot, &sample) in self.buffer[..copy_len].iter_mut().zip(frame.iter()) {
            *slot = Complex {
                re: sample,
                im: 0.0,
            };
        }
        apply_hann(&mut self.buffer[..copy_len]);
        self.fft
            .process_with_scratch(&mut self.buffer, &mut self.scratch);

        spectral_flatness(&self.buffer[..self.fft_size / 2])
    }
}

fn apply_hann(buf: &mut [Complex<f32>]) {
    let n = buf.len() as f32;
    for (i, slot) in buf.iter_mut().enumerate() {
        let w = 0.5 - 0.5 * (std::f32::consts::TAU * i as f32 / (n - 1.0).max(1.0)).cos();
        slot.re *= w;
        slot.im *= w;
    }
}

fn spectral_flatness(spectrum: &[Complex<f32>]) -> f32 {
    if spectrum.is_empty() {
        return 0.0;
    }
    let magnitudes: Vec<f32> = spectrum
        .iter()
        .map(|c| (c.re * c.re + c.im * c.im).sqrt().max(1e-12))
        .collect();

    let n = magnitudes.len() as f32;
    let log_sum: f32 = magnitudes.iter().map(|m| m.ln()).sum();
    let geometric_mean = (log_sum / n).exp();
    let arithmetic_mean: f32 = magnitudes.iter().sum::<f32>() / n;

    if arithmetic_mean <= 0.0 {
        0.0
    } else {
        (geometric_mean / arithmetic_mean).clamp(0.0, 1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rms_db_silence_is_negative_infinity() {
        let samples = vec![0.0f32; 100];
        assert!(rms_db(&samples).is_infinite());
    }

    #[test]
    fn rms_db_full_scale_is_zero() {
        let samples = vec![1.0f32; 100];
        let db = rms_db(&samples);
        assert!((db - 0.0).abs() < 0.01);
    }

    #[test]
    fn flatness_of_white_noise_is_high() {
        let mut analyzer = FlatnessAnalyzer::new(512);
        let mut x: u32 = 0x1234_5678;
        let frame: Vec<f32> = (0..512)
            .map(|_| {
                x = x.wrapping_mul(1_103_515_245).wrapping_add(12_345);
                (((x >> 16) & 0x7fff) as f32 / 32_768.0) * 2.0 - 1.0
            })
            .collect();
        let flat = analyzer.flatness(&frame);
        assert!(
            flat > 0.3,
            "white noise flatness should be high, got {flat}"
        );
    }

    #[test]
    fn flatness_of_sine_is_low() {
        let mut analyzer = FlatnessAnalyzer::new(512);
        let frame: Vec<f32> = (0..512)
            .map(|i| (std::f32::consts::TAU * 50.0 * i as f32 / 512.0).sin())
            .collect();
        let flat = analyzer.flatness(&frame);
        assert!(flat < 0.1, "single sine flatness should be low, got {flat}");
    }
}
