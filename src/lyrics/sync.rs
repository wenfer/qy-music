//! 歌词同步：根据播放位置 + 手动偏移计算当前句。
//!
//! `lyric_offset_ms` 为用户手动校准值（正表示歌词延后显示）。

use std::time::Duration;

use crate::lyrics::lrc::Lrc;

/// 计算当前应高亮的歌词行下标。
///
/// `pos` 为播放位置，`lyric_offset_ms` 为用户校准偏移（毫秒，正 = 延后）。
pub fn current_line(lrc: &Lrc, pos: Duration, lyric_offset_ms: i64) -> usize {
    let adjusted = if lyric_offset_ms >= 0 {
        pos + Duration::from_millis(lyric_offset_ms as u64)
    } else {
        pos.checked_sub(Duration::from_millis((-lyric_offset_ms) as u64))
            .unwrap_or(Duration::ZERO)
    };
    lrc.current_line(adjusted)
}

/// 返回当前句与下一句（用于滚动预览），无下一句时返回 `None`。
pub fn current_and_next(lrc: &Lrc, pos: Duration, lyric_offset_ms: i64) -> (usize, Option<usize>) {
    let idx = current_line(lrc, pos, lyric_offset_ms);
    let next = if idx + 1 < lrc.lines.len() {
        Some(idx + 1)
    } else {
        None
    };
    (idx, next)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Lrc {
        // 10s / 20s / 30s 三行
        Lrc::parse(b"[00:10.00]A\n[00:20.00]B\n[00:30.00]C\n").unwrap()
    }

    #[test]
    fn zero_offset_matches_plain_current_line() {
        let lrc = sample();
        assert_eq!(current_line(&lrc, Duration::from_millis(15_000), 0), 0);
        assert_eq!(current_line(&lrc, Duration::from_millis(20_000), 0), 1);
        assert_eq!(current_line(&lrc, Duration::from_millis(35_000), 0), 2);
    }

    #[test]
    fn positive_offset_delays_highlight() {
        // 正偏移 = 歌词延后显示：把播放位置“提前”查表。
        let lrc = sample();
        // pos=9500 + 600ms → 10100 → 落在第一行
        assert_eq!(current_line(&lrc, Duration::from_millis(9_500), 600), 0);
        // pos=9500 无偏移时也在第 0 行（对照）
        assert_eq!(current_line(&lrc, Duration::from_millis(9_500), 0), 0);
        // pos=19500 + 600 → 20100 → 第 1 行
        assert_eq!(current_line(&lrc, Duration::from_millis(19_500), 600), 1);
    }

    #[test]
    fn negative_offset_advances_and_saturates_at_zero() {
        let lrc = sample();
        // 负偏移 = 歌词提前显示：把播放位置“推后”查表。
        // pos=10500 - 600 → 9900 → 早于第一行 → 仍为第 0 行。
        assert_eq!(current_line(&lrc, Duration::from_millis(10_500), -600), 0);
        // pos=20500 - 600 → 19900 → 第 0 行（未到 20s）。
        assert_eq!(current_line(&lrc, Duration::from_millis(20_500), -600), 0);
        // 位置小于负偏移量：checked_sub 饱和到 0，不 panic。
        assert_eq!(current_line(&lrc, Duration::from_millis(100), -10_000), 0);
    }

    #[test]
    fn current_and_next_middle_has_next() {
        let lrc = sample();
        let (idx, next) = current_and_next(&lrc, Duration::from_millis(20_000), 0);
        assert_eq!(idx, 1);
        assert_eq!(next, Some(2));
    }

    #[test]
    fn current_and_next_last_line_has_no_next() {
        let lrc = sample();
        let (idx, next) = current_and_next(&lrc, Duration::from_secs(31), 0);
        assert_eq!(idx, 2);
        assert_eq!(next, None);
    }

    #[test]
    fn current_and_next_empty_lrc() {
        let lrc = Lrc::default();
        let (idx, next) = current_and_next(&lrc, Duration::from_secs(1), 0);
        assert_eq!(idx, 0);
        assert_eq!(next, None);
    }
}
