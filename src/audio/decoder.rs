//! Symphonia 解码器：打开音频文件 → 逐包产出 `f32` 交错采样。
//!
//! 全程 `f32`、交错（见架构文档 §7）。每包调用 [`SymphoniaDecoder::next_packet`]
//! 返回一包解码后的交错样本（或 `None` 表示 EOF）。

use std::fs::File;
use std::path::Path;
use std::time::Duration;

use symphonia::core::codecs::audio::AudioDecoder;
use symphonia::core::codecs::CodecParameters;
use symphonia::core::formats::probe::Hint;
use symphonia::core::formats::TrackType;
use symphonia::core::io::MediaSourceStream;
use symphonia::default::{get_codecs, get_probe};

use crate::error::{LingfengError, Result};

/// Symphonia 解码器封装。
pub struct SymphoniaDecoder {
    format_reader: Box<dyn symphonia::core::formats::FormatReader>,
    decoder: Box<dyn AudioDecoder>,
    /// 当前轨 id（保留，便于将来多轨过滤）。
    #[allow(dead_code)]
    track_id: u32,
    sample_rate: u32,
    channels: u16,
    /// 估算时长（基于 `n_frames`）。
    duration: Option<Duration>,
}

impl SymphoniaDecoder {
    /// 打开本地音频文件，返回 `(解码器, 采样率, 声道数, 时长)`。
    pub fn open(path: &Path) -> Result<(Self, u32, u16, Option<Duration>)> {
        let file = File::open(path).map_err(LingfengError::Io)?;
        let ext = path.extension().and_then(|e| e.to_str());
        Self::open_source(Box::new(file), ext)
    }

    /// 打开任意实现了 `MediaSource` 的媒体流（支持网络边下边播流与缓存文件）。
    pub fn open_source(
        source: Box<dyn symphonia::core::io::MediaSource>,
        ext_hint: Option<&str>,
    ) -> Result<(Self, u32, u16, Option<Duration>)> {
        let mss = MediaSourceStream::new(source, Default::default());

        let mut hint = Hint::new();
        if let Some(ext) = ext_hint {
            hint.with_extension(ext);
        }

        let format_reader = get_probe()
            .probe(&hint, mss, Default::default(), Default::default())
            .map_err(LingfengError::from)?;

        let track = format_reader
            .default_track(TrackType::Audio)
            .ok_or_else(|| LingfengError::other("未找到可用音轨"))?;
        let track_id = track.id;

        let codec_params = match &track.codec_params {
            Some(CodecParameters::Audio(ref params)) => params,
            _ => return Err(LingfengError::other("未找到可用音频编解码参数")),
        };

        let sample_rate = codec_params
            .sample_rate
            .ok_or_else(|| LingfengError::other("未知采样率"))?;
        let channels = codec_params
            .channels
            .as_ref()
            .map(|c| c.count() as u16)
            .unwrap_or(2);
        let duration = track
            .num_frames
            .zip(Some(sample_rate))
            .map(|(n, sr)| Duration::from_secs_f64(n as f64 / sr as f64));

        let decoder = get_codecs()
            .make_audio_decoder(codec_params, &Default::default())
            .map_err(LingfengError::from)?;

        let decoder = Self {
            format_reader,
            decoder,
            track_id,
            sample_rate,
            channels,
            duration,
        };
        Ok((decoder, sample_rate, channels, duration))
    }

    /// 采样率（Hz）。
    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    /// 声道数。
    pub fn channels(&self) -> u16 {
        self.channels
    }

    /// 估算时长。
    pub fn duration(&self) -> Option<Duration> {
        self.duration
    }

    /// 解码下一包，返回交错 `f32` 样本；EOF 或出错返回 `None`。
    ///
    /// 解码错误以 `Error` 形式经通道上报（不 panic）；这里对可恢复错误返回 `None`。
    pub fn next_packet(&mut self) -> Option<Vec<f32>> {
        let packet = match self.format_reader.next_packet() {
            Ok(Some(p)) => p,
            Ok(None) => {
                log::debug!("音频流播放完毕 (EOF)");
                return None;
            }
            Err(e) => {
                log::debug!("解码流读取出错: {e}");
                return None;
            }
        };

        let decoded = match self.decoder.decode(&packet) {
            Ok(buf) => buf,
            Err(e) => {
                log::warn!("解码包失败: {e}");
                return None;
            }
        };

        let mut samples = Vec::with_capacity(decoded.frames() * self.channels as usize);
        decoded.copy_to_vec_interleaved(&mut samples);
        Some(samples)
    }
}
