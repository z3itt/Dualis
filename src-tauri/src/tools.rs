use std::path::{Path, PathBuf};

use crate::error::{AppError, AppResult};
use crate::models::{self, ModelInfo, ModelSpec};
use crate::paths::AppPaths;
use futures_util::StreamExt;
use serde::Serialize;
use tokio::io::AsyncWriteExt;

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeInfo {
    pub ytdlp_path: Option<String>,
    pub ytdlp_source: String,
    pub model_path: Option<String>,
    pub model_ready: bool,
    pub selected_model: String,
    pub execution_provider: String,
    pub compiled_providers: Vec<String>,
    pub models: Vec<ModelInfo>,
    pub cookies_browser: String,
    pub cookies_file: Option<String>,
    pub download_format: String,
    pub playback_port: u16,
}

pub fn ytdlp_path(paths: &AppPaths) -> PathBuf {
    paths.tools.join(ytdlp_filename())
}

pub fn model_file(paths: &AppPaths, spec: &ModelSpec) -> PathBuf {
    paths.models.join(spec.filename)
}

#[cfg(windows)]
fn ytdlp_filename() -> &'static str {
    "yt-dlp.exe"
}

#[cfg(not(windows))]
fn ytdlp_filename() -> &'static str {
    "yt-dlp"
}

#[cfg(windows)]
fn ytdlp_download_url() -> &'static str {
    "https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp.exe"
}

#[cfg(all(unix, target_arch = "x86_64"))]
fn ytdlp_download_url() -> &'static str {
    "https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp_linux"
}

#[cfg(all(unix, target_arch = "aarch64"))]
fn ytdlp_download_url() -> &'static str {
    "https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp_linux_aarch64"
}

#[cfg(all(unix, not(any(target_arch = "x86_64", target_arch = "aarch64"))))]
fn ytdlp_download_url() -> &'static str {
    "https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp_linux"
}

pub fn sidecar_candidates() -> Vec<PathBuf> {
    let mut out = Vec::new();
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            out.push(dir.join(ytdlp_filename()));
            out.push(dir.join(format!("{}-{}", ytdlp_filename().trim_end_matches(".exe"), current_triple())));
            #[cfg(windows)]
            out.push(dir.join("yt-dlp.exe"));
        }
    }
    out
}

fn current_triple() -> &'static str {
    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    {
        "x86_64-unknown-linux-gnu"
    }
    #[cfg(all(target_os = "linux", target_arch = "aarch64"))]
    {
        "aarch64-unknown-linux-gnu"
    }
    #[cfg(all(windows, target_arch = "x86_64"))]
    {
        "x86_64-pc-windows-msvc"
    }
    #[cfg(all(windows, target_arch = "aarch64"))]
    {
        "aarch64-pc-windows-msvc"
    }
    #[cfg(not(any(
        all(target_os = "linux", target_arch = "x86_64"),
        all(target_os = "linux", target_arch = "aarch64"),
        all(windows, target_arch = "x86_64"),
        all(windows, target_arch = "aarch64")
    )))]
    {
        "unknown"
    }
}

pub fn resolve_ytdlp(paths: &AppPaths) -> Option<(PathBuf, &'static str)> {
    for candidate in sidecar_candidates() {
        if candidate.is_file() {
            return Some((candidate, "sidecar"));
        }
    }
    let cached = ytdlp_path(paths);
    if cached.is_file() {
        return Some((cached, "cache"));
    }
    which::which("yt-dlp").ok().map(|path| (path, "system"))
}

pub async fn ensure_ytdlp(paths: &AppPaths) -> AppResult<PathBuf> {
    if let Some((existing, _)) = resolve_ytdlp(paths) {
        return Ok(existing);
    }
    let dest = ytdlp_path(paths);
    download_file(ytdlp_download_url(), &dest).await?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&dest)?.permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&dest, perms)?;
    }
    Ok(dest)
}

pub fn model_ready(paths: &AppPaths, spec: &ModelSpec) -> bool {
    let dest = model_file(paths, spec);
    dest.is_file() && dest.metadata().map(|m| m.len() > 1_000_000).unwrap_or(false)
}

pub fn list_models(paths: &AppPaths) -> Vec<ModelInfo> {
    models::catalog()
        .iter()
        .map(|spec| ModelInfo {
            id: spec.id.into(),
            name: spec.name.into(),
            architecture: match spec.architecture {
                crate::models::Architecture::Mdx => "mdx".into(),
                crate::models::Architecture::Roformer => "roformer".into(),
            },
            ready: model_ready(paths, spec),
            description: spec.description.into(),
        })
        .collect()
}

pub async fn ensure_model(paths: &AppPaths, spec: &ModelSpec) -> AppResult<PathBuf> {
    let dest = model_file(paths, spec);
    if model_ready(paths, spec) {
        return Ok(dest);
    }
    let mut last_err = AppError::msg("No model URL configured");
    for url in spec.urls {
        match download_file(url, &dest).await {
            Ok(()) => return Ok(dest),
            Err(err) => last_err = err,
        }
    }
    Err(AppError::msg(format!(
        "Could not download {} ({last_err}). Place the ONNX file at {} and retry.",
        spec.name,
        dest.display()
    )))
}

pub fn compiled_providers() -> Vec<String> {
    let mut providers = Vec::new();
    #[cfg(windows)]
    providers.push("DirectML".into());
    #[cfg(feature = "cuda")]
    providers.push("CUDA".into());
    #[cfg(feature = "rocm")]
    providers.push("ROCm".into());
    #[cfg(feature = "openvino")]
    providers.push("OpenVINO".into());
    #[cfg(target_os = "linux")]
    providers.push("WebGPU".into());
    providers.push("CPU".into());
    providers
}

async fn download_file(url: &str, dest: &Path) -> AppResult<()> {
    let tmp = dest.with_extension("partial");
    let client = http_client()?;
    let response = client.get(url).send().await?.error_for_status()?;
    let mut stream = response.bytes_stream();
    let mut file = tokio::fs::File::create(&tmp).await?;
    while let Some(chunk) = stream.next().await {
        file.write_all(&chunk?).await?;
    }
    file.flush().await?;
    drop(file);
    tokio::fs::rename(tmp, dest).await?;
    Ok(())
}

pub fn http_client() -> AppResult<reqwest::Client> {
    Ok(reqwest::Client::builder()
        .user_agent("Dualis/0.1 (desktop; rust)")
        .timeout(std::time::Duration::from_secs(180))
        .redirect(reqwest::redirect::Policy::limited(10))
        .build()?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compiled_providers_include_cpu() {
        assert!(compiled_providers().iter().any(|p| p == "CPU"));
    }

    #[test]
    fn sidecar_candidates_not_empty() {
        assert!(!sidecar_candidates().is_empty() || std::env::current_exe().is_err());
    }
}
