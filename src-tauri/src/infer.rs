use std::path::Path;
use std::time::Instant;

use crate::dsp::decode::{self, DecodedAudio};
use crate::dsp::stft::{overlap_add, StftEngine};
use crate::dsp::wav::{peak_normalize, write_peaks, write_stereo_wav};
use crate::error::{AppError, AppResult};
use crate::models::{Architecture, ModelConfig, ModelSpec, WaveLayout};
use crate::paths::AppPaths;
use crate::tools;
use num_complex::Complex32;
use ort::session::builder::GraphOptimizationLevel;
use ort::session::Session;
use ort::value::{Tensor, ValueType};
use serde::Serialize;

pub struct LoadedModel {
    pub session: Session,
    pub ep_name: String,
    pub config: ModelConfig,
    pub architecture: Architecture,
    pub input_name: String,
    pub model_id: String,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SeparationResult {
    pub vocals_path: String,
    pub instrumental_path: String,
    pub peaks_path: String,
    pub duration_ms: i64,
    pub sample_rate: u32,
    pub execution_provider: String,
    pub bits_per_sample: u16,
}

pub fn load_model(paths: &AppPaths, spec: &ModelSpec, prefer_gpu: bool) -> AppResult<LoadedModel> {
    let model = tools::model_file(paths, spec);
    if !model.is_file() {
        return Err(AppError::msg(format!("{} is not installed yet", spec.name)));
    }
    if prefer_gpu {
        match try_load(&model, spec, true) {
            Ok(loaded) => return Ok(loaded),
            Err(err) => {
                eprintln!("GPU execution provider failed, falling back to CPU: {err}");
            }
        }
    }
    try_load(&model, spec, false)
}

fn map_ort<T, E: std::fmt::Display>(result: Result<T, E>) -> AppResult<T> {
    result.map_err(|e| AppError::msg(e.to_string()))
}

fn try_load(model: &Path, spec: &ModelSpec, gpu: bool) -> AppResult<LoadedModel> {
    let opt_level = if gpu {
        // WebGPU/DirectML are more stable with fewer graph fusions.
        GraphOptimizationLevel::Level1
    } else {
        GraphOptimizationLevel::Level3
    };
    let mut builder = map_ort(
        Session::builder()?
            .with_optimization_level(opt_level),
    )?;
    builder = map_ort(builder.with_intra_threads(num_cpus::get().clamp(1, 8)))?;

    let mut ep_name = String::from("CPU");
    if gpu {
        let providers = gpu_providers();
        if !providers.is_empty() {
            builder = map_ort(builder.with_execution_providers(providers))?;
            ep_name = active_ep_label();
        }
    }

    let session = map_ort(builder.commit_from_file(model))?;
    let input_name = session
        .inputs()
        .first()
        .map(|input| input.name().to_string())
        .unwrap_or_else(|| "input".into());
    let (architecture, config) = apply_input_shape(&session, spec);
    Ok(LoadedModel {
        session,
        ep_name,
        config,
        architecture,
        input_name,
        model_id: spec.id.into(),
    })
}

pub fn apply_input_shape(session: &Session, spec: &ModelSpec) -> (Architecture, ModelConfig) {
    let mut cfg = spec.config.clone();
    let mut arch = spec.architecture;
    let Some(input) = session.inputs().first() else {
        return (arch, cfg);
    };
    let ValueType::Tensor { shape, .. } = input.dtype() else {
        return (arch, cfg);
    };
    let dims: Vec<i64> = shape.iter().copied().collect();
    match dims.as_slice() {
        [_, 4, f, t] if *f > 8 && *t > 8 => {
            arch = Architecture::Mdx;
            if *f > 0 {
                cfg.dim_f = *f as usize;
            }
            if *t > 0 {
                cfg.dim_t = *t as usize;
            }
            if spec.config.n_fft >= cfg.dim_f * 2 {
                cfg.n_fft = spec.config.n_fft;
            } else {
                cfg.n_fft = cfg.dim_f * 2;
            }
        }
        [_, 2, n] if *n > 64 => {
            arch = Architecture::Roformer;
            cfg.waveform_len = *n as usize;
            cfg.layout = WaveLayout::ChannelFirst;
        }
        [_, n, 2] if *n > 64 => {
            arch = Architecture::Roformer;
            cfg.waveform_len = *n as usize;
            cfg.layout = WaveLayout::ChannelLast;
        }
        _ => {}
    }
    (arch, cfg)
}

fn gpu_providers() -> Vec<ort::ep::ExecutionProviderDispatch> {
    let mut providers = Vec::new();
    #[cfg(windows)]
    {
        providers.push(ort::ep::DirectML::default().build());
    }
    #[cfg(feature = "cuda")]
    {
        providers.push(ort::ep::CUDA::default().build());
    }
    #[cfg(feature = "rocm")]
    {
        providers.push(ort::ep::ROCm::default().build());
    }
    #[cfg(feature = "openvino")]
    {
        providers.push(ort::ep::OpenVINO::default().build());
    }
    #[cfg(target_os = "linux")]
    {
        providers.push(ort::ep::WebGPU::default().build());
    }
    providers
}

fn active_ep_label() -> String {
    #[cfg(feature = "cuda")]
    {
        return "CUDA".into();
    }
    #[cfg(feature = "rocm")]
    {
        return "ROCm".into();
    }
    #[cfg(feature = "openvino")]
    {
        return "OpenVINO".into();
    }
    #[cfg(windows)]
    {
        return "DirectML".into();
    }
    #[cfg(target_os = "linux")]
    {
        return "WebGPU".into();
    }
    #[allow(unreachable_code)]
    "GPU".into()
}

pub fn is_oom(err: &AppError) -> bool {
    should_fallback_to_cpu(err)
        && err.to_string().to_lowercase().contains("memory")
}

/// True when GPU/WebGPU inference failed and CPU should be retried.
pub fn should_fallback_to_cpu(err: &AppError) -> bool {
    let text = err.to_string().to_lowercase();
    [
        "out of memory",
        "oom",
        "cuda_error_memory",
        "failed to allocate",
        "cublas",
        "vkallocate",
        "dxgi_error",
        "enomem",
        "resource exhausted",
        "d3d12",
        "non-zero status code",
        "running conv node",
        "running gemm node",
        "raw_hash_map",
        "webgpu",
        "webgpu_buffer",
        "sequential_executor",
        "executekernel",
        "all inputs must be tensors",
    ]
    .iter()
    .any(|needle| text.contains(needle))
}

pub fn cpu_fallback_message(err: &AppError) -> String {
    if is_oom(err) {
        "GPU memory exhausted, retrying on CPU".into()
    } else {
        "GPU inference failed, retrying on CPU".into()
    }
}

pub fn separate_file(
    model: &mut LoadedModel,
    source: &Path,
    dest_dir: &Path,
    mut on_progress: impl FnMut(f32, String, Option<f32>),
) -> AppResult<SeparationResult> {
    on_progress(0.02, "Decoding audio".into(), None);
    let decoded = decode::decode_path(source)?;
    match model.architecture {
        Architecture::Roformer => run_roformer(model, &decoded, dest_dir, &mut on_progress),
        Architecture::Mdx => run_mdx(model, &decoded, dest_dir, &mut on_progress),
    }
}

fn run_mdx(
    model: &mut LoadedModel,
    decoded: &DecodedAudio,
    dest_dir: &Path,
    on_progress: &mut impl FnMut(f32, String, Option<f32>),
) -> AppResult<SeparationResult> {
    let cfg = model.config.clone();
    let engine = StftEngine::new(cfg.n_fft, cfg.hop);
    let chunk = cfg.chunk_size();
    let step = ((chunk as f32) * (1.0 - cfg.overlap)).round().max(1.0) as usize;
    let fade = chunk.saturating_sub(step);
    let n_samples = decoded.left.len();
    let mut vocal_l = vec![0.0f32; n_samples];
    let mut vocal_r = vec![0.0f32; n_samples];
    let total_chunks = n_samples.div_ceil(step).max(1);
    let started = Instant::now();

    on_progress(0.08, "Running ONNX inference".into(), None);

    for (chunk_idx, offset) in (0..n_samples).step_by(step).enumerate() {
        let mut left = vec![0.0f32; chunk];
        let mut right = vec![0.0f32; chunk];
        let avail = (n_samples - offset).min(chunk);
        left[..avail].copy_from_slice(&decoded.left[offset..offset + avail]);
        right[..avail].copy_from_slice(&decoded.right[offset..offset + avail]);

        let spec_l = engine.stft(&left)?;
        let spec_r = engine.stft(&right)?;
        let n_frames = spec_l.len().min(cfg.dim_t);
        let mut input = vec![0.0f32; 4 * cfg.dim_f * cfg.dim_t];
        let idx = |c: usize, f: usize, t: usize| ((c * cfg.dim_f) + f) * cfg.dim_t + t;
        for t in 0..n_frames {
            for f in 0..cfg.dim_f {
                let l = spec_l
                    .get(t)
                    .and_then(|frame| frame.get(f))
                    .copied()
                    .unwrap_or(Complex32::new(0.0, 0.0));
                let r = spec_r
                    .get(t)
                    .and_then(|frame| frame.get(f))
                    .copied()
                    .unwrap_or(Complex32::new(0.0, 0.0));
                input[idx(0, f, t)] = l.re;
                input[idx(1, f, t)] = l.im;
                input[idx(2, f, t)] = r.re;
                input[idx(3, f, t)] = r.im;
            }
        }

        let tensor = Tensor::from_array(([1usize, 4, cfg.dim_f, cfg.dim_t], input))?;
        let outputs = model
            .session
            .run(ort::inputs![model.input_name.as_str() => tensor])?;
        let (shape, output) = outputs[0]
            .try_extract_tensor::<f32>()
            .map_err(|e| AppError::msg(e.to_string()))?;
        if shape.len() != 4 {
            return Err(AppError::msg(format!(
                "Unexpected ONNX output rank {} (wanted 4)",
                shape.len()
            )));
        }

        let mut out_l = vec![vec![Complex32::new(0.0, 0.0); engine.n_bins()]; spec_l.len()];
        let mut out_r = vec![vec![Complex32::new(0.0, 0.0); engine.n_bins()]; spec_r.len()];
        let dim_f_out = shape[2] as usize;
        let dim_t_out = shape[3] as usize;
        let out_idx = |c: usize, f: usize, t: usize| ((c * dim_f_out) + f) * dim_t_out + t;
        let dim_f = cfg.dim_f.min(dim_f_out);
        let dim_t = n_frames.min(dim_t_out);
        for t in 0..dim_t {
            for f in 0..dim_f {
                let lr = output.get(out_idx(0, f, t)).copied().unwrap_or(0.0);
                let li = output.get(out_idx(1, f, t)).copied().unwrap_or(0.0);
                let rr = output.get(out_idx(2, f, t)).copied().unwrap_or(0.0);
                let ri = output.get(out_idx(3, f, t)).copied().unwrap_or(0.0);
                if let Some(bin) = out_l[t].get_mut(f) {
                    *bin = Complex32::new(lr, li) * cfg.compensate;
                }
                if let Some(bin) = out_r[t].get_mut(f) {
                    *bin = Complex32::new(rr, ri) * cfg.compensate;
                }
            }
        }

        let rec_l = engine.istft(&out_l, chunk)?;
        let rec_r = engine.istft(&out_r, chunk)?;
        overlap_add(&mut vocal_l, &rec_l, offset, fade);
        overlap_add(&mut vocal_r, &rec_r, offset, fade);

        let pct = 0.08 + 0.82 * ((chunk_idx + 1) as f32 / total_chunks as f32);
        let elapsed = started.elapsed().as_secs_f32();
        let eta = if chunk_idx + 1 > 0 {
            (elapsed / (chunk_idx + 1) as f32) * (total_chunks.saturating_sub(chunk_idx + 1) as f32)
        } else {
            0.0
        };
        on_progress(
            pct,
            format!("Separating chunk {}/{} · ETA {eta:.0}s", chunk_idx + 1, total_chunks),
            Some(eta),
        );
    }

    write_stems(dest_dir, decoded, &vocal_l, &vocal_r, &model.ep_name, on_progress)
}

fn run_roformer(
    model: &mut LoadedModel,
    decoded: &DecodedAudio,
    dest_dir: &Path,
    on_progress: &mut impl FnMut(f32, String, Option<f32>),
) -> AppResult<SeparationResult> {
    let cfg = model.config.clone();
    let chunk = cfg.waveform_len.max(4096);
    let step = ((chunk as f32) * (1.0 - cfg.overlap)).round().max(1.0) as usize;
    let fade = chunk.saturating_sub(step);
    let n_samples = decoded.left.len();
    let mut vocal_l = vec![0.0f32; n_samples];
    let mut vocal_r = vec![0.0f32; n_samples];
    let total_chunks = n_samples.div_ceil(step).max(1);
    let started = Instant::now();
    on_progress(0.08, "Running Roformer inference".into(), None);

    for (chunk_idx, offset) in (0..n_samples).step_by(step).enumerate() {
        let avail = (n_samples - offset).min(chunk);
        let mut left = vec![0.0f32; chunk];
        let mut right = vec![0.0f32; chunk];
        left[..avail].copy_from_slice(&decoded.left[offset..offset + avail]);
        right[..avail].copy_from_slice(&decoded.right[offset..offset + avail]);
        let input = pack_waveform(&left, &right, cfg.layout);
        let shape = match cfg.layout {
            WaveLayout::ChannelFirst => [1usize, 2, chunk],
            WaveLayout::ChannelLast => [1usize, chunk, 2],
        };
        let tensor = Tensor::from_array((shape, input))?;
        let outputs = model
            .session
            .run(ort::inputs![model.input_name.as_str() => tensor])?;
        let (out_shape, output) = outputs[0]
            .try_extract_tensor::<f32>()
            .map_err(|e| AppError::msg(e.to_string()))?;
        let dims: Vec<i64> = out_shape.iter().copied().collect();
        let (rec_l, rec_r) = unpack_waveform(output, &dims, chunk, cfg.layout);
        overlap_add(&mut vocal_l, &rec_l, offset, fade);
        overlap_add(&mut vocal_r, &rec_r, offset, fade);
        let pct = 0.08 + 0.82 * ((chunk_idx + 1) as f32 / total_chunks as f32);
        let elapsed = started.elapsed().as_secs_f32();
        let eta = (elapsed / (chunk_idx + 1) as f32) * (total_chunks.saturating_sub(chunk_idx + 1) as f32);
        on_progress(
            pct,
            format!("Roformer chunk {}/{} · ETA {eta:.0}s", chunk_idx + 1, total_chunks),
            Some(eta),
        );
    }

    write_stems(dest_dir, decoded, &vocal_l, &vocal_r, &model.ep_name, on_progress)
}

fn pack_waveform(left: &[f32], right: &[f32], layout: WaveLayout) -> Vec<f32> {
    let n = left.len();
    let mut input = vec![0.0f32; n * 2];
    match layout {
        WaveLayout::ChannelFirst => {
            input[..n].copy_from_slice(left);
            input[n..].copy_from_slice(right);
        }
        WaveLayout::ChannelLast => {
            for i in 0..n {
                input[i * 2] = left[i];
                input[i * 2 + 1] = right[i];
            }
        }
    }
    input
}

fn unpack_waveform(output: &[f32], shape: &[i64], chunk: usize, layout: WaveLayout) -> (Vec<f32>, Vec<f32>) {
    let mut left = vec![0.0f32; chunk];
    let mut right = vec![0.0f32; chunk];
    let inferred = if shape.len() >= 3 && shape[1] == 2 {
        WaveLayout::ChannelFirst
    } else if shape.len() >= 3 && shape[shape.len() - 1] == 2 {
        WaveLayout::ChannelLast
    } else {
        layout
    };
    match inferred {
        WaveLayout::ChannelFirst => {
            for i in 0..chunk {
                left[i] = output.get(i).copied().unwrap_or(0.0);
                right[i] = output.get(chunk + i).copied().unwrap_or(0.0);
            }
        }
        WaveLayout::ChannelLast => {
            for i in 0..chunk {
                left[i] = output.get(i * 2).copied().unwrap_or(0.0);
                right[i] = output.get(i * 2 + 1).copied().unwrap_or(0.0);
            }
        }
    }
    (left, right)
}

fn write_stems(
    dest_dir: &Path,
    decoded: &DecodedAudio,
    vocal_l: &[f32],
    vocal_r: &[f32],
    ep_name: &str,
    on_progress: &mut impl FnMut(f32, String, Option<f32>),
) -> AppResult<SeparationResult> {
    on_progress(0.92, "Reconstructing stems".into(), Some(0.0));
    let n_samples = decoded.left.len();
    let mut vocals_l = vocal_l.to_vec();
    let mut vocals_r = vocal_r.to_vec();
    vocals_l.resize(n_samples, 0.0);
    vocals_r.resize(n_samples, 0.0);
    let mut inst_l = vec![0.0f32; n_samples];
    let mut inst_r = vec![0.0f32; n_samples];
    for i in 0..n_samples {
        inst_l[i] = decoded.left[i] - vocals_l[i];
        inst_r[i] = decoded.right[i] - vocals_r[i];
    }
    peak_normalize(&mut vocals_l, &mut vocals_r, 0.98);
    peak_normalize(&mut inst_l, &mut inst_r, 0.98);

    let vocals_path = dest_dir.join("vocals.wav");
    let instrumental_path = dest_dir.join("instrumental.wav");
    let peaks_path = dest_dir.join("peaks.json");
    write_stereo_wav(&vocals_path, &vocals_l, &vocals_r, decode::TARGET_RATE)?;
    write_stereo_wav(&instrumental_path, &inst_l, &inst_r, decode::TARGET_RATE)?;
    write_peaks(&peaks_path, &vocals_l, &vocals_r, 160)?;
    on_progress(1.0, "Stems ready".into(), Some(0.0));

    Ok(SeparationResult {
        vocals_path: vocals_path.to_string_lossy().into(),
        instrumental_path: instrumental_path.to_string_lossy().into(),
        peaks_path: peaks_path.to_string_lossy().into(),
        duration_ms: decoded.duration_ms(),
        sample_rate: decode::TARGET_RATE,
        execution_provider: ep_name.to_string(),
        bits_per_sample: crate::dsp::wav::WAV_BITS,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn oom_detector_catches_common_strings() {
        assert!(is_oom(&AppError::msg("CUDA_ERROR_MEMORY allocation failed")));
        assert!(is_oom(&AppError::msg("vkAllocateMemory out of memory")));
        assert!(!is_oom(&AppError::msg("invalid input shape")));
    }

    #[test]
    fn webgpu_conv_error_triggers_cpu_fallback() {
        let err = AppError::msg(
            "Non-zero status code returned while running Conv node. Name:'Conv_0_token_1' Status Message: absl::container_internal::raw_hash_map<>::at",
        );
        assert!(should_fallback_to_cpu(&err));
        assert!(!is_oom(&err));
    }
}
