//! 音频线程与 UI 之间的事件 / 命令协议。
//!
//! - [`AudioEvent`]: 音频线程 → UI（位置 / 频谱 / 结束 / 错误）。
//! - [`EngineCommand`]: UI → 音频线程（播放 / 暂停 / 跳转 / 均衡器 / 音量）。

use std::time::Duration;

use crate::audio::effects::AudioEffects;
use crate::audio::equalizer::Equalizer;
use crate::playlist::Track;
use crate::visualizer::spectrum::SpectrumData;

/// 实时音频格式与信号链规格信息。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct AudioFormatInfo {
    /// 音频源采样率 (Hz)。
    pub sample_rate: u32,
    /// 音频源声道数。
    pub channels: u16,
    /// 声卡硬件输出采样率 (Hz)。
    pub device_rate: u32,
    /// 是否为 Hi-Res 高解析度音源（>= 48kHz）。
    pub is_hi_res: bool,
}

/// 音频引擎回传给 UI 的事件。
///
/// 由 cpal 回调线程 / 解码线程产生，经 `crossbeam` 通道与 iced `Subscription` 转成
/// [`AppMessage`](crate::app::AppMessage)。
#[derive(Clone, Debug)]
pub enum AudioEvent {
    /// 播放进度更新：`(position, duration)`。
    Position(Duration, Duration),
    /// 一帧频谱数据。
    Spectrum(SpectrumData),
    /// 音频规格与信号链信息。
    Format(AudioFormatInfo),
    /// 网络流缓冲卡顿状态：true 表示正在网络卡顿缓冲中，false 表示已缓冲完毕恢复播放。
    Buffering(bool),
    /// 实时网络缓冲下载进度：`(已下载字节, 总字节, 进度比例 0.0..=1.0)`。
    BufferProgress {
        buffered_bytes: u64,
        total_bytes: u64,
        ratio: f32,
    },
    /// 当前曲目播放结束。
    Ended,
    /// 音频线程发生错误（描述）。
    Error(String),
}

/// UI 下发给音频引擎的命令。
///
/// 引擎在 cpal 回调开头 `try_recv` 消费，避免跨线程锁。
#[derive(Clone, Debug)]
pub enum EngineCommand {
    /// 播放指定曲目（会重建解码 / 重采样链并（重新）打开输出流）。
    Play(Track),
    /// 暂停输出流。
    Pause,
    /// 恢复输出流。
    Resume,
    /// 跳转到指定位置。
    Seek(Duration),
    /// 实时替换均衡器参数（改系数不重启流）。
    SetEqualizer(Equalizer),
    /// 实时替换音效参数 (3D拓宽/低音/人声通透)。
    SetAudioEffects(AudioEffects),
    /// 设置音量（0.0..=1.0）。
    SetVolume(f32),
    /// 设置静音。
    SetMuted(bool),
}
