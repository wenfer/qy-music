//! 主窗口视图（T12：千千静听经典竖窄单列七分区布局，极简克制，精致高效）。
//!
//! 纵向七分区，所有分区宽度 `Length::Fill`（拉宽仍是单列，内容自适应）：
//! ① 标题栏 ② 当前曲目 ③ LED 频谱窄条（Fixed 80） ④ 进度 + 时间
//! ⑤ 控制行（上下首 / 播放 / 音量 / 循环） ⑥ 列表 / 歌词 Tab 主体（Fill ≥260）
//! ⑦ 可折叠均衡器面板（收起 28px / 展开 160px 内部滚动）。

use crate::app::message::{AppMessage, MainTab};
use crate::app::state::AppState;
use crate::ui::controls;
use crate::ui::equalizer_view;
use crate::ui::lyrics_view;
use crate::ui::playlist_view;
use crate::ui::style::{
    hifi_pick_list_style, pill_tab_button_style, subtle_button_style, window_container_style,
    Palette,
};
use crate::ui::widgets::spectrum_widget;

use iced::widget::{button, column, container, pick_list, row, space, text};
use iced::{Background, Border, Color, Element, Length};

/// 主窗口视图：纵向单列七分区（一体化通透布局，导航与工具栏水平整合）。
pub fn main_view(state: &AppState) -> Element<'_, AppMessage> {
    let palette = Palette::from_skin(&state.skin);

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
    .spacing(7)
    .padding(8)
    .width(Length::Fill)
    .height(Length::Fill);

    container(content)
        .style(window_container_style(palette))
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

/// ① 标题栏：左侧极简品牌字标，右侧小巧功能入口。
fn build_header(state: &AppState) -> Element<'_, AppMessage> {
    let palette = Palette::from_skin(&state.skin);

    let skin_picker = pick_list(
        crate::theme::Skin::builtin_static(),
        Some(&state.skin),
        |skin| AppMessage::SetSkin(skin.id.clone()),
    )
    .placeholder("皮肤")
    .text_size(11.0)
    .style(hifi_pick_list_style(palette));

    let mini_lyrics_btn = button(text("歌词").size(11.0))
        .padding([3, 7])
        .style(subtle_button_style(palette))
        .on_press(AppMessage::ToggleMiniLyrics);

    let mini_btn = button(text("迷你").size(11.0))
        .padding([3, 7])
        .style(subtle_button_style(palette))
        .on_press(AppMessage::EnterMiniMode);

    row![skin_picker, space::horizontal(), mini_lyrics_btn, mini_btn,]
        .spacing(5)
        .align_y(iced::alignment::Vertical::Center)
        .width(Length::Fill)
        .into()
}

/// ② 当前曲目：纯净展示歌曲与歌手，未播放时仅居中显示“未播放”。
fn build_now_playing(state: &AppState) -> Element<'_, AppMessage> {
    let palette = Palette::from_skin(&state.skin);

    if let Some(track) = state.current_track() {
        let artist_str = if track.artist.is_empty() {
            "—".to_string()
        } else {
            track.artist.clone()
        };

        let title = text(track.title.clone())
            .size(14.0)
            .style(move |_theme: &iced::Theme| text::Style {
                color: Some(palette.text_main),
            });

        let artist = text(artist_str)
            .size(11.0)
            .style(move |_theme: &iced::Theme| text::Style {
                color: Some(palette.text_muted),
            });

        let title_row =
            if track.is_lossless() {
                let badge = container(text("无损").size(9.0).style(move |_theme: &iced::Theme| {
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
                });

                row![title, badge]
                    .spacing(6)
                    .align_y(iced::alignment::Vertical::Center)
            } else {
                row![title].align_y(iced::alignment::Vertical::Center)
            };

        column![title_row, artist]
            .spacing(1)
            .align_x(iced::alignment::Horizontal::Center)
            .width(Length::Fill)
            .into()
    } else {
        container(
            text("未播放")
                .size(12.0)
                .style(move |_theme: &iced::Theme| text::Style {
                    color: Some(palette.text_muted),
                }),
        )
        .padding([3, 0])
        .align_x(iced::alignment::Horizontal::Center)
        .width(Length::Fill)
        .into()
    }
}

/// ③ LED 频谱窄条（高 56，宽度 Fill，轻盈灵动通透）。
fn build_spectrum_bar(state: &AppState) -> Element<'_, AppMessage> {
    container(spectrum_widget(state))
        .width(Length::Fill)
        .height(Length::Fixed(56.0))
        .into()
}

/// ⑥ 列表 / 歌词 Tab 主体（Tab 导航与操作按钮水平整合，极致节省垂直空间）。
fn build_tab_body(state: &AppState) -> Element<'_, AppMessage> {
    let palette = Palette::from_skin(&state.skin);
    let track_count = state.playlist.tracks.len();

    let playlist_label = if track_count > 0 {
        format!("播放列表 ({track_count})")
    } else {
        "播放列表".to_string()
    };

    let tabs = row![
        tab_button(&playlist_label, MainTab::Playlist, state),
        tab_button("歌词", MainTab::Lyrics, state),
    ]
    .spacing(4);

    // 右侧与当前激活 Tab 一致的快捷操作栏
    let action_bar = match state.main_tab {
        MainTab::Playlist => {
            let add_folder_btn = button(text("＋ 文件夹").size(10.5))
                .padding([2, 6])
                .style(subtle_button_style(palette))
                .on_press(AppMessage::AddFolder);

            let add_file_btn = button(text("＋ 文件").size(10.5))
                .padding([2, 6])
                .style(subtle_button_style(palette))
                .on_press(AppMessage::AddFiles);

            let clear_btn = button(text("清空").size(10.5))
                .padding([2, 6])
                .style(subtle_button_style(palette))
                .on_press(AppMessage::ClearPlaylist);

            row![add_folder_btn, add_file_btn, clear_btn].spacing(4)
        }
        MainTab::Lyrics => {
            let load_btn = button(text("加载").size(10.5))
                .padding([2, 6])
                .style(subtle_button_style(palette))
                .on_press(AppMessage::LoadLyrics(std::path::PathBuf::new()));

            let mini_lyrics_btn = button(text("浮窗").size(10.5))
                .padding([2, 6])
                .style(subtle_button_style(palette))
                .on_press(AppMessage::ToggleMiniLyrics);

            row![load_btn, mini_lyrics_btn].spacing(4)
        }
    };

    let header_row = row![tabs, space::horizontal(), action_bar,]
        .align_y(iced::alignment::Vertical::Center)
        .width(Length::Fill);

    let body: Element<'_, AppMessage> = match state.main_tab {
        MainTab::Playlist => playlist_view::playlist_view(state),
        MainTab::Lyrics => lyrics_view::lyrics_view(state),
    };

    column![
        header_row,
        container(body).width(Length::Fill).height(Length::Fill)
    ]
    .spacing(4)
    .width(Length::Fill)
    .height(Length::Fill)
    .into()
}

/// Tab 按钮：极简紧凑胶囊按钮。
fn tab_button<'a>(label: &str, tab: MainTab, state: &AppState) -> Element<'a, AppMessage> {
    let is_active = state.main_tab == tab;
    let palette = Palette::from_skin(&state.skin);

    button(text(label.to_string()).size(11.0))
        .padding([4, 8])
        .style(pill_tab_button_style(palette, is_active))
        .on_press(AppMessage::SwitchMainTab(tab))
        .into()
}
