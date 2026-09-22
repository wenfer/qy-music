//! UI 设计系统：颜色层级、设计 Token 与组件样式（简洁、统一、清爽、美观）。
//!
//! 提供基于 [`Skin`] 动态计算的高级色彩 Token 与现代组件样式。

use iced::widget::{button, container, pick_list, scrollable, slider};
use iced::{Background, Border, Color, Shadow, Theme, Vector};

use crate::theme::hex_to_rgb;
use crate::theme::skin::Skin;

/// 设计系统调色板：由当前皮肤动态派生的高级色彩 Token。
#[derive(Clone, Copy, Debug)]
pub struct Palette {
    /// 机身主底色（深邃极简纯净背景）。
    pub bg: Color,
    /// 功能卡片与面板底色。
    pub surface: Color,
    /// 凸起表面（如悬停条目、选中的 Tab、按键）。
    pub surface_elevated: Color,
    /// 仪表显示屏底色（清爽通透微暗）。
    pub lcd_bg: Color,
    /// 仪表屏微弱轮廓线。
    pub lcd_border: Color,
    /// 主高亮文本色（纯净高白）。
    pub text_main: Color,
    /// 次级文本色（柔和青灰/暖灰）。
    pub text_sub: Color,
    /// 暗淡辅助文本色（灰色/注释）。
    pub text_muted: Color,
    /// 核心强调色（播放键、进度、高亮）。
    pub accent: Color,
    /// 强调色悬停高亮。
    pub accent_hover: Color,
    /// 强调色半透明光晕底色。
    pub accent_subtle: Color,
    /// 状态荧光色（经典青绿）。
    pub vfd_green: Color,
    /// 精细分界线与边框色（极细柔和）。
    pub border: Color,
    /// 微弱内部分割线。
    pub border_subtle: Color,
}

impl Palette {
    /// 根据当前皮肤生成完整调色板。
    pub fn from_skin(skin: &Skin) -> Self {
        let (br, bg, bb) = hex_to_rgb(&skin.colors.bg);
        let (fr, fg, fb) = hex_to_rgb(&skin.colors.fg);
        let (ar, ag, ab) = hex_to_rgb(&skin.colors.accent);

        // 深邃、纯净、现代的机身背景色
        let base_bg = Color::from_rgb(br * 0.75, bg * 0.75, bb * 0.75);
        // 卡片表面：比背景稍亮，纯净平整，带极微弱层次
        let surface = Color::from_rgb(
            (br * 0.95 + 0.02).min(1.0),
            (bg * 0.95 + 0.02).min(1.0),
            (bb * 0.95 + 0.03).min(1.0),
        );
        let surface_elevated = Color::from_rgb(
            (br * 1.25 + 0.05).min(1.0),
            (bg * 1.25 + 0.05).min(1.0),
            (bb * 1.25 + 0.07).min(1.0),
        );
        // 仪表屏区：比卡片稍暗，通透不压抑
        let lcd_bg = Color::from_rgb(br * 0.6, bg * 0.6, bb * 0.65);
        let lcd_border = Color::from_rgba(1.0, 1.0, 1.0, 0.06);

        let text_main = Color::from_rgb(
            (fr * 0.95 + 0.05).min(1.0),
            (fg * 0.95 + 0.05).min(1.0),
            (fb * 0.95 + 0.05).min(1.0),
        );
        let text_sub = Color::from_rgb(fr * 0.65 + 0.05, fg * 0.65 + 0.05, fb * 0.65 + 0.08);
        let text_muted = Color::from_rgb(fr * 0.42, fg * 0.42, fb * 0.46);

        let accent = Color::from_rgb(ar, ag, ab);
        let accent_hover = Color::from_rgb(
            (ar * 1.15 + 0.05).min(1.0),
            (ag * 1.15 + 0.05).min(1.0),
            (ab * 1.15 + 0.05).min(1.0),
        );
        let accent_subtle = Color::from_rgba(ar, ag, ab, 0.12);

        let vfd_green = Color::from_rgb(0.20, 0.85, 0.65);

        let border = Color::from_rgba(1.0, 1.0, 1.0, 0.06);
        let border_subtle = Color::from_rgba(1.0, 1.0, 1.0, 0.035);

        Self {
            bg: base_bg,
            surface,
            surface_elevated,
            lcd_bg,
            lcd_border,
            text_main,
            text_sub,
            text_muted,
            accent,
            accent_hover,
            accent_subtle,
            vfd_green,
            border,
            border_subtle,
        }
    }
}

/// 主机身容器样式：深邃通透纯净背景。
pub fn window_container_style(palette: Palette) -> impl Fn(&Theme) -> container::Style {
    move |_theme| container::Style {
        background: Some(Background::Color(palette.bg)),
        border: Border::default(),
        ..Default::default()
    }
}

/// 统一功能卡片容器样式（8px 现代圆角 + 1px 极细微光边框）。
pub fn card_style(palette: Palette) -> impl Fn(&Theme) -> container::Style {
    move |_theme| container::Style {
        background: Some(Background::Color(palette.surface)),
        border: Border {
            color: palette.border,
            width: 1.0,
            radius: 8.0.into(),
        },
        shadow: Shadow {
            color: Color::from_rgba(0.0, 0.0, 0.0, 0.20),
            offset: Vector::new(0.0, 2.0),
            blur_radius: 6.0,
        },
        ..Default::default()
    }
}

/// 仪表显控容器样式（清爽平整，无黑铁丝网感）。
pub fn lcd_display_style(palette: Palette) -> impl Fn(&Theme) -> container::Style {
    move |_theme| container::Style {
        background: Some(Background::Color(palette.lcd_bg)),
        border: Border {
            color: palette.lcd_border,
            width: 1.0,
            radius: 8.0.into(),
        },
        ..Default::default()
    }
}

/// 核心主播放/暂停按键（36×36 正圆，纯净优雅 Accent，微光悬停）。
pub fn primary_play_button_style(
    palette: Palette,
) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |_theme, status| match status {
        button::Status::Hovered => button::Style {
            background: Some(Background::Color(palette.accent_hover)),
            text_color: Color::WHITE,
            border: Border {
                color: Color::from_rgba(1.0, 1.0, 1.0, 0.35),
                width: 1.0,
                radius: 18.0.into(),
            },
            shadow: Shadow {
                color: Color::from_rgba(palette.accent.r, palette.accent.g, palette.accent.b, 0.40),
                offset: Vector::new(0.0, 0.0),
                blur_radius: 8.0,
            },
            ..Default::default()
        },
        button::Status::Pressed => button::Style {
            background: Some(Background::Color(palette.accent)),
            text_color: Color::WHITE,
            border: Border::default().rounded(18.0),
            shadow: Shadow::default(),
            ..Default::default()
        },
        button::Status::Disabled => button::Style {
            background: Some(Background::Color(palette.surface_elevated)),
            text_color: palette.text_muted,
            border: Border::default().rounded(18.0),
            shadow: Shadow::default(),
            ..Default::default()
        },
        button::Status::Active => button::Style {
            background: Some(Background::Color(palette.accent)),
            text_color: Color::WHITE,
            border: Border {
                color: Color::from_rgba(1.0, 1.0, 1.0, 0.15),
                width: 1.0,
                radius: 18.0.into(),
            },
            shadow: Shadow {
                color: Color::from_rgba(palette.accent.r, palette.accent.g, palette.accent.b, 0.25),
                offset: Vector::new(0.0, 1.0),
                blur_radius: 4.0,
            },
            ..Default::default()
        },
    }
}

/// 切歌按钮（上一首/下一首：30×30 正圆，半透明底，与播放键视觉完美协调呼应）。
pub fn transport_button_style(
    palette: Palette,
) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |_theme, status| match status {
        button::Status::Hovered => button::Style {
            background: Some(Background::Color(palette.surface_elevated)),
            text_color: palette.text_main,
            border: Border {
                color: palette.border,
                width: 1.0,
                radius: 15.0.into(),
            },
            shadow: Shadow::default(),
            ..Default::default()
        },
        button::Status::Pressed => button::Style {
            background: Some(Background::Color(palette.surface)),
            text_color: palette.accent,
            border: Border::default().rounded(15.0),
            shadow: Shadow::default(),
            ..Default::default()
        },
        button::Status::Disabled => button::Style {
            background: Some(Background::Color(Color::TRANSPARENT)),
            text_color: palette.text_muted,
            border: Border::default().rounded(15.0),
            shadow: Shadow::default(),
            ..Default::default()
        },
        button::Status::Active => button::Style {
            background: Some(Background::Color(Color::from_rgba(1.0, 1.0, 1.0, 0.05))),
            text_color: palette.text_sub,
            border: Border {
                color: Color::from_rgba(1.0, 1.0, 1.0, 0.04),
                width: 1.0,
                radius: 15.0.into(),
            },
            shadow: Shadow::default(),
            ..Default::default()
        },
    }
}

/// Segmented Control 分段药丸切换键（统一胶囊底槽，均分宽度，左右对称）。
pub fn pill_tab_button_style(
    palette: Palette,
    is_active: bool,
) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |_theme, status| {
        if is_active {
            button::Style {
                background: Some(Background::Color(palette.surface_elevated)),
                text_color: palette.text_main,
                border: Border {
                    color: Color::from_rgba(1.0, 1.0, 1.0, 0.08),
                    width: 1.0,
                    radius: 6.0.into(),
                },
                shadow: Shadow {
                    color: Color::from_rgba(0.0, 0.0, 0.0, 0.25),
                    offset: Vector::new(0.0, 1.0),
                    blur_radius: 2.0,
                },
                ..Default::default()
            }
        } else {
            match status {
                button::Status::Hovered => button::Style {
                    background: Some(Background::Color(Color::from_rgba(1.0, 1.0, 1.0, 0.04))),
                    text_color: palette.text_main,
                    border: Border::default().rounded(6.0),
                    shadow: Shadow::default(),
                    ..Default::default()
                },
                _ => button::Style {
                    background: Some(Background::Color(Color::TRANSPARENT)),
                    text_color: palette.text_muted,
                    border: Border::default().rounded(6.0),
                    shadow: Shadow::default(),
                    ..Default::default()
                },
            }
        }
    }
}

/// DSP 音效切换胶囊按钮（开启时显眼高亮，关闭时克制内敛）。
pub fn effect_toggle_button_style(
    palette: Palette,
    is_active: bool,
) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |_theme, status| {
        if is_active {
            button::Style {
                background: Some(Background::Color(Color::from_rgba(
                    palette.accent.r,
                    palette.accent.g,
                    palette.accent.b,
                    0.20,
                ))),
                text_color: palette.accent,
                border: Border {
                    color: Color::from_rgba(
                        palette.accent.r,
                        palette.accent.g,
                        palette.accent.b,
                        0.45,
                    ),
                    width: 1.0,
                    radius: 5.0.into(),
                },
                shadow: Shadow {
                    color: Color::from_rgba(
                        palette.accent.r,
                        palette.accent.g,
                        palette.accent.b,
                        0.15,
                    ),
                    offset: Vector::new(0.0, 1.0),
                    blur_radius: 3.0,
                },
                ..Default::default()
            }
        } else {
            match status {
                button::Status::Hovered => button::Style {
                    background: Some(Background::Color(Color::from_rgba(1.0, 1.0, 1.0, 0.06))),
                    text_color: palette.text_main,
                    border: Border {
                        color: Color::from_rgba(1.0, 1.0, 1.0, 0.08),
                        width: 1.0,
                        radius: 5.0.into(),
                    },
                    shadow: Shadow::default(),
                    ..Default::default()
                },
                _ => button::Style {
                    background: Some(Background::Color(Color::from_rgba(1.0, 1.0, 1.0, 0.02))),
                    text_color: palette.text_muted,
                    border: Border {
                        color: Color::from_rgba(1.0, 1.0, 1.0, 0.04),
                        width: 1.0,
                        radius: 5.0.into(),
                    },
                    shadow: Shadow::default(),
                    ..Default::default()
                },
            }
        }
    }
}

/// 极简幽灵小按键（+文件、清空、小功能入口等，无突兀黑方框）。
pub fn subtle_button_style(palette: Palette) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |_theme, status| match status {
        button::Status::Hovered => button::Style {
            background: Some(Background::Color(palette.surface_elevated)),
            text_color: palette.text_main,
            border: Border {
                color: Color::from_rgba(1.0, 1.0, 1.0, 0.08),
                width: 1.0,
                radius: 4.0.into(),
            },
            shadow: Shadow::default(),
            ..Default::default()
        },
        button::Status::Pressed => button::Style {
            background: Some(Background::Color(palette.surface)),
            text_color: palette.accent,
            border: Border::default().rounded(4.0),
            shadow: Shadow::default(),
            ..Default::default()
        },
        _ => button::Style {
            background: Some(Background::Color(Color::from_rgba(1.0, 1.0, 1.0, 0.04))),
            text_color: palette.text_sub,
            border: Border {
                color: Color::from_rgba(1.0, 1.0, 1.0, 0.03),
                width: 1.0,
                radius: 4.0.into(),
            },
            shadow: Shadow::default(),
            ..Default::default()
        },
    }
}

/// 极简透明图标按钮（音量静音、列表删除等）。
pub fn icon_button_style(palette: Palette) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |_theme, status| match status {
        button::Status::Hovered => button::Style {
            background: Some(Background::Color(Color::from_rgba(1.0, 1.0, 1.0, 0.08))),
            text_color: palette.text_main,
            border: Border::default().rounded(4.0),
            shadow: Shadow::default(),
            ..Default::default()
        },
        button::Status::Pressed => button::Style {
            background: Some(Background::Color(Color::from_rgba(1.0, 1.0, 1.0, 0.12))),
            text_color: palette.accent,
            border: Border::default().rounded(4.0),
            shadow: Shadow::default(),
            ..Default::default()
        },
        _ => button::Style {
            background: Some(Background::Color(Color::TRANSPARENT)),
            text_color: palette.text_sub,
            border: Border::default().rounded(4.0),
            shadow: Shadow::default(),
            ..Default::default()
        },
    }
}

/// 极简纤细发光导轨滑块样式（进度条、音量条、EQ 推子）。
pub fn hifi_slider_style(palette: Palette) -> impl Fn(&Theme, slider::Status) -> slider::Style {
    move |_theme, status| {
        let (handle_bg, handle_border_color) = match status {
            slider::Status::Hovered => (Color::WHITE, palette.accent_hover),
            slider::Status::Dragged => (Color::WHITE, palette.accent),
            slider::Status::Active => (palette.text_main, palette.accent),
        };

        slider::Style {
            rail: slider::Rail {
                backgrounds: (
                    Background::Color(palette.accent),
                    Background::Color(Color::from_rgba(1.0, 1.0, 1.0, 0.08)),
                ),
                width: 3.0,
                border: Border {
                    radius: 1.5.into(),
                    ..Default::default()
                },
            },
            handle: slider::Handle {
                shape: slider::HandleShape::Circle { radius: 4.0 },
                background: Background::Color(handle_bg),
                border_width: 1.0,
                border_color: handle_border_color,
            },
        }
    }
}

/// 极简扁平下拉选择框样式（皮肤、循环模式、EQ 预设）。
pub fn hifi_pick_list_style(
    palette: Palette,
) -> impl Fn(&Theme, pick_list::Status) -> pick_list::Style {
    move |_theme, status| match status {
        pick_list::Status::Hovered => pick_list::Style {
            text_color: palette.text_main,
            placeholder_color: palette.text_muted,
            handle_color: palette.accent,
            background: Background::Color(palette.surface_elevated),
            border: Border {
                color: Color::from_rgba(1.0, 1.0, 1.0, 0.12),
                width: 1.0,
                radius: 6.0.into(),
            },
        },
        pick_list::Status::Opened { .. } => pick_list::Style {
            text_color: palette.text_main,
            placeholder_color: palette.text_muted,
            handle_color: palette.accent,
            background: Background::Color(palette.surface_elevated),
            border: Border {
                color: palette.accent,
                width: 1.0,
                radius: 6.0.into(),
            },
        },
        _ => pick_list::Style {
            text_color: palette.text_sub,
            placeholder_color: palette.text_muted,
            handle_color: palette.text_muted,
            background: Background::Color(Color::from_rgba(1.0, 1.0, 1.0, 0.04)),
            border: Border {
                color: Color::from_rgba(1.0, 1.0, 1.0, 0.03),
                width: 1.0,
                radius: 6.0.into(),
            },
        },
    }
}

/// 极简细滚动条样式。
pub fn hifi_scrollable_style(
    palette: Palette,
) -> impl Fn(&Theme, scrollable::Status) -> scrollable::Style {
    move |theme, status| {
        use iced::widget::scrollable::Catalog;
        let scroller_color = match status {
            scrollable::Status::Hovered { .. } => Color::from_rgba(1.0, 1.0, 1.0, 0.25),
            scrollable::Status::Dragged { .. } => palette.accent,
            scrollable::Status::Active { .. } => Color::from_rgba(1.0, 1.0, 1.0, 0.12),
        };
        let mut style = theme.style(&<Theme as Catalog>::default(), status);
        style.vertical_rail.scroller.background = Background::Color(scroller_color);
        style.vertical_rail.scroller.border = Border::default().rounded(2.0);
        style.vertical_rail.background = Some(Background::Color(Color::TRANSPARENT));
        style.vertical_rail.border = Border::default();
        style.horizontal_rail.scroller.background = Background::Color(scroller_color);
        style.horizontal_rail.scroller.border = Border::default().rounded(2.0);
        style.horizontal_rail.background = Some(Background::Color(Color::TRANSPARENT));
        style.horizontal_rail.border = Border::default();
        style
    }
}
