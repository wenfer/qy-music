//! 播放列表模块。

pub mod metadata;
#[allow(clippy::module_inception)]
pub mod playlist;
pub mod track;

pub use playlist::{LoopMode, Playlist};
pub use track::Track;
