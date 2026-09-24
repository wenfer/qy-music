//! 播放控制栏（简洁、统一、清爽、美观）：
//! [`progress_row`] 进度 + 时间、[`transport_row`] 上下首 / 播放 / 循环模式、
//! [`volume_row`] 音量 + 静音。所有行宽度 `Length::Fill`，窄窗口不溢出。

use std::time::Duration;

use crate::app::message::AppMessage;
use crate::app::state::AppState;
use crate::app::update::format_duration;
use crate::audio::PlaybackState;
use crate::playlist::LoopMode;
use crate::ui::style::{
    hifi_pick_list_style, hifi_slider_style, icon_button_style, primary_play_button_style,
    transport_button_style, Palette,
};

use iced::widget::{button, column, container, pick_list, progress_bar, row, slider, space, text};
use iced::{Color, Element, Length};

/// 组合全部控制行（供迷你模式等复用）。
pub fn controls(state: &AppState) -> Element<'_, AppMessage> {
    column![progress_row(state), transport_row(state), volume_row(state),]
        .spacing(8)
        .into()
}

/// 进度行：当前位置 + 极细可拖动进度条（slider）+ 总时长，宽度 Fill。
pub fn progress_row(state: &AppState) -> Element<'_, AppMessage> {
    let dur = state.player.duration;
    let pos = state.player.position;
    let total_secs = dur.as_secs_f32().max(0.0);
    let palette = Palette::from_skin(&state.skin);

    let cur_time_str = if state.is_buffering {
        format!("{} (缓冲中)", format_duration(pos))
    } else {
        format_duration(pos)
    };

    let cur_time = text(cur_time_str)
        .size(10.5)
        .style(move |_theme: &iced::Theme| text::Style {
            color: Some(if state.is_buffering {
                palette.accent
            } else {
                palette.text_muted
            }),
        });

    let total_time_str = if state.buffer_ratio < 0.999 && state.buffer_ratio > 0.001 {
        format!(
            "{} [{:.0}%]",
            format_duration(dur),
            state.buffer_ratio * 100.0
        )
    } else {
        format_duration(dur)
    };

    let total_time = text(total_time_str)
        .size(10.5)
        .style(move |_theme: &iced::Theme| text::Style {
            color: Some(palette.text_muted),
        });

    let seek_slider = slider(0.0..=total_secs.max(0.01), pos.as_secs_f32(), |v| {
        AppMessage::Seek(Duration::from_secs_f32(v))
    })
    .step(0.1_f32)
    .width(Length::Fill)
    .style(hifi_slider_style(palette));

    let progress_widget: Element<'_, AppMessage> = if state.buffer_ratio < 0.999 {
        // 双层复合进度条：底层显示网络下载缓冲条，顶层显示播放拖拽滑块
        let buffer_bar = progress_bar(0.0..=1.0, state.buffer_ratio)
            .length(Length::Fill)
            .style(move |_theme: &iced::Theme| progress_bar::Style {
                background: iced::Background::Color(Color::from_rgba(1.0, 1.0, 1.0, 0.04)),
                bar: iced::Background::Color(palette.accent_subtle),
                border: iced::Border::default().rounded(2.0),
            });

        iced::widget::stack![
            container(buffer_bar)
                .height(Length::Fixed(16.0))
                .align_y(iced::alignment::Vertical::Center),
            seek_slider,
        ]
        .width(Length::Fill)
        .into()
    } else {
        seek_slider.into()
    };

    row![cur_time, progress_widget, total_time]
        .spacing(8)
        .align_y(iced::alignment::Vertical::Center)
        .width(Length::Fill)
        .into()
}

/// 传输行：上一首 / 播放暂停 / 下一首 + 循环模式（居中对称、现代胶囊）。
pub fn transport_row(state: &AppState) -> Element<'_, AppMessage> {
    let palette = Palette::from_skin(&state.skin);

    let prev_btn = button(
        container(text("◀◀").size(10.5))
            .width(Length::Fill)
            .height(Length::Fill)
            .align_x(iced::alignment::Horizontal::Center)
            .align_y(iced::alignment::Vertical::Center),
    )
    .width(Length::Fixed(30.0))
    .height(Length::Fixed(30.0))
    .padding(0)
    .style(transport_button_style(palette))
    .on_press(AppMessage::Prev);

    let play_btn = match state.player.state {
        PlaybackState::Playing => {
            let bar = || {
                container(space::horizontal())
                    .width(Length::Fixed(3.5))
                    .height(Length::Fixed(12.0))
                    .style(|_theme: &iced::Theme| container::Style {
                        background: Some(iced::Background::Color(Color::WHITE)),
                        border: iced::Border::default().rounded(1.5),
                        ..Default::default()
                    })
            };
            let pause_icon = container(
                row![bar(), bar()]
                    .spacing(4.0)
                    .align_y(iced::alignment::Vertical::Center),
            )
            .width(Length::Fill)
            .height(Length::Fill)
            .align_x(iced::alignment::Horizontal::Center)
            .align_y(iced::alignment::Vertical::Center);

            button(pause_icon)
                .width(Length::Fixed(36.0))
                .height(Length::Fixed(36.0))
                .padding(0)
                .style(primary_play_button_style(palette))
                .on_press(AppMessage::TogglePlay)
        }
        _ => button(
            container(text("▶").size(14.0))
                .width(Length::Fill)
                .height(Length::Fill)
                .align_x(iced::alignment::Horizontal::Center)
                .align_y(iced::alignment::Vertical::Center),
        )
        .width(Length::Fixed(36.0))
        .height(Length::Fixed(36.0))
        .padding(0)
        .style(primary_play_button_style(palette))
        .on_press(AppMessage::TogglePlay),
    };

    let next_btn = button(
        container(text("▶▶").size(10.5))
            .width(Length::Fill)
            .height(Length::Fill)
            .align_x(iced::alignment::Horizontal::Center)
            .align_y(iced::alignment::Vertical::Center),
    )
    .width(Length::Fixed(30.0))
    .height(Length::Fixed(30.0))
    .padding(0)
    .style(transport_button_style(palette))
    .on_press(AppMessage::Next);

    let loop_picker = pick_list(
        &LoopMode::ALL[..],
        Some(&state.playlist.loop_mode),
        AppMessage::SetLoopMode,
    )
    .placeholder("循环")
    .text_size(11.0)
    .style(hifi_pick_list_style(palette));

    row![
        prev_btn,
        play_btn,
        next_btn,
        space::horizontal(),
        loop_picker,
    ]
    .spacing(8)
    .align_y(iced::alignment::Vertical::Center)
    .width(Length::Fill)
    .into()
}

/// 音量行：静音切换 + 极细音量滑条 + 百分比。
pub fn volume_row(state: &AppState) -> Element<'_, AppMessage> {
    let palette = Palette::from_skin(&state.skin);
    let (mute_label, label_color) = if state.player.muted {
        ("静音", Color::from_rgb(0.95, 0.40, 0.40))
    } else {
        ("音量", palette.text_muted)
    };

    let mute_btn = button(
        text(mute_label)
            .size(11.0)
            .style(move |_theme: &iced::Theme| text::Style {
                color: Some(label_color),
            }),
    )
    .padding([2, 4])
    .style(icon_button_style(palette))
    .on_press(AppMessage::ToggleMute);

    let vol_slider = slider(0.0..=1.0, state.player.volume, AppMessage::SetVolume)
        .step(0.01_f32)
        .width(Length::Fill)
        .style(hifi_slider_style(palette));

    let vol_pct = if state.player.muted {
        "0%".to_string()
    } else {
        format!("{:.0}%", state.player.volume * 100.0)
    };

    let vol_text = text(vol_pct)
        .size(11.0)
        .style(move |_theme: &iced::Theme| text::Style {
            color: Some(palette.text_muted),
        });

    row![mute_btn, vol_slider, vol_text,]
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
    progress_bar(0.0..=1.0, progress)
        .length(Length::Fill)
        .into()
}
