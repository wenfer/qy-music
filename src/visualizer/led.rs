//! 频谱 LED 渲染：用 iced `canvas` 把 [`SpectrumData`] 画成「灵动的音频律动柱」。
//!
//! 仅在 `gui` 特性下编译。频段按列排布，每段按点亮级数点亮（低端色 → 高端色插值）。
//! 采用极简清爽的微间距晶粒柱，未点亮段保持通透呼吸感，杜绝黑铁丝网感。

use crate::visualizer::spectrum::SpectrumData;

use iced::mouse;
use iced::widget::canvas::{Canvas, Frame, Geometry, Program};
use iced::{Color, Element, Length, Point, Rectangle, Renderer, Size, Theme};

/// LED 频谱画布程序。
pub struct LedSpectrum {
    /// 当前频谱数据。
    data: SpectrumData,
    /// 低端色。
    low: Color,
    /// 高端色。
    high: Color,
}

impl LedSpectrum {
    /// 构造。
    pub fn new(data: &SpectrumData, low: Color, high: Color) -> Self {
        Self {
            data: data.clone(),
            low,
            high,
        }
    }
}

impl<Message> Program<Message> for LedSpectrum {
    type State = ();

    fn draw(
        &self,
        _state: &(),
        renderer: &Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<Geometry> {
        let mut frame = Frame::new(renderer, bounds.size());
        let bands = &self.data.bands;
        let n = bands.len();
        let seg = self.data.led_segments.max(1);

        if n == 0 || bounds.width <= 0.0 || bounds.height <= 0.0 {
            return vec![frame.into_geometry()];
        }

        let gap = 2.0_f32;
        let band_w = ((bounds.width - gap * (n as f32 - 1.0).max(0.0)) / n as f32).max(1.0);
        let led_h = (bounds.height / seg as f32).max(1.0);
        let item_h = (led_h - 1.0).max(1.0);

        // 底部基准微线（极其淡雅，标示声学基准面）
        frame.fill_rectangle(
            Point::new(0.0, bounds.height - 1.0),
            Size::new(bounds.width, 1.0),
            Color::from_rgba(1.0, 1.0, 1.0, 0.05),
        );

        for (i, &level) in bands.iter().enumerate() {
            let lit = (level.clamp(0.0, 1.0) * seg as f32).round() as usize;
            let x = i as f32 * (band_w + gap);

            for s in 0..seg {
                let y = (seg - 1 - s) as f32 * led_h;
                let progress = (s + 1) as f32 / seg as f32;

                if s < lit {
                    // 点亮段：清爽鲜艳的渐变色 + 顶峰高光
                    let is_peak = s == lit - 1;
                    let base_color = lerp_color(self.low, self.high, progress);
                    let final_color = if is_peak && lit > 1 {
                        // 顶端高光晶粒：纯白微光
                        Color::from_rgb(
                            (base_color.r * 1.25 + 0.2).min(1.0),
                            (base_color.g * 1.25 + 0.2).min(1.0),
                            (base_color.b * 1.25 + 0.2).min(1.0),
                        )
                    } else {
                        base_color
                    };

                    frame.fill_rectangle(Point::new(x, y), Size::new(band_w, item_h), final_color);
                }
                // 未点亮段完全透明，保持背景绝对纯净深邃，杜绝暗斑网格
            }
        }

        vec![frame.into_geometry()]
    }
}

/// 在 low → high 之间线性插值颜色，兼顾色相生动度。
fn lerp_color(a: Color, b: Color, t: f32) -> Color {
    Color::from_rgb(
        a.r + (b.r - a.r) * t,
        a.g + (b.g - a.g) * t,
        a.b + (b.b - a.b) * t,
    )
}

/// 构造一个占满布局的频谱画布（供 UI 复用）。
#[allow(dead_code)]
pub fn view(data: &SpectrumData, low: Color, high: Color) -> Element<'_, ()> {
    Canvas::new(LedSpectrum::new(data, low, high))
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}
