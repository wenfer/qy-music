//! 皮肤（Skin）：JSON 反序列化 + 内置 2 套原创皮肤。
//!
//! 皮肤 schema（REQ-405 前置，首版 ≥2 套内置）：
//! ```json
//! {
//!   "id": "classic_dark",
//!   "name": "墨蓝经典",
//!   "colors": { "bg": "#0f1424", "fg": "#e6ecff", "accent": "#4f8cff",
//!               "spectrum_low": "#1b3a8f", "spectrum_high": "#5fd0ff" },
//!   "layout": { "show_spectrum": true, "mini_size": [320, 140], "density": 1.0 }
//! }
//! ```

use std::fmt;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::error::{LingfengError, Result};

/// 皮肤配色（十六进制字符串）。
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct SkinColors {
    /// 背景色。
    pub bg: String,
    /// 前景（文字）色。
    pub fg: String,
    /// 强调色（按钮 / 高亮）。
    pub accent: String,
    /// 频谱低端色。
    pub spectrum_low: String,
    /// 频谱高端色。
    pub spectrum_high: String,
}

/// 皮肤布局参数。
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct SkinLayout {
    /// 是否显示频谱。
    pub show_spectrum: bool,
    /// 迷你窗口尺寸 (宽, 高)。
    pub mini_size: (u32, u32),
    /// 布局密度（1.0 = 标准）。
    pub density: f32,
}

/// 皮肤。
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Skin {
    /// 唯一 id。
    pub id: String,
    /// 显示名。
    pub name: String,
    /// 配色。
    pub colors: SkinColors,
    /// 布局。
    pub layout: SkinLayout,
}

impl Skin {
    /// 从 JSON 文件反序列化。
    pub fn from_json(path: &Path) -> Result<Skin> {
        let text = std::fs::read_to_string(path)
            .map_err(|e| LingfengError::other(format!("读取皮肤失败 {}: {e}", path.display())))?;
        let skin: Skin = serde_json::from_str(&text)
            .map_err(|e| LingfengError::other(format!("解析皮肤 {} 失败: {e}", path.display())))?;
        Ok(skin)
    }

    /// 序列化为 JSON 字符串。
    pub fn to_json(&self) -> Result<String> {
        Ok(serde_json::to_string_pretty(self)?)
    }

    /// 校验皮肤字段完整性（P1，简易校验）。
    pub fn validate(&self) -> Result<()> {
        let colors = [
            &self.colors.bg,
            &self.colors.fg,
            &self.colors.accent,
            &self.colors.spectrum_low,
            &self.colors.spectrum_high,
        ];
        for c in colors {
            if !c.starts_with('#') || c.len() != 7 {
                return Err(LingfengError::other(format!("皮肤 {} 配色非法: {c}", self.id)));
            }
        }
        if self.layout.density <= 0.0 {
            return Err(LingfengError::other(format!("皮肤 {} density 必须为正", self.id)));
        }
        Ok(())
    }

    /// 内置 2 套原创皮肤（墨蓝经典 / 暖橙怀旧）。
    pub fn builtin() -> Vec<Skin> {
        vec![
            Skin {
                id: "classic_dark".to_string(),
                name: "墨蓝经典".to_string(),
                colors: SkinColors {
                    bg: "#0f1424".to_string(),
                    fg: "#e6ecff".to_string(),
                    accent: "#4f8cff".to_string(),
                    spectrum_low: "#1b3a8f".to_string(),
                    spectrum_high: "#5fd0ff".to_string(),
                },
                layout: SkinLayout {
                    show_spectrum: true,
                    mini_size: (320, 140),
                    density: 1.0,
                },
            },
            Skin {
                id: "warm_orange".to_string(),
                name: "暖橙怀旧".to_string(),
                colors: SkinColors {
                    bg: "#2a1a12".to_string(),
                    fg: "#fff1e0".to_string(),
                    accent: "#ff9d4d".to_string(),
                    spectrum_low: "#7a3b12".to_string(),
                    spectrum_high: "#ffd27f".to_string(),
                },
                layout: SkinLayout {
                    show_spectrum: true,
                    mini_size: (320, 140),
                    density: 1.0,
                },
            },
        ]
    }

    /// 按 id 从内置皮肤中查找。
    pub fn builtin_by_id(id: &str) -> Option<Skin> {
        Self::builtin().into_iter().find(|s| s.id == id)
    }

    /// 内置皮肤的 `'static` 切片（供 `pick_list` 等需要静态切片、且元素生命
    /// 周期需长于视图的场景复用，避免借用临时值）。
    pub fn builtin_static() -> &'static [Skin] {
        static SKINS: std::sync::OnceLock<Vec<Skin>> = std::sync::OnceLock::new();
        SKINS.get_or_init(Skin::builtin)
    }
}

impl fmt::Display for Skin {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_two_skins() {
        let skins = Skin::builtin();
        assert_eq!(skins.len(), 2);
        assert!(skins.iter().any(|s| s.id == "classic_dark"));
        assert!(skins.iter().any(|s| s.id == "warm_orange"));
    }

    #[test]
    fn from_json_roundtrip() {
        let skin = Skin::builtin_by_id("classic_dark").unwrap();
        let json = skin.to_json().unwrap();
        let parsed: Skin = serde_json::from_str(&json).unwrap();
        assert_eq!(skin, parsed);
    }

    #[test]
    fn validate_rejects_bad_color() {
        let mut skin = Skin::builtin_by_id("classic_dark").unwrap();
        skin.colors.bg = "notacolor".to_string();
        assert!(skin.validate().is_err());
    }

    // ──────────────────────────────────────────────────────────────
    // QA 补充测试（JSON / 校验 / 边界）
    // ──────────────────────────────────────────────────────────────

    use std::sync::atomic::{AtomicU32, Ordering};

    static COUNTER: AtomicU32 = AtomicU32::new(0);

    fn temp_file(tag: &str) -> std::path::PathBuf {
        let n = COUNTER.fetch_add(1, Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!(
            "lingfeng_skin_{}_{}_{}",
            std::process::id(),
            tag,
            n
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir.join("skin.json")
    }

    #[test]
    fn builtin_by_id_unknown_is_none() {
        assert!(Skin::builtin_by_id("no_such_skin").is_none());
    }

    #[test]
    fn builtin_static_matches_builtin() {
        assert_eq!(Skin::builtin_static(), Skin::builtin().as_slice());
    }

    #[test]
    fn validate_accepts_all_builtin_skins() {
        for s in Skin::builtin() {
            assert!(s.validate().is_ok(), "内置皮肤 {} 应通过校验", s.id);
        }
    }

    #[test]
    fn validate_rejects_rgba_color_format() {
        let mut skin = Skin::builtin_by_id("classic_dark").unwrap();
        // 校验只接受 #rrggbb；rgba() 形式应被拒绝。
        skin.colors.accent = "rgba(79,140,255,1)".to_string();
        assert!(skin.validate().is_err());
    }

    #[test]
    fn validate_rejects_short_hex() {
        let mut skin = Skin::builtin_by_id("classic_dark").unwrap();
        skin.colors.fg = "#fff".to_string(); // 长度非 7
        assert!(skin.validate().is_err());
    }

    #[test]
    fn validate_rejects_non_positive_density() {
        let mut skin = Skin::builtin_by_id("classic_dark").unwrap();
        skin.layout.density = 0.0;
        assert!(skin.validate().is_err());
        skin.layout.density = -1.0;
        assert!(skin.validate().is_err());
    }

    #[test]
    fn from_json_missing_field_errors() {
        let path = temp_file("missing");
        // 缺少 layout 字段
        std::fs::write(
            &path,
            r##"{"id":"x","name":"X","colors":{"bg":"#000000","fg":"#ffffff","accent":"#111111","spectrum_low":"#222222","spectrum_high":"#333333"}}"##,
        )
        .unwrap();
        assert!(Skin::from_json(&path).is_err(), "缺字段应报错");
    }

    #[test]
    fn from_json_malformed_errors() {
        let path = temp_file("malformed");
        std::fs::write(&path, b"{ this is not json ").unwrap();
        assert!(Skin::from_json(&path).is_err());
    }

    #[test]
    fn from_json_missing_file_errors() {
        let missing = std::path::PathBuf::from("/nonexistent/lingfeng_qa/no.json");
        assert!(Skin::from_json(&missing).is_err());
    }

    #[test]
    fn from_json_ignores_unknown_fields() {
        let path = temp_file("unknown");
        std::fs::write(
            &path,
            r##"{"id":"x","name":"X","extra_field":123,
               "colors":{"bg":"#000000","fg":"#ffffff","accent":"#111111","spectrum_low":"#222222","spectrum_high":"#333333","unused":"#444444"},
               "layout":{"show_spectrum":true,"mini_size":[320,140],"density":1.0,"note":"hi"}}"##,
        )
        .unwrap();
        let skin = Skin::from_json(&path).expect("未知字段应被忽略");
        assert_eq!(skin.id, "x");
        assert!(skin.validate().is_ok());
    }

    #[test]
    fn from_json_file_roundtrip() {
        let path = temp_file("roundtrip");
        let skin = Skin::builtin_by_id("warm_orange").unwrap();
        std::fs::write(&path, skin.to_json().unwrap()).unwrap();
        let back = Skin::from_json(&path).unwrap();
        assert_eq!(skin, back);
    }
}
