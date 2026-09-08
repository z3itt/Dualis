use std::fs::File;
use std::path::Path;

use crate::error::{AppError, AppResult};
use rubato::{FastFixedIn, PolynomialDegree, Resampler};
use symphonia::core::audio::{AudioBufferRef, Signal};
use symphonia::core::codecs::{DecoderOptions, CODEC_TYPE_NULL};
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;

pub const TARGET_RATE: u32 = 44_100;

pub struct DecodedAudio {
    pub sample_rate: u32,
    pub left: Vec<f32>,
    pub right: Vec<f32>,
}

impl DecodedAudio {
    pub fn duration_ms(&self) -> i64 {
        if self.sample_rate == 0 {
            return 0;
        }
        (self.left.len() as i64 * 1000) / self.sample_rate as i64
    }
}

pub fn decode_path(path: &Path) -> AppResult<DecodedAudio> {
    let file = File::open(path)?;
    let mss = MediaSourceStream::new(Box::new(file), Default::default());
    let mut hint = Hint::new();
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        hint.with_extension(ext);
    }
    let probed = symphonia::default::get_probe()
        .format(
            &hint,
            mss,
            &FormatOptions {
                enable_gapless: true,
                ..Default::default()
            },
            &MetadataOptions::default(),
        )
        .map_err(|e| AppError::msg(format!("Cannot probe audio: {e}")))?;

    let mut format = probed.format;
    let track = format
        .tracks()
        .iter()
        .find(|t| t.codec_params.codec != CODEC_TYPE_NULL)
        .ok_or_else(|| AppError::msg("No audio track in file"))?
        .clone();
    let sample_rate = track
        .codec_params
        .sample_rate
        .ok_or_else(|| AppError::msg("Missing sample rate"))?;
    let mut decoder = symphonia::default::get_codecs()
        .make(&track.codec_params, &DecoderOptions::default())
        .map_err(|e| AppError::msg(format!("Cannot create decoder: {e}")))?;

    let mut left = Vec::new();
    let mut right = Vec::new();

    loop {
        let packet = match format.next_packet() {
            Ok(packet) => packet,
            Err(symphonia::core::errors::Error::IoError(err))
                if err.kind() == std::io::ErrorKind::UnexpectedEof =>
            {
                break;
            }
            Err(symphonia::core::errors::Error::ResetRequired) => {
                decoder.reset();
                continue;
            }
            Err(_) => break,
        };
        if packet.track_id() != track.id {
            continue;
        }
        let decoded = match decoder.decode(&packet) {
            Ok(buf) => buf,
            Err(symphonia::core::errors::Error::DecodeError(_)) => continue,
            Err(e) => return Err(AppError::msg(format!("Decode failed: {e}"))),
        };
        append_buffer(decoded, &mut left, &mut right);
    }

    if left.is_empty() {
        return Err(AppError::msg("Decoded audio is empty"));
    }
    if right.is_empty() {
        right.clone_from(&left);
    }
    let n = left.len().min(right.len());
    left.truncate(n);
    right.truncate(n);

    if sample_rate != TARGET_RATE {
        left = resample_channel(&left, sample_rate, TARGET_RATE)?;
        right = resample_channel(&right, sample_rate, TARGET_RATE)?;
        let n = left.len().min(right.len());
        left.truncate(n);
        right.truncate(n);
    }

    Ok(DecodedAudio {
        sample_rate: TARGET_RATE,
        left,
        right,
    })
}

fn append_buffer(buffer: AudioBufferRef, left: &mut Vec<f32>, right: &mut Vec<f32>) {
    match buffer {
        AudioBufferRef::F32(buf) => copy_signal(&*buf, left, right),
        AudioBufferRef::U8(buf) => copy_signal(&*buf, left, right),
        AudioBufferRef::U16(buf) => copy_signal(&*buf, left, right),
        AudioBufferRef::U24(buf) => copy_signal(&*buf, left, right),
        AudioBufferRef::U32(buf) => copy_signal(&*buf, left, right),
        AudioBufferRef::S8(buf) => copy_signal(&*buf, left, right),
        AudioBufferRef::S16(buf) => copy_signal(&*buf, left, right),
        AudioBufferRef::S24(buf) => copy_signal(&*buf, left, right),
        AudioBufferRef::S32(buf) => copy_signal(&*buf, left, right),
        AudioBufferRef::F64(buf) => copy_signal(&*buf, left, right),
    }
}

fn copy_signal<S>(buf: &symphonia::core::audio::AudioBuffer<S>, left: &mut Vec<f32>, right: &mut Vec<f32>)
where
    S: symphonia::core::sample::Sample,
    f32: sample_convert::FromSample<S>,
{
    use sample_convert::FromSample;
    let spec = buf.spec();
    let frames = buf.frames();
    left.reserve(frames);
    right.reserve(frames);
    let channels = spec.channels.count();
    if channels == 1 {
        for i in 0..frames {
            let s = f32::from_sample(buf.chan(0)[i]);
            left.push(s);
            right.push(s);
        }
        return;
    }
    for i in 0..frames {
        left.push(f32::from_sample(buf.chan(0)[i]));
        right.push(f32::from_sample(buf.chan(1.min(channels - 1))[i]));
    }
}

mod sample_convert {
    pub trait FromSample<S> {
        fn from_sample(sample: S) -> Self;
    }

    impl FromSample<f32> for f32 {
        fn from_sample(sample: f32) -> Self {
            sample
        }
    }
    impl FromSample<f64> for f32 {
        fn from_sample(sample: f64) -> Self {
            sample as f32
        }
    }
    impl FromSample<i8> for f32 {
        fn from_sample(sample: i8) -> Self {
            sample as f32 / i8::MAX as f32
        }
    }
    impl FromSample<i16> for f32 {
        fn from_sample(sample: i16) -> Self {
            sample as f32 / i16::MAX as f32
        }
    }
    impl FromSample<i32> for f32 {
        fn from_sample(sample: i32) -> Self {
            sample as f32 / i32::MAX as f32
        }
    }
    impl FromSample<u8> for f32 {
        fn from_sample(sample: u8) -> Self {
            (sample as f32 - 128.0) / 128.0
        }
    }
    impl FromSample<u16> for f32 {
        fn from_sample(sample: u16) -> Self {
            (sample as f32 / u16::MAX as f32) * 2.0 - 1.0
        }
    }
    impl FromSample<u32> for f32 {
        fn from_sample(sample: u32) -> Self {
            (sample as f32 / u32::MAX as f32) * 2.0 - 1.0
        }
    }
    impl FromSample<symphonia::core::sample::i24> for f32 {
        fn from_sample(sample: symphonia::core::sample::i24) -> Self {
            sample.inner() as f32 / 8_388_607.0
        }
    }
    impl FromSample<symphonia::core::sample::u24> for f32 {
        fn from_sample(sample: symphonia::core::sample::u24) -> Self {
            (sample.inner() as f32 / 16_777_215.0) * 2.0 - 1.0
        }
    }
}

fn resample_channel(input: &[f32], from: u32, to: u32) -> AppResult<Vec<f32>> {
    if from == to {
        return Ok(input.to_vec());
    }
    let chunk = 2048;
    let mut resampler = FastFixedIn::<f32>::new(
        to as f64 / from as f64,
        2.0,
        PolynomialDegree::Linear,
        chunk,
        1,
    )
    .map_err(|e| AppError::msg(format!("Resampler init failed: {e}")))?;
    let mut output = Vec::with_capacity(((input.len() as f64 * to as f64 / from as f64) as usize) + 16);
    let mut offset = 0;
    while offset < input.len() {
        let end = (offset + chunk).min(input.len());
        let mut block = vec![0.0f32; chunk];
        block[..end - offset].copy_from_slice(&input[offset..end]);
        let waves = resampler
            .process(&[block], None)
            .map_err(|e| AppError::msg(format!("Resample failed: {e}")))?;
        output.extend_from_slice(&waves[0]);
        offset = end;
    }
    Ok(output)
}
