//! 播放列表：曲目集合 + 队列 + 循环模式 + 选曲。
//!
//! 对应类图 [`Playlist`] / [`LoopMode`]。纯逻辑，便于单元测试。

use std::collections::VecDeque;
use std::fmt;
use std::path::PathBuf;

use rand::seq::SliceRandom;
use rand::thread_rng;

use crate::playlist::metadata::read_metadata;
use crate::playlist::track::Track;

/// 循环 / 顺序模式。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LoopMode {
    /// 单曲循环。
    Single,
    /// 列表循环（播完回到开头）。
    List,
    /// 随机（不重复不遗漏）。
    Random,
    /// 顺序（播完停止）。
    Sequential,
}

impl LoopMode {
    /// 全部模式（供 UI 下拉列表枚举）。
    pub const ALL: [LoopMode; 4] = [
        LoopMode::Single,
        LoopMode::List,
        LoopMode::Random,
        LoopMode::Sequential,
    ];

    /// 返回人类可读名称（中文）。
    pub fn label(self) -> &'static str {
        match self {
            LoopMode::Single => "单曲循环",
            LoopMode::List => "列表循环",
            LoopMode::Random => "随机",
            LoopMode::Sequential => "顺序",
        }
    }
}

impl fmt::Display for LoopMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

impl Default for LoopMode {
    fn default() -> Self {
        Self::List
    }
}

/// 播放列表。
#[derive(Clone, Debug, Default)]
pub struct Playlist {
    /// 曲目集合。
    pub tracks: Vec<Track>,
    /// 独立「播放队列」（优先于循环逻辑）。存放曲目下标。
    pub queue: VecDeque<usize>,
    /// 当前曲目下标。
    pub current_index: Option<usize>,
    /// 循环模式。
    pub loop_mode: LoopMode,
    /// 随机模式剩余待播下标（不重复不遗漏）。
    remaining: Vec<usize>,
}

impl Playlist {
    /// 构造空列表。
    pub fn new() -> Self {
        Self::default()
    }

    /// 添加单个文件（读取元数据）。
    pub fn add(&mut self, path: PathBuf) {
        let track = read_metadata(&path);
        self.tracks.push(track);
        if self.current_index.is_none() {
            self.current_index = Some(0);
        }
        self.remaining.clear();
    }

    /// 批量添加。
    pub fn add_many(&mut self, paths: Vec<PathBuf>) {
        for p in paths {
            self.add(p);
        }
    }

    /// 移除指定下标的曲目，并修正当前下标与队列。
    pub fn remove(&mut self, i: usize) {
        if i >= self.tracks.len() {
            return;
        }
        self.tracks.remove(i);
        self.queue.clear();
        self.remaining.clear();
        match self.current_index {
            Some(cur) if cur == i => self.current_index = None,
            Some(cur) if cur > i => self.current_index = Some(cur - 1),
            _ => {}
        }
    }

    /// 移动曲目：把 `from` 移到 `to` 位置（播放列表排序 / 拖拽）。
    pub fn move_item(&mut self, from: usize, to: usize) {
        if from >= self.tracks.len() || to >= self.tracks.len() || from == to {
            return;
        }
        let item = self.tracks.remove(from);
        self.tracks.insert(to, item);
        self.queue.clear();
        self.remaining.clear();
        // 修正当前下标
        if let Some(cur) = self.current_index {
            self.current_index = Some(adjust_index(cur, from, to));
        }
    }

    /// 清空列表。
    pub fn clear(&mut self) {
        self.tracks.clear();
        self.queue.clear();
        self.remaining.clear();
        self.current_index = None;
    }

    /// 加入「播放队列」（下一首优先）。
    pub fn enqueue(&mut self, i: usize) {
        if i < self.tracks.len() {
            self.queue.push_back(i);
        }
    }

    /// 设置循环模式（切换为随机时清空剩余序列以便重新洗牌）。
    pub fn set_loop_mode(&mut self, mode: LoopMode) {
        self.loop_mode = mode;
        if mode == LoopMode::Random {
            self.remaining.clear();
        }
    }

    /// 当前曲目引用。
    pub fn current(&self) -> Option<&Track> {
        self.current_index.and_then(|i| self.tracks.get(i))
    }

    /// 设置当前下标。
    pub fn set_current(&mut self, i: Option<usize>) {
        self.current_index = i;
        self.remaining.clear();
    }

    /// 计算下一首下标（按队列与循环模式），并更新 `current_index`。
    pub fn next(&mut self) -> Option<usize> {
        if self.tracks.is_empty() {
            return None;
        }
        // 队列优先
        if let Some(i) = self.queue.pop_front() {
            if i < self.tracks.len() {
                self.current_index = Some(i);
                return Some(i);
            }
        }
        let idx = match self.loop_mode {
            LoopMode::Single => self.current_index,
            LoopMode::Sequential => {
                let cur = self.current_index?;
                if cur + 1 < self.tracks.len() {
                    Some(cur + 1)
                } else {
                    None // 到末尾停止
                }
            }
            LoopMode::List => {
                let cur = self.current_index.unwrap_or(0);
                Some((cur + 1) % self.tracks.len())
            }
            LoopMode::Random => self.next_random(),
        };
        self.current_index = idx;
        idx
    }

    /// 计算上一首下标，并更新 `current_index`。
    pub fn prev(&mut self) -> Option<usize> {
        if self.tracks.is_empty() {
            return None;
        }
        let idx = match self.loop_mode {
            LoopMode::Single => self.current_index,
            LoopMode::Sequential => {
                let cur = self.current_index?;
                if cur > 0 {
                    Some(cur - 1)
                } else {
                    None
                }
            }
            LoopMode::List => {
                let cur = self.current_index.unwrap_or(0);
                let n = self.tracks.len();
                Some((cur + n - 1) % n)
            }
            LoopMode::Random => self.prev_random(),
        };
        self.current_index = idx;
        idx
    }

    /// 随机模式：从剩余序列取下一首，耗尽则重新洗牌（保证不重复不遗漏）。
    ///
    /// 边界：当列表仅剩「当前一首」可播（如单曲列表 + 随机模式）时，
    /// `reshuffle` 会因排除当前曲而使 `remaining` 仍为空——此时重复当前曲，
    /// 避免对空 `Vec` 取下标记越界崩溃（BUG-001）。
    fn next_random(&mut self) -> Option<usize> {
        if self.remaining.is_empty() {
            self.reshuffle();
            if self.remaining.is_empty() {
                // 仅当前一首可播：返回当前曲（缺省 0），保证播放继续且不 panic。
                return self.current_index.or(Some(0));
            }
        }
        let n = self.remaining.remove(0);
        Some(n)
    }

    /// 随机模式：上一首（随机取一个非当前的曲目）。
    fn prev_random(&mut self) -> Option<usize> {
        let cur = self.current_index?;
        if self.tracks.len() == 1 {
            return Some(cur);
        }
        let mut candidates: Vec<usize> = (0..self.tracks.len()).filter(|&i| i != cur).collect();
        candidates.shuffle(&mut thread_rng());
        candidates.first().copied()
    }

    /// 重建随机序列：除当前外全部下标打乱。
    fn reshuffle(&mut self) {
        let mut v: Vec<usize> = (0..self.tracks.len()).collect();
        if let Some(c) = self.current_index {
            v.retain(|&x| x != c);
        }
        v.shuffle(&mut thread_rng());
        self.remaining = v;
    }
}

/// 移动元素后修正引用下标的辅助函数。
fn adjust_index(cur: usize, from: usize, to: usize) -> usize {
    if cur == from {
        to
    } else if from < to {
        if cur > from && cur <= to {
            cur - 1
        } else {
            cur
        }
    } else {
        // from > to
        if cur >= to && cur < from {
            cur + 1
        } else {
            cur
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dummy_paths(n: usize) -> Vec<PathBuf> {
        (0..n)
            .map(|i| PathBuf::from(format!("/tmp/track_{i}.flac")))
            .collect()
    }

    #[test]
    fn add_and_current() {
        let mut pl = Playlist::new();
        pl.add_many(dummy_paths(3));
        assert_eq!(pl.tracks.len(), 3);
        assert_eq!(pl.current_index, Some(0));
        assert_eq!(pl.current().unwrap().title, "track_0");
    }

    #[test]
    fn remove_adjusts_current() {
        let mut pl = Playlist::new();
        pl.add_many(dummy_paths(3));
        pl.set_current(Some(2));
        pl.remove(0);
        assert_eq!(pl.current_index, Some(1));
        assert_eq!(pl.tracks.len(), 2);
    }

    #[test]
    fn move_item_reorders() {
        let mut pl = Playlist::new();
        pl.add_many(dummy_paths(3));
        pl.move_item(0, 2); // track_0 -> 位置2
        assert_eq!(pl.tracks[2].title, "track_0");
        assert_eq!(pl.tracks[0].title, "track_1");
    }

    #[test]
    fn list_loop_wraps() {
        let mut pl = Playlist::new();
        pl.add_many(dummy_paths(3));
        pl.set_loop_mode(LoopMode::List);
        pl.set_current(Some(2));
        assert_eq!(pl.next(), Some(0));
        assert_eq!(pl.next(), Some(1));
    }

    #[test]
    fn sequential_stops_at_end() {
        let mut pl = Playlist::new();
        pl.add_many(dummy_paths(3));
        pl.set_loop_mode(LoopMode::Sequential);
        pl.set_current(Some(1));
        assert_eq!(pl.next(), Some(2));
        assert_eq!(pl.next(), None);
    }

    #[test]
    fn single_repeats() {
        let mut pl = Playlist::new();
        pl.add_many(dummy_paths(3));
        pl.set_loop_mode(LoopMode::Single);
        pl.set_current(Some(1));
        assert_eq!(pl.next(), Some(1));
        assert_eq!(pl.prev(), Some(1));
    }

    #[test]
    fn random_covers_all_without_repeat_until_exhausted() {
        let mut pl = Playlist::new();
        pl.add_many(dummy_paths(4));
        pl.set_loop_mode(LoopMode::Random);
        pl.set_current(Some(0));
        let mut seen = std::collections::HashSet::new();
        // 第一轮：1,2,3 应各出现一次（不含当前 0）
        for _ in 0..3 {
            let n = pl.next().unwrap();
            assert_ne!(n, 0, "随机下一首不应等于当前 0（首轮）");
            seen.insert(n);
        }
        assert_eq!(seen.len(), 3, "首轮应覆盖其余 3 首且不重复");
        // 第二轮应重新洗牌并继续
        let n = pl.next().unwrap();
        assert!(n < 4);
    }

    #[test]
    fn queue_takes_priority() {
        let mut pl = Playlist::new();
        pl.add_many(dummy_paths(3));
        pl.enqueue(2);
        assert_eq!(pl.next(), Some(2), "队列应优先于循环逻辑");
    }

    // ──────────────────────────────────────────────────────────────
    // QA 补充测试（边界 / 错误路径）
    // ──────────────────────────────────────────────────────────────

    #[test]
    fn empty_playlist_next_and_prev_are_none() {
        let mut pl = Playlist::new();
        assert_eq!(pl.next(), None);
        assert_eq!(pl.prev(), None);
        assert!(pl.current().is_none());
    }

    #[test]
    fn single_track_random_next_should_return_same_track() {
        // 边界：仅 1 首曲目 + 随机模式切下一首，应返回该曲（不应崩溃）。
        let mut pl = Playlist::new();
        pl.add_many(dummy_paths(1));
        pl.set_loop_mode(LoopMode::Random);
        assert_eq!(
            pl.next(),
            Some(0),
            "单曲列表随机模式下 next() 应返回该曲"
        );
    }

    #[test]
    fn single_track_sequential_next_is_none() {
        let mut pl = Playlist::new();
        pl.add_many(dummy_paths(1));
        pl.set_loop_mode(LoopMode::Sequential);
        assert_eq!(pl.next(), None, "单元素顺序模式到末尾应停止");
        assert_eq!(pl.prev(), None, "单元素顺序模式在开头应停止");
    }

    #[test]
    fn single_track_list_mode_wraps_to_self() {
        let mut pl = Playlist::new();
        pl.add_many(dummy_paths(1));
        pl.set_loop_mode(LoopMode::List);
        assert_eq!(pl.next(), Some(0));
        assert_eq!(pl.prev(), Some(0));
    }

    #[test]
    fn single_track_single_mode_repeats() {
        let mut pl = Playlist::new();
        pl.add_many(dummy_paths(1));
        pl.set_loop_mode(LoopMode::Single);
        assert_eq!(pl.next(), Some(0));
        assert_eq!(pl.next(), Some(0));
    }

    #[test]
    fn random_first_cycle_covers_all_others_without_repeat() {
        let mut pl = Playlist::new();
        pl.add_many(dummy_paths(4));
        pl.set_loop_mode(LoopMode::Random);
        pl.set_current(Some(0));

        let mut seen = std::collections::HashSet::new();
        for _ in 0..3 {
            let n = pl.next().expect("首轮应有 3 首可播");
            assert_ne!(n, 0, "首轮不应重复当前曲目 0");
            assert!(seen.insert(n), "同一轮内不应出现重复曲目 {n}");
        }
        assert_eq!(seen.len(), 3, "首轮应覆盖除当前外的全部曲目");

        // 第二轮开始后仍应为合法下标
        let n = pl.next().unwrap();
        assert!(n < 4);
    }

    #[test]
    fn random_cycle_never_repeats_until_exhausted() {
        // 更强约束：连续 n-1 次 next 在一个洗牌周期内两两不同。
        let mut pl = Playlist::new();
        pl.add_many(dummy_paths(6));
        pl.set_loop_mode(LoopMode::Random);
        pl.set_current(Some(3));

        let mut cycle = std::collections::HashSet::new();
        for _ in 0..5 {
            // 5 = n-1
            let n = pl.next().unwrap();
            assert!(cycle.insert(n), "洗牌周期内不应重复 {n}");
        }
        assert_eq!(cycle.len(), 5);
    }

    #[test]
    fn set_loop_mode_random_clears_pending_shuffle() {
        let mut pl = Playlist::new();
        pl.add_many(dummy_paths(3));
        pl.remaining = vec![0, 1, 2]; // 模拟已存在的随机序列
        pl.set_loop_mode(LoopMode::Random);
        assert!(pl.remaining.is_empty(), "切换到随机模式应清空剩余序列");
    }

    #[test]
    fn random_prev_on_single_track_returns_self() {
        let mut pl = Playlist::new();
        pl.add_many(dummy_paths(1));
        pl.set_loop_mode(LoopMode::Random);
        assert_eq!(pl.prev(), Some(0));
    }

    #[test]
    fn queue_then_empty_queue_falls_through_to_loop() {
        let mut pl = Playlist::new();
        pl.add_many(dummy_paths(3));
        pl.set_loop_mode(LoopMode::List);
        pl.set_current(Some(0));
        pl.enqueue(2);
        assert_eq!(pl.next(), Some(2), "队列优先");
        // 队列空后回落到列表循环：当前为 2 → (2+1)%3 = 0
        assert_eq!(pl.next(), Some(0));
    }

    #[test]
    fn enqueue_out_of_range_is_ignored() {
        let mut pl = Playlist::new();
        pl.add_many(dummy_paths(2));
        pl.enqueue(5);
        assert!(pl.queue.is_empty(), "越界下标不应入队");
    }

    #[test]
    fn remove_out_of_range_is_noop() {
        let mut pl = Playlist::new();
        pl.add_many(dummy_paths(3));
        pl.remove(99);
        assert_eq!(pl.tracks.len(), 3);
        assert_eq!(pl.current_index, Some(0));
    }

    #[test]
    fn remove_current_clears_current_index() {
        let mut pl = Playlist::new();
        pl.add_many(dummy_paths(3));
        pl.set_current(Some(1));
        pl.remove(1);
        assert_eq!(pl.current_index, None);
        assert_eq!(pl.tracks.len(), 2);
    }

    #[test]
    fn remove_index_before_current_shifts_current() {
        let mut pl = Playlist::new();
        pl.add_many(dummy_paths(4));
        pl.set_current(Some(3));
        pl.remove(0);
        assert_eq!(pl.current_index, Some(2));
    }

    #[test]
    fn move_item_out_of_range_is_noop() {
        let mut pl = Playlist::new();
        pl.add_many(dummy_paths(3));
        pl.move_item(0, 9);
        assert_eq!(pl.tracks[0].title, "track_0");
    }

    #[test]
    fn move_item_adjusts_current_index() {
        let mut pl = Playlist::new();
        pl.add_many(dummy_paths(3));
        pl.set_current(Some(1));
        pl.move_item(0, 2); // 顺序变为 [t1, t2, t0]
        assert_eq!(pl.current_index, Some(0));
        assert_eq!(pl.current().unwrap().title, "track_1");
    }

    #[test]
    fn clear_resets_all_state() {
        let mut pl = Playlist::new();
        pl.add_many(dummy_paths(3));
        pl.enqueue(1);
        pl.clear();
        assert!(pl.tracks.is_empty());
        assert!(pl.queue.is_empty());
        assert!(pl.remaining.is_empty());
        assert_eq!(pl.current_index, None);
    }

    #[test]
    fn sequential_prev_at_start_returns_none() {
        let mut pl = Playlist::new();
        pl.add_many(dummy_paths(3));
        pl.set_loop_mode(LoopMode::Sequential);
        pl.set_current(Some(0));
        assert_eq!(pl.prev(), None);
    }

    #[test]
    fn list_mode_prev_wraps_from_head_to_tail() {
        let mut pl = Playlist::new();
        pl.add_many(dummy_paths(3));
        pl.set_loop_mode(LoopMode::List);
        pl.set_current(Some(0));
        assert_eq!(pl.prev(), Some(2));
    }

    #[test]
    fn no_panic_across_all_loop_modes_for_empty_list() {
        // 回归加固：空列表在四种循环模式下 next()/prev() 均不得 panic。
        for mode in LoopMode::ALL {
            let mut pl = Playlist::new();
            pl.set_loop_mode(mode);
            assert_eq!(pl.next(), None, "{mode:?} 空列表 next 应为 None");
            assert_eq!(pl.prev(), None, "{mode:?} 空列表 prev 应为 None");
        }
    }

    #[test]
    fn no_panic_across_all_loop_modes_for_single_track() {
        // 回归加固（BUG-001）：单曲列表在四种循环模式下 next()/prev() 均不得 panic。
        for mode in LoopMode::ALL {
            let mut pl = Playlist::new();
            pl.add_many(dummy_paths(1));
            pl.set_loop_mode(mode);

            let n = pl.next();
            assert!(
                n.is_none() || n == Some(0),
                "{mode:?} 单曲列表 next 应返回 Some(0) 或 None，实际 {n:?}"
            );
            let p = pl.prev();
            assert!(
                p.is_none() || p == Some(0),
                "{mode:?} 单曲列表 prev 应返回 Some(0) 或 None，实际 {p:?}"
            );
        }
    }

    #[test]
    fn single_track_random_repeated_next_is_stable() {
        // 回归（BUG-001）：单曲随机模式连续多次 next() 应稳定返回 0，不崩溃。
        let mut pl = Playlist::new();
        pl.add_many(dummy_paths(1));
        pl.set_loop_mode(LoopMode::Random);
        for _ in 0..5 {
            assert_eq!(pl.next(), Some(0));
        }
    }

    #[test]
    fn single_track_all_modes_prev_ok() {
        for mode in LoopMode::ALL {
            let mut pl = Playlist::new();
            pl.add_many(dummy_paths(1));
            pl.set_loop_mode(mode);
            let p = pl.prev();
            assert!(p.is_none() || p == Some(0), "{mode:?} prev 越界: {p:?}");
        }
    }
}
