//! 播放列表视图：曲目列表 + 播放 / 删除（极简实用，单行对齐，杜绝折行）。

use crate::app::message::AppMessage;
use crate::app::state::AppState;
use crate::app::update::format_duration;
use crate::ui::style::{hifi_scrollable_style, subtle_button_style, Palette};

use iced::widget::{button, column, container, row, scrollable, text};
use iced::{Background, Border, Color, Element, Length};

/// 播放列表主体滚动视图（纯净曲目列表，不含冗余双重工具条）。
pub fn playlist_view(state: &AppState) -> Element<'_, AppMessage> {
    let palette = Palette::from_skin(&state.skin);

    if state.playlist.tracks.is_empty() {
        let empty_btn = button(text("＋ 添加本地音乐文件夹").size(11.5))
            .padding([6, 14])
            .style(subtle_button_style(palette))
            .on_press(AppMessage::AddFolder);

        let webdav_btn = button(text("☁ 挂载 WebDAV 云端音乐").size(11.5))
            .padding([6, 14])
            .style(subtle_button_style(palette))
            .on_press(AppMessage::ToggleWebDavWindow);

        return container(
            column![empty_btn, webdav_btn]
                .spacing(8)
                .align_x(iced::alignment::Horizontal::Center),
        )
        .padding(36)
        .align_x(iced::alignment::Horizontal::Center)
        .width(Length::Fill)
        .height(Length::Fill)
        .into();
    }

    let items: Vec<Element<'_, AppMessage>> = state
        .playlist
        .tracks
        .iter()
        .enumerate()
        .map(|(i, track)| {
            let is_current = state.playlist.current_index == Some(i);
            let mark = if is_current { "▶ " } else { "" };
            let display_title = format!("{}{}", mark, track.display_name());
            let duration_text = if track.duration.is_zero() {
                "--:--".to_string()
            } else {
                format_duration(track.duration)
            };

            let track_btn = button(
                row![
                    text(format!("{:02}", i + 1))
                        .size(10.0)
                        .width(Length::Fixed(18.0))
                        .style(move |_theme: &iced::Theme| text::Style {
                            color: Some(if is_current {
                                palette.accent
                            } else {
                                palette.text_muted
                            }),
                        }),
                    text(display_title).size(12.0).width(Length::Fill).style(
                        move |_theme: &iced::Theme| text::Style {
                            color: Some(if is_current {
                                palette.accent
                            } else {
                                palette.text_main
                            }),
                        }
                    ),
                ]
                .spacing(6)
                .align_y(iced::alignment::Vertical::Center),
            )
            .on_press(AppMessage::PlayTrack(i))
            .width(Length::Fill)
            .padding([3, 4])
            .style(move |_theme, status| match status {
                button::Status::Hovered => button::Style {
                    background: Some(Background::Color(palette.surface_elevated)),
                    border: Border::default().rounded(4.0),
                    ..Default::default()
                },
                _ => button::Style {
                    background: Some(Background::Color(Color::TRANSPARENT)),
                    ..Default::default()
                },
            });

            let dur_label = text(duration_text)
                .size(10.0)
                .width(Length::Fixed(34.0))
                .style(move |_theme: &iced::Theme| text::Style {
                    color: Some(if is_current {
                        palette.accent
                    } else {
                        palette.text_muted
                    }),
                });

            let del_btn = button(text("×").size(11.0))
                .padding([2, 5])
                .on_press(AppMessage::RemoveTrack(i))
                .style(move |_theme, status| match status {
                    button::Status::Hovered => button::Style {
                        background: Some(Background::Color(Color::from_rgba8(239, 68, 68, 0.2))),
                        text_color: Color::from_rgb8(239, 68, 68),
                        border: Border::default().rounded(3.0),
                        ..Default::default()
                    },
                    _ => button::Style {
                        text_color: palette.text_muted,
                        ..Default::default()
                    },
                });

            let badge_slot: Element<'_, AppMessage> = if track.is_remote() {
                container(
                    container(text("云端").size(9.0).style(move |_theme: &iced::Theme| {
                        text::Style {
                            color: Some(palette.accent),
                        }
                    }))
                    .padding([1, 4])
                    .style(move |_theme| container::Style {
                        background: Some(Background::Color(palette.accent_subtle)),
                        border: Border {
                            color: Color::from_rgba(
                                palette.accent.r,
                                palette.accent.g,
                                palette.accent.b,
                                0.40,
                            ),
                            width: 1.0,
                            radius: 3.0.into(),
                        },
                        ..Default::default()
                    }),
                )
                .width(Length::Fixed(32.0))
                .align_x(iced::alignment::Horizontal::Center)
                .into()
            } else if track.is_lossless() {
                container(
                    container(text("无损").size(9.0).style(move |_theme: &iced::Theme| {
                        text::Style {
                            color: Some(palette.vfd_green),
                        }
                    }))
                    .padding([1, 4])
                    .style(move |_theme| container::Style {
                        background: Some(Background::Color(Color::from_rgba(
                            palette.vfd_green.r,
                            palette.vfd_green.g,
                            palette.vfd_green.b,
                            0.12,
                        ))),
                        border: Border {
                            color: Color::from_rgba(
                                palette.vfd_green.r,
                                palette.vfd_green.g,
                                palette.vfd_green.b,
                                0.40,
                            ),
                            width: 1.0,
                            radius: 3.0.into(),
                        },
                        ..Default::default()
                    }),
                )
                .width(Length::Fixed(32.0))
                .align_x(iced::alignment::Horizontal::Center)
                .into()
            } else {
                iced::widget::Space::new().width(32.0).into()
            };

            let line = row![track_btn, badge_slot, dur_label, del_btn]
                .spacing(4)
                .align_y(iced::alignment::Vertical::Center)
                .width(Length::Fill);

            if is_current {
                container(line)
                    .padding([1, 2])
                    .style(move |_theme| container::Style {
                        background: Some(Background::Color(palette.accent_subtle)),
                        border: Border {
                            color: Color::from_rgba(
                                palette.accent.r,
                                palette.accent.g,
                                palette.accent.b,
                                0.25,
                            ),
                            width: 1.0,
                            radius: 4.0.into(),
                        },
                        ..Default::default()
                    })
                    .into()
            } else {
                container(line).padding([1, 2]).into()
            }
        })
        .collect();

    scrollable(column(items).spacing(1))
        .style(hifi_scrollable_style(palette))
        .height(Length::Fill)
        .into()
}
