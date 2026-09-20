//! 标签元数据读取（基于 lofty，避免整曲解码）。
//!
//! 仅抽取标题 / 艺术家 / 专辑 / 时长，不读取封面（封面字段暂留空，
//! 后续可扩展）。

use std::path::Path;

use crate::error::{LingfengError, Result};
use crate::playlist::track::Track;

/// 读取音频文件元数据并填充 [`Track`]。
///
/// 读取失败时回退为 [`Track::from_path`]（标题取文件名），不报错中断导入。
pub fn read_metadata(path: &Path) -> Track {
    let mut track = Track::from_path(path.to_path_buf());
    match probe_metadata(path) {
        Ok((title, artist, album, duration)) => {
            if let Some(t) = title {
                if !t.is_empty() {
                    track.title = t;
                }
            }
            if let Some(a) = artist {
                track.artist = a;
            }
            track.album = album;
            if let Some(d) = duration {
                track.duration = d;
            }
        }
        Err(e) => {
            log::warn!("读取元数据失败 {}: {}", path.display(), e);
        }
    }
    track
}

/// 探测元数据，返回 (标题, 艺术家, 专辑, 时长)。
fn probe_metadata(
    path: &Path,
) -> Result<(
    Option<String>,
    Option<String>,
    Option<String>,
    Option<std::time::Duration>,
)> {
    use lofty::file::TaggedFileExt;
    use lofty::probe::Probe;
    use lofty::tag::ItemKey;

    let tagged = Probe::open(path)
        .map_err(|e| LingfengError::other(e.to_string()))?
        .guess_file_type()
        .map_err(|e| LingfengError::other(e.to_string()))?
        .read()
        .map_err(|e| LingfengError::other(e.to_string()))?;

    let (title, artist, album) = match tagged.primary_tag().or_else(|| tagged.first_tag()) {
        Some(tag) => (
            tag.get_string(&ItemKey::TrackTitle).map(str::to_string),
            tag.get_string(&ItemKey::TrackArtist).map(str::to_string),
            tag.get_string(&ItemKey::AlbumTitle).map(str::to_string),
        ),
        None => (None, None, None),
    };

    // TaggedFile 不直接暴露时长；播放时由解码器补充 `Track.duration`。
    let duration = None;
    Ok((title, artist, album, duration))
}
