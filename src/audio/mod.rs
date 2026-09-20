//! 音频子系统：解码 → 重采样 → DSP(均衡器) → 频谱 tap → cpal 输出。
//!
//! 整条 PCM 流水线（全程 `f32` 交错）在一个 cpal 回调线程内拉模式运行；
//! 解码在独立的后台任务中完成并推入环形缓冲，回调线程消费 PCM 写入声卡，
//! 同时把已处理的 PCM 喂给频谱分析器。音频线程只通过 [`events`] 中定义的
//! [`AudioEvent`] 经 `crossbeam` 通道回传给 UI，绝不直接修改应用状态。

pub mod decoder;
pub mod dsp;
pub mod engine;
pub mod equalizer;
pub mod events;
pub mod output;
pub mod resampler;

pub use decoder::SymphoniaDecoder;
pub use dsp::{Biquad, BiquadCoeff, DspChain};
pub use engine::{PlaybackEngine, PlaybackEngineHandle};
pub use equalizer::{Equalizer, EqPreset};
pub use events::{AudioEvent, EngineCommand};

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::OnceLock;
use std::time::Duration;

use crossbeam_channel::{Receiver, Sender, TryRecvError};

/// 播放状态（与 [`PlayerStatus`] 配合）。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlaybackState {
    /// 停止（无当前曲目或已结束未自动续播）。
    Stopped,
    /// 正在播放。
    Playing,
    /// 已暂停。
    Paused,
}

impl Default for PlaybackState {
    fn default() -> Self {
        Self::Stopped
    }
}

/// 播放器当前状态快照。
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PlayerStatus {
    /// 播放状态。
    pub state: PlaybackState,
    /// 当前播放位置。
    pub position: Duration,
    /// 当前曲目总时长。
    pub duration: Duration,
    /// 音量（0.0..=1.0）。
    pub volume: f32,
    /// 是否静音。
    pub muted: bool,
}

impl Default for PlayerStatus {
    fn default() -> Self {
        Self {
            state: PlaybackState::Stopped,
            position: Duration::ZERO,
            duration: Duration::ZERO,
            volume: 1.0,
            muted: false,
        }
    }
}

/// 跨线程安全的 `f32` 原子量（标准库未提供）。
#[derive(Debug)]
pub struct AtomicF32 {
    inner: AtomicU32,
}

impl AtomicF32 {
    /// 构造，初始值 `value`。
    pub fn new(value: f32) -> Self {
        Self {
            inner: AtomicU32::new(value.to_bits()),
        }
    }

    /// 加载当前值（宽松内存序，对音频音量足够）。
    pub fn load(&self, order: Ordering) -> f32 {
        f32::from_bits(self.inner.load(order))
    }

    /// 存储新值。
    pub fn store(&self, value: f32, order: Ordering) {
        self.inner.store(value.to_bits(), order);
    }
}

/// 全局音频事件发送 / 接收端（音频线程 → UI 订阅）。
static AUDIO_EVENT_TX: OnceLock<Sender<AudioEvent>> = OnceLock::new();
static AUDIO_EVENT_RX: OnceLock<Receiver<AudioEvent>> = OnceLock::new();

/// 初始化全局通道（应在 `AppState::new` 中调用一次）。
///
/// 重复调用安全（仅首次生效）。音频通道始终初始化；托盘通道仅在 `gui` 特性下初始化。
pub fn init_channels() {
    AUDIO_EVENT_TX.get_or_init(|| {
        let (tx, rx) = crossbeam_channel::unbounded();
        AUDIO_EVENT_RX.set(rx).ok();
        tx
    });
    #[cfg(feature = "gui")]
    TRAY_EVENT_TX.get_or_init(|| {
        let (tx, rx) = crossbeam_channel::unbounded();
        TRAY_EVENT_RX.set(rx).ok();
        tx
    });
}

/// 取得音频事件发送端（克隆）。
pub fn audio_event_sender() -> Option<Sender<AudioEvent>> {
    AUDIO_EVENT_TX.get().cloned()
}

/// 取得音频事件接收端（克隆，供订阅使用）。
pub fn audio_event_receiver() -> Option<Receiver<AudioEvent>> {
    AUDIO_EVENT_RX.get().cloned()
}

/// 发送音频事件（忽略通道未初始化 / 已关闭的情况）。
pub fn send_audio_event(event: AudioEvent) {
    if let Some(tx) = AUDIO_EVENT_TX.get() {
        let _ = tx.send(event);
    }
}

/// 全局托盘动作发送 / 接收端（托盘菜单 → UI 订阅）。仅在 `gui` 特性下存在。
#[cfg(feature = "gui")]
static TRAY_EVENT_TX: OnceLock<Sender<crate::app::TrayAction>> = OnceLock::new();
#[cfg(feature = "gui")]
static TRAY_EVENT_RX: OnceLock<Receiver<crate::app::TrayAction>> = OnceLock::new();

/// 取得托盘动作发送端（克隆）。
#[cfg(feature = "gui")]
pub fn tray_event_sender() -> Option<Sender<crate::app::TrayAction>> {
    TRAY_EVENT_TX.get().cloned()
}

/// 托盘动作接收端（克隆，供订阅使用）。
#[cfg(feature = "gui")]
pub fn tray_event_receiver() -> Option<Receiver<crate::app::TrayAction>> {
    TRAY_EVENT_RX.get().cloned()
}

/// 发送托盘动作（忽略未初始化 / 已关闭）。
#[cfg(feature = "gui")]
pub fn send_tray_action(action: crate::app::TrayAction) {
    if let Some(tx) = TRAY_EVENT_TX.get() {
        let _ = tx.send(action);
    }
}

/// 尝试从音频事件通道非阻塞接收（供测试 / 内部使用）。
pub fn try_recv_audio_event() -> Result<AudioEvent, TryRecvError> {
    match AUDIO_EVENT_RX.get() {
        Some(rx) => rx.try_recv(),
        None => Err(TryRecvError::Disconnected),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::Ordering;

    #[test]
    fn atomic_f32_store_and_load_roundtrip() {
        let a = AtomicF32::new(0.75);
        assert!((a.load(Ordering::Relaxed) - 0.75).abs() < 1e-6);
        a.store(0.0, Ordering::Relaxed);
        assert_eq!(a.load(Ordering::Relaxed), 0.0);
        a.store(1.0, Ordering::Relaxed);
        assert_eq!(a.load(Ordering::Relaxed), 1.0);
    }

    #[test]
    fn engine_handle_volume_and_muted_are_shared_atomically() {
        // REQ-105：音量 / 静音经共享原子量即时生效（不依赖 cpal 设备）。
        let (handle, _rx) = PlaybackEngineHandle::new();
        assert_eq!(handle.volume(), 1.0, "初始音量应为 1.0");
        assert!(!handle.muted(), "初始不应静音");

        handle.set_volume(0.33);
        assert!((handle.volume() - 0.33).abs() < 1e-6);
        // 共享原子量应与句柄读取一致（供 cpal 回调读取）
        assert!((handle.volume_arc().load(Ordering::Relaxed) - 0.33).abs() < 1e-6);

        handle.set_muted(true);
        assert!(handle.muted());
        assert!(handle.muted_arc().load(Ordering::Relaxed));
        handle.set_muted(false);
        assert!(!handle.muted());
    }

    #[test]
    fn player_status_default_is_stopped_and_sane() {
        let s = PlayerStatus::default();
        assert_eq!(s.state, PlaybackState::Stopped);
        assert_eq!(s.position, Duration::ZERO);
        assert_eq!(s.duration, Duration::ZERO);
        assert_eq!(s.volume, 1.0);
        assert!(!s.muted);
    }
}
