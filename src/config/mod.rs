//! 配置模块。

pub mod persist;
pub mod settings;

pub use settings::{
    DEFAULT_WINDOW_HEIGHT, DEFAULT_WINDOW_WIDTH, Settings, WindowSize,
};
