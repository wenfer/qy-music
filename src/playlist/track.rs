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
    /// 是否为无损音质格式（FLAC, WAV, APE, ALAC 等）。
    pub is_lossless: bool,
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
        let is_lossless = Self::detect_lossless(&path);
        Self {
            id,
            path,
            title,
            artist: String::new(),
            album: None,
            duration: Duration::ZERO,
            cover: None,
            is_lossless,
        }
    }

    /// 检测指定路径是否属于已知无损音频格式。
    pub fn detect_lossless(path: &std::path::Path) -> bool {
        let ext = path
            .extension()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        matches!(
            ext.as_str(),
            "flac"
                | "wav"
                | "wave"
                | "ape"
                | "aiff"
                | "aif"
                | "dsf"
                | "dff"
                | "wv"
                | "alac"
                | "pcm"
        )
    }

    /// 返回当前曲目是否为无损音频格式。
    pub fn is_lossless(&self) -> bool {
        self.is_lossless || Self::detect_lossless(&self.path)
    }

    /// 是否为远程 WebDAV / 网络流媒体曲目。
    pub fn is_remote(&self) -> bool {
        let s = self.path.to_string_lossy();
        s.starts_with("http://") || s.starts_with("https://") || s.starts_with("webdav://")
    }

    /// 提取远程 URL（如果属于网络资源）。
    pub fn remote_url(&self) -> Option<&str> {
        let s = self.path.to_str()?;
        if s.starts_with("http://") || s.starts_with("https://") || s.starts_with("webdav://") {
            Some(s)
        } else {
            None
        }
    }

    /// 返回格式大写简称（如 FLAC, WAV, MP3 等）。
    pub fn format_name(&self) -> &'static str {
        let ext = self
            .path
            .extension()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        match ext.as_str() {
            "flac" => "FLAC",
            "wav" | "wave" => "WAV",
            "ape" => "APE",
            "alac" => "ALAC",
            "aiff" | "aif" => "AIFF",
            "dsf" | "dff" => "DSD",
            "wv" => "WV",
            "pcm" => "PCM",
            "mp3" => "MP3",
            "ogg" | "oga" => "OGG",
            "opus" => "Opus",
            "m4a" => "M4A",
            "aac" => "AAC",
            "wma" => "WMA",
            _ => "AUDIO",
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

    #[test]
    fn test_lossless_and_format_detection() {
        let flac = Track::from_path(PathBuf::from("/music/song.flac"));
        assert!(flac.is_lossless());
        assert_eq!(flac.format_name(), "FLAC");

        let wav = Track::from_path(PathBuf::from("/music/song.wav"));
        assert!(wav.is_lossless());
        assert_eq!(wav.format_name(), "WAV");

        let ape = Track::from_path(PathBuf::from("/music/song.ape"));
        assert!(ape.is_lossless());
        assert_eq!(ape.format_name(), "APE");

        let mp3 = Track::from_path(PathBuf::from("/music/song.mp3"));
        assert!(!mp3.is_lossless());
        assert_eq!(mp3.format_name(), "MP3");

        let ogg = Track::from_path(PathBuf::from("/music/song.ogg"));
        assert!(!ogg.is_lossless());
        assert_eq!(ogg.format_name(), "OGG");
    }
}
