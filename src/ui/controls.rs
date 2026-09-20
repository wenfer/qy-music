//! 播放控制栏（T12：按 340 宽竖窄窗口拆分为三行）：
//! [`progress_row`] 进度 + 时间、[`transport_row`] 上下首 / 播放 / 循环模式、
//! [`volume_row`] 音量 + 静音。所有行宽度 `Length::Fill`，窄窗口不溢出。

use std::time::Duration;

use crate::app::message::AppMessage;
use crate::app::state::AppState;
use crate::app::update::format_duration;
use crate::audio::PlaybackState;
use crate::playlist::LoopMode;

use iced::widget::{button, column, horizontal_space, pick_list, progress_bar, row, slider, text};
use iced::{Element, Length};

/// 组合全部控制行（供迷你模式等复用）。
pub fn controls(state: &AppState) -> Element<'_, AppMessage> {
    column![
        progress_row(state),
        transport_row(state),
        volume_row(state),
    ]
    .spacing(6)
    .into()
}

/// 进度行：当前位置 + 可拖动进度条（slider）+ 总时长，宽度 Fill。
pub fn progress_row(state: &AppState) -> Element<'_, AppMessage> {
    let dur = state.player.duration;
    let pos = state.player.position;
    let total_secs = dur.as_secs_f32().max(0.0);

    row![
        text(format_duration(pos)).size(12.0),
        slider(0.0..=total_secs.max(0.01), pos.as_secs_f32(), |v| {
            AppMessage::Seek(Duration::from_secs_f32(v))
        })
        .step(0.1)
        .width(Length::Fill),
        text(format_duration(dur)).size(12.0),
    ]
    .spacing(6)
    .align_y(iced::alignment::Vertical::Center)
    .width(Length::Fill)
    .into()
}

/// 传输行：上一首 / 播放暂停 / 下一首 + 循环模式（图标按钮，适配窄宽度）。
pub fn transport_row(state: &AppState) -> Element<'_, AppMessage> {
    let toggle_label = match state.player.state {
        PlaybackState::Playing => "‖",
        PlaybackState::Paused => "▶",
        PlaybackState::Stopped => "▶",
    };

    row![
        button(text("◀◀").size(16.0)).on_press(AppMessage::Prev),
        button(text(toggle_label).size(16.0)).on_press(AppMessage::TogglePlay),
        button(text("▶▶").size(16.0)).on_press(AppMessage::Next),
        horizontal_space(),
        pick_list(
            &LoopMode::ALL[..],
            Some(&state.playlist.loop_mode),
            AppMessage::SetLoopMode,
        )
        .placeholder("循环"),
    ]
    .spacing(8)
    .align_y(iced::alignment::Vertical::Center)
    .width(Length::Fill)
    .into()
}

/// 音量行：静音切换 + 音量滑条（`Fill`，原 `Fixed(160)` 在 340 宽会溢出）+ 百分比。
pub fn volume_row(state: &AppState) -> Element<'_, AppMessage> {
    let mute_label = if state.player.muted { "静音" } else { "音量" };
    row![
        button(text(mute_label).size(14.0)).on_press(AppMessage::ToggleMute),
        slider(0.0..=1.0, state.player.volume, AppMessage::SetVolume)
            .step(0.01)
            .width(Length::Fill),
        text(format!("{:.0}%", state.player.volume * 100.0)).size(12.0),
    ]
    .spacing(8)
    .align_y(iced::alignment::Vertical::Center)
    .width(Length::Fill)
    .into()
}

/// 视觉进度条（0..=1 比例条；可拖动进度见 [`progress_row`]）。
pub fn progress_bar_row(state: &AppState) -> Element<'_, AppMessage> {
    let dur = state.player.duration;
    let pos = state.player.position;
    let total_secs = dur.as_secs_f32().max(0.0);
    let progress = if total_secs > 0.0 {
        (pos.as_secs_f32() / total_secs).clamp(0.0, 1.0)
    } else {
        0.0
    };
    progress_bar(0.0..=1.0, progress).width(Length::Fill).into()
}
