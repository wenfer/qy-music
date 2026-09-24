//! 主状态 [`AppState`] 与初始化。

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use iced::window::Id as WindowId;
use iced::Task;

use crate::audio::{
    audio_event_sender, init_channels, AudioEvent, EqPreset, Equalizer, PlaybackEngine,
    PlaybackEngineHandle, PlaybackState, PlayerStatus,
};
use crate::config::persist;
use crate::config::Settings;
use crate::lyrics::load_for_track;
use crate::playlist::{Playlist, Track};
use crate::theme::{default_skin, Skin};
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
    /// 独立音效控制台窗口 id。
    pub effects_window_id: Option<WindowId>,
    /// 独立 WebDAV 云端控制台窗口 id。
    pub webdav_window_id: Option<WindowId>,
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
    /// DSP 音效增强配置 (全景/胆机/低音/人声/纯净直通)。
    pub effects: crate::audio::AudioEffects,
    /// 用户自定义音效预设列表。
    pub custom_presets: Vec<crate::audio::SoundPreset>,
    /// 当前激活的音效预设 ID。
    pub active_preset_id: String,
    /// 音效控制台中新预设名称输入框内容。
    pub preset_name_input: String,
    /// 实时音频格式与信号链规格信息 (采样率, 设备采样率, 是否 Hi-Res 等)。
    pub format_info: Option<crate::audio::events::AudioFormatInfo>,
    /// 窗口尺寸是否有待落盘的变更（T11，1s tick 落盘）。
    pub(crate) window_size_dirty: bool,

    // ── WebDAV 与流媒体缓存扩展字段 ──
    /// 是否正处于网络卡顿缓冲等待状态。
    pub is_buffering: bool,
    /// 当前网络下载缓冲完成比例 (0.0..=1.0)。
    pub buffer_ratio: f32,
    /// WebDAV 控制台当前分页。
    pub webdav_tab: crate::app::message::WebDavTab,
    /// 当前选中的 WebDAV 服务器下标。
    pub selected_webdav_server: usize,
    /// WebDAV 表单：服务器名称。
    pub webdav_form_name: String,
    /// WebDAV 表单：端点 URL。
    pub webdav_form_endpoint: String,
    /// WebDAV 表单：用户名。
    pub webdav_form_username: String,
    /// WebDAV 表单：密码。
    pub webdav_form_password: String,
    /// WebDAV 表单：是否允许自签无效 TLS 证书。
    pub webdav_form_allow_insecure: bool,
    /// WebDAV 连接测试状态提示。
    pub webdav_test_status: Option<String>,
    /// WebDAV 当前浏览路径。
    pub webdav_current_path: String,
    /// WebDAV 当前目录下的远端文件列表。
    pub webdav_remote_items: Vec<crate::webdav::RemoteItem>,
    /// WebDAV 异步操作加载中标识。
    pub webdav_is_loading: bool,
    /// 本地磁盘持久化缓存已占用字节数。
    pub cache_used_bytes: u64,
    /// 缓存操作提示状态（例如 "已清理释放 120MB"）。
    pub cache_status_msg: Option<String>,
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
        let audio_tx =
            audio_event_sender().unwrap_or_else(|| crossbeam_channel::unbounded::<AudioEvent>().0);
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

        let start_mini = settings.mini_mode;

        // daemon 不自动开窗：由初始任务打开主窗口（尺寸取持久化值，T11），并记录其 id。
        let (main_window_id, open_task) = iced::window::open(iced::window::Settings {
            visible: !start_mini,
            ..crate::app::main_window_settings((
                settings.window_size.width,
                settings.window_size.height,
            ))
        });

        let (mini_window_id, mini_open_task) = if start_mini {
            let (w, h) = skin.layout.mini_size;
            let (id, task) = iced::window::open(iced::window::Settings {
                size: iced::Size::new(w as f32, h as f32),
                resizable: false,
                decorations: true,
                exit_on_close_request: false,
                ..Default::default()
            });
            (Some(id), Some(task))
        } else {
            (None, None)
        };

        let effects = settings.effects;
        handle.set_audio_effects(effects);

        let state = Self {
            playlist,
            player,
            equalizer,
            lyrics: None,
            lyric_offset_ms: settings.lyric_offset_ms,
            spectrum: SpectrumData::silent(32, 16),
            skin,
            mini_window_id,
            mini_lyrics_id: None,
            effects_window_id: None,
            webdav_window_id: None,
            engine: Arc::new(handle),
            main_window_id: Some(main_window_id),
            _engine: engine,
            last_error: None,
            gain_clipped: false,
            main_tab: crate::app::message::MainTab::default(),
            eq_expanded: false,
            effects,
            custom_presets: settings.custom_presets.clone(),
            active_preset_id: settings.active_preset_id.clone(),
            preset_name_input: String::new(),
            settings,
            format_info: None,
            window_size_dirty: false,
            is_buffering: false,
            buffer_ratio: 1.0,
            webdav_tab: crate::app::message::WebDavTab::default(),
            selected_webdav_server: 0,
            webdav_form_name: String::new(),
            webdav_form_endpoint: "http://".to_string(),
            webdav_form_username: String::new(),
            webdav_form_password: String::new(),
            webdav_form_allow_insecure: false,
            webdav_test_status: None,
            webdav_current_path: "/".to_string(),
            webdav_remote_items: Vec::new(),
            webdav_is_loading: false,
            cache_used_bytes: crate::cache::CacheManager::new()
                .map(|m| m.total_cache_size())
                .unwrap_or(0),
            cache_status_msg: None,
        };
        let mut tasks = vec![open_task.map(|_| AppMessage::Noop)];
        if let (Some(m_id), Some(m_task)) = (mini_window_id, mini_open_task) {
            tasks.push(m_task.map(|_| AppMessage::Noop));
            tasks.push(iced::window::gain_focus(m_id));
        } else {
            tasks.push(iced::window::gain_focus(main_window_id));
        }
        let task = iced::Task::batch(tasks);
        (state, task)
    }

    /// 播放指定下标曲目（并自动加载同名歌词）。
    pub fn play_index(&mut self, i: usize) {
        self.playlist.set_current(Some(i));
        if let Some(track) = self.playlist.current().cloned() {
            self.engine.play(track.clone());
            self.player.state = PlaybackState::Playing;
            self.player.duration = track.duration;
            if track.is_remote() {
                self.is_buffering = true;
                self.buffer_ratio = 0.0;
                self.load_remote_lyrics_if_present(&track);
            } else {
                self.is_buffering = false;
                self.buffer_ratio = 1.0;
                self.lyrics = load_for_track(&track.path);
            }
        }
    }

    /// 嗅探或加载 WebDAV 远端同名歌词。
    pub fn load_remote_lyrics_if_present(&mut self, track: &Track) {
        let url = track.path.to_string_lossy().to_string();
        let lrc_url = if let Some(dot_pos) = url.rfind('.') {
            format!("{}.lrc", &url[..dot_pos])
        } else {
            format!("{}.lrc", url)
        };

        // 1. 本地缓存检查
        if let Ok(mut mgr) = crate::cache::CacheManager::new() {
            if let Some(cached_lrc) = mgr.is_cached(&lrc_url) {
                if let Ok(bytes) = std::fs::read(&cached_lrc) {
                    if let Ok(lrc) = crate::lyrics::Lrc::parse(&bytes) {
                        self.lyrics = Some(lrc);
                        return;
                    }
                }
            }
        }

        // 2. 异步向 WebDAV 服务器探测同名 .lrc 文件并拉取写入缓存
        let client = crate::webdav::resolve_client_for_url(&url);
        std::thread::spawn({
            let lrc_url_clone = lrc_url.clone();
            move || {
                if client.check_exists(&lrc_url_clone) {
                    if let Ok(text) = client.fetch_text(&lrc_url_clone) {
                        if let Ok(mut mgr) = crate::cache::CacheManager::new() {
                            let _ = mgr.record_complete(
                                &lrc_url_clone,
                                "lyrics.lrc",
                                "lrc",
                                text.len() as u64,
                            );
                            let target_path = mgr.get_complete_path(
                                &crate::cache::CacheManager::hash_url(&lrc_url_clone),
                                "lrc",
                            );
                            let _ = std::fs::write(target_path, text.as_bytes());
                        }
                    }
                }
            }
        });
        self.lyrics = None;
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
        let paths: Vec<PathBuf> = self
            .playlist
            .tracks
            .iter()
            .map(|t| t.path.clone())
            .collect();
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

    /// 获取全部音效预设列表（内置经典预设 + 用户自定义预设）。
    pub fn all_presets(&self) -> Vec<crate::audio::SoundPreset> {
        let mut list = crate::audio::SoundPreset::builtin_presets();
        list.extend(self.custom_presets.clone());
        list
    }

    /// 获取当前激活的音效预设。
    pub fn active_preset(&self) -> Option<crate::audio::SoundPreset> {
        self.all_presets()
            .into_iter()
            .find(|p| p.id == self.active_preset_id)
    }

    /// 应用音效预设（更新 EQ、DSP、引擎并持久化）。
    pub fn apply_sound_preset(&mut self, preset: &crate::audio::SoundPreset) {
        self.equalizer.bands = preset.bands;
        self.equalizer.master_gain_db = preset.master_gain_db;
        self.equalizer.preset = crate::audio::EqPreset::from_str(&preset.name)
            .unwrap_or(crate::audio::EqPreset::Custom);
        self.engine.set_equalizer(self.equalizer);
        self.settings.bands = self.equalizer.bands;
        self.settings.master_gain_db = self.equalizer.master_gain_db;
        self.settings.eq_preset = self.equalizer.preset.to_string();

        self.effects = preset.effects;
        self.engine.set_audio_effects(self.effects);
        self.settings.effects = self.effects;

        self.active_preset_id = preset.id.clone();
        self.settings.active_preset_id = preset.id.clone();
        self.save_settings();
    }

    /// 根据预设 ID 查找并应用预设。
    pub fn apply_sound_preset_by_id(&mut self, id: &str) {
        if let Some(p) = self.all_presets().into_iter().find(|p| p.id == id) {
            self.apply_sound_preset(&p);
        }
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new().0
    }
}
