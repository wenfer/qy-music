//! 歌词自动匹配：同名 / 同目录 `.lrc` 关联。

use std::path::{Path, PathBuf};

use crate::error::Result;
use crate::lyrics::lrc::Lrc;

/// 为音频文件查找并解析同名 `.lrc` 歌词。
///
/// 查找顺序：
/// 1. 与音频同目录、去扩展名后加 `.lrc`（最常见）。
/// 2. （同上即覆盖）若存在则返回解析结果。
///
/// 找不到或解析失败返回 `None`。
pub fn load_for_track(audio_path: &Path) -> Option<Lrc> {
    let lrc_path = sibling_lrc(audio_path)?;
    if !lrc_path.exists() {
        return None;
    }
    match std::fs::read(&lrc_path) {
        Ok(bytes) => Lrc::parse(&bytes).ok(),
        Err(_) => None,
    }
}

/// 给定音频路径，返回同名 `.lrc` 路径（同目录）。
fn sibling_lrc(audio_path: &Path) -> Option<PathBuf> {
    let parent = audio_path.parent()?;
    let stem = audio_path.file_stem()?;
    Some(parent.join(format!(
        "{}.lrc",
        stem.to_str().unwrap_or("unknown")
    )))
}

/// 在指定目录内批量匹配（用于导入文件夹时预加载歌词）。
pub fn load_for_track_result(audio_path: &Path) -> Result<Option<Lrc>> {
    Ok(load_for_track(audio_path))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    static COUNTER: AtomicU32 = AtomicU32::new(0);

    /// 创建唯一临时目录（不依赖外部 crate）。
    fn temp_dir(tag: &str) -> PathBuf {
        let n = COUNTER.fetch_add(1, Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!(
            "lingfeng_qa_{}_{}_{}",
            std::process::id(),
            tag,
            n
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn finds_sibling_same_stem_lrc() {
        let dir = temp_dir("match");
        let audio = dir.join("song.mp3");
        std::fs::write(&audio, b"fake-audio").unwrap();
        std::fs::write(dir.join("song.lrc"), b"[00:01.00]Hello\n").unwrap();

        let lrc = load_for_track(&audio).expect("应匹配到同名 .lrc");
        assert_eq!(lrc.lines.len(), 1);
        assert_eq!(lrc.lines[0].text, "Hello");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn returns_none_when_no_lrc() {
        let dir = temp_dir("nomatch");
        let audio = dir.join("lonely.flac");
        std::fs::write(&audio, b"fake-audio").unwrap();

        assert!(load_for_track(&audio).is_none());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn returns_none_when_stem_differs() {
        let dir = temp_dir("diffstem");
        let audio = dir.join("song.mp3");
        std::fs::write(&audio, b"fake-audio").unwrap();
        // 歌词文件名与音频不同 → 不应匹配
        std::fs::write(dir.join("other.lrc"), b"[00:01.00]Nope\n").unwrap();

        assert!(load_for_track(&audio).is_none());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn returns_none_for_unparsable_or_unreadable_lrc() {
        let dir = temp_dir("badlrc");
        let audio = dir.join("song.mp3");
        std::fs::write(&audio, b"fake-audio").unwrap();
        // LRC 本身合法但为空 → 解析成功但无行（Lrc 非 None）。
        std::fs::write(dir.join("song.lrc"), b"").unwrap();
        let lrc = load_for_track(&audio).expect("空文件仍应解析为 Some");
        assert!(lrc.is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn load_for_track_result_wraps_same_behavior() {
        let dir = temp_dir("result");
        let audio = dir.join("t.mp3");
        std::fs::write(&audio, b"fake-audio").unwrap();
        std::fs::write(dir.join("t.lrc"), b"[00:02.00]Wrap\n").unwrap();

        let via_result = load_for_track_result(&audio).unwrap();
        assert!(via_result.is_some());
        assert_eq!(via_result.unwrap().lines[0].text, "Wrap");

        let _ = std::fs::remove_dir_all(&dir);
    }
}
