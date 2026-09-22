//! 实时频谱 FFT（rustfft 实 FFT / R2C + Hann 窗）。
//!
//! 默认参数（见架构文档 §7）：FFT 窗口 1024 点 @ 设备率，Hann 窗，
//! 取 FFT 前半（0–`rate/2`）的幅度谱。本模块只负责「喂采样 → 算幅度谱」，
//! 频段能量分块与归一化在 [`spectrum`](crate::visualizer::spectrum) 完成。

use std::collections::VecDeque;

use rustfft::num_complex::Complex;
use rustfft::FftPlanner;

use crate::visualizer::spectrum::SpectrumData;

/// 频谱分析器：维护固定长度单声道环形缓冲（Hann 窗），满窗即计算幅度谱。
pub struct SpectrumAnalyzer {
    fft_size: usize,
    sample_rate: f32,
    window: Vec<f32>,
    buffer: VecDeque<f32>,
    /// FFT 规划器（构造期用于生成 `fft`；实例保留以便将来按需复用/换窗）。
    #[allow(dead_code)]
    planner: FftPlanner<f32>,
    fft: std::sync::Arc<dyn rustfft::Fft<f32>>,
}

impl SpectrumAnalyzer {
    /// 构造：指定采样率与 FFT 窗口大小（建议 1024）。
    pub fn new(sample_rate: f32, fft_size: usize) -> Self {
        let window: Vec<f32> = (0..fft_size)
            .map(|i| {
                let n = i as f32;
                // Hann 窗：0.5 * (1 - cos(2π n / (N-1)))
                0.5 * (1.0 - (2.0 * std::f32::consts::PI * n / (fft_size as f32 - 1.0)).cos())
            })
            .collect();
        let mut planner = FftPlanner::<f32>::new();
        let fft = planner.plan_fft_forward(fft_size);
        Self {
            fft_size,
            sample_rate,
            window,
            buffer: VecDeque::with_capacity(fft_size),
            planner,
            fft,
        }
    }

    /// 采样率（Hz）。
    pub fn sample_rate(&self) -> f32 {
        self.sample_rate
    }

    /// 窗口大小（点）。
    pub fn fft_size(&self) -> usize {
        self.fft_size
    }

    /// 推入单声道采样（已下混）。
    pub fn push_mono(&mut self, s: f32) {
        if self.buffer.len() >= self.fft_size {
            self.buffer.pop_front();
        }
        self.buffer.push_back(s);
    }

    /// 推入一帧交错 PCM，按下混为单声道后喂入缓冲。
    pub fn push_frame(&mut self, frame: &[f32], channels: usize) {
        let channels = channels.max(1);
        if channels == 1 {
            for &s in frame {
                self.push_mono(s);
            }
        } else {
            for chunk in frame.chunks(channels) {
                let mono: f32 = chunk.iter().sum::<f32>() / channels as f32;
                self.push_mono(mono);
            }
        }
    }

    /// 若缓冲已满，计算并返回前半幅度谱（长度 = fft_size/2）。
    ///
    /// 否则返回 `None`（需继续累积采样）。
    pub fn compute(&mut self) -> Option<Vec<f32>> {
        if self.buffer.len() < self.fft_size {
            return None;
        }
        let mut input: Vec<Complex<f32>> = self
            .buffer
            .iter()
            .zip(self.window.iter())
            .map(|(s, w)| Complex::new(s * w, 0.0))
            .collect();
        self.fft.process(&mut input);
        // 仅取前半（实信号对称），幅度 = sqrt(re^2 + im^2)
        let half = self.fft_size / 2;
        let mags: Vec<f32> = input[..half]
            .iter()
            .map(|c| (c.re * c.re + c.im * c.im).sqrt())
            .collect();
        Some(mags)
    }

    /// 便捷：计算幅度谱并直接映射为 32 频段 [`SpectrumData`]（默认参数）。
    pub fn compute_spectrum(
        &mut self,
        band_count: usize,
        led_segments: usize,
    ) -> Option<SpectrumData> {
        self.compute().map(|mags| {
            SpectrumData::from_magnitudes(&mags, self.sample_rate, band_count, led_segments)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hann_window_symmetric_and_ends_zero() {
        let a = SpectrumAnalyzer::new(44100.0, 64);
        assert!((a.window[0]).abs() < 1e-5, "Hann 窗两端应≈0");
        assert!((a.window[a.fft_size - 1]).abs() < 1e-5);
        let mid = a.window[a.fft_size / 2];
        assert!(mid > 0.9 && mid <= 1.0, "Hann 窗中心≈1");
    }

    #[test]
    fn compute_returns_magnitudes_when_full() {
        let mut a = SpectrumAnalyzer::new(44100.0, 64);
        // 推入足够多的静音 + 一个正弦，缓冲满后 compute 有输出
        for _ in 0..64 {
            a.push_mono(0.0);
        }
        let mags = a.compute().expect("缓冲满应可计算");
        assert_eq!(mags.len(), 32);
    }

    #[test]
    fn compute_none_when_not_full() {
        let mut a = SpectrumAnalyzer::new(44100.0, 64);
        for _ in 0..10 {
            a.push_mono(0.5);
        }
        assert!(a.compute().is_none());
    }

    // ──────────────────────────────────────────────────────────────
    // QA 补充测试
    // ──────────────────────────────────────────────────────────────

    #[test]
    fn accessors_report_construction_params() {
        let a = SpectrumAnalyzer::new(48_000.0, 1024);
        assert_eq!(a.sample_rate(), 48_000.0);
        assert_eq!(a.fft_size(), 1024);
    }

    #[test]
    fn compute_none_when_one_sample_short() {
        let mut a = SpectrumAnalyzer::new(44_100.0, 64);
        for _ in 0..63 {
            a.push_mono(0.1);
        }
        assert!(a.compute().is_none(), "差 1 个样本不满窗，应返回 None");
    }

    #[test]
    fn silence_produces_zero_magnitudes() {
        let mut a = SpectrumAnalyzer::new(44_100.0, 64);
        for _ in 0..64 {
            a.push_mono(0.0);
        }
        let mags = a.compute().unwrap();
        assert!(mags.iter().all(|&m| m.abs() < 1e-3), "静音幅度谱应≈0");
    }

    #[test]
    fn buffer_is_capped_at_fft_size() {
        let mut a = SpectrumAnalyzer::new(44_100.0, 64);
        for _ in 0..(64 * 3) {
            a.push_mono(0.25);
        }
        assert_eq!(a.buffer.len(), 64, "环形缓冲不应超过 fft_size");
    }

    #[test]
    fn push_frame_downmixes_stereo_to_mono() {
        let mut a = SpectrumAnalyzer::new(44_100.0, 4);
        // 两帧立体声：(1.0, 0.0) 与 (0.0, 1.0) → 单声道均值均为 0.5
        a.push_frame(&[1.0, 0.0, 0.0, 1.0], 2);
        let buf: Vec<f32> = a.buffer.iter().copied().collect();
        assert_eq!(buf, vec![0.5, 0.5]);
    }

    #[test]
    fn push_frame_mono_passthrough() {
        let mut a = SpectrumAnalyzer::new(44_100.0, 4);
        a.push_frame(&[0.1, 0.2, 0.3], 1);
        let buf: Vec<f32> = a.buffer.iter().copied().collect();
        assert_eq!(buf, vec![0.1, 0.2, 0.3]);
    }

    #[test]
    fn compute_spectrum_maps_to_requested_bands() {
        let mut a = SpectrumAnalyzer::new(44_100.0, 64);
        for _ in 0..64 {
            a.push_mono(0.2);
        }
        let sd = a.compute_spectrum(16, 8).expect("满窗应产出频谱");
        assert_eq!(sd.bands.len(), 16);
        assert_eq!(sd.band_count, 16);
        assert_eq!(sd.led_segments, 8);
    }

    #[test]
    fn compute_spectrum_none_when_not_full() {
        let mut a = SpectrumAnalyzer::new(44_100.0, 64);
        for _ in 0..10 {
            a.push_mono(0.2);
        }
        assert!(a.compute_spectrum(32, 16).is_none());
    }
}
