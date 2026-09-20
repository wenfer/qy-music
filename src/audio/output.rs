//! cpal 输出流封装。
//!
//! 选择默认输出设备与其首选配置，构建 `f32` 拉模式输出流。实际的
//! `data_callback`（PCM 消费 / 音量 / 频谱 tap）由 [`engine`](crate::audio::engine)
//! 提供。

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{
    BufferSize, OutputCallbackInfo, SampleRate, Stream, StreamConfig, StreamError,
    SupportedBufferSize,
};

use crate::error::{LingfengError, Result};

/// 打开默认输出设备流。
///
/// 返回 `(流, 设备采样率, 设备声道数)`。回调 `data_cb` 在音频线程以拉模式被调用，
/// `err_cb` 用于上报流错误。
pub fn open_default_stream<F, E>(
    data_cb: F,
    err_cb: E,
) -> Result<(Stream, u32, u16)>
where
    F: FnMut(&mut [f32], &OutputCallbackInfo) + Send + 'static,
    E: FnMut(StreamError) + Send + 'static,
{
    let host = cpal::default_host();
    let device = host
        .default_output_device()
        .ok_or_else(|| LingfengError::other("未找到默认音频输出设备"))?;
    let supported = device
        .default_output_config()
        .map_err(LingfengError::from)?;

    let stream_config = StreamConfig {
        channels: supported.channels(),
        sample_rate: supported.sample_rate(),
        buffer_size: match supported.buffer_size() {
            SupportedBufferSize::Range { min, .. } => BufferSize::Fixed(*min),
            SupportedBufferSize::Unknown => BufferSize::Default,
        },
    };
    let sample_rate = stream_config.sample_rate.0;
    let channels = stream_config.channels;

    let stream = device
        .build_output_stream(&stream_config, data_cb, err_cb, None)
        .map_err(LingfengError::from)?;

    Ok((stream, sample_rate, channels))
}

/// 便捷：启动流（忽略停止错误）。
pub fn play_stream(stream: &Stream) {
    if let Err(e) = stream.play() {
        log::warn!("启动音频流失败: {e}");
    }
}

/// 便捷：暂停流。
pub fn pause_stream(stream: &Stream) {
    if let Err(e) = stream.pause() {
        log::warn!("暂停音频流失败: {e}");
    }
}

/// 由采样率构造 [`SampleRate`]（占位辅助）。
pub fn sample_rate(rate: u32) -> SampleRate {
    SampleRate(rate)
}
