//! 歌词模块。

pub mod lrc;
pub mod matcher;
pub mod sync;

pub use lrc::{Lrc, LrcLine};
pub use matcher::load_for_track;
pub use sync::current_line;
