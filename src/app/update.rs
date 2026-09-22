//! `AppState::update` 主分发。
//!
//! 每个消息要么直接修改状态，要么经 [`PlaybackEngineHandle`] 下发 [`EngineCommand`]，
//! 绝不在 UI 线程阻塞音频处理。

use std::path::PathBuf;
use std::time::Duration;

use iced::window::Id as WindowId;
use iced::{window, Task};

use crate::app::message::AppMessage;
use crate::audio::{AudioEvent, EqPreset, PlaybackState};
use crate::config::persist;

/// 音频文件扩展名（用于文件夹导入）。
const AUDIO_EXTS: &[&str] = &["mp3", "flac", "wav", "ogg", "m4a", "aac", "opus", "wma"];

impl crate::app::state::AppState {
    /// MVU 更新入口。
    pub fn update(&mut self, msg: AppMessage) -> Task<AppMessage> {
        match msg {
            // ── 播放控制 ──
            AppMessage::TogglePlay => self.toggle_play(),
            AppMessage::Play => self.play(),
            AppMessage::Pause => {
                self.engine.pause();
                self.player.state = PlaybackState::Paused;
                // 不立即 self.spectrum.zero()，让频谱随音频 500ms 渐出缓冲自然平滑回落
                Task::none()
            }
            AppMessage::Next => self.advance(false),
            AppMessage::Prev => self.advance(true),
            AppMessage::Seek(d) => {
                self.engine.seek(d);
                self.player.position = d;
                Task::none()
            }
            AppMessage::SetVolume(v) => {
                let v = v.clamp(0.0, 1.0);
                self.engine.set_volume(v);
                self.player.volume = v;
                self.settings.volume = v;
                self.save_settings();
                Task::none()
            }
            AppMessage::ToggleMute => {
                let m = !self.player.muted;
                self.engine.set_muted(m);
                self.player.muted = m;
                Task::none()
            }
            AppMessage::SetLoopMode(m) => {
                self.playlist.set_loop_mode(m);
                Task::none()
            }

            // ── 播放列表 ──
            AppMessage::AddFiles => self.add_files(),
            AppMessage::AddFolder => self.add_folder(),
            AppMessage::RemoveTrack(i) => {
                let was_current = self.playlist.current_index == Some(i);
                self.playlist.remove(i);
                self.save_playlist();
                if was_current {
                    self.player.state = PlaybackState::Stopped;
                }
                Task::none()
            }
            AppMessage::PlayTrack(i) => {
                self.play_index(i);
                Task::none()
            }
            AppMessage::MoveTrack(from, to) => {
                self.playlist.move_item(from, to);
                self.save_playlist();
                Task::none()
            }
            AppMessage::ClearPlaylist => {
                self.playlist.clear();
                self.save_playlist();
                self.player.state = PlaybackState::Stopped;
                Task::none()
            }

            // ── 音频事件 ──
            AppMessage::AudioEvent(ev) => self.on_audio_event(ev),

            // ── 歌词 ──
            AppMessage::LoadLyrics(path) => {
                if path.as_os_str().is_empty() {
                    if let Some(file) = rfd::FileDialog::new()
                        .add_filter("歌词", &["lrc"])
                        .pick_file()
                    {
                        self.load_lyrics_file(&file);
                    }
                } else {
                    self.load_lyrics_file(&path);
                }
                Task::none()
            }
            AppMessage::SetLyricOffset(ms) => {
                self.lyric_offset_ms = ms;
                self.settings.lyric_offset_ms = ms;
                self.save_settings();
                Task::none()
            }
            AppMessage::ToggleMiniLyrics => self.toggle_mini_lyrics(),

            // ── 均衡器 ──
            AppMessage::SetEqualizerBands(bands) => {
                self.equalizer.preset = EqPreset::Custom;
                self.equalizer.bands = bands;
                self.engine.set_equalizer(self.equalizer);
                self.settings.bands = bands;
                self.settings.eq_preset = "Custom".to_string();
                self.save_settings();
                Task::none()
            }
            AppMessage::ApplyEqPreset(p) => {
                self.equalizer.apply_preset(p);
                self.engine.set_equalizer(self.equalizer);
                self.settings.bands = self.equalizer.bands;
                self.settings.eq_preset = p.to_string();
                self.save_settings();
                Task::none()
            }
            AppMessage::SetMasterGain(db) => {
                self.equalizer.master_gain_db = db;
                self.gain_clipped = self.equalizer.clamp_master_gain();
                self.engine.set_equalizer(self.equalizer);
                self.settings.master_gain_db = self.equalizer.master_gain_db;
                self.save_settings();
                Task::none()
            }

            // ── DSP 音效增强 ──
            AppMessage::TogglePureDirect => {
                self.effects.pure_direct = !self.effects.pure_direct;
                self.engine.set_audio_effects(self.effects);
                self.settings.effects = self.effects;
                self.save_settings();
                Task::none()
            }
            AppMessage::ToggleTubeWarmth => {
                self.effects.tube_warmth_enabled = !self.effects.tube_warmth_enabled;
                self.engine.set_audio_effects(self.effects);
                self.settings.effects = self.effects;
                self.save_settings();
                Task::none()
            }
            AppMessage::SetTubeWarmthLevel(lvl) => {
                self.effects.tube_warmth_level = lvl.clamp(0.0, 1.0);
                self.engine.set_audio_effects(self.effects);
                self.settings.effects = self.effects;
                self.save_settings();
                Task::none()
            }
            AppMessage::ToggleBs2bMode => {
                self.effects.bs2b_mode = !self.effects.bs2b_mode;
                self.engine.set_audio_effects(self.effects);
                self.settings.effects = self.effects;
                self.save_settings();
                Task::none()
            }
            AppMessage::ToggleSpatialAudio | AppMessage::ToggleStereoWidener => {
                self.effects.spatial_audio_enabled = !self.effects.spatial_audio_enabled;
                self.engine.set_audio_effects(self.effects);
                self.settings.effects = self.effects;
                self.save_settings();
                Task::none()
            }
            AppMessage::SetSpatialAudioLevel(lvl) | AppMessage::SetStereoWidenerLevel(lvl) => {
                self.effects.spatial_audio_level = lvl.clamp(0.0, 1.0);
                self.engine.set_audio_effects(self.effects);
                self.settings.effects = self.effects;
                self.save_settings();
                Task::none()
            }
            AppMessage::SetDialogueClarityLevel(lvl) => {
                self.effects.dialogue_clarity_level = lvl.clamp(0.0, 1.0);
                self.engine.set_audio_effects(self.effects);
                self.settings.effects = self.effects;
                self.save_settings();
                Task::none()
            }
            AppMessage::ToggleBassBoost => {
                self.effects.bass_boost_enabled = !self.effects.bass_boost_enabled;
                self.engine.set_audio_effects(self.effects);
                self.settings.effects = self.effects;
                self.save_settings();
                Task::none()
            }
            AppMessage::SetBassBoostLevel(lvl) => {
                self.effects.bass_boost_level = lvl.clamp(0.0, 1.0);
                self.engine.set_audio_effects(self.effects);
                self.settings.effects = self.effects;
                self.save_settings();
                Task::none()
            }
            AppMessage::ToggleVocalCrystalizer => {
                self.effects.vocal_crystalizer_enabled = !self.effects.vocal_crystalizer_enabled;
                self.engine.set_audio_effects(self.effects);
                self.settings.effects = self.effects;
                self.save_settings();
                Task::none()
            }
            AppMessage::SetVocalCrystalizerLevel(lvl) => {
                self.effects.vocal_crystalizer_level = lvl.clamp(0.0, 1.0);
                self.engine.set_audio_effects(self.effects);
                self.settings.effects = self.effects;
                self.save_settings();
                Task::none()
            }

            // ── 皮肤 / 窗口 ──
            AppMessage::SetSkin(id) => {
                if let Some(skin) = crate::theme::Skin::builtin_by_id(&id) {
                    self.skin = skin;
                } else if let Some(skin) = persist::list_custom_skins()
                    .into_iter()
                    .find(|s| s.id == id)
                {
                    self.skin = skin;
                }
                self.settings.skin_id = id;
                self.save_settings();
                Task::none()
            }
            AppMessage::EnterMiniMode => self.enter_mini_mode(),
            AppMessage::ExitMiniMode => self.exit_mini_mode(),

            // ── 托盘 ──
            AppMessage::TrayAction(a) => match a {
                crate::app::message::TrayAction::PlayPause => self.toggle_play(),
                crate::app::message::TrayAction::Next => self.advance(false),
                crate::app::message::TrayAction::Prev => self.advance(true),
                crate::app::message::TrayAction::ShowMainWindow => self.show_main_window(),
                crate::app::message::TrayAction::MiniMode => self.enter_mini_mode(),
                crate::app::message::TrayAction::Quit => self.quit(),
            },

            // ── 窗口事件 ──
            AppMessage::WindowClose(id) => self.on_window_close(id),
            AppMessage::WindowOpened(id) => {
                if self.main_window_id.is_none() {
                    self.main_window_id = Some(id);
                }
                Task::none()
            }
            // ── 布局 / 持久化（增量设计 v1.1）──
            AppMessage::SwitchMainTab(tab) => {
                self.main_tab = tab;
                Task::none()
            }
            AppMessage::ToggleEqPanel => {
                self.eq_expanded = !self.eq_expanded;
                Task::none()
            }
            AppMessage::OpenEffectsWindow => self.open_effects_window(),
            AppMessage::ToggleEffectsWindow => self.toggle_effects_window(),
            AppMessage::CloseEffectsWindow => self.close_effects_window(),
            AppMessage::ResetAllEffects => self.reset_all_effects(),
            AppMessage::SelectSoundPreset(id) => {
                self.apply_sound_preset_by_id(&id);
                Task::none()
            }
            AppMessage::SetPresetNameInput(name) => {
                self.preset_name_input = name;
                Task::none()
            }
            AppMessage::SaveCurrentAsNewPreset => {
                let name = if self.preset_name_input.trim().is_empty() {
                    format!("自定义音效 {}", self.custom_presets.len() + 1)
                } else {
                    self.preset_name_input.trim().to_string()
                };
                let preset = crate::audio::SoundPreset::new_custom(
                    name,
                    self.equalizer.bands,
                    self.equalizer.master_gain_db,
                    self.effects,
                );
                self.active_preset_id = preset.id.clone();
                self.custom_presets.push(preset);
                self.settings.custom_presets = self.custom_presets.clone();
                self.settings.active_preset_id = self.active_preset_id.clone();
                self.preset_name_input.clear();
                self.save_settings();
                Task::none()
            }
            AppMessage::SaveActivePreset => {
                if let Some(pos) = self
                    .custom_presets
                    .iter()
                    .position(|p| p.id == self.active_preset_id)
                {
                    self.custom_presets[pos].bands = self.equalizer.bands;
                    self.custom_presets[pos].master_gain_db = self.equalizer.master_gain_db;
                    self.custom_presets[pos].effects = self.effects;
                    self.settings.custom_presets = self.custom_presets.clone();
                    self.save_settings();
                }
                Task::none()
            }
            AppMessage::DeletePreset(id) => {
                self.custom_presets.retain(|p| p.id != id);
                if self.active_preset_id == id {
                    self.apply_sound_preset_by_id("builtin_flat");
                }
                self.settings.custom_presets = self.custom_presets.clone();
                self.save_settings();
                Task::none()
            }
            AppMessage::ExportPreset => self.export_active_preset(),
            AppMessage::ImportPreset => self.import_preset(),
            AppMessage::WindowResized(id, w, h) => {
                if self.main_window_id == Some(id) && w >= 100.0 && h >= 100.0 {
                    self.settings.window_size.width = w;
                    self.settings.window_size.height = h;
                    self.window_size_dirty = true;
                }
                Task::none()
            }
            AppMessage::PersistTick => {
                if self.window_size_dirty {
                    self.window_size_dirty = false;
                    self.save_settings();
                }
                Task::none()
            }
            AppMessage::Noop => Task::none(),
        }
    }

    // ── 播放控制辅助 ──

    fn toggle_play(&mut self) -> Task<AppMessage> {
        match self.player.state {
            PlaybackState::Playing => {
                self.engine.pause();
                self.player.state = PlaybackState::Paused;
                // 不立即 self.spectrum.zero()，让频谱随音频 500ms 渐出自然平滑回落
            }
            PlaybackState::Paused => {
                self.engine.resume();
                self.player.state = PlaybackState::Playing;
            }
            PlaybackState::Stopped => {
                if self.playlist.current().is_some() {
                    if let Some(i) = self.playlist.current_index {
                        self.play_index(i);
                    }
                } else if !self.playlist.tracks.is_empty() {
                    self.play_index(0);
                }
            }
        }
        Task::none()
    }

    fn play(&mut self) -> Task<AppMessage> {
        if self.playlist.current().is_some() {
            if let Some(i) = self.playlist.current_index {
                self.play_index(i);
            }
        } else if !self.playlist.tracks.is_empty() {
            self.play_index(0);
        }
        Task::none()
    }

    fn advance(&mut self, backward: bool) -> Task<AppMessage> {
        let next = if backward {
            self.playlist.prev()
        } else {
            self.playlist.next()
        };
        if let Some(i) = next {
            self.play_index(i);
        } else if !backward {
            // 列表播完且非循环：停止
            self.player.state = PlaybackState::Stopped;
        }
        Task::none()
    }

    fn on_audio_event(&mut self, ev: AudioEvent) -> Task<AppMessage> {
        match ev {
            AudioEvent::Position(p, d) => {
                self.player.position = p;
                self.player.duration = d;
            }
            AudioEvent::Spectrum(mut sd) => {
                if self.player.state != PlaybackState::Stopped && !self.player.muted {
                    // 双阶阻尼平滑律动（Classic Winamp / 千千静听 经典手感）：
                    // 升起时稳健追随（Attack: 0.28），平滑消除高频毛刺与抽搐跳跃；
                    // 跌落时模拟机械重力恒速滑落（Decay: 0.038/frame @ 15FPS），如羽毛般温润沉浮
                    for (curr, prev) in sd.bands.iter_mut().zip(self.spectrum.bands.iter()) {
                        let target = *curr;
                        let next = if target > *prev {
                            *prev + (target - *prev) * 0.28
                        } else {
                            (*prev - 0.038).max(target)
                        };
                        *curr = next.clamp(0.0, 1.0);
                    }
                    self.spectrum = sd;
                } else {
                    self.spectrum.zero();
                }
            }
            AudioEvent::Format(info) => {
                self.format_info = Some(info);
            }
            AudioEvent::Ended => {
                if let Some(i) = self.playlist.next() {
                    self.play_index(i);
                } else {
                    self.player.state = PlaybackState::Stopped;
                    self.player.position = Duration::ZERO;
                }
            }
            AudioEvent::Error(e) => {
                self.last_error = Some(e);
                log::error!("音频错误: {}", self.last_error.as_deref().unwrap_or(""));
            }
        }
        Task::none()
    }

    // ── 列表辅助 ──

    fn add_files(&mut self) -> Task<AppMessage> {
        if let Some(files) = rfd::FileDialog::new()
            .add_filter("音频", &["mp3", "flac", "wav", "ogg", "m4a", "aac", "opus"])
            .pick_files()
        {
            for f in files {
                self.playlist.add(f);
            }
            self.save_playlist();
        }
        Task::none()
    }

    fn add_folder(&mut self) -> Task<AppMessage> {
        if let Some(dir) = rfd::FileDialog::new().pick_folder() {
            let files = collect_audio_files(&dir);
            for f in files {
                self.playlist.add(f);
            }
            self.save_playlist();
        }
        Task::none()
    }

    // ── 歌词辅助 ──

    fn toggle_mini_lyrics(&mut self) -> Task<AppMessage> {
        if let Some(id) = self.mini_lyrics_id {
            self.mini_lyrics_id = None;
            window::close(id)
        } else {
            let (id, task) = window::open(window::Settings {
                size: iced::Size::new(360.0, 120.0),
                resizable: false,
                transparent: true,
                decorations: true,
                exit_on_close_request: false,
                ..Default::default()
            });
            self.mini_lyrics_id = Some(id);
            task.map(|_| AppMessage::Noop)
        }
    }

    // ── 均衡器 / 皮肤辅助见 update 主分发 ──

    // ── 窗口辅助 ──

    fn enter_mini_mode(&mut self) -> Task<AppMessage> {
        self.settings.mini_mode = true;
        self.save_settings();

        let mut tasks = Vec::new();

        // 隐藏主窗口，保持仅显示迷你窗口
        if let Some(main_id) = self.main_window_id {
            tasks.push(window::set_mode(main_id, window::Mode::Hidden));
        }

        // 打开或显示迷你窗口并置顶聚焦
        if let Some(id) = self.mini_window_id {
            tasks.push(window::set_mode(id, window::Mode::Windowed));
            tasks.push(window::gain_focus(id));
        } else {
            let (w, h) = self.skin.layout.mini_size;
            let (id, task) = window::open(window::Settings {
                size: iced::Size::new(w as f32, h as f32),
                resizable: false,
                decorations: true,
                exit_on_close_request: false,
                ..Default::default()
            });
            self.mini_window_id = Some(id);
            tasks.push(task.map(|_| AppMessage::Noop));
            tasks.push(window::gain_focus(id));
        }

        Task::batch(tasks)
    }

    fn exit_mini_mode(&mut self) -> Task<AppMessage> {
        self.show_main_window()
    }

    fn show_main_window(&mut self) -> Task<AppMessage> {
        self.settings.mini_mode = false;
        self.save_settings();

        let mut tasks = Vec::new();

        // 退出迷你模式：关闭迷你窗口
        if let Some(id) = self.mini_window_id.take() {
            tasks.push(window::close(id));
        }

        if let Some(id) = self.main_window_id {
            // 取消主窗口隐藏并请求前台焦点
            tasks.push(window::set_mode(id, window::Mode::Windowed));
            tasks.push(window::gain_focus(id));
        } else {
            // 主窗口已被销毁：重新打开（尺寸取持久化值，T11）并聚焦
            let (id, task) = window::open(crate::app::main_window_settings((
                self.settings.window_size.width,
                self.settings.window_size.height,
            )));
            self.main_window_id = Some(id);
            tasks.push(task.map(|_| AppMessage::Noop));
            tasks.push(window::gain_focus(id));
        }
        Task::batch(tasks)
    }

    fn quit(&mut self) -> Task<AppMessage> {
        // 退出前强制落盘（窗口尺寸等，T11）
        self.window_size_dirty = false;
        self.save_settings();
        // daemon 常驻：显式请求退出进程
        iced::exit()
    }

    fn on_window_close(&mut self, id: WindowId) -> Task<AppMessage> {
        // 关闭迷你窗口：还原并显示主窗口
        if self.mini_window_id == Some(id) {
            return self.show_main_window();
        }
        if self.mini_lyrics_id == Some(id) {
            self.mini_lyrics_id = None;
            return window::close(id);
        }
        if self.effects_window_id == Some(id) {
            self.effects_window_id = None;
            return window::close(id);
        }
        // 主窗口关闭请求：最小化到托盘（隐藏）或直接退出进程
        self.main_window_id = Some(id);
        if self.settings.close_to_tray {
            // 强制落盘窗口尺寸（T11），再隐藏（托盘「显示主窗口」可再次恢复）
            self.window_size_dirty = false;
            self.save_settings();
            window::set_mode(id, window::Mode::Hidden)
        } else {
            self.window_size_dirty = false;
            self.save_settings();
            iced::exit()
        }
    }

    fn open_effects_window(&mut self) -> Task<AppMessage> {
        if let Some(id) = self.effects_window_id {
            window::gain_focus(id)
        } else {
            let (id, task) = window::open(window::Settings {
                size: iced::Size::new(580.0, 580.0),
                min_size: Some(iced::Size::new(500.0, 480.0)),
                resizable: true,
                decorations: true,
                exit_on_close_request: false,
                ..Default::default()
            });
            self.effects_window_id = Some(id);
            Task::batch([task.map(|_| AppMessage::Noop), window::gain_focus(id)])
        }
    }

    fn toggle_effects_window(&mut self) -> Task<AppMessage> {
        if let Some(id) = self.effects_window_id {
            self.effects_window_id = None;
            window::close(id)
        } else {
            self.open_effects_window()
        }
    }

    fn close_effects_window(&mut self) -> Task<AppMessage> {
        if let Some(id) = self.effects_window_id.take() {
            window::close(id)
        } else {
            Task::none()
        }
    }

    fn reset_all_effects(&mut self) -> Task<AppMessage> {
        self.equalizer.preset = EqPreset::Flat;
        self.equalizer.bands = [0.0; 10];
        self.equalizer.master_gain_db = 0.0;
        self.engine.set_equalizer(self.equalizer);
        self.settings.eq_preset = self.equalizer.preset.to_string();
        self.settings.bands = self.equalizer.bands;
        self.settings.master_gain_db = self.equalizer.master_gain_db;

        self.effects = crate::audio::AudioEffects::default();
        self.engine.set_audio_effects(self.effects);
        self.settings.effects = self.effects;

        self.save_settings();
        Task::none()
    }

    fn export_active_preset(&mut self) -> Task<AppMessage> {
        let preset = self.active_preset().unwrap_or_else(|| {
            crate::audio::SoundPreset::new_custom(
                "当前音效参数",
                self.equalizer.bands,
                self.equalizer.master_gain_db,
                self.effects,
            )
        });
        let filename = format!("{}.json", preset.name.replace(['/', '\\'], "_"));
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("音效预设 (*.json)", &["json"])
            .set_file_name(&filename)
            .save_file()
        {
            if let Ok(json) = preset.to_json() {
                if let Err(e) = std::fs::write(&path, json) {
                    log::error!("导出预设失败: {e}");
                }
            }
        }
        Task::none()
    }

    fn import_preset(&mut self) -> Task<AppMessage> {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("音效预设 (*.json)", &["json"])
            .pick_file()
        {
            match std::fs::read_to_string(&path) {
                Ok(text) => match crate::audio::SoundPreset::from_json(&text) {
                    Ok(mut preset) => {
                        // 确保 ID 唯一
                        if self.custom_presets.iter().any(|p| p.id == preset.id) {
                            let ts = std::time::SystemTime::now()
                                .duration_since(std::time::UNIX_EPOCH)
                                .map(|d| d.as_millis())
                                .unwrap_or(0);
                            preset.id = format!("custom_{ts}");
                        }
                        self.custom_presets.push(preset.clone());
                        self.settings.custom_presets = self.custom_presets.clone();
                        self.apply_sound_preset(&preset);
                    }
                    Err(e) => log::warn!("预设文件解析失败: {e}"),
                },
                Err(e) => log::warn!("读取预设文件失败: {e}"),
            }
        }
        Task::none()
    }
}

/// 递归收集目录下所有音频文件。
fn collect_audio_files(dir: &std::path::Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                out.extend(collect_audio_files(&path));
            } else if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                if AUDIO_EXTS.contains(&ext.to_ascii_lowercase().as_str()) {
                    out.push(path);
                }
            }
        }
    }
    out
}

/// 时间格式化 `mm:ss`（供 UI 复用）。
pub fn format_duration(d: Duration) -> String {
    let total = d.as_secs();
    let m = total / 60;
    let s = total % 60;
    format!("{m:02}:{s:02}")
}

// 让 UI 直接引用循环模式枚举
pub use crate::playlist::LoopMode as _LoopModeAlias;

#[cfg(test)]
mod tests {
    use super::format_duration;
    use std::time::Duration;

    use super::AppMessage;
    use crate::app::message::MainTab;
    use crate::app::state::AppState;

    #[test]
    fn format_duration_zero() {
        assert_eq!(format_duration(Duration::ZERO), "00:00");
    }

    #[test]
    fn format_duration_pads_seconds() {
        assert_eq!(format_duration(Duration::from_secs(65)), "01:05");
        assert_eq!(format_duration(Duration::from_secs(9)), "00:09");
    }

    #[test]
    fn format_duration_over_one_hour_still_minutes() {
        // 超过 1 小时仍以「总分钟:秒」表示（与千千静听风格一致）。
        assert_eq!(format_duration(Duration::from_secs(3661)), "61:01");
    }

    // ──────────────────────────────────────────────────────────────
    // 增量设计 v1.1：Tab 切换（T13）/ EQ 折叠（T14）/ 窗口尺寸（T11）
    // ──────────────────────────────────────────────────────────────

    #[test]
    fn main_tab_defaults_to_playlist_and_switches() {
        assert_eq!(MainTab::default(), MainTab::Playlist);
        let mut state = AppState::default();
        assert_eq!(state.main_tab, MainTab::Playlist);
        let _ = state.update(AppMessage::SwitchMainTab(MainTab::Lyrics));
        assert_eq!(state.main_tab, MainTab::Lyrics);
        let _ = state.update(AppMessage::SwitchMainTab(MainTab::Playlist));
        assert_eq!(state.main_tab, MainTab::Playlist);
    }

    #[test]
    fn eq_panel_toggles_collapsed_by_default() {
        let mut state = AppState::default();
        assert!(!state.eq_expanded, "EQ 面板默认收起");
        let _ = state.update(AppMessage::ToggleEqPanel);
        assert!(state.eq_expanded);
        let _ = state.update(AppMessage::ToggleEqPanel);
        assert!(!state.eq_expanded);
    }

    #[test]
    fn window_resize_marks_dirty_and_tick_flushes() {
        let mut state = AppState::default();
        let id = state.main_window_id.expect("new() 应记录主窗口 id");
        let _ = state.update(AppMessage::WindowResized(id, 360.0, 600.0));
        assert!(state.window_size_dirty);
        assert_eq!(state.settings.window_size.width, 360.0);
        assert_eq!(state.settings.window_size.height, 600.0);
        // 1s tick 落盘后清除脏标记
        let _ = state.update(AppMessage::PersistTick);
        assert!(!state.window_size_dirty);
        // 非主窗口的 resize 不影响持久化尺寸
        let _ = state.update(AppMessage::WindowResized(
            iced::window::Id::unique(),
            100.0,
            100.0,
        ));
        assert!(!state.window_size_dirty);
        assert_eq!(state.settings.window_size.width, 360.0);
    }

    #[test]
    fn mini_mode_enter_and_exit_updates_state() {
        let mut state = AppState::default();
        state.settings.mini_mode = false;
        state.mini_window_id = None;

        let _ = state.update(AppMessage::EnterMiniMode);
        assert!(state.settings.mini_mode);
        assert!(state.mini_window_id.is_some());

        let _ = state.update(AppMessage::ExitMiniMode);
        assert!(!state.settings.mini_mode);
        assert!(state.mini_window_id.is_none());
    }

    #[test]
    fn mini_mode_close_restores_main_window_state() {
        let mut state = AppState::default();
        state.settings.mini_mode = false;
        state.mini_window_id = None;

        let _ = state.update(AppMessage::EnterMiniMode);
        let mini_id = state.mini_window_id.expect("mini window opened");
        assert!(state.settings.mini_mode);

        let _ = state.update(AppMessage::WindowClose(mini_id));
        assert!(!state.settings.mini_mode);
        assert!(state.mini_window_id.is_none());
    }

    #[test]
    fn effects_window_open_toggle_close_lifecycle() {
        let mut state = AppState::default();
        assert!(state.effects_window_id.is_none());

        // 打开控制台窗口
        let _ = state.update(AppMessage::OpenEffectsWindow);
        assert!(state.effects_window_id.is_some());
        let effects_id = state.effects_window_id.unwrap();

        // 再次调用 OpenEffectsWindow 保持原 id
        let _ = state.update(AppMessage::OpenEffectsWindow);
        assert_eq!(state.effects_window_id, Some(effects_id));

        // Toggle 关闭
        let _ = state.update(AppMessage::ToggleEffectsWindow);
        assert!(state.effects_window_id.is_none());

        // Toggle 再次打开
        let _ = state.update(AppMessage::ToggleEffectsWindow);
        assert!(state.effects_window_id.is_some());
        let new_effects_id = state.effects_window_id.unwrap();

        // CloseRequested / WindowClose 关闭
        let _ = state.update(AppMessage::WindowClose(new_effects_id));
        assert!(state.effects_window_id.is_none());
    }

    #[test]
    fn reset_all_effects_clears_dsp_and_eq() {
        let mut state = AppState::default();
        state.effects.pure_direct = true;
        state.effects.tube_warmth_enabled = true;
        state.effects.spatial_audio_enabled = true;
        state.equalizer.master_gain_db = 3.5;
        state.equalizer.bands[0] = 5.0;

        let _ = state.update(AppMessage::ResetAllEffects);

        assert!(!state.effects.pure_direct);
        assert!(!state.effects.tube_warmth_enabled);
        assert!(!state.effects.spatial_audio_enabled);
        assert_eq!(state.equalizer.master_gain_db, 0.0);
        assert_eq!(state.equalizer.bands[0], 0.0);
    }

    #[test]
    fn sound_preset_crud_operations() {
        let mut state = AppState::default();
        let initial_count = state.all_presets().len();
        assert!(initial_count >= 12);

        // 1. 切换内置预设
        let _ = state.update(AppMessage::SelectSoundPreset("builtin_rock".to_string()));
        assert_eq!(state.active_preset_id, "builtin_rock");
        assert!(state.effects.bass_boost_enabled);

        // 2. 添加自定义预设
        let _ = state.update(AppMessage::SetPresetNameInput("我的监听大耳".to_string()));
        state.equalizer.master_gain_db = 2.0;
        state.effects.tube_warmth_enabled = true;
        let _ = state.update(AppMessage::SaveCurrentAsNewPreset);

        assert_eq!(state.custom_presets.len(), 1);
        let custom_id = state.custom_presets[0].id.clone();
        assert_eq!(state.custom_presets[0].name, "我的监听大耳");
        assert_eq!(state.active_preset_id, custom_id);
        assert_eq!(state.all_presets().len(), initial_count + 1);

        // 3. 编辑并保存当前自定义预设
        state.equalizer.master_gain_db = 3.0;
        let _ = state.update(AppMessage::SaveActivePreset);
        assert_eq!(state.custom_presets[0].master_gain_db, 3.0);

        // 4. 删除自定义预设
        let _ = state.update(AppMessage::DeletePreset(custom_id));
        assert_eq!(state.custom_presets.len(), 0);
        assert_eq!(state.active_preset_id, "builtin_flat");
    }
}
