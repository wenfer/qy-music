//! 歌词视图：根据播放位置高亮当前句，并提供手动加载 / 偏移微调。
//! 另提供迷你歌词窗口视图（仅显示当前 / 下一句）。

use crate::app::message::AppMessage;
use crate::app::state::AppState;
use crate::lyrics::sync;

use iced::widget::{button, column, row, scrollable, slider, text};
use iced::{Element, Length};

/// 主歌词视图（带工具栏与滚动列表；工具栏两行排布以适配 340 宽竖窄窗口）。
pub fn lyrics_view(state: &AppState) -> Element<'_, AppMessage> {
    let actions = row![
        button(text("加载歌词").size(12.0)).on_press(AppMessage::LoadLyrics(
            // 通过文件对话框在 update 中处理（空路径触发对话框）。
            std::path::PathBuf::new(),
        )),
        button(text("迷你歌词").size(12.0)).on_press(AppMessage::ToggleMiniLyrics),
    ]
    .spacing(6);

    let offset_row = row![
        text(format!("偏移 {} ms", state.lyric_offset_ms)).size(11.0),
        slider(-2000.0..=2000.0, state.lyric_offset_ms as f32, |v| {
            AppMessage::SetLyricOffset(v as i64)
        })
        .step(10.0)
        .width(Length::Fill),
    ]
    .spacing(6)
    .align_y(iced::alignment::Vertical::Center)
    .width(Length::Fill);

    let toolbar = column![actions, offset_row].spacing(4).width(Length::Fill);

    let body: Element<'_, AppMessage> = match &state.lyrics {
        Some(lrc) => {
            let idx = sync::current_line(lrc, state.player.position, state.lyric_offset_ms);
            let lines: Vec<Element<'_, AppMessage>> = lrc
                .lines
                .iter()
                .enumerate()
                .map(|(i, line)| {
                    let size = if i == idx { 18.0 } else { 14.0 };
                    let color = if i == idx {
                        iced::Color::from_rgb(0.31, 0.55, 1.0)
                    } else {
                        iced::Color::from_rgb(0.8, 0.85, 1.0)
                    };
                    text(line.text.clone())
                        .size(size)
                        .style(move |_theme: &iced::Theme| text::Style { color: Some(color) })
                        .into()
                })
                .collect();
            scrollable(column(lines).spacing(6))
                .height(Length::Fill)
                .into()
        }
        None => column![text("暂无歌词").size(14.0)].into(),
    };

    column![toolbar, body].spacing(8).into()
}

/// 迷你歌词窗口视图（透明背景，仅显示当前 / 下一句）。
pub fn mini_lyrics_view(state: &AppState) -> Element<'_, AppMessage> {
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
                text(cur).size(22.0).style(|_theme: &iced::Theme| text::Style {
                    color: Some(iced::Color::WHITE),
                }),
                text(nxt)
                    .size(14.0)
                    .style(|_theme: &iced::Theme| text::Style {
                        color: Some(iced::Color::from_rgba8(255, 255, 255, 0.63)),
                    }),
            ]
            .spacing(6)
            .into()
        }
        None => text("暂无歌词").size(16.0).into(),
    };

    // 双击关闭迷你歌词
    let close = button(text("×").size(12.0)).on_press(AppMessage::ToggleMiniLyrics);
    column![row![iced::widget::horizontal_space(), close], body]
        .spacing(4)
        .into()
}
