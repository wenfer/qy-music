//! 统一错误类型定义。
//!
//! 所有模块函数返回 [`Result<T, LingfengError>`]，UI 层通过
//! [`LingfengError`] 转译为状态提示，绝不 panic。

use thiserror::Error;

/// 播放器统一错误类型。
#[derive(Debug, Error)]
pub enum LingfengError {
    /// I/O 错误（文件读写、目录创建等）。
    #[error("I/O 错误: {0}")]
    Io(#[from] std::io::Error),

    /// JSON 序列化 / 反序列化错误。
    #[error("JSON 错误: {0}")]
    Json(#[from] serde_json::Error),

    /// Symphonia 解码错误。
    #[error("解码错误(symphonia): {0}")]
    Symphonia(String),

    /// cpal 音频输出错误。
    #[error("音频输出错误(cpal): {0}")]
    Cpal(String),

    /// LRC / 文本解析错误。
    #[error("解析错误: {0}")]
    Parse(String),

    /// 音频事件 / 命令通道已关闭。
    #[error("内部通道已关闭")]
    ChannelClosed,

    /// 未找到匹配资源（如 .lrc 歌词文件）。
    #[error("未找到资源: {0}")]
    NotFound(String),

    /// 其它通用错误。
    #[error("{0}")]
    Other(String),
}

impl LingfengError {
    /// 构造通用错误。
    pub fn other(msg: impl Into<String>) -> Self {
        Self::Other(msg.into())
    }

    /// 构造解析错误。
    pub fn parse(msg: impl Into<String>) -> Self {
        Self::Parse(msg.into())
    }
}

/// Symphonia 错误 → [`LingfengError`] 的桥接。
impl From<symphonia::core::errors::Error> for LingfengError {
    fn from(e: symphonia::core::errors::Error) -> Self {
        Self::Symphonia(e.to_string())
    }
}

/// cpal 各子类错误统一桥接为 [`LingfengError::Cpal`]。
macro_rules! impl_cpal_err {
    ($($t:ty),* $(,)?) => {
        $(
            impl From<$t> for LingfengError {
                fn from(e: $t) -> Self {
                    Self::Cpal(e.to_string())
                }
            }
        )*
    };
}

impl_cpal_err!(
    cpal::DevicesError,
    cpal::DefaultStreamConfigError,
    cpal::SupportedStreamConfigsError,
    cpal::BuildStreamError,
    cpal::PlayStreamError,
    cpal::PauseStreamError,
    cpal::StreamError,
);

/// 便捷结果别名。
pub type Result<T> = std::result::Result<T, LingfengError>;
