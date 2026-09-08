use crate::error::{AppError, AppResult};
use num_complex::Complex32;
use realfft::{ComplexToReal, RealFftPlanner, RealToComplex};
use std::f32::consts::PI;
use std::sync::Arc;

/// Matches torch.stft used by UVR MDX-Net: Hann (periodic), center=True, onesided.
pub struct StftEngine {
    n_fft: usize,
    hop: usize,
    window: Vec<f32>,
    forward: Arc<dyn RealToComplex<f32>>,
    inverse: Arc<dyn ComplexToReal<f32>>,
}

impl StftEngine {
    pub fn new(n_fft: usize, hop: usize) -> Self {
        let mut planner = RealFftPlanner::<f32>::new();
        let forward = planner.plan_fft_forward(n_fft);
        let inverse = planner.plan_fft_inverse(n_fft);
        Self {
            n_fft,
            hop,
            window: hann_periodic(n_fft),
            forward,
            inverse,
        }
    }

    pub fn n_bins(&self) -> usize {
        self.n_fft / 2 + 1
    }

    pub fn stft(&self, samples: &[f32]) -> AppResult<Vec<Vec<Complex32>>> {
        let pad = self.n_fft / 2;
        let padded = reflect_pad(samples, pad);
        let n_frames = if padded.len() < self.n_fft {
            0
        } else {
            1 + (padded.len() - self.n_fft) / self.hop
        };
        let n_bins = self.n_bins();
        let mut frames = Vec::with_capacity(n_frames);
        let mut scratch = self.forward.make_scratch_vec();
        for i in 0..n_frames {
            let start = i * self.hop;
            let mut input = self.forward.make_input_vec();
            for (n, slot) in input.iter_mut().enumerate() {
                *slot = padded[start + n] * self.window[n];
            }
            let mut spectrum = self.forward.make_output_vec();
            self.forward
                .process_with_scratch(&mut input, &mut spectrum, &mut scratch)
                .map_err(|e| AppError::msg(format!("STFT failed: {e}")))?;
            spectrum.truncate(n_bins);
            frames.push(spectrum);
        }
        Ok(frames)
    }

    pub fn istft(&self, frames: &[Vec<Complex32>], out_len: usize) -> AppResult<Vec<f32>> {
        let pad = self.n_fft / 2;
        let padded_len = out_len + 2 * pad;
        let mut acc = vec![0.0f32; padded_len.max(self.n_fft)];
        let mut window_sum = vec![0.0f32; acc.len()];
        let mut scratch = self.inverse.make_scratch_vec();
        let n_bins = self.n_bins();
        let scale = 1.0 / self.n_fft as f32;

        for (i, frame) in frames.iter().enumerate() {
            let mut spectrum = self.inverse.make_input_vec();
            for (bin, value) in frame.iter().take(n_bins).enumerate() {
                spectrum[bin] = *value;
            }
            sanitize_onesided_spectrum(&mut spectrum);
            let mut time = self.inverse.make_output_vec();
            self.inverse
                .process_with_scratch(&mut spectrum, &mut time, &mut scratch)
                .map_err(|e| AppError::msg(format!("iSTFT failed: {e}")))?;
            let start = i * self.hop;
            for (n, sample) in time.iter().enumerate() {
                let idx = start + n;
                if idx >= acc.len() {
                    break;
                }
                let w = self.window[n];
                acc[idx] += sample * w * scale;
                window_sum[idx] += w * w;
            }
        }

        for (sample, norm) in acc.iter_mut().zip(window_sum.iter()) {
            if *norm > 1e-8 {
                *sample /= *norm;
            }
        }

        let end = (pad + out_len).min(acc.len());
        Ok(acc[pad..end].to_vec())
    }
}

fn sanitize_onesided_spectrum(spectrum: &mut [Complex32]) {
    if spectrum.is_empty() {
        return;
    }
    // Real iFFT requires DC and Nyquist bins to be purely real.
    spectrum[0].im = 0.0;
    if spectrum.len() > 1 {
        let nyq = spectrum.len() - 1;
        spectrum[nyq].im = 0.0;
    }
}

fn hann_periodic(n: usize) -> Vec<f32> {
    (0..n)
        .map(|i| 0.5 - 0.5 * (2.0 * PI * i as f32 / n as f32).cos())
        .collect()
}

fn reflect_pad(samples: &[f32], pad: usize) -> Vec<f32> {
    if samples.is_empty() {
        return vec![0.0; pad * 2];
    }
    let mut out = Vec::with_capacity(samples.len() + pad * 2);
    for i in 0..pad {
        let src = reflect_index(i, samples.len(), true);
        out.push(samples[src]);
    }
    out.extend_from_slice(samples);
    for i in 0..pad {
        let src = reflect_index(i, samples.len(), false);
        out.push(samples[src]);
    }
    out
}

fn reflect_index(i: usize, len: usize, front: bool) -> usize {
    if len == 1 {
        return 0;
    }
    if front {
        (i + 1) % (len - 1)
    } else {
        len - 2 - (i % (len - 1))
    }
}

pub fn overlap_add(dst: &mut [f32], src: &[f32], offset: usize, fade: usize) {
    for (i, sample) in src.iter().enumerate() {
        let idx = offset + i;
        if idx >= dst.len() {
            break;
        }
        if fade > 0 && i < fade && offset > 0 {
            let t = i as f32 / fade as f32;
            let prev = dst[idx];
            dst[idx] = prev * (1.0 - t) + sample * t;
        } else {
            dst[idx] = *sample;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn istft_tolerates_nonzero_dc_imag_from_model_output() {
        let engine = StftEngine::new(2048, 512);
        let samples: Vec<f32> = (0..4096).map(|i| (i as f32 * 0.01).sin()).collect();
        let frames = engine.stft(&samples).unwrap();
        let mut distorted = frames.clone();
        for frame in distorted.iter_mut() {
            frame[0] = Complex32::new(frame[0].re, 0.5);
            if frame.len() > 1 {
                let last = frame.len() - 1;
                frame[last] = Complex32::new(frame[last].re, -0.25);
            }
        }
        assert!(engine.istft(&distorted, samples.len()).is_ok());
    }

    #[test]
    fn stft_istft_roundtrip_is_finite() {
        let engine = StftEngine::new(6144, 1024);
        let samples: Vec<f32> = (0..8000).map(|i| (i as f32 * 0.003).sin()).collect();
        let frames = engine.stft(&samples).unwrap();
        let rebuilt = engine.istft(&frames, samples.len()).unwrap();
        assert_eq!(rebuilt.len(), samples.len());
        assert!(rebuilt.iter().all(|v| v.is_finite()));
    }
}
