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

    // ── DSP 音效增强 ──
    /// 切换 Hi-Fi 纯净直通模式（绕过全部 EQ 与 DSP）。
    TogglePureDirect,
    /// 切换电子管胆机暖音开关。
    ToggleTubeWarmth,
    /// 设置电子管胆机暖音浓度 (0.0..=1.0)。
    SetTubeWarmthLevel(f32),
    /// 切换经典发烧耳放 BS2B 纯净互馈模式。
    ToggleBs2bMode,
    /// 切换全景空间声场开关。
    ToggleSpatialAudio,
    /// 设置全景空间声场宽度 (0.0..=1.0)。
    SetSpatialAudioLevel(f32),
    /// 设置对白 / 人声居中清晰度 (0.0..=1.0)。
    SetDialogueClarityLevel(f32),
    /// 切换 3D 立体声拓宽开关（别名兼容）。
    ToggleStereoWidener,
    /// 设置 3D 立体声拓宽强度 (0.0..=1.0)（别名兼容）。
    SetStereoWidenerLevel(f32),
    /// 切换动态低音增强开关。
    ToggleBassBoost,
    /// 设置动态低音增强强度 (0.0..=1.0)。
    SetBassBoostLevel(f32),
    /// 切换人声水晶通透开关。
    ToggleVocalCrystalizer,
    /// 设置人声水晶通透强度 (0.0..=1.0)。
    SetVocalCrystalizerLevel(f32),

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

    // ── 布局 / 独立窗口（增量设计）──
    /// 切换主界面主体 Tab（列表 / 歌词）（T13）。
    SwitchMainTab(MainTab),
    /// 展开 / 折叠主界面内嵌均衡器面板（T14）。
    ToggleEqPanel,
    /// 打开独立音效控制台窗口（若已打开则置顶聚焦）。
    OpenEffectsWindow,
    /// 切换独立音效控制台窗口。
    ToggleEffectsWindow,
    /// 关闭独立音效控制台窗口。
    CloseEffectsWindow,
    /// 重置全部音效与均衡器为默认平直无染色状态。
    ResetAllEffects,

    // ── 音效预设库管理（增量设计）──
    /// 选择并切换当前音效预设（通过预设 id）。
    SelectSoundPreset(String),
    /// 更新新建预设名称输入框内容。
    SetPresetNameInput(String),
    /// 将当前调音参数保存为新的自定义预设。
    SaveCurrentAsNewPreset,
    /// 更新/覆盖当前选中的自定义预设。
    SaveActivePreset,
    /// 删除指定 ID 的自定义预设。
    DeletePreset(String),
    /// 导出当前激活预设为 JSON 文件。
    ExportPreset,
    /// 从外部 JSON 文件导入音效预设。
    ImportPreset,

    // ── WebDAV 与云端流媒体 ──
    /// 打开/聚焦/切换 WebDAV 管理控制台窗口。
    ToggleWebDavWindow,
    /// 关闭 WebDAV 控制台窗口。
    CloseWebDavWindow,
    /// 切换 WebDAV 控制台当前 Tab。
    SwitchWebDavTab(WebDavTab),
    /// 选择当前操作的 WebDAV 服务器。
    SelectWebDavServer(usize),
    /// 更新新建/编辑服务器表单字段。
    SetWebDavFormName(String),
    SetWebDavFormEndpoint(String),
    SetWebDavFormUsername(String),
    SetWebDavFormPassword(String),
    SetWebDavFormAllowInsecure(bool),
    /// 保存服务器配置。
    SaveWebDavServer,
    /// 删除指定下标的 WebDAV 服务器。
    DeleteWebDavServer(usize),
    /// 测试指定服务器连接。
    TestWebDavConnection(usize),
    /// 远端连接测试完成结果回传。
    WebDavConnectionResult(usize, Result<Duration, String>),
    /// 浏览指定远端路径。
    ExploreWebDavDir(String),
    /// 远端目录加载完成。
    WebDavDirLoaded(Result<Vec<crate::webdav::RemoteItem>, String>),
    /// 导入单个远端文件到播放列表。
    ImportRemoteTrack(crate::webdav::RemoteItem),
    /// 批量导入当前目录中全部音频文件。
    ImportAllRemoteAudios,
    /// 一键清空本地持久化磁盘缓存。
    ClearDiskCache,
    /// 磁盘缓存清理完成通知。
    DiskCacheCleared(Result<u64, String>),
    /// 设置最大缓存配额 (MB)。
    SetCacheLimitMb(u64),
}

/// WebDAV 控制台 Tab 分页。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum WebDavTab {
    /// 服务器配置管理（默认）。
    #[default]
    Servers,
    /// 云端文件浏览器。
    Explorer,
    /// 缓存与缓冲中心。
    Cache,
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
