//! 均衡器视图：10 段增益滑块 + 预设下拉 + 主增益（简洁、统一、现代）。

use crate::app::message::AppMessage;
use crate::app::state::AppState;
use crate::audio::EqPreset;
use crate::ui::style::{
    card_style, effect_toggle_button_style, hifi_pick_list_style, hifi_scrollable_style,
    hifi_slider_style, subtle_button_style, Palette,
};

use iced::widget::{button, column, container, pick_list, row, scrollable, slider, space, text};
use iced::{Background, Color, Element, Length};

/// 10 个频点（Hz）标签。
const FREQ_LABELS: [&str; 10] = [
    "31", "62", "125", "250", "500", "1k", "2k", "4k", "8k", "16k",
];

/// 均衡器展开视图。
pub fn equalizer_view(state: &AppState) -> Element<'_, AppMessage> {
    let palette = Palette::from_skin(&state.skin);

    let preset_picker = pick_list(
        &EqPreset::ALL[..],
        Some(&state.equalizer.preset),
        AppMessage::ApplyEqPreset,
    )
    .placeholder("预设")
    .text_size(11.0)
    .style(hifi_pick_list_style(palette));

    let reset_btn = button(text("重置").size(11.0))
        .padding([3, 7])
        .style(subtle_button_style(palette))
        .on_press(AppMessage::ApplyEqPreset(EqPreset::Flat));

    let preset_row = row![
        text("预设:")
            .size(11.0)
            .style(move |_theme: &iced::Theme| text::Style {
                color: Some(palette.text_sub),
            }),
        preset_picker,
        space::horizontal(),
        reset_btn,
    ]
    .spacing(6)
    .align_y(iced::alignment::Vertical::Center)
    .width(Length::Fill);

    // ── DSP 音效增强模块 ──
    let fx = &state.effects;
    let widener_pct = (fx.stereo_widener_level * 100.0).round() as u32;
    let bass_pct = (fx.bass_boost_level * 100.0).round() as u32;
    let crystal_pct = (fx.vocal_crystalizer_level * 100.0).round() as u32;

    let widener_row = row![
        button(text("3D 空间环绕").size(10.0))
            .padding([2, 6])
            .style(effect_toggle_button_style(
                palette,
                fx.stereo_widener_enabled
            ))
            .on_press(AppMessage::ToggleStereoWidener),
        slider(
            0.0..=1.0,
            fx.stereo_widener_level,
            AppMessage::SetStereoWidenerLevel
        )
        .step(0.05)
        .width(Length::Fill)
        .style(hifi_slider_style(palette)),
        text(format!("{widener_pct}%"))
            .size(10.0)
            .width(Length::Fixed(32.0))
            .style(move |_theme: &iced::Theme| text::Style {
                color: Some(if fx.stereo_widener_enabled {
                    palette.accent
                } else {
                    palette.text_muted
                }),
            }),
    ]
    .spacing(6)
    .align_y(iced::alignment::Vertical::Center);

    let bass_row = row![
        button(text("动态低音").size(10.0))
            .padding([2, 6])
            .style(effect_toggle_button_style(palette, fx.bass_boost_enabled))
            .on_press(AppMessage::ToggleBassBoost),
        slider(
            0.0..=1.0,
            fx.bass_boost_level,
            AppMessage::SetBassBoostLevel
        )
        .step(0.05)
        .width(Length::Fill)
        .style(hifi_slider_style(palette)),
        text(format!("{bass_pct}%"))
            .size(10.0)
            .width(Length::Fixed(32.0))
            .style(move |_theme: &iced::Theme| text::Style {
                color: Some(if fx.bass_boost_enabled {
                    palette.accent
                } else {
                    palette.text_muted
                }),
            }),
    ]
    .spacing(6)
    .align_y(iced::alignment::Vertical::Center);

    let crystal_row = row![
        button(text("人声水晶").size(10.0))
            .padding([2, 6])
            .style(effect_toggle_button_style(
                palette,
                fx.vocal_crystalizer_enabled
            ))
            .on_press(AppMessage::ToggleVocalCrystalizer),
        slider(
            0.0..=1.0,
            fx.vocal_crystalizer_level,
            AppMessage::SetVocalCrystalizerLevel
        )
        .step(0.05)
        .width(Length::Fill)
        .style(hifi_slider_style(palette)),
        text(format!("{crystal_pct}%"))
            .size(10.0)
            .width(Length::Fixed(32.0))
            .style(move |_theme: &iced::Theme| text::Style {
                color: Some(if fx.vocal_crystalizer_enabled {
                    palette.accent
                } else {
                    palette.text_muted
                }),
            }),
    ]
    .spacing(6)
    .align_y(iced::alignment::Vertical::Center);

    let effects_section = column![
        text("DSP 音效增强")
            .size(10.0)
            .style(move |_theme: &iced::Theme| text::Style {
                color: Some(palette.text_muted),
            }),
        widener_row,
        bass_row,
        crystal_row,
    ]
    .spacing(4);

    let master_row = row![
        text("主增益:")
            .size(11.0)
            .width(Length::Fixed(40.0))
            .style(move |_theme: &iced::Theme| text::Style {
                color: Some(palette.text_sub),
            }),
        slider(
            -6.0..=6.0,
            state.equalizer.master_gain_db,
            AppMessage::SetMasterGain
        )
        .step(0.5)
        .width(Length::Fill)
        .style(hifi_slider_style(palette)),
        text(format!("{:.1}dB", state.equalizer.master_gain_db))
            .size(10.0)
            .width(Length::Fixed(44.0))
            .style(move |_theme: &iced::Theme| text::Style {
                color: Some(palette.text_main),
            }),
    ]
    .spacing(6)
    .align_y(iced::alignment::Vertical::Center);

    let bands = state.equalizer.bands;
    let mut bands_column =
        column![text("10 段参数均衡器 (EQ)")
            .size(10.0)
            .style(move |_theme: &iced::Theme| text::Style {
                color: Some(palette.text_muted),
            })]
        .spacing(3);

    for (i, &freq) in FREQ_LABELS.iter().enumerate() {
        let value = bands[i];
        let on_change = move |v: f32| {
            let mut new_bands = bands;
            new_bands[i] = v;
            AppMessage::SetEqualizerBands(new_bands)
        };
        let slider_row = row![
            text(format!("{freq}Hz"))
                .size(10.0)
                .width(Length::Fixed(40.0))
                .style(move |_theme: &iced::Theme| text::Style {
                    color: Some(palette.text_sub),
                }),
            slider(-12.0..=12.0, value, on_change)
                .step(0.5)
                .width(Length::Fill)
                .style(hifi_slider_style(palette)),
            text(format!("{value:+.1}dB"))
                .size(10.0)
                .width(Length::Fixed(44.0))
                .style(move |_theme: &iced::Theme| text::Style {
                    color: Some(if value.abs() > 0.1 {
                        palette.accent
                    } else {
                        palette.text_muted
                    }),
                }),
        ]
        .spacing(6)
        .align_y(iced::alignment::Vertical::Center);
        bands_column = bands_column.push(slider_row);
    }

    column![preset_row, master_row, effects_section, bands_column]
        .spacing(8)
        .padding(4)
        .into()
}

/// 可折叠均衡器面板（T14，主界面⑦区）。
///
/// - 收起（默认）：仅一行头部，高约 28px，不遮挡列表 / 歌词；
/// - 展开：160px 高面板，内部 `scrollable` 竖向滚动，
///   保证 10 段（31Hz–16kHz）+ 预设 + 主增益 + DSP 音效 **完整可见可调**。
pub fn eq_panel(state: &AppState) -> Element<'_, AppMessage> {
    let palette = Palette::from_skin(&state.skin);
    let arrow = if state.eq_expanded { "▼" } else { "▶" };

    let toggle_btn = button(text(format!("EQ 均衡器与音效 {arrow}")).size(11.0))
        .padding([3, 7])
        .style(subtle_button_style(palette))
        .on_press(AppMessage::ToggleEqPanel);

    let mut effect_tags = Vec::new();
    if state.effects.stereo_widener_enabled {
        effect_tags.push("3D环绕");
    }
    if state.effects.bass_boost_enabled {
        effect_tags.push("低音");
    }
    if state.effects.vocal_crystalizer_enabled {
        effect_tags.push("人声");
    }

    let summary_str = if effect_tags.is_empty() {
        format!(
            "{} · 主增益 {:.1} dB",
            state.equalizer.preset, state.equalizer.master_gain_db
        )
    } else {
        format!(
            "{} · {} · {:.1} dB",
            state.equalizer.preset,
            effect_tags.join("+"),
            state.equalizer.master_gain_db
        )
    };

    let summary_text = text(summary_str)
        .size(10.0)
        .style(move |_theme: &iced::Theme| text::Style {
            color: Some(if effect_tags.is_empty() {
                palette.text_muted
            } else {
                palette.accent
            }),
        });

    let header_content = row![toggle_btn, summary_text, space::horizontal(),]
        .spacing(6)
        .align_y(iced::alignment::Vertical::Center)
        .width(Length::Fill);

    let header = container(header_content)
        .padding([2, 4])
        .style(move |_theme| container::Style {
            background: Some(Background::Color(Color::from_rgba(1.0, 1.0, 1.0, 0.02))),
            border: iced::Border::default().rounded(6.0),
            ..Default::default()
        })
        .width(Length::Fill);

    if state.eq_expanded {
        let body = scrollable(equalizer_view(state))
            .style(hifi_scrollable_style(palette))
            .width(Length::Fill)
            .height(Length::Fill);

        let body_container = container(body)
            .padding(4)
            .style(card_style(palette))
            .height(Length::Fixed(160.0))
            .width(Length::Fill);

        column![header, body_container].spacing(4).into()
    } else {
        header.into()
    }
}
