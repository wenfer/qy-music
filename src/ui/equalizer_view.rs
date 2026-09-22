//! 主窗口底部音效控制条（Zone 7：千千静听经典单列底部紧凑音效栏）。
//!
//! 提供快捷打开独立「音效控制台」弹窗按钮、音效预设下拉选择、活跃效果标签提示与纯净直通开关。

use crate::app::message::AppMessage;
use crate::app::state::AppState;
use crate::ui::style::{effect_toggle_button_style, hifi_pick_list_style, Palette};

use iced::widget::{button, container, pick_list, row, space, text};
use iced::{Background, Color, Element, Length};

/// 主界面 Zone 7：紧凑音效控制条。
pub fn eq_panel(state: &AppState) -> Element<'_, AppMessage> {
    let palette = Palette::from_skin(&state.skin);

    let console_btn = button(
        text(if state.effects_window_id.is_some() {
            "◈ 控制台已开启"
        } else {
            "◈ 音效控制台 ↗"
        })
        .size(10.5),
    )
    .padding([3, 7])
    .style(effect_toggle_button_style(
        palette,
        state.effects_window_id.is_some(),
    ))
    .on_press(AppMessage::OpenEffectsWindow);

    let all_presets = state.all_presets();
    let current_preset = state.active_preset();

    let preset_picker = pick_list(
        all_presets,
        current_preset,
        |p: crate::audio::SoundPreset| AppMessage::SelectSoundPreset(p.id),
    )
    .placeholder("选择音效预设")
    .text_size(10.5)
    .style(hifi_pick_list_style(palette));

    let direct_btn = button(
        text(if state.effects.pure_direct {
            "DIRECT 直通"
        } else {
            "直通"
        })
        .size(10.0),
    )
    .padding([3, 6])
    .style(effect_toggle_button_style(
        palette,
        state.effects.pure_direct,
    ))
    .on_press(AppMessage::TogglePureDirect);

    let mut effect_tags = Vec::new();
    if state.effects.pure_direct {
        effect_tags.push("直通");
    } else {
        if state.effects.tube_warmth_enabled {
            effect_tags.push("胆机");
        }
        if state.effects.spatial_audio_enabled {
            if state.effects.bs2b_mode {
                effect_tags.push("BS2B");
            } else {
                effect_tags.push("全景");
            }
        }
        if state.effects.bass_boost_enabled {
            effect_tags.push("低音");
        }
        if state.effects.vocal_crystalizer_enabled {
            effect_tags.push("人声");
        }
    }

    let summary_text = if !effect_tags.is_empty() {
        text(effect_tags.join("+"))
            .size(9.5)
            .style(move |_theme: &iced::Theme| text::Style {
                color: Some(palette.accent),
            })
    } else {
        text(format!("{:.1}dB", state.equalizer.master_gain_db))
            .size(9.5)
            .style(move |_theme: &iced::Theme| text::Style {
                color: Some(palette.text_muted),
            })
    };

    let bar_content = row![
        console_btn,
        preset_picker,
        space::horizontal(),
        summary_text,
        direct_btn,
    ]
    .spacing(5)
    .align_y(iced::alignment::Vertical::Center)
    .width(Length::Fill);

    container(bar_content)
        .padding([2, 4])
        .style(move |_theme| container::Style {
            background: Some(Background::Color(Color::from_rgba(1.0, 1.0, 1.0, 0.025))),
            border: iced::Border {
                color: palette.border_subtle,
                width: 1.0,
                radius: 6.0.into(),
            },
            ..Default::default()
        })
        .width(Length::Fill)
        .into()
}
