//! 迷你模式窗口视图：紧凑播放器（标题 + 控制 + 频谱 + 退出迷你）。

use crate::app::message::AppMessage;
use crate::app::state::AppState;
use crate::audio::PlaybackState;
use crate::ui::controls;
use crate::ui::style::{
    lcd_display_style, primary_play_button_style, subtle_button_style, transport_button_style,
    window_container_style, Palette,
};
use crate::ui::widgets::spectrum_widget;

use iced::widget::{button, column, container, row, space, text};
use iced::{Element, Length};

/// 迷你模式视图。
pub fn mini_mode_view(state: &AppState) -> Element<'_, AppMessage> {
    let palette = Palette::from_skin(&state.skin);

    let title = state
        .current_track()
        .map(|t| t.display_name())
        .unwrap_or_else(|| "LFPlayer".to_string());

    let title_badge = if state.current_track().is_some_and(|t| t.is_lossless()) {
        Some(
            container(
                text("无损")
                    .size(9.0)
                    .style(move |_theme: &iced::Theme| text::Style {
                        color: Some(palette.vfd_green),
                    }),
            )
            .padding([1, 4])
            .style(move |_theme| container::Style {
                background: Some(iced::Background::Color(iced::Color::from_rgba(
                    palette.vfd_green.r,
                    palette.vfd_green.g,
                    palette.vfd_green.b,
                    0.12,
                ))),
                border: iced::Border {
                    color: iced::Color::from_rgba(
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
    } else {
        None
    };

    let mut title_items: Vec<Element<'_, AppMessage>> = vec![text(title)
        .size(13.0)
        .style(move |_theme: &iced::Theme| text::Style {
            color: Some(palette.text_main),
        })
        .into()];
    if let Some(b) = title_badge {
        title_items.push(b.into());
    }
    title_items.push(space::horizontal().into());
    title_items.push(
        button(text("▢ 还原").size(11.0))
            .padding([2, 6])
            .style(subtle_button_style(palette))
            .on_press(AppMessage::ExitMiniMode)
            .into(),
    );

    let title_row = row(title_items)
        .spacing(6)
        .align_y(iced::alignment::Vertical::Center)
        .width(Length::Fill);

    let play_btn = match state.player.state {
        PlaybackState::Playing => {
            let bar = || {
                container(space::horizontal())
                    .width(Length::Fixed(3.0))
                    .height(Length::Fixed(11.0))
                    .style(|_theme: &iced::Theme| container::Style {
                        background: Some(iced::Background::Color(iced::Color::WHITE)),
                        border: iced::Border::default().rounded(1.5),
                        ..Default::default()
                    })
            };
            let pause_icon = container(
                row![bar(), bar()]
                    .spacing(3.5)
                    .align_y(iced::alignment::Vertical::Center),
            )
            .width(Length::Fill)
            .height(Length::Fill)
            .align_x(iced::alignment::Horizontal::Center)
            .align_y(iced::alignment::Vertical::Center);

            button(pause_icon)
                .width(Length::Fixed(34.0))
                .height(Length::Fixed(34.0))
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
        .width(Length::Fixed(34.0))
        .height(Length::Fixed(34.0))
        .padding(0)
        .style(primary_play_button_style(palette))
        .on_press(AppMessage::TogglePlay),
    };

    let prev_btn = button(
        container(text("◀◀").size(10.0))
            .width(Length::Fill)
            .height(Length::Fill)
            .align_x(iced::alignment::Horizontal::Center)
            .align_y(iced::alignment::Vertical::Center),
    )
    .width(Length::Fixed(28.0))
    .height(Length::Fixed(28.0))
    .padding(0)
    .style(transport_button_style(palette))
    .on_press(AppMessage::Prev);

    let next_btn = button(
        container(text("▶▶").size(10.0))
            .width(Length::Fill)
            .height(Length::Fill)
            .align_x(iced::alignment::Horizontal::Center)
            .align_y(iced::alignment::Vertical::Center),
    )
    .width(Length::Fixed(28.0))
    .height(Length::Fixed(28.0))
    .padding(0)
    .style(transport_button_style(palette))
    .on_press(AppMessage::Next);

    let transport = row![prev_btn, play_btn, next_btn]
        .spacing(6)
        .align_y(iced::alignment::Vertical::Center);

    let spectrum = container(spectrum_widget(state))
        .padding([2, 4])
        .height(Length::Fixed(34.0))
        .width(Length::Fill)
        .style(lcd_display_style(palette));

    let vol_slider = iced::widget::slider(0.0..=1.0, state.player.volume, AppMessage::SetVolume)
        .step(0.02_f32)
        .width(Length::Fixed(80.0))
        .style(crate::ui::style::hifi_slider_style(palette));

    let mute_btn = button(
        text(if state.player.muted {
            "静音"
        } else {
            "音量"
        })
        .size(11.0),
    )
    .padding([3, 6])
    .style(subtle_button_style(palette))
    .on_press(AppMessage::ToggleMute);

    let bottom_row = row![transport, space::horizontal(), mute_btn, vol_slider,]
        .spacing(6)
        .align_y(iced::alignment::Vertical::Center)
        .width(Length::Fill);

    let content = column![
        title_row,
        spectrum,
        controls::progress_row(state),
        bottom_row,
    ]
    .spacing(6)
    .padding(8);

    container(content)
        .style(window_container_style(palette))
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}
