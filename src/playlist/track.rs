//! 曲目数据结构。

use std::path::PathBuf;
use std::time::Duration;

/// 单首曲目。
#[derive(Clone, Debug)]
pub struct Track {
    /// 唯一标识（默认取路径字符串，保证同文件去重）。
    pub id: String,
    /// 音频文件绝对 / 相对路径。
    pub path: PathBuf,
    /// 标题（缺失时回退为文件名）。
    pub title: String,
    /// 艺术家（可能为空）。
    pub artist: String,
    /// 专辑（可能为空）。
    pub album: Option<String>,
    /// 时长。
    pub duration: Duration,
    /// 封面图（可选，暂未填充）。
    pub cover: Option<Vec<u8>>,
}

impl Track {
    /// 由路径构造空元数据曲目（标题回退为文件名）。
    pub fn from_path(path: PathBuf) -> Self {
        let title = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("未知曲目")
            .to_string();
        let id = path.to_string_lossy().into_owned();
        Self {
            id,
            path,
            title,
            artist: String::new(),
            album: None,
            duration: Duration::ZERO,
            cover: None,
        }
    }

    /// 显示名：`艺术家 - 标题` 或仅 `标题`。
    pub fn display_name(&self) -> String {
        if self.artist.is_empty() {
            self.title.clone()
        } else {
            format!("{} - {}", self.artist, self.title)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_path_uses_file_stem_as_title() {
        let t = Track::from_path(PathBuf::from("/music/My Song.mp3"));
        assert_eq!(t.title, "My Song");
        assert_eq!(t.id, "/music/My Song.mp3");
        assert!(t.artist.is_empty());
        assert!(t.album.is_none());
        assert_eq!(t.duration, Duration::ZERO);
    }

    #[test]
    fn display_name_returns_title_when_artist_empty() {
        let t = Track::from_path(PathBuf::from("/m/a.flac"));
        assert_eq!(t.display_name(), "a");
    }

    #[test]
    fn display_name_combines_artist_and_title() {
        let mut t = Track::from_path(PathBuf::from("/m/a.flac"));
        t.artist = "歌手".to_string();
        assert_eq!(t.display_name(), "歌手 - a");
    }

    #[test]
    fn from_path_without_stem_falls_back() {
        // 以 ".." 结尾时 file_stem 为空 → 回退占位标题，且不 panic。
        let t = Track::from_path(PathBuf::from("/music/"));
        assert!(!t.title.is_empty());
    }
}
