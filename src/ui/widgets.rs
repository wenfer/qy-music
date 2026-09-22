//! 共享 UI 控件与主题辅助。
//!
//! 提供：背景容器、LCD 屏包装器、文字、频谱 LED 渲染封装。颜色统一从当前 [`Skin`] 取得。

use crate::app::message::AppMessage;
use crate::app::state::AppState;
use crate::theme::skin_color;
use crate::ui::style::{card_style, lcd_display_style, Palette};

use iced::widget::canvas::Canvas;
use iced::widget::{container, row, text};
use iced::{Background, Color, Element, Length, Theme};

use crate::visualizer::led::LedSpectrum;

/// 以皮肤机身背景色包裹内容。
pub fn themed_container<'a>(
    content: impl Into<Element<'a, AppMessage>>,
    state: &AppState,
) -> container::Container<'a, AppMessage> {
    let palette = Palette::from_skin(&state.skin);
    container(content).style(crate::ui::style::window_container_style(palette))
}

/// 以功能卡片表面样式包裹内容。
pub fn card_panel<'a>(
    content: impl Into<Element<'a, AppMessage>>,
    state: &AppState,
) -> container::Container<'a, AppMessage> {
    let palette = Palette::from_skin(&state.skin);
    container(content).style(card_style(palette))
}

/// 以 LCD 液晶仪表屏样式包裹内容。
pub fn lcd_panel<'a>(
    content: impl Into<Element<'a, AppMessage>>,
    state: &AppState,
) -> container::Container<'a, AppMessage> {
    let palette = Palette::from_skin(&state.skin);
    container(content).style(lcd_display_style(palette))
}

/// 以皮肤主前景色渲染文字。
pub fn themed_text<'a>(content: impl Into<String>, state: &AppState) -> text::Text<'a> {
    let palette = Palette::from_skin(&state.skin);
    text(content.into()).style(move |_theme: &Theme| text::Style {
        color: Some(palette.text_main),
    })
}

/// 以次级柔和色渲染文字。
pub fn sub_text<'a>(content: impl Into<String>, state: &AppState) -> text::Text<'a> {
    let palette = Palette::from_skin(&state.skin);
    text(content.into()).style(move |_theme: &Theme| text::Style {
        color: Some(palette.text_sub),
    })
}

/// 以皮肤强调色渲染文字。
pub fn accent_text<'a>(content: impl Into<String>, state: &AppState) -> text::Text<'a> {
    let palette = Palette::from_skin(&state.skin);
    text(content.into()).style(move |_theme: &Theme| text::Style {
        color: Some(palette.accent),
    })
}

/// 经典 VFD 荧光数码管微型状态胶囊标签（如 STEREO、FLAC、Hi-Res 等）。
pub fn vfd_badge<'a>(label: impl Into<String>, state: &AppState) -> Element<'a, AppMessage> {
    let palette = Palette::from_skin(&state.skin);
    container(
        text(label.into())
            .size(10.0)
            .style(move |_theme: &Theme| text::Style {
                color: Some(palette.vfd_green),
            }),
    )
    .padding([1, 5])
    .style(move |_theme: &Theme| container::Style {
        background: Some(Background::Color(Color::from_rgba(
            palette.vfd_green.r,
            palette.vfd_green.g,
            palette.vfd_green.b,
            0.12,
        ))),
        border: iced::Border {
            color: Color::from_rgba(
                palette.vfd_green.r,
                palette.vfd_green.g,
                palette.vfd_green.b,
                0.28,
            ),
            width: 1.0,
            radius: 3.0.into(),
        },
        ..Default::default()
    })
    .into()
}

/// 液晶数码管暗底时间指示框（如 `01:23`）。
pub fn digital_time_badge<'a>(time_str: String, state: &AppState) -> Element<'a, AppMessage> {
    let palette = Palette::from_skin(&state.skin);
    container(
        text(time_str)
            .size(11.0)
            .style(move |_theme: &Theme| text::Style {
                color: Some(palette.text_main),
            }),
    )
    .padding([2, 6])
    .style(move |_theme: &Theme| container::Style {
        background: Some(Background::Color(Color::from_rgba(0.0, 0.0, 0.0, 0.45))),
        border: iced::Border {
            color: palette.border_subtle,
            width: 1.0,
            radius: 3.0.into(),
        },
        ..Default::default()
    })
    .into()
}

/// 频谱 LED 控件（占满可用空间）。
pub fn spectrum_widget(state: &AppState) -> Element<'_, AppMessage> {
    if !state.skin.layout.show_spectrum {
        return container(text("频谱已隐藏")).into();
    }
    let low = skin_color(&state.skin.colors.spectrum_low);
    let high = skin_color(&state.skin.colors.spectrum_high);
    let program = LedSpectrum::new(&state.spectrum, low, high);
    Canvas::new(program)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

/// 简单的横向留白。
pub fn spacer() -> Element<'static, AppMessage> {
    row![].width(Length::Fill).height(Length::Shrink).into()
}

/// 用强调色按钮构造器（返回 `Button`）。
pub fn action_button<'a>(
    label: &'a str,
    on_press: AppMessage,
) -> iced::widget::button::Button<'a, AppMessage> {
    iced::widget::button(text(label).size(14.0)).on_press(on_press)
}

/// 让两个元素左右分布的行。
pub fn spread_row<'a>(
    left: impl Into<Element<'a, AppMessage>>,
    right: impl Into<Element<'a, AppMessage>>,
) -> iced::widget::Row<'a, AppMessage> {
    row![left.into(), iced::widget::space::horizontal(), right.into()]
}
