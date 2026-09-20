//! 皮肤 JSON Schema 文档与校验（REQ-405 前置，P1）。
//!
//! 本文件不直接参与运行期渲染，仅作为皮肤格式的权威说明与简易校验入口，
//! 便于第三方制作皮肤（P2 自定义皮肤导入）。正式 JSON Schema 可选用
//! `schemars` 生成，这里以文档 + [`Skin::validate`](crate::theme::skin::Skin::validate)
//! 实现等价的运行时校验。
//!
//! ## 字段说明
//!
//! | 字段 | 类型 | 说明 |
//! |------|------|------|
//! | `id` | string | 唯一标识，建议小写蛇形（如 `classic_dark`） |
//! | `name` | string | 显示名称（建议中文） |
//! | `colors.bg` | string(#rrggbb) | 主背景色 |
//! | `colors.fg` | string(#rrggbb) | 前景/文字色 |
//! | `colors.accent` | string(#rrggbb) | 强调色（按钮、进度、高亮） |
//! | `colors.spectrum_low` | string(#rrggbb) | 频谱低端颜色 |
//! | `colors.spectrum_high` | string(#rrggbb) | 频谱高端颜色 |
//! | `layout.show_spectrum` | bool | 是否显示频谱 |
//! | `layout.mini_size` | [u32,u32] | 迷你窗口尺寸 |
//! | `layout.density` | f32(>0) | 布局密度系数 |
//!
//! ## 示例
//!
//! ```json
//! {
//!   "id": "classic_dark",
//!   "name": "墨蓝经典",
//!   "colors": {
//!     "bg": "#0f1424",
//!     "fg": "#e6ecff",
//!     "accent": "#4f8cff",
//!     "spectrum_low": "#1b3a8f",
//!     "spectrum_high": "#5fd0ff"
//!   },
//!   "layout": { "show_spectrum": true, "mini_size": [320, 140], "density": 1.0 }
//! }
//! ```

/// 皮肤 schema 版本（供未来演进时比对）。
pub const SKIN_SCHEMA_VERSION: &str = "1.0";

/// 返回 schema 版本（占位 API，便于扩展）。
pub fn schema_version() -> &'static str {
    SKIN_SCHEMA_VERSION
}
