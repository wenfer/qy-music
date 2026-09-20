//! 应用设置（持久化到 `dirs::config_dir()/Lingfeng/settings.json`）。
//!
//! 覆盖：皮肤 id、关闭最小化、迷你模式、EQ 预设 / 各段增益 / 主增益、
//! 音量、歌词偏移。列表路径由 [`persist`](crate::config::persist) 单独持久化。

use serde::{Deserialize, Serialize};

use crate::config::persist;
use crate::error::Result;
use crate::theme::DEFAULT_SKIN_ID;

/// 主窗口默认宽度（竖窄窗口，REQ-409）。
pub const DEFAULT_WINDOW_WIDTH: f32 = 340.0;
/// 主窗口默认高度。
pub const DEFAULT_WINDOW_HEIGHT: f32 = 640.0;

/// 持久化的主窗口尺寸（T11 / REQ-409）。
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct WindowSize {
    /// 窗口宽度（逻辑像素）。
    pub width: f32,
    /// 窗口高度（逻辑像素）。
    pub height: f32,
}

impl Default for WindowSize {
    fn default() -> Self {
        Self {
            width: DEFAULT_WINDOW_WIDTH,
            height: DEFAULT_WINDOW_HEIGHT,
        }
    }
}

/// 应用设置。
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Settings {
    /// 当前皮肤 id。
    pub skin_id: String,
    /// 关闭主窗口是否最小化到托盘。
    pub close_to_tray: bool,
    /// 是否以迷你模式启动。
    pub mini_mode: bool,
    /// 均衡器预设名（字符串，便于序列化）。
    pub eq_preset: String,
    /// 10 段均衡器增益（dB）。
    pub bands: [f32; 10],
    /// 主增益（dB）。
    pub master_gain_db: f32,
    /// 音量（0.0..=1.0）。
    pub volume: f32,
    /// 歌词偏移（毫秒，正 = 延后）。
    pub lyric_offset_ms: i64,
    /// 主窗口尺寸（字段级 `serde(default)`：旧 settings.json 缺该字段仍可解析，
    /// 自动取默认 340×640，不破坏既有配置文件）。
    #[serde(default = "default_window_size")]
    pub window_size: WindowSize,
}

/// `window_size` 的 serde 默认值函数。
fn default_window_size() -> WindowSize {
    WindowSize::default()
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            skin_id: DEFAULT_SKIN_ID.to_string(),
            close_to_tray: true,
            mini_mode: false,
            eq_preset: "Flat".to_string(),
            bands: [0.0; 10],
            master_gain_db: 0.0,
            volume: 1.0,
            lyric_offset_ms: 0,
            window_size: WindowSize::default(),
        }
    }
}

impl Settings {
    /// 加载设置：文件缺失或解析失败则返回默认（不报错，保证可启动）。
    pub fn load() -> Settings {
        let path = persist::settings_path();
        match std::fs::read_to_string(&path) {
            Ok(text) => serde_json::from_str(&text).unwrap_or_else(|e| {
                log::warn!("设置解析失败，使用默认: {e}");
                Settings::default()
            }),
            Err(_) => Settings::default(),
        }
    }

    /// 保存设置到配置目录（自动确保目录存在）。
    pub fn save(&self) -> Result<()> {
        persist::ensure_dirs()?;
        let json = serde_json::to_string_pretty(self)?;
        std::fs::write(persist::settings_path(), json)
            .map_err(crate::error::LingfengError::Io)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_sane() {
        let s = Settings::default();
        assert_eq!(s.skin_id, "classic_dark");
        assert!(s.close_to_tray);
        assert!((0.0..=1.0).contains(&s.volume));
    }

    #[test]
    fn serde_roundtrip() {
        let s = Settings::default();
        let json = serde_json::to_string(&s).unwrap();
        let back: Settings = serde_json::from_str(&json).unwrap();
        assert_eq!(s, back);
    }

    // ──────────────────────────────────────────────────────────────
    // QA 补充测试
    // ──────────────────────────────────────────────────────────────

    #[test]
    fn default_values_match_spec() {
        let s = Settings::default();
        assert_eq!(s.skin_id, crate::theme::DEFAULT_SKIN_ID);
        assert!(s.close_to_tray);
        assert!(!s.mini_mode);
        assert_eq!(s.eq_preset, "Flat");
        assert_eq!(s.bands, [0.0; 10]);
        assert_eq!(s.master_gain_db, 0.0);
        assert_eq!(s.volume, 1.0);
        assert_eq!(s.lyric_offset_ms, 0);
        assert_eq!(s.window_size, WindowSize::default());
    }

    #[test]
    fn serde_roundtrip_custom_values() {
        let s = Settings {
            skin_id: "warm_orange".to_string(),
            close_to_tray: false,
            mini_mode: true,
            eq_preset: "Rock".to_string(),
            bands: [1.0, -2.0, 3.0, -4.0, 5.0, -6.0, 7.0, -8.0, 9.0, -10.0],
            master_gain_db: -3.5,
            volume: 0.42,
            lyric_offset_ms: -750,
            window_size: WindowSize {
                width: 400.0,
                height: 720.0,
            },
        };
        let json = serde_json::to_string_pretty(&s).unwrap();
        let back: Settings = serde_json::from_str(&json).unwrap();
        assert_eq!(s, back);
    }

    // ──────────────────────────────────────────────────────────────
    // T11：window_size 持久化测试
    // ──────────────────────────────────────────────────────────────

    #[test]
    fn window_size_defaults_to_340x640() {
        let s = Settings::default();
        assert_eq!(s.window_size, WindowSize { width: 340.0, height: 640.0 });
    }

    #[test]
    fn old_json_without_window_size_still_parses() {
        // 旧 settings.json 无 window_size 字段 → 字段级 serde(default) 兜底 340×640。
        let s = Settings::default();
        let mut v = serde_json::to_value(&s).unwrap();
        v.as_object_mut().unwrap().remove("window_size").unwrap();
        let back: Settings = serde_json::from_value(v).unwrap();
        assert_eq!(back.window_size, WindowSize::default());
        assert_eq!(back, Settings::default());
    }

    #[test]
    fn window_size_roundtrip() {
        let mut s = Settings::default();
        s.window_size = WindowSize {
            width: 500.0,
            height: 800.0,
        };
        let json = serde_json::to_string(&s).unwrap();
        let back: Settings = serde_json::from_str(&json).unwrap();
        assert_eq!(back.window_size.width, 500.0);
        assert_eq!(back.window_size.height, 800.0);
    }

    #[test]
    fn missing_field_is_rejected_by_deserializer() {
        // 说明：Settings 未声明 #[serde(default)]，缺字段会使整体反序列化失败；
        // Settings::load() 捕获该错误并整体回退到默认值（见 load 实现）。
        let partial = r#"{"skin_id":"classic_dark"}"#;
        assert!(
            serde_json::from_str::<Settings>(partial).is_err(),
            "缺字段应反序列化失败（由 load() 统一回退默认）"
        );
    }

    #[test]
    fn corrupt_json_is_rejected() {
        assert!(serde_json::from_str::<Settings>("{ not valid json").is_err());
    }

    #[test]
    fn unknown_fields_are_ignored() {
        let mut v: serde_json::Value =
            serde_json::to_value(Settings::default()).unwrap();
        v.as_object_mut()
            .unwrap()
            .insert("future_field".to_string(), serde_json::json!(42));
        let back: Settings = serde_json::from_value(v).unwrap();
        assert_eq!(back, Settings::default());
    }

    #[test]
    fn absent_settings_file_loads_default_without_panic() {
        // 不直接改写真实配置目录；仅验证 load() 在无文件时能安全返回默认值的契约。
        let s = Settings::load();
        assert!((0.0..=1.0).contains(&s.volume));
    }
}
