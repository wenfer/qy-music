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
            // ── WebDAV 与云端流媒体 ──
            AppMessage::ToggleWebDavWindow => self.toggle_webdav_window(),
            AppMessage::CloseWebDavWindow => self.close_webdav_window(),
            AppMessage::SwitchWebDavTab(tab) => {
                self.webdav_tab = tab;
                if tab == crate::app::message::WebDavTab::Cache {
                    if let Ok(mgr) = crate::cache::CacheManager::new() {
                        self.cache_used_bytes = mgr.total_cache_size();
                    }
                    Task::none()
                } else if tab == crate::app::message::WebDavTab::Explorer
                    && self.webdav_remote_items.is_empty()
                    && !self.settings.webdav_servers.is_empty()
                {
                    self.explore_webdav_dir(self.webdav_current_path.clone())
                } else {
                    Task::none()
                }
            }
            AppMessage::SelectWebDavServer(idx) => {
                if idx < self.settings.webdav_servers.len() {
                    self.selected_webdav_server = idx;
                    self.webdav_tab = crate::app::message::WebDavTab::Explorer;
                    self.webdav_current_path = "/".to_string();
                    self.explore_webdav_dir("/".to_string())
                } else {
                    Task::none()
                }
            }
            AppMessage::SetWebDavFormName(s) => {
                self.webdav_form_name = s;
                Task::none()
            }
            AppMessage::SetWebDavFormEndpoint(s) => {
                self.webdav_form_endpoint = s;
                Task::none()
            }
            AppMessage::SetWebDavFormUsername(s) => {
                self.webdav_form_username = s;
                Task::none()
            }
            AppMessage::SetWebDavFormPassword(s) => {
                self.webdav_form_password = s;
                Task::none()
            }
            AppMessage::SetWebDavFormAllowInsecure(b) => {
                self.webdav_form_allow_insecure = b;
                Task::none()
            }
            AppMessage::SaveWebDavServer => {
                if self.webdav_form_endpoint.trim().is_empty() {
                    return Task::none();
                }
                let name = if self.webdav_form_name.trim().is_empty() {
                    format!("WebDAV 服务器 {}", self.settings.webdav_servers.len() + 1)
                } else {
                    self.webdav_form_name.trim().to_string()
                };
                let id = format!(
                    "srv_{}",
                    std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .map(|d| d.as_millis())
                        .unwrap_or(0)
                );
                let mut server = crate::webdav::WebDavServerConfig::new(
                    id,
                    name,
                    self.webdav_form_endpoint.trim(),
                    self.webdav_form_username.trim(),
                    &self.webdav_form_password,
                );
                server.allow_insecure_cert = self.webdav_form_allow_insecure;
                self.settings.webdav_servers.push(server);
                self.save_settings();
                self.webdav_form_name.clear();
                self.webdav_form_endpoint.clear();
                self.webdav_form_username.clear();
                self.webdav_form_password.clear();
                self.webdav_form_allow_insecure = false;
                Task::none()
            }
            AppMessage::DeleteWebDavServer(idx) => {
                if idx < self.settings.webdav_servers.len() {
                    self.settings.webdav_servers.remove(idx);
                    if self.selected_webdav_server >= self.settings.webdav_servers.len()
                        && !self.settings.webdav_servers.is_empty()
                    {
                        self.selected_webdav_server = self.settings.webdav_servers.len() - 1;
                    }
                    self.webdav_remote_items.clear();
                    self.save_settings();
                }
                Task::none()
            }
            AppMessage::TestWebDavConnection(idx) => {
                if let Some(srv) = self.settings.webdav_servers.get(idx) {
                    let srv_config = srv.clone();
                    self.webdav_test_status = Some("正在连接测试中...".to_string());
                    run_blocking_task(move || {
                        let res = match crate::webdav::WebDavClient::new(&srv_config) {
                            Ok(client) => client.test_connection().map_err(|e| e.to_string()),
                            Err(e) => Err(e.to_string()),
                        };
                        AppMessage::WebDavConnectionResult(idx, res)
                    })
                } else {
                    Task::none()
                }
            }
            AppMessage::WebDavConnectionResult(idx, result) => {
                if self.selected_webdav_server == idx {
                    match result {
                        Ok(latency) => {
                            self.webdav_test_status =
                                Some(format!("连接成功 (RTT: {}ms)", latency.as_millis()));
                        }
                        Err(e) => {
                            self.webdav_test_status = Some(format!("连接失败: {e}"));
                        }
                    }
                }
                Task::none()
            }
            AppMessage::ExploreWebDavDir(path) => self.explore_webdav_dir(path),
            AppMessage::WebDavDirLoaded(result) => {
                self.webdav_is_loading = false;
                match result {
                    Ok(items) => {
                        self.webdav_remote_items = items;
                    }
                    Err(e) => {
                        self.last_error = Some(format!("读取远端目录失败: {e}"));
                        self.webdav_remote_items.clear();
                    }
                }
                Task::none()
            }
            AppMessage::ImportRemoteTrack(item) => {
                if let Some(srv) = self
                    .settings
                    .webdav_servers
                    .get(self.selected_webdav_server)
                {
                    let full_url = srv.build_url(&item.href);
                    let mut track = crate::playlist::Track::from_path(PathBuf::from(&full_url));
                    track.title = item.name.clone();
                    if let Some(dot) = track.title.rfind('.') {
                        track.title = track.title[..dot].to_string();
                    }
                    self.playlist.tracks.push(track);
                    if self.playlist.current_index.is_none() {
                        self.playlist.current_index = Some(0);
                    }
                    self.save_playlist();
                }
                Task::none()
            }
            AppMessage::ImportAllRemoteAudios => {
                if let Some(srv) = self
                    .settings
                    .webdav_servers
                    .get(self.selected_webdav_server)
                {
                    let mut added = false;
                    for item in &self.webdav_remote_items {
                        if item.is_audio_file() {
                            let full_url = srv.build_url(&item.href);
                            let mut track =
                                crate::playlist::Track::from_path(PathBuf::from(&full_url));
                            track.title = item.name.clone();
                            if let Some(dot) = track.title.rfind('.') {
                                track.title = track.title[..dot].to_string();
                            }
                            self.playlist.tracks.push(track);
                            added = true;
                        }
                    }
                    if added {
                        if self.playlist.current_index.is_none() {
                            self.playlist.current_index = Some(0);
                        }
                        self.save_playlist();
                    }
                }
                Task::none()
            }
            AppMessage::ClearDiskCache => {
                self.cache_status_msg = Some("正在清空磁盘缓存...".to_string());
                run_blocking_task(move || {
                    let res = match crate::cache::CacheManager::new() {
                        Ok(mut mgr) => {
                            let freed = mgr.total_cache_size();
                            mgr.clear_all().map(|_| freed).map_err(|e| e.to_string())
                        }
                        Err(e) => Err(e.to_string()),
                    };
                    AppMessage::DiskCacheCleared(res)
                })
            }
            AppMessage::DiskCacheCleared(res) => {
                match res {
                    Ok(freed_bytes) => {
                        let freed_mb = freed_bytes as f64 / 1_048_576.0;
                        self.cache_used_bytes = 0;
                        self.cache_status_msg =
                            Some(format!("已成功清空磁盘缓存，释放 {:.1} MB 空间", freed_mb));
                    }
                    Err(e) => {
                        self.cache_status_msg = Some(format!("清空缓存失败: {e}"));
                    }
                }
                Task::none()
            }
            AppMessage::SetCacheLimitMb(mb) => {
                self.settings.cache_config.max_size_mb = mb;
                self.save_settings();
                let limit = mb;
                run_blocking_task(move || {
                    if let Ok(mut mgr) = crate::cache::CacheManager::new() {
                        let _ = mgr.evict_if_needed(limit);
                    }
                    AppMessage::Noop
                })
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
            AudioEvent::Buffering(b) => {
                self.is_buffering = b;
            }
            AudioEvent::BufferProgress {
                buffered_bytes: _,
                total_bytes: _,
                ratio,
            } => {
                self.buffer_ratio = ratio.clamp(0.0, 1.0);
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
        if self.webdav_window_id == Some(id) {
            self.webdav_window_id = None;
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

    // ── WebDAV 控制台辅助 ──

    fn toggle_webdav_window(&mut self) -> Task<AppMessage> {
        if let Some(id) = self.webdav_window_id {
            self.webdav_window_id = None;
            window::close(id)
        } else {
            self.open_webdav_window()
        }
    }

    fn open_webdav_window(&mut self) -> Task<AppMessage> {
        if let Some(id) = self.webdav_window_id {
            return Task::batch([
                window::set_mode(id, window::Mode::Windowed),
                window::gain_focus(id),
            ]);
        }
        if let Ok(mgr) = crate::cache::CacheManager::new() {
            self.cache_used_bytes = mgr.total_cache_size();
        }
        let (id, task) = window::open(window::Settings {
            size: iced::Size::new(620.0, 480.0),
            min_size: Some(iced::Size::new(540.0, 400.0)),
            resizable: true,
            decorations: true,
            exit_on_close_request: false,
            ..Default::default()
        });
        self.webdav_window_id = Some(id);
        let mut tasks = vec![task.map(|_| AppMessage::Noop), window::gain_focus(id)];
        if !self.settings.webdav_servers.is_empty() && self.webdav_remote_items.is_empty() {
            tasks.push(self.explore_webdav_dir("/".to_string()));
        }
        Task::batch(tasks)
    }

    fn close_webdav_window(&mut self) -> Task<AppMessage> {
        if let Some(id) = self.webdav_window_id.take() {
            window::close(id)
        } else {
            Task::none()
        }
    }

    fn explore_webdav_dir(&mut self, path: String) -> Task<AppMessage> {
        if let Some(srv) = self
            .settings
            .webdav_servers
            .get(self.selected_webdav_server)
        {
            let srv_config = srv.clone();
            self.webdav_current_path = path.clone();
            self.webdav_is_loading = true;
            self.webdav_remote_items.clear();
            run_blocking_task(move || {
                let res = match crate::webdav::WebDavClient::new(&srv_config) {
                    Ok(client) => client.list_dir(&path).map_err(|e| e.to_string()),
                    Err(e) => Err(e.to_string()),
                };
                AppMessage::WebDavDirLoaded(res)
            })
        } else {
            Task::none()
        }
    }
}

/// 在独立 OS 线程中运行阻塞式任务，并通过 oneshot 通道异步回传 AppMessage。
fn run_blocking_task(f: impl FnOnce() -> AppMessage + Send + 'static) -> Task<AppMessage> {
    let (tx, rx) = iced::futures::channel::oneshot::channel();
    std::thread::spawn(move || {
        let res = f();
        let _ = tx.send(res);
    });
    Task::perform(async move { rx.await.unwrap_or(AppMessage::Noop) }, |msg| {
        msg
    })
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

    #[test]
    fn webdav_window_open_toggle_close_lifecycle() {
        let mut state = AppState::default();
        assert!(state.webdav_window_id.is_none());

        // 1. 打开 WebDAV 控制台
        let _ = state.update(AppMessage::ToggleWebDavWindow);
        assert!(state.webdav_window_id.is_some());
        let _dav_id = state.webdav_window_id.unwrap();

        // 2. Toggle 关闭
        let _ = state.update(AppMessage::ToggleWebDavWindow);
        assert!(state.webdav_window_id.is_none());

        // 3. 再次打开并通过 WindowClose 关闭
        let _ = state.update(AppMessage::ToggleWebDavWindow);
        assert!(state.webdav_window_id.is_some());
        let new_dav_id = state.webdav_window_id.unwrap();
        let _ = state.update(AppMessage::WindowClose(new_dav_id));
        assert!(state.webdav_window_id.is_none());
    }

    #[test]
    fn webdav_server_config_management() {
        let mut state = AppState::default();
        state.settings.webdav_servers.clear();

        // 1. 填写表单
        let _ = state.update(AppMessage::SetWebDavFormName("我的群晖 NAS".to_string()));
        let _ = state.update(AppMessage::SetWebDavFormEndpoint(
            "http://192.168.1.100:5005/music".to_string(),
        ));
        let _ = state.update(AppMessage::SetWebDavFormUsername("admin".to_string()));
        let _ = state.update(AppMessage::SetWebDavFormPassword("secret123".to_string()));
        let _ = state.update(AppMessage::SetWebDavFormAllowInsecure(true));

        assert_eq!(state.webdav_form_name, "我的群晖 NAS");
        assert_eq!(
            state.webdav_form_endpoint,
            "http://192.168.1.100:5005/music"
        );
        assert!(state.webdav_form_allow_insecure);

        // 2. 保存服务器
        let _ = state.update(AppMessage::SaveWebDavServer);
        assert_eq!(state.settings.webdav_servers.len(), 1);
        let srv = &state.settings.webdav_servers[0];
        assert_eq!(srv.name, "我的群晖 NAS");
        assert_eq!(srv.endpoint, "http://192.168.1.100:5005/music");
        assert_eq!(srv.username, "admin");
        assert_eq!(srv.get_password(), "secret123");
        assert!(srv.allow_insecure_cert);

        // 表单已被重置
        assert!(state.webdav_form_name.is_empty());
        assert!(state.webdav_form_endpoint.is_empty());

        // 3. 删除服务器
        let _ = state.update(AppMessage::DeleteWebDavServer(0));
        assert_eq!(state.settings.webdav_servers.len(), 0);
    }

    #[test]
    fn webdav_remote_tracks_import_and_buffering() {
        let mut state = AppState::default();
        state.settings.webdav_servers.clear();
        state.playlist.clear();

        let srv = crate::webdav::WebDavServerConfig::new(
            "srv_1",
            "测试云盘",
            "http://alist.local:5244/dav",
            "guest",
            "guest",
        );
        state.settings.webdav_servers.push(srv);
        state.selected_webdav_server = 0;

        // 1. 单曲导入
        let item1 = crate::webdav::RemoteItem {
            name: "晴天.flac".to_string(),
            href: "/dav/Jay/晴天.flac".to_string(),
            is_dir: false,
            size: 25_000_000,
            last_modified: None,
        };
        let _ = state.update(AppMessage::ImportRemoteTrack(item1));
        assert_eq!(state.playlist.tracks.len(), 1);
        let t1 = &state.playlist.tracks[0];
        assert_eq!(t1.title, "晴天");
        assert!(t1.is_remote());
        assert_eq!(
            t1.path.to_string_lossy(),
            "http://alist.local:5244/dav/Jay/晴天.flac"
        );

        // 2. 批量导入
        state.webdav_remote_items = vec![
            crate::webdav::RemoteItem {
                name: "子目录".to_string(),
                href: "/dav/Jay/sub/".to_string(),
                is_dir: true,
                size: 0,
                last_modified: None,
            },
            crate::webdav::RemoteItem {
                name: "七里香.mp3".to_string(),
                href: "/dav/Jay/七里香.mp3".to_string(),
                is_dir: false,
                size: 8_000_000,
                last_modified: None,
            },
            crate::webdav::RemoteItem {
                name: "歌词.lrc".to_string(),
                href: "/dav/Jay/七里香.lrc".to_string(),
                is_dir: false,
                size: 1_200,
                last_modified: None,
            },
        ];
        let _ = state.update(AppMessage::ImportAllRemoteAudios);
        // 只有七里香.mp3 被追加（子目录和歌词被过滤）
        assert_eq!(state.playlist.tracks.len(), 2);
        assert_eq!(state.playlist.tracks[1].title, "七里香");
        assert!(state.playlist.tracks[1].is_remote());

        // 3. 缓冲进度与卡顿状态事件
        let _ = state.update(AppMessage::AudioEvent(crate::audio::AudioEvent::Buffering(
            true,
        )));
        assert!(state.is_buffering);

        let _ = state.update(AppMessage::AudioEvent(
            crate::audio::AudioEvent::BufferProgress {
                buffered_bytes: 5_000_000,
                total_bytes: 8_000_000,
                ratio: 0.625,
            },
        ));
        assert_eq!(state.buffer_ratio, 0.625);

        let _ = state.update(AppMessage::AudioEvent(crate::audio::AudioEvent::Buffering(
            false,
        )));
        assert!(!state.is_buffering);
    }

    #[test]
    fn cache_quota_and_status_message() {
        let mut state = AppState::default();

        let _ = state.update(AppMessage::SetCacheLimitMb(4096));
        assert_eq!(state.settings.cache_config.max_size_mb, 4096);

        let _ = state.update(AppMessage::DiskCacheCleared(Ok(1048576 * 50)));
        assert_eq!(state.cache_used_bytes, 0);
        assert!(state
            .cache_status_msg
            .as_deref()
            .unwrap()
            .contains("50.0 MB"));
    }
}
