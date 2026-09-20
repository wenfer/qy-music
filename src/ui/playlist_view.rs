//! 播放列表视图：曲目列表 + 增删 / 播放 / 清空。

use crate::app::message::AppMessage;
use crate::app::state::AppState;
use crate::app::update::format_duration;

use iced::widget::{button, column, container, row, scrollable, text};
use iced::{Element, Length};

/// 播放列表视图。
pub fn playlist_view(state: &AppState) -> Element<'_, AppMessage> {
    let toolbar = row![
        button(text("＋ 添加文件").size(13.0)).on_press(AppMessage::AddFiles),
        button(text("＋ 添加文件夹").size(13.0)).on_press(AppMessage::AddFolder),
        button(text("清空").size(13.0)).on_press(AppMessage::ClearPlaylist),
    ]
    .spacing(8);

    let items: Vec<Element<'_, AppMessage>> = state
        .playlist
        .tracks
        .iter()
        .enumerate()
        .map(|(i, track)| {
            let is_current = state.playlist.current_index == Some(i);
            let idx_label = if is_current { "▶ " } else { "" };
            let artist = if track.artist.is_empty() {
                "未知艺术家".to_string()
            } else {
                track.artist.clone()
            };
            let title = format!("{}{} — {}", idx_label, track.display_name(), artist);
            let duration_text = if track.duration.is_zero() {
                "--:--".to_string()
            } else {
                format_duration(track.duration)
            };
            let line = row![
                button(text(title).size(13.0))
                    .on_press(AppMessage::PlayTrack(i))
                    .width(Length::Fill),
                text(duration_text).size(11.0),
                button(text("×").size(11.0))
                    .on_press(AppMessage::RemoveTrack(i)),
            ]
            .spacing(6)
            .align_y(iced::alignment::Vertical::Center)
            .width(Length::Fill)
            .padding(4);

            if is_current {
                container(line)
                    .style(|_theme: &iced::Theme| container::Style {
                        background: Some(iced::Background::Color(iced::Color::from_rgba8(
                            79, 140, 255, 0.235,
                        ))),
                        ..Default::default()
                    })
                    .into()
            } else {
                line.into()
            }
        })
        .collect();

    let list = scrollable(column(items).spacing(2)).height(Length::Fill);

    column![toolbar, list].spacing(8).into()
}
