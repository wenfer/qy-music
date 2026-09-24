//! LFPlayer — 跨平台音乐播放器
//!
//! 模块分层：
//! - `error`    : 统一错误类型 [`LingfengError`]
//! - `audio`    : 解码 / 重采样 / DSP / 均衡器 / 输出引擎 / 音频事件
//! - `playlist` : 曲目与播放列表（循环模式）
//! - `lyrics`   : LRC 解析 / 自动匹配 / 同步
//! - `visualizer`: FFT / 频谱映射 / LED 渲染（LED 仅在 `gui` 特性下）
//! - `theme`    : 皮肤 / 主题
//! - `config`   : 配置持久化
//!
//! 仅纯逻辑（不含 iced GUI）可在 `--no-default-features` 下编译与单元测试：
//!
//! ```bash
//! cargo build --lib --no-default-features
//! cargo test  --lib --no-default-features
//! ```
//!
//! 完整 GUI（含 托盘、迷你窗口）需默认特性 `gui`：
//!
//! ```bash
//! cargo build --release
//! cargo run --release
//! ```

pub mod audio;
pub mod cache;
pub mod config;
pub mod error;
pub mod lyrics;
pub mod playlist;
pub mod theme;
pub mod visualizer;
pub mod webdav;

// ── GUI 相关模块（仅在 gui 特性下编译）──
#[cfg(feature = "gui")]
pub mod app;
#[cfg(feature = "gui")]
pub mod ui;
