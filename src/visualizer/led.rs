//! 频谱 LED 渲染：用 iced `canvas` 把 [`SpectrumData`] 画成「点亮的 LED 段」。
//!
//! 仅在 `gui` 特性下编译。频段按列排布，每段按点亮级数点亮（低端色 → 高端色插值），
//! 未点亮的段以暗色占位，模拟硬件频谱灯。

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
        let band_w = (bounds.width - gap * (n as f32 - 1.0).max(0.0)) / n as f32;
        let led_h = (bounds.height / seg as f32).max(1.0);

        for (i, &level) in bands.iter().enumerate() {
            let lit = (level.clamp(0.0, 1.0) * seg as f32).round() as usize;
            let x = i as f32 * (band_w + gap);
            for s in 0..seg {
                let y = (seg - 1 - s) as f32 * led_h;
                let progress = (s + 1) as f32 / seg as f32;
                let color = if s < lit {
                    lerp_color(self.low, self.high, progress)
                } else {
                    Color::from_rgba8(60, 60, 72, 0.35)
                };
                frame.fill_rectangle(
                    Point::new(x, y),
                    Size::new(band_w, (led_h - 1.0).max(1.0)),
                    color,
                );
            }
        }

        vec![frame.into_geometry()]
    }
}

/// 在 low → high 之间线性插值颜色。
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
