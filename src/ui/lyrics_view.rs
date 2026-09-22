//! 歌词视图：根据播放位置高亮当前句，并提供手动加载 / 偏移微调（极简克制）。
//! 另提供迷你歌词窗口视图（仅显示当前 / 下一句）。

use crate::app::message::AppMessage;
use crate::app::state::AppState;
use crate::lyrics::sync;
use crate::ui::style::{hifi_scrollable_style, hifi_slider_style, subtle_button_style, Palette};

use iced::widget::{button, column, container, row, scrollable, slider, space, text};
use iced::{Background, Border, Color, Element, Length};

/// 主歌词视图（带工具栏与滚动列表）。
pub fn lyrics_view(state: &AppState) -> Element<'_, AppMessage> {
    let palette = Palette::from_skin(&state.skin);

    let offset_str = if state.lyric_offset_ms == 0 {
        "0ms".to_string()
    } else {
        format!("{:+4}ms", state.lyric_offset_ms)
    };

    let actions = row![
        text(format!("微调: {}", offset_str))
            .size(10.0)
            .style(move |_theme: &iced::Theme| text::Style {
                color: Some(palette.text_muted),
            }),
        space::horizontal(),
        button(text("加载").size(11.0))
            .padding([2, 6])
            .style(subtle_button_style(palette))
            .on_press(AppMessage::LoadLyrics(
                // 通过文件对话框在 update 中处理（空路径触发对话框）。
                std::path::PathBuf::new(),
            )),
        button(text("浮窗").size(11.0))
            .padding([2, 6])
            .style(subtle_button_style(palette))
            .on_press(AppMessage::ToggleMiniLyrics),
    ]
    .spacing(6)
    .align_y(iced::alignment::Vertical::Center)
    .width(Length::Fill);

    let offset_slider = slider(-2000.0..=2000.0, state.lyric_offset_ms as f32, |v| {
        AppMessage::SetLyricOffset(v as i64)
    })
    .step(10.0)
    .width(Length::Fill)
    .style(hifi_slider_style(palette));

    let toolbar = column![actions, offset_slider]
        .spacing(3)
        .width(Length::Fill);

    let body: Element<'_, AppMessage> = match &state.lyrics {
        Some(lrc) => {
            let idx = sync::current_line(lrc, state.player.position, state.lyric_offset_ms);
            let lines: Vec<Element<'_, AppMessage>> = lrc
                .lines
                .iter()
                .enumerate()
                .map(|(i, line)| {
                    let diff = (i as isize - idx as isize).abs();
                    let is_current = i == idx;

                    let (size, color) = match diff {
                        0 => (15.0, palette.accent),
                        1 => (13.0, palette.text_main),
                        2 => (12.0, palette.text_sub),
                        _ => (11.0, palette.text_muted),
                    };

                    let line_text = text(line.text.clone())
                        .size(size)
                        .style(move |_theme: &iced::Theme| text::Style { color: Some(color) });

                    let line_container = container(line_text)
                        .align_x(iced::alignment::Horizontal::Center)
                        .width(Length::Fill);

                    if is_current {
                        container(line_container)
                            .padding([4, 8])
                            .style(move |_theme| container::Style {
                                background: Some(Background::Color(palette.accent_subtle)),
                                border: Border {
                                    color: Color::from_rgba(
                                        palette.accent.r,
                                        palette.accent.g,
                                        palette.accent.b,
                                        0.20,
                                    ),
                                    width: 1.0,
                                    radius: 4.0.into(),
                                },
                                ..Default::default()
                            })
                            .width(Length::Fill)
                            .into()
                    } else {
                        line_container.into()
                    }
                })
                .collect();

            scrollable(
                column(lines)
                    .spacing(8)
                    .align_x(iced::alignment::Horizontal::Center)
                    .width(Length::Fill),
            )
            .style(hifi_scrollable_style(palette))
            .height(Length::Fill)
            .into()
        }
        None => container(
            text("暂无歌词")
                .size(11.0)
                .style(move |_theme: &iced::Theme| text::Style {
                    color: Some(palette.text_muted),
                }),
        )
        .padding(24)
        .align_x(iced::alignment::Horizontal::Center)
        .width(Length::Fill)
        .into(),
    };

    column![toolbar, body].spacing(6).into()
}

/// 迷你歌词窗口视图（半透明背景，仅显示当前 / 下一句）。
pub fn mini_lyrics_view(state: &AppState) -> Element<'_, AppMessage> {
    let palette = Palette::from_skin(&state.skin);

    let body: Element<'_, AppMessage> = match &state.lyrics {
        Some(lrc) => {
            let (idx, next) =
                sync::current_and_next(lrc, state.player.position, state.lyric_offset_ms);
            let cur = lrc
                .lines
                .get(idx)
                .map(|l| l.text.clone())
                .unwrap_or_else(|| "…".to_string());
            let nxt = next
                .and_then(|n| lrc.lines.get(n))
                .map(|l| l.text.clone())
                .unwrap_or_default();
            column![
                text(cur)
                    .size(18.0)
                    .style(move |_theme: &iced::Theme| text::Style {
                        color: Some(palette.accent),
                    }),
                text(nxt)
                    .size(12.0)
                    .style(move |_theme: &iced::Theme| text::Style {
                        color: Some(palette.text_sub),
                    }),
            ]
            .spacing(4)
            .into()
        }
        None => text("暂无歌词")
            .size(12.0)
            .style(move |_theme: &iced::Theme| text::Style {
                color: Some(palette.text_muted),
            })
            .into(),
    };

    let close = button(text("×").size(11.0))
        .padding([2, 6])
        .style(subtle_button_style(palette))
        .on_press(AppMessage::ToggleMiniLyrics);

    column![row![space::horizontal(), close], body]
        .spacing(4)
        .padding(8)
        .into()
}
