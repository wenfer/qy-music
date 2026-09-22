//! 独立音效与 DSP 调音台窗口视图（独立弹窗、宽阔舒适、专业发烧）。
//!
//! 提供 10 段参数均衡器（左右对称两列推杆）、发烧耳机专属校准预设、
//! 模拟电子管胆机暖音、经典 BS2B 耳放跨耳互馈、影院全景空间声场、
//! 动态低音增强、人声水晶通透与实时音频链路诊断。

use crate::app::message::AppMessage;
use crate::app::state::AppState;
use crate::audio::EqPreset;
use crate::ui::style::{
    card_style, effect_toggle_button_style, hifi_pick_list_style, hifi_scrollable_style,
    hifi_slider_style, hifi_text_input_style, lcd_display_style, subtle_button_style,
    window_container_style, Palette,
};

use iced::widget::{
    button, column, container, pick_list, row, scrollable, slider, space, text, text_input,
};
use iced::{Background, Border, Color, Element, Length};

use crate::audio::EQ_BAND_LABELS as FREQ_LABELS;

/// 独立音效控制台主视图。
pub fn effects_console_view(state: &AppState) -> Element<'_, AppMessage> {
    let palette = Palette::from_skin(&state.skin);

    let content = column![
        build_header_card(state, palette),
        build_preset_card(state, palette),
        build_inspector_card(state, palette),
        build_eq_card(state, palette),
        build_dsp_card(state, palette),
    ]
    .spacing(12)
    .width(Length::Fill);

    let scroll = scrollable(content)
        .style(hifi_scrollable_style(palette))
        .width(Length::Fill)
        .height(Length::Fill);

    container(scroll)
        .padding(14)
        .style(window_container_style(palette))
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

/// 顶部主控栏：标题、直通切换、重置、关闭。
fn build_header_card(state: &AppState, palette: Palette) -> Element<'_, AppMessage> {
    let title = column![
        text("◈ 音频 DSP 与音效控制台")
            .size(15.0)
            .style(move |_theme: &iced::Theme| text::Style {
                color: Some(palette.text_main),
            }),
        text("10-Band EQ · Triode Tube · BS2B Binaural · Bit-Perfect")
            .size(9.5)
            .style(move |_theme: &iced::Theme| text::Style {
                color: Some(palette.text_muted),
            }),
    ]
    .spacing(2);

    let direct_btn = button(
        text(if state.effects.pure_direct {
            "DIRECT 直通中 ◈"
        } else {
            "Hi-Fi 纯净直通"
        })
        .size(11.0),
    )
    .padding([4, 10])
    .style(effect_toggle_button_style(
        palette,
        state.effects.pure_direct,
    ))
    .on_press(AppMessage::TogglePureDirect);

    let reset_btn = button(text("重置全部").size(11.0))
        .padding([4, 8])
        .style(subtle_button_style(palette))
        .on_press(AppMessage::ResetAllEffects);

    let close_btn = button(text("关闭 ×").size(11.0))
        .padding([4, 8])
        .style(subtle_button_style(palette))
        .on_press(AppMessage::CloseEffectsWindow);

    let action_row = row![direct_btn, reset_btn, close_btn,]
        .spacing(8)
        .align_y(iced::alignment::Vertical::Center);

    row![title, space::horizontal(), action_row,]
        .align_y(iced::alignment::Vertical::Center)
        .width(Length::Fill)
        .into()
}

/// 音效预设管理卡片（支持选择、新建保存、编辑修改、删除、导入、导出）。
fn build_preset_card(state: &AppState, palette: Palette) -> Element<'_, AppMessage> {
    let all_presets = state.all_presets();
    let current_preset = state.active_preset();

    let preset_picker = pick_list(
        all_presets,
        current_preset.clone(),
        |p: crate::audio::SoundPreset| AppMessage::SelectSoundPreset(p.id),
    )
    .placeholder("选择音效预设")
    .text_size(11.0)
    .style(hifi_pick_list_style(palette));

    let is_custom = current_preset
        .as_ref()
        .map(|p| !p.is_builtin)
        .unwrap_or(false);

    let mut actions_row = row![
        text("预设:")
            .size(11.0)
            .style(move |_theme: &iced::Theme| text::Style {
                color: Some(palette.text_sub),
            }),
        preset_picker,
    ]
    .spacing(6)
    .align_y(iced::alignment::Vertical::Center);

    if is_custom {
        let save_active_btn = button(text("保存修改").size(10.5))
            .padding([3, 7])
            .style(subtle_button_style(palette))
            .on_press(AppMessage::SaveActivePreset);

        let delete_btn = button(text("✕ 删除").size(10.5))
            .padding([3, 7])
            .style(subtle_button_style(palette))
            .on_press(AppMessage::DeletePreset(state.active_preset_id.clone()));

        actions_row = actions_row.push(save_active_btn).push(delete_btn);
    }

    let export_btn = button(text("导出预设 ↗").size(10.5))
        .padding([3, 7])
        .style(subtle_button_style(palette))
        .on_press(AppMessage::ExportPreset);

    let import_btn = button(text("导入预设 ＋").size(10.5))
        .padding([3, 7])
        .style(subtle_button_style(palette))
        .on_press(AppMessage::ImportPreset);

    let top_row = row![actions_row, space::horizontal(), import_btn, export_btn,]
        .spacing(6)
        .align_y(iced::alignment::Vertical::Center)
        .width(Length::Fill);

    let name_input = text_input("输入新预设名称...", &state.preset_name_input)
        .size(11.0)
        .padding([3, 7])
        .width(Length::Fixed(160.0))
        .style(hifi_text_input_style(palette))
        .on_input(AppMessage::SetPresetNameInput);

    let save_new_btn = button(text("＋ 保存为新预设").size(10.5))
        .padding([3, 8])
        .style(effect_toggle_button_style(palette, true))
        .on_press(AppMessage::SaveCurrentAsNewPreset);

    let tip_text = text(if is_custom {
        "当前为自定义预设，调节参数后点击【保存修改】更新，或另存为新预设"
    } else {
        "内置预设为只读模板，调节 EQ 与音效参数后点击【保存为新预设】即可保存"
    })
    .size(9.5)
    .style(move |_theme: &iced::Theme| text::Style {
        color: Some(palette.text_muted),
    });

    let add_row = row![
        text("新建:")
            .size(10.5)
            .style(move |_theme: &iced::Theme| text::Style {
                color: Some(palette.text_sub),
            }),
        name_input,
        save_new_btn,
        space::horizontal(),
        tip_text,
    ]
    .spacing(6)
    .align_y(iced::alignment::Vertical::Center)
    .width(Length::Fill);

    container(column![top_row, add_row].spacing(6))
        .padding([8, 12])
        .style(card_style(palette))
        .width(Length::Fill)
        .into()
}

/// 实时音频链路与 Hi-Res 诊断 LCD 卡片。
fn build_inspector_card(state: &AppState, palette: Palette) -> Element<'_, AppMessage> {
    let mut badges = Vec::new();

    if let Some(info) = &state.format_info {
        if info.is_hi_res {
            badges.push(build_badge("Hi-Res ◈", palette.accent, palette));
        }
    }

    if state.effects.pure_direct {
        badges.push(build_badge("纯净直通", palette.vfd_green, palette));
    } else {
        if state.effects.tube_warmth_enabled {
            badges.push(build_badge(
                "电子管胆味",
                Color::from_rgb(0.96, 0.65, 0.28),
                palette,
            ));
        }
        if state.effects.spatial_audio_enabled {
            if state.effects.bs2b_mode {
                badges.push(build_badge(
                    "BS2B耳放互馈",
                    Color::from_rgb(0.35, 0.75, 1.0),
                    palette,
                ));
            } else {
                badges.push(build_badge("全景空间", palette.accent, palette));
            }
        }
    }

    let mut badges_row =
        row![text("实时信号链路诊断")
            .size(10.0)
            .style(move |_theme: &iced::Theme| text::Style {
                color: Some(palette.text_muted),
            })]
        .spacing(6)
        .align_y(iced::alignment::Vertical::Center);

    for b in badges {
        badges_row = badges_row.push(b);
    }

    let chain_desc = if let Some(info) = &state.format_info {
        let dsp_status = if state.effects.pure_direct {
            "零染色直通 (Bypass)"
        } else {
            "10段EQ + DSP处理"
        };
        format!(
            "音频源: {} kHz / {} 声道  ➔  重采样/DSP: {}  ➔  DAC输出: {} kHz CPAL 低延迟",
            info.sample_rate / 1000,
            info.channels,
            dsp_status,
            info.device_rate / 1000
        )
    } else if state.effects.pure_direct {
        "Hi-Fi 纯净直通模式已就绪 · 绕过全部 EQ 与 DSP 算法，100% 原始位深位完美输出".to_string()
    } else {
        "DAC 声卡设备就绪 · 48.0 kHz 32-bit Float · 实时 DSP 处理链待命".to_string()
    };

    let chain_text = text(chain_desc)
        .size(10.5)
        .style(move |_theme: &iced::Theme| text::Style {
            color: Some(if state.effects.pure_direct {
                palette.vfd_green
            } else {
                palette.text_main
            }),
        });

    container(column![badges_row, chain_text].spacing(4))
        .padding([8, 12])
        .style(lcd_display_style(palette))
        .width(Length::Fill)
        .into()
}

/// 徽章生成辅助。
fn build_badge<'a>(label: &'a str, color: Color, _palette: Palette) -> Element<'a, AppMessage> {
    container(
        text(label)
            .size(9.5)
            .style(move |_theme: &iced::Theme| text::Style { color: Some(color) }),
    )
    .padding([2, 6])
    .style(move |_theme| container::Style {
        background: Some(Background::Color(Color::from_rgba(
            color.r, color.g, color.b, 0.12,
        ))),
        border: Border {
            color: Color::from_rgba(color.r, color.g, color.b, 0.40),
            width: 1.0,
            radius: 4.0.into(),
        },
        ..Default::default()
    })
    .into()
}

/// 10 段参数均衡器卡片（两列宽阔推杆，同时全部可见，支持预设与主增益）。
fn build_eq_card(state: &AppState, palette: Palette) -> Element<'_, AppMessage> {
    let preset_picker = pick_list(
        &EqPreset::ALL[..],
        Some(&state.equalizer.preset),
        AppMessage::ApplyEqPreset,
    )
    .placeholder("选择调音预设")
    .text_size(11.0)
    .style(hifi_pick_list_style(palette));

    let reset_eq_btn = button(text("复位平直").size(10.5))
        .padding([3, 7])
        .style(subtle_button_style(palette))
        .on_press(AppMessage::ApplyEqPreset(EqPreset::Flat));

    let master_slider = slider(
        -6.0..=6.0,
        state.equalizer.master_gain_db,
        AppMessage::SetMasterGain,
    )
    .step(0.5_f32)
    .width(Length::Fixed(120.0))
    .style(hifi_slider_style(palette));

    let clip_text = if state.gain_clipped() {
        text(" [CLIP]")
            .size(10.0)
            .style(move |_theme: &iced::Theme| text::Style {
                color: Some(Color::from_rgb(0.95, 0.35, 0.35)),
            })
    } else {
        text("")
            .size(10.0)
            .style(move |_theme: &iced::Theme| text::Style {
                color: Some(palette.text_muted),
            })
    };

    let master_group = row![
        text("主增益:")
            .size(10.5)
            .style(move |_theme: &iced::Theme| text::Style {
                color: Some(palette.text_sub),
            }),
        master_slider,
        text(format!("{:.1}dB", state.equalizer.master_gain_db))
            .size(10.0)
            .style(move |_theme: &iced::Theme| text::Style {
                color: Some(palette.text_main),
            }),
        clip_text,
    ]
    .spacing(6)
    .align_y(iced::alignment::Vertical::Center);

    let card_header = row![
        text("10 段专业参数均衡器 (EQ)")
            .size(12.5)
            .style(move |_theme: &iced::Theme| text::Style {
                color: Some(palette.text_main),
            }),
        preset_picker,
        reset_eq_btn,
        space::horizontal(),
        master_group,
    ]
    .spacing(8)
    .align_y(iced::alignment::Vertical::Center)
    .width(Length::Fill);

    let bands = state.equalizer.bands;

    // 左列 5 个频点（低频段 31Hz - 500Hz）
    let mut left_col = column![].spacing(6).width(Length::FillPortion(1));
    for i in 0..5 {
        let freq = FREQ_LABELS[i];
        let value = bands[i];
        let on_change = move |v: f32| {
            let mut new_bands = bands;
            new_bands[i] = v;
            AppMessage::SetEqualizerBands(new_bands)
        };
        let row_elem = row![
            text(format!("{freq:>4}Hz"))
                .size(10.5)
                .width(Length::Fixed(50.0))
                .style(move |_theme: &iced::Theme| text::Style {
                    color: Some(palette.text_sub),
                }),
            slider(-12.0..=12.0, value, on_change)
                .step(0.5_f32)
                .width(Length::Fill)
                .style(hifi_slider_style(palette)),
            text(format!("{value:+.1}dB"))
                .size(10.0)
                .width(Length::Fixed(46.0))
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
        left_col = left_col.push(row_elem);
    }

    // 右列 5 个频点（高频段 1kHz - 16kHz）
    let mut right_col = column![].spacing(6).width(Length::FillPortion(1));
    for i in 5..10 {
        let freq = FREQ_LABELS[i];
        let value = bands[i];
        let on_change = move |v: f32| {
            let mut new_bands = bands;
            new_bands[i] = v;
            AppMessage::SetEqualizerBands(new_bands)
        };
        let row_elem = row![
            text(format!("{freq:>4}Hz"))
                .size(10.5)
                .width(Length::Fixed(50.0))
                .style(move |_theme: &iced::Theme| text::Style {
                    color: Some(palette.text_sub),
                }),
            slider(-12.0..=12.0, value, on_change)
                .step(0.5_f32)
                .width(Length::Fill)
                .style(hifi_slider_style(palette)),
            text(format!("{value:+.1}dB"))
                .size(10.0)
                .width(Length::Fixed(46.0))
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
        right_col = right_col.push(row_elem);
    }

    let sliders_grid = row![left_col, space::horizontal().width(16), right_col,]
        .width(Length::Fill)
        .align_y(iced::alignment::Vertical::Top);

    container(column![card_header, sliders_grid].spacing(10))
        .padding(12)
        .style(card_style(palette))
        .width(Length::Fill)
        .into()
}

/// 发烧级 DSP 音效增强机架卡片。
fn build_dsp_card(state: &AppState, palette: Palette) -> Element<'_, AppMessage> {
    let fx = &state.effects;
    let tube_pct = (fx.tube_warmth_level * 100.0).round() as u32;
    let spatial_pct = (fx.spatial_audio_level * 100.0).round() as u32;
    let dialogue_pct = (fx.dialogue_clarity_level * 100.0).round() as u32;
    let bass_pct = (fx.bass_boost_level * 100.0).round() as u32;
    let crystal_pct = (fx.vocal_crystalizer_level * 100.0).round() as u32;

    let dsp_title = text("发烧级 DSP 音效机架")
        .size(12.5)
        .style(move |_theme: &iced::Theme| text::Style {
            color: Some(palette.text_main),
        });

    // ── 1. 模拟电子管胆机暖音 ──
    let tube_toggle = button(text("模拟胆机暖音").size(10.5))
        .padding([3, 8])
        .style(effect_toggle_button_style(palette, fx.tube_warmth_enabled))
        .on_press(AppMessage::ToggleTubeWarmth);

    let tube_desc =
        text("模拟三极管非线性偶次谐波微失真，温润模拟韵味，消除数码燥感 (带 DC Blocker)")
            .size(10.0)
            .style(move |_theme: &iced::Theme| text::Style {
                color: Some(palette.text_muted),
            });

    let tube_header_row = row![tube_toggle, tube_desc,]
        .spacing(8)
        .align_y(iced::alignment::Vertical::Center);

    let tube_slider_row = row![
        text("暖音浓度:")
            .size(10.5)
            .width(Length::Fixed(60.0))
            .style(move |_theme: &iced::Theme| text::Style {
                color: Some(if fx.tube_warmth_enabled {
                    palette.text_main
                } else {
                    palette.text_muted
                }),
            }),
        slider(
            0.0..=1.0,
            fx.tube_warmth_level,
            AppMessage::SetTubeWarmthLevel
        )
        .step(0.01_f32)
        .width(Length::Fill)
        .style(hifi_slider_style(palette)),
        text(format!("{tube_pct}%"))
            .size(10.0)
            .width(Length::Fixed(36.0))
            .style(move |_theme: &iced::Theme| text::Style {
                color: Some(if fx.tube_warmth_enabled {
                    palette.accent
                } else {
                    palette.text_muted
                }),
            }),
    ]
    .spacing(6)
    .align_y(iced::alignment::Vertical::Center);

    let tube_box = container(column![tube_header_row, tube_slider_row].spacing(6))
        .padding([8, 10])
        .style(move |_theme| container::Style {
            background: Some(Background::Color(Color::from_rgba(1.0, 1.0, 1.0, 0.02))),
            border: Border::default().rounded(6.0),
            ..Default::default()
        })
        .width(Length::Fill);

    // ── 2. 发烧耳放互馈与空间声场 ──
    let spatial_toggle = button(text("空间声场 / 互馈").size(10.5))
        .padding([3, 8])
        .style(effect_toggle_button_style(
            palette,
            fx.spatial_audio_enabled,
        ))
        .on_press(AppMessage::ToggleSpatialAudio);

    let bs2b_btn = button(
        text(if fx.bs2b_mode {
            "◈ BS2B发烧耳放互馈 (当前)"
        } else {
            "BS2B耳放互馈模式"
        })
        .size(10.0),
    )
    .padding([3, 7])
    .style(effect_toggle_button_style(palette, fx.bs2b_mode))
    .on_press(AppMessage::ToggleBs2bMode);

    let cinema_btn = button(
        text(if !fx.bs2b_mode {
            "◈ 影院全景环绕 (当前)"
        } else {
            "影院全景声场模式"
        })
        .size(10.0),
    )
    .padding([3, 7])
    .style(effect_toggle_button_style(palette, !fx.bs2b_mode))
    .on_press(AppMessage::ToggleBs2bMode);

    let spatial_mode_row = row![spatial_toggle, bs2b_btn, cinema_btn,]
        .spacing(6)
        .align_y(iced::alignment::Vertical::Center);

    let spatial_width_row = row![
        text("声场宽度:")
            .size(10.5)
            .width(Length::Fixed(60.0))
            .style(move |_theme: &iced::Theme| text::Style {
                color: Some(if fx.spatial_audio_enabled {
                    palette.text_main
                } else {
                    palette.text_muted
                }),
            }),
        slider(
            0.0..=1.0,
            fx.spatial_audio_level,
            AppMessage::SetSpatialAudioLevel
        )
        .step(0.01_f32)
        .width(Length::Fill)
        .style(hifi_slider_style(palette)),
        text(format!("{spatial_pct}%"))
            .size(10.0)
            .width(Length::Fixed(36.0))
            .style(move |_theme: &iced::Theme| text::Style {
                color: Some(if fx.spatial_audio_enabled {
                    palette.accent
                } else {
                    palette.text_muted
                }),
            }),
    ]
    .spacing(6)
    .align_y(iced::alignment::Vertical::Center);

    let dialogue_row = row![
        text("对白人声:")
            .size(10.5)
            .width(Length::Fixed(60.0))
            .style(move |_theme: &iced::Theme| text::Style {
                color: Some(if fx.spatial_audio_enabled {
                    palette.text_main
                } else {
                    palette.text_muted
                }),
            }),
        slider(
            0.0..=1.0,
            fx.dialogue_clarity_level,
            AppMessage::SetDialogueClarityLevel
        )
        .step(0.01_f32)
        .width(Length::Fill)
        .style(hifi_slider_style(palette)),
        text(format!("{dialogue_pct}%"))
            .size(10.0)
            .width(Length::Fixed(36.0))
            .style(move |_theme: &iced::Theme| text::Style {
                color: Some(if fx.spatial_audio_enabled {
                    palette.accent
                } else {
                    palette.text_muted
                }),
            }),
    ]
    .spacing(6)
    .align_y(iced::alignment::Vertical::Center);

    let spatial_box =
        container(column![spatial_mode_row, spatial_width_row, dialogue_row].spacing(6))
            .padding([8, 10])
            .style(move |_theme| container::Style {
                background: Some(Background::Color(Color::from_rgba(1.0, 1.0, 1.0, 0.02))),
                border: Border::default().rounded(6.0),
                ..Default::default()
            })
            .width(Length::Fill);

    // ── 3. 动态低音与人声水晶（左右并列） ──
    let bass_col = {
        let toggle = button(text("动态低音增强").size(10.5))
            .padding([3, 7])
            .style(effect_toggle_button_style(palette, fx.bass_boost_enabled))
            .on_press(AppMessage::ToggleBassBoost);
        let slider_row = row![
            slider(
                0.0..=1.0,
                fx.bass_boost_level,
                AppMessage::SetBassBoostLevel
            )
            .step(0.01_f32)
            .width(Length::Fill)
            .style(hifi_slider_style(palette)),
            text(format!("{bass_pct}%"))
                .size(10.0)
                .width(Length::Fixed(36.0))
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

        container(
            column![
                row![
                    toggle,
                    text("75Hz 心理声学低频下潜")
                        .size(10.0)
                        .style(move |_theme: &iced::Theme| text::Style {
                            color: Some(palette.text_muted),
                        })
                ]
                .spacing(6)
                .align_y(iced::alignment::Vertical::Center),
                slider_row,
            ]
            .spacing(6),
        )
        .padding([8, 10])
        .style(move |_theme| container::Style {
            background: Some(Background::Color(Color::from_rgba(1.0, 1.0, 1.0, 0.02))),
            border: Border::default().rounded(6.0),
            ..Default::default()
        })
        .width(Length::FillPortion(1))
    };

    let crystal_col = {
        let toggle = button(text("人声水晶通透").size(10.5))
            .padding([3, 7])
            .style(effect_toggle_button_style(
                palette,
                fx.vocal_crystalizer_enabled,
            ))
            .on_press(AppMessage::ToggleVocalCrystalizer);
        let slider_row = row![
            slider(
                0.0..=1.0,
                fx.vocal_crystalizer_level,
                AppMessage::SetVocalCrystalizerLevel
            )
            .step(0.01_f32)
            .width(Length::Fill)
            .style(hifi_slider_style(palette)),
            text(format!("{crystal_pct}%"))
                .size(10.0)
                .width(Length::Fixed(36.0))
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

        container(
            column![
                row![
                    toggle,
                    text("3.2kHz 纯净高频泛音提升").size(10.0).style(
                        move |_theme: &iced::Theme| text::Style {
                            color: Some(palette.text_muted),
                        }
                    )
                ]
                .spacing(6)
                .align_y(iced::alignment::Vertical::Center),
                slider_row,
            ]
            .spacing(6),
        )
        .padding([8, 10])
        .style(move |_theme| container::Style {
            background: Some(Background::Color(Color::from_rgba(1.0, 1.0, 1.0, 0.02))),
            border: Border::default().rounded(6.0),
            ..Default::default()
        })
        .width(Length::FillPortion(1))
    };

    let dual_row = row![bass_col, space::horizontal().width(8), crystal_col,].width(Length::Fill);

    container(column![dsp_title, tube_box, spatial_box, dual_row].spacing(10))
        .padding(12)
        .style(card_style(palette))
        .width(Length::Fill)
        .into()
}
