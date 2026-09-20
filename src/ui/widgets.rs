//! 共享 UI 控件与主题辅助。
//!
//! 提供：背景容器、文字、频谱 LED 渲染封装。颜色统一从当前 [`Skin`] 取得。
//!
//! 说明（iced 0.13）：`Text` 控件不带消息类型参数，其泛型为
//! `Text<'a, Theme, Renderer>`；`text()` 的内容需实现 `text::IntoFragment`，
//! 因此这里先把 `impl Into<String>` 归一为 `String` 再传入。

use crate::app::message::AppMessage;
use crate::app::state::AppState;
use crate::theme::skin_color;

use iced::widget::canvas::Canvas;
use iced::widget::{container, row, text};
use iced::{Background, Color, Element, Length, Theme};

use crate::visualizer::led::LedSpectrum;

/// 以皮肤背景色包裹内容。
pub fn themed_container<'a>(
    content: impl Into<Element<'a, AppMessage>>,
    state: &AppState,
) -> container::Container<'a, AppMessage> {
    let bg = skin_color(&state.skin.colors.bg);
    container(content).style(move |_theme: &Theme| container::Style {
        background: Some(Background::Color(bg)),
        ..Default::default()
    })
}

/// 以皮肤前景色渲染文字（返回 `Text`，可继续链式调用 `.size()` 等）。
pub fn themed_text<'a>(content: impl Into<String>, state: &AppState) -> text::Text<'a> {
    let fg = skin_color(&state.skin.colors.fg);
    text(content.into()).style(move |_theme: &Theme| text::Style { color: Some(fg) })
}

/// 以皮肤强调色渲染文字。
pub fn accent_text<'a>(content: impl Into<String>, state: &AppState) -> text::Text<'a> {
    let accent = skin_color(&state.skin.colors.accent);
    text(content.into()).style(move |_theme: &Theme| text::Style { color: Some(accent) })
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
    row![left.into(), iced::widget::horizontal_space(), right.into()]
}

/// 占位色（避免未使用导入告警）。
#[allow(dead_code)]
fn _unused(_c: Color) {}
