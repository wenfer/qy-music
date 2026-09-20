//! 频谱可视化模块。
//!
//! - `fft`    : rustfft 实 FFT + Hann 窗
//! - `spectrum`: 频段对数分块映射 + LED 段
//! - `led`    : iced canvas LED 渲染（仅 `gui` 特性下编译）

pub mod fft;
pub mod spectrum;

#[cfg(feature = "gui")]
pub mod led;

pub use spectrum::SpectrumData;
