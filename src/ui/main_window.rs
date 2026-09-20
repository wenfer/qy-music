//! 主窗口视图（T12：千千静听经典竖窄单列布局）。
//!
//! 纵向七分区，所有分区宽度 `Length::Fill`（拉宽仍是单列，内容自适应）：
//! ① 标题栏 ② 当前曲目 ③ LED 频谱窄条（Fixed 80） ④ 进度 + 时间
//! ⑤ 控制行（上下首 / 播放 / 音量 / 循环） ⑥ 列表 / 歌词 Tab 主体（Fill ≥260）
//! ⑦ 可折叠均衡器面板（收起 28px / 展开 160px 内部滚动）。

use crate::app::message::{AppMessage, MainTab};
use crate::app::state::AppState;
use crate::theme::skin_color;

use crate::ui::controls;
use crate::ui::equalizer_view;
use crate::ui::lyrics_view;
use crate::ui::playlist_view;
use crate::ui::widgets::{accent_text, spectrum_widget, themed_container, themed_text};

use iced::widget::{button, column, container, horizontal_space, row, text};
use iced::{Background, Color, Element, Length, Theme};

/// 主窗口视图：纵向单列七分区。
pub fn main_view(state: &AppState) -> Element<'_, AppMessage> {
    let content = column![
        build_header(state),
        build_now_playing(state),
        build_spectrum_bar(state),
        controls::progress_row(state),
        controls::transport_row(state),
        controls::volume_row(state),
        build_tab_body(state),
        equalizer_view::eq_panel(state),
    ]
    .spacing(6)
    .padding(8)
    .width(Length::Fill)
    .height(Length::Fill);

    themed_container(content, state)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

/// ① 标题栏：标题 + 皮肤切换 + 迷你歌词 / 迷你模式入口。
fn build_header(state: &AppState) -> Element<'_, AppMessage> {
    let title = accent_text("聆风", state).size(16.0);

    let skin_picker = iced::widget::pick_list(
        crate::theme::Skin::builtin_static(),
        Some(&state.skin),
        |skin| AppMessage::SetSkin(skin.id.clone()),
    )
    .placeholder("皮肤")
    .text_size(12.0);

    let mini_lyrics_btn = button(text("迷你歌词").size(12.0))
        .on_press(AppMessage::ToggleMiniLyrics);
    let mini_btn = button(text("迷你").size(12.0)).on_press(AppMessage::EnterMiniMode);

    row![
        title,
        horizontal_space(),
        skin_picker,
        mini_lyrics_btn,
        mini_btn,
    ]
    .spacing(6)
    .align_y(iced::alignment::Vertical::Center)
    .width(Length::Fill)
    .into()
}

/// ② 当前曲目：标题 / 艺术家（居中）。
fn build_now_playing(state: &AppState) -> Element<'_, AppMessage> {
    let track = state.current_track();
    let title = themed_text(
        track
            .map(|t| t.title.clone())
            .unwrap_or_else(|| "未播放".to_string()),
        state,
    )
    .size(18.0);

    let artist = themed_text(
        track
            .map(|t| t.artist.clone())
            .filter(|a| !a.is_empty())
            .unwrap_or_else(|| "—".to_string()),
        state,
    )
    .size(13.0);

    column![title, artist]
        .spacing(2)
        .align_x(iced::alignment::Horizontal::Center)
        .width(Length::Fill)
        .into()
}

/// ③ LED 频谱窄条（高 80，宽度 Fill）。
fn build_spectrum_bar(state: &AppState) -> Element<'_, AppMessage> {
    container(spectrum_widget(state))
        .width(Length::Fill)
        .height(Length::Fixed(80.0))
        .into()
}

/// ⑥ 列表 / 歌词 Tab 主体（Fill，最小内容区 ≥260 由 min_size 320×520 保证）。
fn build_tab_body(state: &AppState) -> Element<'_, AppMessage> {
    let tabs = row![
        tab_button("列表", MainTab::Playlist, state),
        tab_button("歌词", MainTab::Lyrics, state),
    ]
    .spacing(6)
    .width(Length::Fill);

    let body: Element<'_, AppMessage> = match state.main_tab {
        MainTab::Playlist => playlist_view::playlist_view(state),
        MainTab::Lyrics => lyrics_view::lyrics_view(state),
    };

    column![tabs, container(body).width(Length::Fill).height(Length::Fill)]
        .spacing(4)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

/// Tab 按钮：当前 Tab 用强调色底，非当前默认样式。
fn tab_button<'a>(label: &str, tab: MainTab, state: &AppState) -> Element<'a, AppMessage> {
    let is_active = state.main_tab == tab;
    let accent = skin_color(&state.skin.colors.accent);
    let btn = button(text(label.to_string()).size(13.0))
        .padding([4, 10])
        .on_press(AppMessage::SwitchMainTab(tab));
    if is_active {
        btn.style(move |_theme: &Theme, _status: iced::widget::button::Status| {
            iced::widget::button::Style {
                background: Some(Background::Color(accent)),
                text_color: Color::WHITE,
                border: iced::Border::default().rounded(4.0),
                ..Default::default()
            }
        })
        .into()
    } else {
        btn.into()
    }
}
