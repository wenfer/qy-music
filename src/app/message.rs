//! 应用主消息枚举 `AppMessage` 与 `TrayAction`。
//!
//! 所有 UI → 状态变更走此单一枚举；音频线程只回传 [`AudioEvent`]，经订阅
//! 转成 [`AppMessage::AudioEvent`]。跨线程通信统一用 `crossbeam` 通道。

use std::path::PathBuf;
use std::time::Duration;

use iced::window;

use crate::audio::events::AudioEvent;
use crate::audio::EqPreset;
use crate::playlist::LoopMode;

/// 应用主消息枚举（iced MVU）。
#[derive(Debug, Clone)]
pub enum AppMessage {
    // ── 播放控制 ──
    /// 切换播放 / 暂停。
    TogglePlay,
    /// 播放（当前曲目）。
    Play,
    /// 暂停。
    Pause,
    /// 下一首。
    Next,
    /// 上一首。
    Prev,
    /// 跳转到指定位置。
    Seek(Duration),
    /// 设置音量（0.0..=1.0）。
    SetVolume(f32),
    /// 切换静音。
    ToggleMute,
    /// 设置循环模式。
    SetLoopMode(LoopMode),

    // ── 播放列表 ──
    /// 添加文件。
    AddFiles,
    /// 添加文件夹。
    AddFolder,
    /// 移除指定下标曲目。
    RemoveTrack(usize),
    /// 播放指定下标曲目。
    PlayTrack(usize),
    /// 移动曲目（拖拽排序）。
    MoveTrack(usize, usize),
    /// 清空播放列表。
    ClearPlaylist,

    // ── 音频事件（订阅回传）──
    /// 音频线程事件。
    AudioEvent(AudioEvent),

    // ── 歌词 ──
    /// 手动加载歌词文件。
    LoadLyrics(PathBuf),
    /// 设置歌词偏移（毫秒）。
    SetLyricOffset(i64),
    /// 切换迷你歌词窗口。
    ToggleMiniLyrics,

    // ── 均衡器 ──
    /// 设置 10 段均衡器增益（dB）。
    SetEqualizerBands([f32; 10]),
    /// 应用预设。
    ApplyEqPreset(EqPreset),
    /// 设置主增益（dB）。
    SetMasterGain(f32),

    // ── 皮肤 / 窗口 ──
    /// 切换皮肤（按 id）。
    SetSkin(String),
    /// 进入迷你模式。
    EnterMiniMode,
    /// 退出迷你模式。
    ExitMiniMode,

    // ── 托盘 ──
    /// 托盘菜单动作。
    TrayAction(TrayAction),
    /// 窗口关闭请求（用于关闭最小化到托盘）。
    WindowClose(window::Id),

    // ── 扩展（订阅 / 事件处理）──
    /// 占位（无操作），用于事件订阅中非目标事件。
    Noop,
    /// 窗口已打开（用于捕获主窗口 id）。
    WindowOpened(window::Id),
    /// 主窗口尺寸变化（T11：更新内存，1s tick 落盘）。
    WindowResized(window::Id, f32, f32),
    /// 持久化 tick（1s）：若有待落盘的窗口尺寸则写入 settings.json。
    PersistTick,

    // ── 布局（增量设计 v1.1）──
    /// 切换主界面主体 Tab（列表 / 歌词）（T13）。
    SwitchMainTab(MainTab),
    /// 展开 / 折叠均衡器面板（T14）。
    ToggleEqPanel,
}

/// 主界面主体 Tab（⑥区，T13）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MainTab {
    /// 播放列表（默认）。
    #[default]
    Playlist,
    /// 歌词。
    Lyrics,
}

/// 托盘菜单动作。
#[derive(Debug, Clone, Copy)]
pub enum TrayAction {
    /// 播放 / 暂停。
    PlayPause,
    /// 上一首。
    Next,
    /// 上一首。
    Prev,
    /// 显示主窗口。
    ShowMainWindow,
    /// 迷你模式。
    MiniMode,
    /// 退出。
    Quit,
}
