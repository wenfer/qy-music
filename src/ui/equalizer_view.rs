//! 均衡器视图：10 段增益滑块 + 预设下拉 + 主增益。

use crate::app::message::AppMessage;
use crate::app::state::AppState;
use crate::audio::EqPreset;

use iced::widget::{button, column, container, horizontal_space, pick_list, row, scrollable, slider, text};
use iced::{Element, Length};

/// 10 个频点（Hz）标签。
const FREQ_LABELS: [&str; 10] = [
    "31", "62", "125", "250", "500", "1k", "2k", "4k", "8k", "16k",
];

/// 均衡器视图。
pub fn equalizer_view(state: &AppState) -> Element<'_, AppMessage> {
    let preset_row = row![
        text("预设：").size(13.0),
        pick_list(&EqPreset::ALL[..], Some(&state.equalizer.preset), AppMessage::ApplyEqPreset,)
            .placeholder("预设"),
    ]
    .spacing(8)
    .align_y(iced::alignment::Vertical::Center);

    let bands = state.equalizer.bands;
    let mut bands_column = column![].spacing(6);
    for (i, &freq) in FREQ_LABELS.iter().enumerate() {
        let value = bands[i];
        let on_change = move |v: f32| {
            let mut new_bands = bands;
            new_bands[i] = v;
            AppMessage::SetEqualizerBands(new_bands)
        };
        let slider_row = row![
            text(format!("{freq}Hz")).size(11.0).width(Length::Fixed(46.0)),
            slider(-12.0..=12.0, value, on_change)
                .step(0.5)
                .width(Length::Fill),
            text(format!("{value:+.1}dB")).size(11.0).width(Length::Fixed(54.0)),
        ]
        .spacing(6)
        .align_y(iced::alignment::Vertical::Center);
        bands_column = bands_column.push(slider_row);
    }

    let master_row = row![
        text("主增益：").size(13.0),
        slider(-6.0..=6.0, state.equalizer.master_gain_db, AppMessage::SetMasterGain)
            .step(0.5)
            .width(Length::Fill),
        text(format!("{:.1}dB", state.equalizer.master_gain_db)).size(12.0),
    ]
    .spacing(8)
    .align_y(iced::alignment::Vertical::Center);

    let reset = button(text("重置").size(12.0)).on_press(AppMessage::ApplyEqPreset(EqPreset::Flat));

    column![preset_row, bands_column, master_row, reset]
        .spacing(8)
        .into()
}

/// 可折叠均衡器面板（T14，主界面⑦区）。
///
/// - 收起（默认）：仅一行头部，高约 28px，不遮挡列表 / 歌词；
/// - 展开：160px 高面板，内部 `scrollable` 竖向滚动，
///   保证 10 段（31Hz–16kHz）+ 预设 + 主增益 **完整可见可调**。
pub fn eq_panel(state: &AppState) -> Element<'_, AppMessage> {
    let arrow = if state.eq_expanded { "▼" } else { "▶" };
    let header = row![
        button(text(format!("均衡器 {arrow}")).size(12.0))
            .on_press(AppMessage::ToggleEqPanel),
        text(format!(
            "{} · 主增益 {:.1} dB",
            state.equalizer.preset, state.equalizer.master_gain_db
        ))
        .size(11.0),
        horizontal_space(),
    ]
    .spacing(8)
    .align_y(iced::alignment::Vertical::Center)
    .width(Length::Fill);

    if state.eq_expanded {
        let body = scrollable(equalizer_view(state))
            .width(Length::Fill)
            .height(Length::Fill);
        column![header, container(body).height(Length::Fixed(160.0)).width(Length::Fill)]
            .spacing(4)
            .into()
    } else {
        header.into()
    }
}
