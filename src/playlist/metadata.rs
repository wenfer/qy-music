//! 标签元数据读取（基于 lofty，避免整曲解码）。
//!
//! 仅抽取标题 / 艺术家 / 专辑 / 时长，不读取封面（封面字段暂留空，
//! 后续可扩展）。

use std::path::Path;

use crate::error::{LingfengError, Result};
use crate::playlist::track::Track;

/// 探测到的文件元数据。
struct ProbedMetadata {
    title: Option<String>,
    artist: Option<String>,
    album: Option<String>,
    duration: Option<std::time::Duration>,
    is_lossless: Option<bool>,
}

/// 读取音频文件元数据并填充 [`Track`]。
///
/// 读取失败时回退为 [`Track::from_path`]（标题取文件名），不报错中断导入。
pub fn read_metadata(path: &Path) -> Track {
    let mut track = Track::from_path(path.to_path_buf());
    match probe_metadata(path) {
        Ok(meta) => {
            if let Some(t) = meta.title {
                if !t.is_empty() {
                    track.title = t;
                }
            }
            if let Some(a) = meta.artist {
                track.artist = a;
            }
            track.album = meta.album;
            if let Some(d) = meta.duration {
                track.duration = d;
            }
            if let Some(l) = meta.is_lossless {
                track.is_lossless = l;
            }
        }
        Err(e) => {
            log::warn!("读取元数据失败 {}: {}", path.display(), e);
        }
    }
    track
}

/// 探测元数据，返回 `ProbedMetadata`。
fn probe_metadata(path: &Path) -> Result<ProbedMetadata> {
    use lofty::file::{AudioFile, FileType, TaggedFileExt};
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
            tag.get_string(ItemKey::TrackTitle).map(str::to_string),
            tag.get_string(ItemKey::TrackArtist).map(str::to_string),
            tag.get_string(ItemKey::AlbumTitle).map(str::to_string),
        ),
        None => (None, None, None),
    };

    let duration = Some(tagged.properties().duration()).filter(|d| !d.is_zero());
    let is_lossless = match tagged.file_type() {
        FileType::Flac | FileType::Wav | FileType::Aiff | FileType::Ape => Some(true),
        FileType::Mpeg | FileType::Vorbis | FileType::Opus => Some(false),
        _ => Some(Track::detect_lossless(path)),
    };

    Ok(ProbedMetadata {
        title,
        artist,
        album,
        duration,
        is_lossless,
    })
}
