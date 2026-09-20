//! LRC 歌词解析器。
//!
//! 支持：
//! - `[mm:ss.xx]` / `[mm:ss]` 时间戳（`.xx` 可为 2 位百分秒或 3 位毫秒）
//! - 一行多时间戳（`[00:10.00][00:40.00]text` → 多个时间点共享同文本）
//! - 元数据标签 `[ti:]` / `[ar:]` / `[al:]` / `[offset:]`（毫秒）
//! - 编码回退：先按 UTF-8，失败按 GB18030（兼容 GBK）
//!
//! 解析得到的行按时间排序，便于二分定位当前句。

use std::time::Duration;

use crate::error::Result;

/// 单行歌词（时间点 + 文本）。
#[derive(Clone, Debug, PartialEq)]
pub struct LrcLine {
    /// 时间点。
    pub time: Duration,
    /// 文本内容。
    pub text: String,
}

/// 解析后的 LRC。
#[derive(Clone, Debug, Default)]
pub struct Lrc {
    /// 按时间升序排列的歌词行。
    pub lines: Vec<LrcLine>,
    /// 标题（来自 `[ti:]`）。
    pub title: Option<String>,
    /// 艺术家（来自 `[ar:]`）。
    pub artist: Option<String>,
}

impl Lrc {
    /// 解析 LRC 字节。
    ///
    /// `bytes` 先按 UTF-8 解释，失败回退 GB18030。
    pub fn parse(bytes: &[u8]) -> Result<Lrc> {
        let text = match std::str::from_utf8(bytes) {
            Ok(s) => s.to_string(),
            Err(_) => {
                let cow = encoding_rs::GB18030.decode_without_bom_handling(bytes);
                cow.0.to_string()
            }
        };

        let mut lrc = Lrc::default();
        let mut offset_ms: i64 = 0;

        for raw_line in text.lines() {
            let line = raw_line.trim_end();
            if line.is_empty() {
                continue;
            }
            let mut rest = line;
            let mut times: Vec<Duration> = Vec::new();
            let mut is_meta = false;

            // 提取行首所有 [..] 标签
            while rest.starts_with('[') {
                let end = match rest.find(']') {
                    Some(i) => i,
                    None => break,
                };
                let content = &rest[1..end];
                rest = &rest[end + 1..];

                if let Some((key, value)) = split_meta(content) {
                    is_meta = true;
                    match key {
                        "ti" => lrc.title = Some(value.to_string()),
                        "ar" => lrc.artist = Some(value.to_string()),
                        "al" => { /* 专辑暂不单独存，可扩展 */ }
                        "offset" => {
                            if let Ok(o) = value.parse::<i64>() {
                                offset_ms = o;
                            }
                        }
                        _ => {}
                    }
                } else if let Some(d) = parse_timestamp(content) {
                    times.push(d);
                }
            }

            let text_content = rest.trim().to_string();
            if is_meta && times.is_empty() {
                // 纯元数据行，无歌词
                continue;
            }
            if times.is_empty() {
                // 无时间戳的行忽略（非歌词）
                continue;
            }
            for t in times {
                let adjusted = apply_offset(t, offset_ms);
                lrc.lines.push(LrcLine {
                    time: adjusted,
                    text: text_content.clone(),
                });
            }
        }

        lrc.lines.sort_by_key(|l| l.time);
        Ok(lrc)
    }

    /// 返回时间点 `<= pos` 的最后一行下标（用于高亮当前句）。
    ///
    /// 若所有行时间都大于 `pos`，返回 0；若为空返回 0。
    pub fn current_line(&self, pos: Duration) -> usize {
        if self.lines.is_empty() {
            return 0;
        }
        let mut lo = 0usize;
        let mut hi = self.lines.len();
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            if self.lines[mid].time <= pos {
                lo = mid + 1;
            } else {
                hi = mid;
            }
        }
        let idx = lo.saturating_sub(1);
        idx.min(self.lines.len() - 1)
    }

    /// 是否存在有效歌词。
    pub fn is_empty(&self) -> bool {
        self.lines.is_empty()
    }
}

/// 拆分元数据标签 `key:value`。
fn split_meta(content: &str) -> Option<(&str, &str)> {
    let idx = content.find(':')?;
    let key = &content[..idx];
    let value = &content[idx + 1..];
    // 仅当 key 为已知元数据且 value 不含数字时间戳特征时视为元数据
    match key {
        "ti" | "ar" | "al" | "offset" | "by" | "au" => Some((key, value)),
        _ => None,
    }
}

/// 解析时间戳 `mm:ss.xx`（xx 可选，2/3 位）。
fn parse_timestamp(content: &str) -> Option<Duration> {
    let (mm_ss, frac) = match content.find(':') {
        Some(i) => (&content[..i], Some(&content[i + 1..])),
        None => (content, None),
    };
    let minutes: u64 = mm_ss.parse().ok()?;
    let secs_str = frac.unwrap_or("0");
    // 秒可能为整数或带小数（`10.00`）
    let (secs_part, sub_part) = match secs_str.find('.') {
        Some(i) => (&secs_str[..i], Some(&secs_str[i + 1..])),
        None => (secs_str, None),
    };
    let secs: u64 = secs_part.parse().ok()?;
    let mut total_ms = minutes * 60_000 + secs * 1000;
    if let Some(f) = sub_part {
        // 小数秒：2 位视为百分秒，>=3 位取前 3 位视为毫秒
        let digits = &f[..f.len().min(3)];
        if let Ok(c) = digits.parse::<u64>() {
            if f.len() == 2 {
                total_ms += c * 10;
            } else {
                total_ms += c;
            }
        }
    }
    Some(Duration::from_millis(total_ms))
}

/// 应用全局 offset：正值偏移使歌词提前显示（按 LRC 规范 `time - offset`）。
fn apply_offset(t: Duration, offset_ms: i64) -> Duration {
    if offset_ms >= 0 {
        t.checked_sub(Duration::from_millis(offset_ms as u64))
            .unwrap_or(Duration::ZERO)
    } else {
        t + Duration::from_millis((-offset_ms) as u64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_basic() {
        let lrc = Lrc::parse(
            b"[ti:Demo]\n[ar:Tester]\n[00:10.00]Hello\n[00:20.50]World\n",
        )
        .unwrap();
        assert_eq!(lrc.title.as_deref(), Some("Demo"));
        assert_eq!(lrc.artist.as_deref(), Some("Tester"));
        assert_eq!(lrc.lines.len(), 2);
        assert_eq!(lrc.lines[0].text, "Hello");
        assert_eq!(lrc.lines[0].time, Duration::from_millis(10_000));
        assert_eq!(lrc.lines[1].time, Duration::from_millis(20_500));
    }

    #[test]
    fn parse_multi_timestamp_same_text() {
        let lrc = Lrc::parse(b"[00:10.00][00:40.00]Repeat me\n").unwrap();
        assert_eq!(lrc.lines.len(), 2);
        assert_eq!(lrc.lines[0].text, "Repeat me");
        assert_eq!(lrc.lines[1].text, "Repeat me");
        assert_eq!(lrc.lines[1].time, Duration::from_millis(40_000));
    }

    #[test]
    fn parse_gb18030_fallback() {
        // “歌词” 的 GBK 编码：歌 = B8 E8，词 = B4 CA
        let gbk = [0xB8u8, 0xE8u8, 0xB4u8, 0xCAu8];
        let mut bytes = b"[00:01.00]".to_vec();
        bytes.extend_from_slice(&gbk);
        bytes.push(b'\n');
        let lrc = Lrc::parse(&bytes).unwrap();
        assert_eq!(lrc.lines.len(), 1);
        // GB18030 解码 “歌词”
        assert_eq!(lrc.lines[0].text, "歌词");
    }

    #[test]
    fn current_line_binary_search() {
        let lrc = Lrc::parse(
            b"[00:10.00]A\n[00:20.00]B\n[00:30.00]C\n",
        )
        .unwrap();
        assert_eq!(lrc.current_line(Duration::from_millis(0)), 0);
        assert_eq!(lrc.current_line(Duration::from_millis(15_000)), 0);
        assert_eq!(lrc.current_line(Duration::from_millis(20_000)), 1);
        assert_eq!(lrc.current_line(Duration::from_millis(25_000)), 1);
        assert_eq!(lrc.current_line(Duration::from_millis(99_000)), 2);
    }

    #[test]
    fn offset_shifts_earlier() {
        let lrc = Lrc::parse(b"[offset:500]\n[00:10.00]Hi\n").unwrap();
        // 应用 offset +500ms → 实际时间 9500ms
        assert_eq!(lrc.lines[0].time, Duration::from_millis(9_500));
    }

    // ──────────────────────────────────────────────────────────────
    // QA 补充测试（独立验证：边界 / 错误路径）
    // ──────────────────────────────────────────────────────────────

    #[test]
    fn parse_empty_file_has_no_lines() {
        let lrc = Lrc::parse(b"").unwrap();
        assert!(lrc.is_empty());
        assert_eq!(lrc.lines.len(), 0);
        assert!(lrc.title.is_none());
    }

    #[test]
    fn parse_only_blank_lines_has_no_lines() {
        let lrc = Lrc::parse(b"\n\n   \n\t\n").unwrap();
        assert!(lrc.is_empty());
    }

    #[test]
    fn parse_lines_without_timestamp_are_ignored() {
        let lrc = Lrc::parse("不加时间戳的纯文本\n第二行\n".as_bytes()).unwrap();
        assert!(lrc.is_empty(), "无时间戳的行不应生成歌词行");
    }

    #[test]
    fn parse_meta_only_file_is_empty() {
        let lrc = Lrc::parse(b"[ti:Only Title]\n[ar:Only Artist]\n").unwrap();
        assert!(lrc.is_empty());
        assert_eq!(lrc.title.as_deref(), Some("Only Title"));
        assert_eq!(lrc.artist.as_deref(), Some("Only Artist"));
    }

    #[test]
    fn parse_invalid_timestamp_format_is_ignored() {
        // [xx:yy] 非法时间戳：既非已知元数据，也解析不出时间 → 整行忽略。
        let lrc = Lrc::parse(b"[xx:yy]Invalid\n[1a:2b]Also bad\n").unwrap();
        assert!(lrc.is_empty(), "非法时间戳行应被忽略");
    }

    #[test]
    fn parse_negative_timestamp_is_ignored() {
        // 负时间戳无法用 u64 解析 → 该行忽略（不 panic）。
        let lrc = Lrc::parse(b"[-00:10.00]Negative\n").unwrap();
        assert!(lrc.is_empty(), "负时间戳行应被忽略而非崩溃");
    }

    #[test]
    fn parse_large_timestamp_ok() {
        // 超大（但合法）时间：120 分钟 = 7_200_000 ms。
        let lrc = Lrc::parse(b"[120:00.00]Long\n").unwrap();
        assert_eq!(lrc.lines.len(), 1);
        assert_eq!(lrc.lines[0].time, Duration::from_millis(7_200_000));
    }

    #[test]
    fn parse_timestamp_three_digit_millis() {
        // .123 视为毫秒
        let lrc = Lrc::parse(b"[00:10.123]Ms\n").unwrap();
        assert_eq!(lrc.lines[0].time, Duration::from_millis(10_123));
    }

    #[test]
    fn parse_same_time_multiple_lines_keeps_last_for_current_line() {
        // 同一时间多行：两行都保留；current_line 返回该时刻的最后一行。
        let lrc = Lrc::parse(b"[00:10.00]First\n[00:10.00]Second\n").unwrap();
        assert_eq!(lrc.lines.len(), 2);
        assert_eq!(lrc.current_line(Duration::from_millis(10_000)), 1);
        assert_eq!(lrc.lines[1].text, "Second");
    }

    #[test]
    fn parse_multi_timestamp_line_is_sorted() {
        // 一行多时间戳，且时间戳未按顺序书写 → 结果按时间升序。
        let lrc = Lrc::parse(b"[00:40.00][00:10.00]Shared\n").unwrap();
        assert_eq!(lrc.lines.len(), 2);
        assert_eq!(lrc.lines[0].time, Duration::from_millis(10_000));
        assert_eq!(lrc.lines[1].time, Duration::from_millis(40_000));
        assert_eq!(lrc.lines[0].text, "Shared");
        assert_eq!(lrc.lines[1].text, "Shared");
    }

    #[test]
    fn offset_negative_shifts_later() {
        let lrc = Lrc::parse(b"[offset:-500]\n[00:10.00]Hi\n").unwrap();
        // offset -500ms → 实际时间延后到 10500ms
        assert_eq!(lrc.lines[0].time, Duration::from_millis(10_500));
    }

    #[test]
    fn offset_larger_than_time_saturates_to_zero() {
        // offset 大于行时间：checked_sub 饱和到 0（不 panic / 不回绕）。
        let lrc = Lrc::parse(b"[offset:20000]\n[00:10.00]Hi\n").unwrap();
        assert_eq!(lrc.lines[0].time, Duration::ZERO);
    }

    #[test]
    fn offset_applies_to_following_lines() {
        let lrc = Lrc::parse(b"[offset:1000]\n[00:10.00]A\n[00:20.00]B\n").unwrap();
        assert_eq!(lrc.lines[0].time, Duration::from_millis(9_000));
        assert_eq!(lrc.lines[1].time, Duration::from_millis(19_000));
    }

    #[test]
    fn parse_gb18030_fallback_longer_text() {
        // “测试歌词” 的 GBK/GB18030 字节：测(B2 E2) 试(CA D4) 歌(B8 E8) 词(B4 CA)
        let gbk = [0xB2u8, 0xE2u8, 0xCAu8, 0xD4u8, 0xB8u8, 0xE8u8, 0xB4u8, 0xCAu8];
        let mut bytes = b"[00:03.00]".to_vec();
        bytes.extend_from_slice(&gbk);
        bytes.push(b'\n');
        let lrc = Lrc::parse(&bytes).unwrap();
        assert_eq!(lrc.lines.len(), 1);
        assert_eq!(lrc.lines[0].text, "测试歌词");
    }

    #[test]
    fn current_line_empty_returns_zero() {
        let lrc = Lrc::default();
        assert_eq!(lrc.current_line(Duration::from_secs(10)), 0);
    }

    #[test]
    fn current_line_before_first_and_after_last() {
        let lrc = Lrc::parse(b"[00:10.00]A\n[00:20.00]B\n").unwrap();
        // 早于第一行 → 仍返回 0（与文档一致）
        assert_eq!(lrc.current_line(Duration::from_millis(1)), 0);
        // 正好等于最后一行
        assert_eq!(lrc.current_line(Duration::from_millis(20_000)), 1);
        // 末行之后
        assert_eq!(lrc.current_line(Duration::from_secs(3_600)), 1);
    }
}
