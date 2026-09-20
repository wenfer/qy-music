//! 迷你模式窗口视图：紧凑播放器（标题 + 控制 + 频谱 + 退出迷你）。

use crate::app::message::AppMessage;
use crate::app::state::AppState;
use crate::audio::PlaybackState;

use crate::ui::controls;
use crate::ui::widgets::spectrum_widget;

use iced::widget::{button, column, container, row, text};
use iced::{Element, Length};

/// 迷你模式视图。
pub fn mini_mode_view(state: &AppState) -> Element<'_, AppMessage> {
    let title = state
        .current_track()
        .map(|t| t.display_name())
        .unwrap_or_else(|| "聆风 / Lingfeng".to_string());

    let toggle_label = match state.player.state {
        PlaybackState::Playing => "‖",
        _ => "▶",
    };

    let transport = row![
        button(text("◀◀").size(16.0)).on_press(AppMessage::Prev),
        button(text(toggle_label).size(16.0)).on_press(AppMessage::TogglePlay),
        button(text("▶▶").size(16.0)).on_press(AppMessage::Next),
        button(text("▢ 退出迷你").size(12.0)).on_press(AppMessage::ExitMiniMode),
    ]
    .spacing(8)
    .align_y(iced::alignment::Vertical::Center);

    let spectrum = container(spectrum_widget(state)).height(Length::Fixed(48.0));

    let content = column![
        text(title).size(16.0),
        transport,
        spectrum,
        controls::controls(state),
    ]
    .spacing(8)
    .padding(10);

    content.into()
}
