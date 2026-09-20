//! 主状态 [`AppState`] 与初始化。

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use iced::Task;
use iced::window::Id as WindowId;

use crate::audio::{
    AudioEvent, Equalizer, EqPreset, PlaybackEngine, PlaybackEngineHandle, PlaybackState,
    PlayerStatus, init_channels, audio_event_sender,
};
use crate::config::persist;
use crate::config::Settings;
use crate::lyrics::load_for_track;
use crate::playlist::{Playlist, Track};
use crate::theme::{Skin, default_skin};
use crate::visualizer::SpectrumData;

use crate::app::message::AppMessage;

/// 应用主状态（MVU 单一状态源）。
pub struct AppState {
    /// 播放列表。
    pub playlist: Playlist,
    /// 播放器状态快照。
    pub player: PlayerStatus,
    /// 均衡器。
    pub equalizer: Equalizer,
    /// 当前歌词（无则 None）。
    pub lyrics: Option<crate::lyrics::Lrc>,
    /// 歌词偏移（毫秒）。
    pub lyric_offset_ms: i64,
    /// 频谱数据。
    pub spectrum: SpectrumData,
    /// 当前皮肤。
    pub skin: Skin,
    /// 设置。
    pub settings: Settings,
    /// 迷你模式窗口 id。
    pub mini_window_id: Option<WindowId>,
    /// 迷你歌词窗口 id。
    pub mini_lyrics_id: Option<WindowId>,
    /// 引擎命令句柄（轻量，可克隆）。
    pub engine: Arc<PlaybackEngineHandle>,

    // ── crate 内部扩展字段（不影响类图公开接口）──
    /// 主窗口 id（`AppState::new` 打开主窗口时记录）。
    pub(crate) main_window_id: Option<WindowId>,
    /// 音频引擎运行时（持有 cpal 流，drop 时停止）。
    _engine: Option<PlaybackEngine>,
    /// 最近一次音频错误（供 UI 提示）。
    pub(crate) last_error: Option<String>,
    /// 主增益是否触发削波告警。
    pub(crate) gain_clipped: bool,
    /// 主界面主体 Tab（列表 / 歌词，T13，默认列表）。
    pub main_tab: crate::app::message::MainTab,
    /// 均衡器面板是否展开（T14，默认收起）。
    pub eq_expanded: bool,
    /// 窗口尺寸是否有待落盘的变更（T11，1s tick 落盘）。
    pub(crate) window_size_dirty: bool,
}

impl AppState {
    /// 构造应用状态与初始任务。
    ///
    /// 加载设置 / 皮肤 / 均衡器，初始化命令通道与音频引擎，恢复播放列表。
    pub fn new() -> (Self, Task<AppMessage>) {
        init_channels();
        let settings = Settings::load();
        persist::install_builtin_skins().ok();

        let skin = Skin::builtin_by_id(&settings.skin_id).unwrap_or_else(default_skin);

        let mut equalizer = Equalizer::flat();
        equalizer.bands = settings.bands;
        equalizer.master_gain_db = settings.master_gain_db;
        equalizer.preset = EqPreset::from_str(&settings.eq_preset).unwrap_or(EqPreset::Flat);
        equalizer.enabled = true;

        let (handle, cmd_rx) = PlaybackEngineHandle::new();
        let audio_tx = audio_event_sender()
            .unwrap_or_else(|| crossbeam_channel::unbounded::<AudioEvent>().0);
        let engine = match PlaybackEngine::start(
            cmd_rx,
            audio_tx,
            equalizer,
            handle.volume_arc(),
            handle.muted_arc(),
        ) {
            Ok(e) => Some(e),
            Err(e) => {
                log::error!("启动音频引擎失败: {e}");
                None
            }
        };

        let mut playlist = Playlist::new();
        for p in persist::load_playlist_paths() {
            playlist.add(p);
        }

        let player = PlayerStatus {
            state: PlaybackState::Stopped,
            position: Duration::ZERO,
            duration: Duration::ZERO,
            volume: settings.volume,
            muted: false,
        };
        // 同步音量到引擎共享原子量
        handle.set_volume(settings.volume);

        // daemon 不自动开窗：由初始任务打开主窗口（尺寸取持久化值，T11），并记录其 id。
        let (main_window_id, open_task) = iced::window::open(crate::app::main_window_settings((
            settings.window_size.width,
            settings.window_size.height,
        )));

        let state = Self {
            playlist,
            player,
            equalizer,
            lyrics: None,
            lyric_offset_ms: settings.lyric_offset_ms,
            spectrum: SpectrumData::silent(32, 16),
            skin,
            settings,
            mini_window_id: None,
            mini_lyrics_id: None,
            engine: Arc::new(handle),
            main_window_id: Some(main_window_id),
            _engine: engine,
            last_error: None,
            gain_clipped: false,
            main_tab: crate::app::message::MainTab::default(),
            eq_expanded: false,
            window_size_dirty: false,
        };
        (state, open_task.map(|_| AppMessage::Noop))
    }

    /// 播放指定下标曲目（并自动加载同名歌词）。
    pub fn play_index(&mut self, i: usize) {
        self.playlist.set_current(Some(i));
        if let Some(track) = self.playlist.current().cloned() {
            self.engine.play(track.clone());
            self.player.state = PlaybackState::Playing;
            self.player.duration = track.duration;
            self.lyrics = load_for_track(&track.path);
        }
    }

    /// 由路径加载歌词（手动）。
    pub fn load_lyrics_for(&mut self, path: &std::path::Path) {
        self.lyrics = load_for_track(path);
    }

    /// 直接加载一个 `.lrc` 文件（手动选择）。解析失败则保持原歌词不变。
    pub fn load_lyrics_file(&mut self, path: &std::path::Path) {
        match std::fs::read(path) {
            Ok(bytes) => match crate::lyrics::Lrc::parse(&bytes) {
                Ok(lrc) => self.lyrics = Some(lrc),
                Err(e) => log::warn!("解析歌词失败 {}: {e}", path.display()),
            },
            Err(e) => log::warn!("读取歌词失败 {}: {e}", path.display()),
        }
    }

    /// 持久化设置。
    pub fn save_settings(&self) {
        if let Err(e) = self.settings.save() {
            log::warn!("保存设置失败: {e}");
        }
    }

    /// 持久化播放列表路径。
    pub fn save_playlist(&self) {
        let paths: Vec<PathBuf> = self.playlist.tracks.iter().map(|t| t.path.clone()).collect();
        if let Err(e) = persist::save_playlist_paths(&paths) {
            log::warn!("保存播放列表失败: {e}");
        }
    }

    /// 当前歌词显示名或占位。
    pub fn current_track(&self) -> Option<&Track> {
        self.playlist.current()
    }

    /// 最近一次音频错误描述（供 UI）。
    pub fn last_error(&self) -> Option<&str> {
        self.last_error.as_deref()
    }

    /// 主增益是否触发削波告警。
    pub fn gain_clipped(&self) -> bool {
        self.gain_clipped
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new().0
    }
}
