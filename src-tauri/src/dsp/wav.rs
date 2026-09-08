use std::path::Path;

use crate::error::{AppError, AppResult};

pub const WAV_BITS: u16 = 32;

pub fn write_stereo_wav(path: &Path, left: &[f32], right: &[f32], sample_rate: u32) -> AppResult<()> {
    let n = left.len().min(right.len());
    let spec = hound::WavSpec {
        channels: 2,
        sample_rate,
        bits_per_sample: WAV_BITS,
        sample_format: hound::SampleFormat::Float,
    };
    let mut writer = hound::WavWriter::create(path, spec)
        .map_err(|e| AppError::msg(format!("Cannot create wav: {e}")))?;
    for i in 0..n {
        writer
            .write_sample(left[i])
            .map_err(|e| AppError::msg(format!("Write wav failed: {e}")))?;
        writer
            .write_sample(right[i])
            .map_err(|e| AppError::msg(format!("Write wav failed: {e}")))?;
    }
    writer
        .finalize()
        .map_err(|e| AppError::msg(format!("Finalize wav failed: {e}")))?;
    Ok(())
}

pub fn peak_normalize(left: &mut [f32], right: &mut [f32], ceiling: f32) {
    let peak = left
        .iter()
        .chain(right.iter())
        .fold(0.0f32, |acc, s| acc.max(s.abs()));
    if peak <= 1e-8 {
        return;
    }
    let gain = ceiling / peak;
    if (gain - 1.0).abs() < 0.01 {
        return;
    }
    for s in left.iter_mut() {
        *s *= gain;
    }
    for s in right.iter_mut() {
        *s *= gain;
    }
}

pub fn compute_peaks(left: &[f32], right: &[f32], bars: usize) -> Vec<f32> {
    let n = left.len().min(right.len());
    if n == 0 || bars == 0 {
        return vec![0.12; bars.max(1)];
    }
    let size = (n / bars).max(1);
    (0..bars)
        .map(|i| {
            let start = i * size;
            let end = (start + size).min(n);
            let mut peak = 0.0f32;
            for idx in start..end {
                peak = peak.max(left[idx].abs()).max(right[idx].abs());
            }
            peak.clamp(0.0, 1.0)
        })
        .collect()
}

pub fn write_peaks(path: &Path, left: &[f32], right: &[f32], bars: usize) -> AppResult<()> {
    let peaks = compute_peaks(left, right, bars);
    let json = serde_json::to_string(&peaks)?;
    std::fs::write(path, json)?;
    Ok(())
}

pub fn read_peaks(path: &Path) -> Option<Vec<f32>> {
    let json = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&json).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn peaks_use_true_max() {
        let mut left = vec![0.0f32; 100];
        let mut right = vec![0.0f32; 100];
        left[10] = 0.8;
        right[60] = 0.4;
        let peaks = compute_peaks(&left, &right, 2);
        assert!((peaks[0] - 0.8).abs() < 1e-4);
        assert!((peaks[1] - 0.4).abs() < 1e-4);
    }

    #[test]
    fn writes_float_wav() {
        let dir = std::env::temp_dir().join(format!("dualis-wav-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("test.wav");
        write_stereo_wav(&path, &[0.1, -0.2], &[0.0, 0.5], 44100).unwrap();
        let reader = hound::WavReader::open(&path).unwrap();
        assert_eq!(reader.spec().bits_per_sample, 32);
        assert_eq!(reader.spec().sample_format, hound::SampleFormat::Float);
        let _ = std::fs::remove_dir_all(dir);
    }
}
