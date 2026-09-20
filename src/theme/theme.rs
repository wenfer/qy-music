//! 主题常量与配色辅助。
//!
//! 把皮肤中的十六进制字符串转换为 iced [`Color`]（在 `gui` 特性下）。
//! 纯逻辑（非 gui）构建仅保留字符串解析辅助，不依赖 iced。

use crate::theme::skin::Skin;

/// 把 `#rrggbb` 解析为 `(r, g, b)` 归一化浮点（0..1）。
///
/// 任何解析失败回退为中性灰。供 `gui` 下转 `iced::Color` 复用。
pub fn hex_to_rgb(hex: &str) -> (f32, f32, f32) {
    let hex = hex.trim_start_matches('#');
    if hex.len() != 6 {
        return (0.5, 0.5, 0.5);
    }
    let r = u8::from_str_radix(&hex[0..2], 16).unwrap_or(128);
    let g = u8::from_str_radix(&hex[2..4], 16).unwrap_or(128);
    let b = u8::from_str_radix(&hex[4..6], 16).unwrap_or(128);
    (r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0)
}

#[cfg(feature = "gui")]
/// 把皮肤颜色字符串转为 iced [`Color`](iced::Color)。
pub fn skin_color(hex: &str) -> iced::Color {
    let (r, g, b) = hex_to_rgb(hex);
    iced::Color::from_rgb(r, g, b)
}

/// 默认皮肤 id（首选项缺失时使用）。
pub const DEFAULT_SKIN_ID: &str = "classic_dark";

/// 取得默认皮肤（内置）。
pub fn default_skin() -> Skin {
    Skin::builtin_by_id(DEFAULT_SKIN_ID).unwrap_or_else(|| Skin::builtin()[0].clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_to_rgb_parses_valid_color() {
        let (r, g, b) = hex_to_rgb("#000000");
        assert_eq!((r, g, b), (0.0, 0.0, 0.0));
        let (r, g, b) = hex_to_rgb("#ffffff");
        assert!((r - 1.0).abs() < 1e-6 && (g - 1.0).abs() < 1e-6 && (b - 1.0).abs() < 1e-6);
    }

    #[test]
    fn hex_to_rgb_accepts_missing_hash() {
        let with = hex_to_rgb("#1b3a8f");
        let without = hex_to_rgb("1b3a8f");
        assert_eq!(with, without);
    }

    #[test]
    fn hex_to_rgb_invalid_length_falls_back_to_gray() {
        // 长度 ≠ 6 的输入直接回退中性灰。
        assert_eq!(hex_to_rgb("#fff"), (0.5, 0.5, 0.5));
        assert_eq!(hex_to_rgb("#12345"), (0.5, 0.5, 0.5));
        assert_eq!(hex_to_rgb("#1234567"), (0.5, 0.5, 0.5));
    }

    #[test]
    fn hex_to_rgb_invalid_digits_fall_back_per_channel() {
        // 长度合法但含非法字符：每通道解析失败回退 128。
        let (r, g, b) = hex_to_rgb("#zzzzzz");
        assert!((r - 128.0 / 255.0).abs() < 1e-6);
        assert!((g - 128.0 / 255.0).abs() < 1e-6);
        assert!((b - 128.0 / 255.0).abs() < 1e-6);
    }

    #[test]
    fn default_skin_is_classic_dark() {
        assert_eq!(DEFAULT_SKIN_ID, "classic_dark");
        assert_eq!(default_skin().id, DEFAULT_SKIN_ID);
    }

    #[test]
    fn builtin_skins_have_valid_hex_colors() {
        for s in Skin::builtin() {
            for c in [
                &s.colors.bg,
                &s.colors.fg,
                &s.colors.accent,
                &s.colors.spectrum_low,
                &s.colors.spectrum_high,
            ] {
                let (r, g, b) = hex_to_rgb(c);
                assert!((0.0..=1.0).contains(&r));
                assert!((0.0..=1.0).contains(&g));
                assert!((0.0..=1.0).contains(&b));
            }
        }
    }
}
