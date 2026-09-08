use serde::Serialize;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Architecture {
    Mdx,
    Roformer,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum WaveLayout {
    ChannelFirst,
    ChannelLast,
}

#[derive(Clone, Debug)]
pub struct ModelConfig {
    pub n_fft: usize,
    pub hop: usize,
    pub dim_f: usize,
    pub dim_t: usize,
    pub compensate: f32,
    pub overlap: f32,
    pub waveform_len: usize,
    pub layout: WaveLayout,
}

impl ModelConfig {
    pub fn chunk_size(&self) -> usize {
        self.hop * (self.dim_t.saturating_sub(1))
    }
}

impl Default for ModelConfig {
    fn default() -> Self {
        Self {
            n_fft: 6144,
            hop: 1024,
            dim_f: 3072,
            dim_t: 256,
            compensate: 1.035,
            overlap: 0.25,
            waveform_len: 0,
            layout: WaveLayout::ChannelFirst,
        }
    }
}

#[derive(Clone, Debug)]
pub struct ModelSpec {
    pub id: &'static str,
    pub name: &'static str,
    pub filename: &'static str,
    pub urls: &'static [&'static str],
    pub architecture: Architecture,
    pub config: ModelConfig,
    pub description: &'static str,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelInfo {
    pub id: String,
    pub name: String,
    pub architecture: String,
    pub ready: bool,
    pub description: String,
}

pub const DEFAULT_MODEL_ID: &str = "kim-vocal-2";

pub fn catalog() -> &'static [ModelSpec] {
    &[
        ModelSpec {
            id: "kim-vocal-2",
            name: "Kim Vocal 2",
            filename: "Kim_Vocal_2.onnx",
            urls: &[
                "https://github.com/TRvlvr/model_repo/releases/download/all_public_uvr_models/Kim_Vocal_2.onnx",
                "https://huggingface.co/seanghay/uvr_models/resolve/main/Kim_Vocal_2.onnx",
            ],
            architecture: Architecture::Mdx,
            config: ModelConfig {
                n_fft: 6144,
                hop: 1024,
                dim_f: 3072,
                dim_t: 256,
                compensate: 1.035,
                overlap: 0.25,
                waveform_len: 0,
                layout: WaveLayout::ChannelFirst,
            },
            description: "MDX-Net vocal model. Fast, reliable default for pop and electronic.",
        },
        ModelSpec {
            id: "uvr-mdx-voc-ft",
            name: "UVR MDX Voc FT",
            filename: "UVR-MDX-NET-Voc_FT.onnx",
            urls: &[
                "https://github.com/TRvlvr/model_repo/releases/download/all_public_uvr_models/UVR-MDX-NET-Voc_FT.onnx",
                "https://huggingface.co/seanghay/uvr_models/resolve/main/UVR-MDX-NET-Voc_FT.onnx",
            ],
            architecture: Architecture::Mdx,
            config: ModelConfig {
                n_fft: 7680,
                hop: 1024,
                dim_f: 3072,
                dim_t: 256,
                compensate: 1.0,
                overlap: 0.25,
                waveform_len: 0,
                layout: WaveLayout::ChannelFirst,
            },
            description: "Fine-tuned MDX vocal net. Different timbre, same spectrogram pipeline.",
        },
        ModelSpec {
            id: "bs-roformer",
            name: "BS-Roformer",
            filename: "model_bs_roformer.onnx",
            urls: &[
                "https://huggingface.co/seanghay/uvr_models/resolve/main/model_bs_roformer_ep_317_sdr_12.9755.onnx",
            ],
            architecture: Architecture::Roformer,
            config: ModelConfig {
                n_fft: 2048,
                hop: 441,
                dim_f: 1024,
                dim_t: 256,
                compensate: 1.0,
                overlap: 0.25,
                waveform_len: 352_800,
                layout: WaveLayout::ChannelFirst,
            },
            description: "Roformer-style waveform ONNX. Shape is read from the file at load time.",
        },
    ]
}

pub fn by_id(id: &str) -> Option<&'static ModelSpec> {
    catalog().iter().find(|spec| spec.id == id)
}

pub fn require(id: &str) -> &'static ModelSpec {
    by_id(id).unwrap_or_else(|| by_id(DEFAULT_MODEL_ID).expect("default model missing"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_has_unique_ids() {
        let mut ids: Vec<_> = catalog().iter().map(|s| s.id).collect();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), catalog().len());
    }

    #[test]
    fn default_model_exists() {
        assert!(by_id(DEFAULT_MODEL_ID).is_some());
        assert_eq!(require("missing").id, DEFAULT_MODEL_ID);
    }
}
