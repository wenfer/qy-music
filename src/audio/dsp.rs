//! DSP 链：10 段 RBJ 峰值（Peaking）Biquad 滤波器串联 + 主增益。
//!
//! 设计约定（见架构文档 §3、§7）：
//! - PCM 全程 `f32` 交错，默认声道数对齐为 2（单声道上混为双声道）。
//! - `DspChain::process` 输入 / 输出长度一致。
//! - 滤波器采用 Direct Form II Transposed（数值稳定、低延迟）。
//!
//! 说明：为正确分离左右声道状态，`DspChain` 内部按「声道数 × 段数」布局
//! `Biquad`（默认立体声，即 20 个滤波器）。这是 [`Biquad`] 字段为
//! `Vec<Biquad>` 的内部排布细节，对外接口与类图一致。

/// 单个 Biquad 的归一化系数。
///
/// 差分方程（Direct Form II Transposed）：
/// `y = b0*x + b1*x1 + b2*x2 - a1*y1 - a2*y2`
use crate::audio::equalizer::Equalizer;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BiquadCoeff {
    /// 分子系数 b0。
    pub b0: f32,
    /// 分子系数 b1。
    pub b1: f32,
    /// 分子系数 b2。
    pub b2: f32,
    /// 分母系数 a1（注意 a0 已归一化为 1）。
    pub a1: f32,
    /// 分母系数 a2。
    pub a2: f32,
}

impl BiquadCoeff {
    /// 构造 RBJ 峰值（Peaking）滤波器系数。
    ///
    /// - `f0`:   中心频率 (Hz)
    /// - `fs`:   采样率 (Hz)
    /// - `gain_db`: 增益 (dB)，正为提升、负为衰减
    /// - `q`:    品质因数（带宽），越大峰越窄
    ///
    /// 公式（Audio EQ Cookbook, Robert Bristow-Johnson）：
    /// ```text
    /// A = 10^(gain_db / 40)
    /// w0 = 2π f0 / fs
    /// alpha = sin(w0) / (2 Q)
    /// b0 = 1 + alpha*A ; b1 = -2 cos(w0) ; b2 = 1 - alpha*A
    /// a0 = 1 + alpha/A  ; a1 = -2 cos(w0) ; a2 = 1 - alpha/A
    /// 全部除以 a0 归一化
    /// ```
    pub fn peaking(f0: f32, fs: f32, gain_db: f32, q: f32) -> Self {
        debug_assert!(f0 > 0.0 && f0 < fs / 2.0, "中心频率必须落在 (0, fs/2)");
        debug_assert!(q > 0.0, "Q 必须为正");

        let a = 10f32.powf(gain_db / 40.0);
        let w0 = 2.0 * std::f32::consts::PI * f0 / fs;
        let cos_w0 = w0.cos();
        let alpha = w0.sin() / (2.0 * q);

        let b0 = 1.0 + alpha * a;
        let b1 = -2.0 * cos_w0;
        let b2 = 1.0 - alpha * a;
        let a0 = 1.0 + alpha / a;
        let a1 = -2.0 * cos_w0;
        let a2 = 1.0 - alpha / a;

        Self {
            b0: b0 / a0,
            b1: b1 / a0,
            b2: b2 / a0,
            a1: a1 / a0,
            a2: a2 / a0,
        }
    }
}

/// 单个 Biquad 滤波器（含延迟状态）。
#[derive(Clone, Copy, Debug)]
pub struct Biquad {
    /// 系数。
    pub coeff: BiquadCoeff,
    /// 上一输入 x[n-1]。
    x1: f32,
    /// 上上输入 x[n-2]。
    x2: f32,
    /// 上一输出 y[n-1]。
    y1: f32,
    /// 上上输出 y[n-2]。
    y2: f32,
}

impl Biquad {
    /// 由系数构造（状态清零）。
    pub fn new(coeff: BiquadCoeff) -> Self {
        Self {
            coeff,
            x1: 0.0,
            x2: 0.0,
            y1: 0.0,
            y2: 0.0,
        }
    }

    /// 处理单样本并返回输出样本。
    pub fn process(&mut self, x: f32) -> f32 {
        let y = self.coeff.b0 * x
            + self.coeff.b1 * self.x1
            + self.coeff.b2 * self.x2
            - self.coeff.a1 * self.y1
            - self.coeff.a2 * self.y2;
        self.x2 = self.x1;
        self.x1 = x;
        self.y2 = self.y1;
        self.y1 = y;
        y
    }

    /// 重置滤波器状态（seek / 换曲时调用，避免爆音）。
    pub fn reset(&mut self) {
        self.x1 = 0.0;
        self.x2 = 0.0;
        self.y1 = 0.0;
        self.y2 = 0.0;
    }
}

/// DSP 链：多段 Biquad 串联 + 链尾主增益。
///
/// 内部布局：`filters` 长度为 `声道数 × 段数`（默认立体声 2×10）。
/// 索引规则：`filters[ch * n_bands + band]` 为第 `ch` 声道第 `band` 段滤波器的状态。
#[derive(Clone, Debug)]
pub struct DspChain {
    /// 每声道每段的 Biquad 串联。
    pub filters: Vec<Biquad>,
    /// 主增益（线性，已按 dB 转换）。
    pub master_gain: f32,
    /// 声道数。
    channels: usize,
}

impl DspChain {
    /// 由均衡器参数构造 DSP 链。
    ///
    /// 假设立体声（2 声道），按 `Equalizer` 的 10 段增益 + 主增益生成系数。
    pub fn from_equalizer(eq: &Equalizer, sample_rate: f32) -> Self {
        let channels = 2usize;
        let coeffs = eq.to_biquad_coeffs(sample_rate);
        let mut filters = Vec::with_capacity(channels * coeffs.len());
        for _ch in 0..channels {
            for c in &coeffs {
                filters.push(Biquad::new(*c));
            }
        }
        let master_gain = if eq.enabled {
            10f32.powf(eq.master_gain_db / 20.0)
        } else {
            1.0
        };
        Self {
            filters,
            master_gain,
            channels,
        }
    }

    /// 处理一帧（交错 `f32`，默认立体声）并返回处理后的帧。
    ///
    /// 每个声道的样本依次过所有 Biquad 段，再乘主增益；输入 / 输出长度一致。
    pub fn process(&mut self, frame: &[f32]) -> Vec<f32> {
        let n_ch = self.channels;
        let n_bands = if self.filters.is_empty() {
            0
        } else {
            self.filters.len() / n_ch
        };
        let mut out = vec![0.0f32; frame.len()];
        for i in 0..frame.len() {
            let ch = i % n_ch;
            let mut v = frame[i];
            for band in 0..n_bands {
                let idx = ch * n_bands + band;
                // 安全：n_bands = filters.len()/n_ch，idx 必在范围内。
                v = unsafe { self.filters.get_unchecked_mut(idx) }.process(v);
            }
            out[i] = v * self.master_gain;
        }
        out
    }

    /// 重置所有滤波器状态（换曲 / seek 时调用，避免相位突变爆音）。
    pub fn reset(&mut self) {
        for f in &mut self.filters {
            f.reset();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 计算 Biquad 在归一化角频率 `w`（rad）下的幅度响应（线性）。
    fn biquad_mag(coeff: &BiquadCoeff, w: f32) -> f32 {
        // H(e^{jw}) = (b0 + b1 e^{-jw} + b2 e^{-2jw}) / (1 + a1 e^{-jw} + a2 e^{-2jw})
        let (re1, im1) = (w.cos(), -w.sin());
        let (re2, im2) = ((2.0 * w).cos(), -(2.0 * w).sin());
        let n_re = coeff.b0 + coeff.b1 * re1 + coeff.b2 * re2;
        let n_im = coeff.b1 * im1 + coeff.b2 * im2;
        let d_re = 1.0 + coeff.a1 * re1 + coeff.a2 * re2;
        let d_im = coeff.a1 * im1 + coeff.a2 * im2;
        let mag = (n_re * n_re + n_im * n_im).sqrt() / (d_re * d_re + d_im * d_im).sqrt();
        mag
    }

    #[test]
    fn biquad_peaking_at_center_is_roughly_flat() {
        // 在中心频率处，峰值滤波器增益应≈ 指定 dB（< 0.1 dB 误差）。
        let fs = 48000.0;
        let f0 = 1000.0;
        let gain_db = 6.0;
        let coeff = BiquadCoeff::peaking(f0, fs, gain_db, 1.0);
        let w0 = 2.0 * std::f32::consts::PI * f0 / fs;
        let mag = biquad_mag(&coeff, w0);
        let measured = 20.0 * mag.log10();
        assert!(
            (measured - gain_db).abs() < 0.1,
            "中心频率增益 ≈ {gain_db}dB，实际 {measured:.3}"
        );
    }

    #[test]
    fn biquad_zero_gain_is_identity() {
        let coeff = BiquadCoeff::peaking(1000.0, 48000.0, 0.0, 1.0);
        // 0 dB 峰值滤波器的分子系数等于分母系数（b0=1, b1=a1, b2=a2），
        // 使传递函数 H(z) ≡ 1（幅度与相位均不变，为恒等滤波器）。
        assert!((coeff.b0 - 1.0).abs() < 1e-5);
        assert!((coeff.b1 - coeff.a1).abs() < 1e-5, "0dB 峰值应为恒等：b1==a1");
        assert!((coeff.b2 - coeff.a2).abs() < 1e-5, "0dB 峰值应为恒等：b2==a2");
    }

    #[test]
    fn dsp_chain_preserves_length() {
        let eq = Equalizer::flat();
        let mut chain = DspChain::from_equalizer(&eq, 48000.0);
        let frame = vec![0.1f32, -0.2, 0.3, -0.05];
        let out = chain.process(&frame);
        assert_eq!(out.len(), frame.len());
        // Flat EQ + 0 dB 主增益 → 输出应等于输入。
        for (a, b) in frame.iter().zip(out.iter()) {
            assert!((a - b).abs() < 1e-4, "flat chain 应接近恒等");
        }
    }

    #[test]
    fn dsp_chain_master_gain_scales() {
        let mut eq = Equalizer::flat();
        eq.master_gain_db = 6.0; // +6 dB ≈ ×2
        let mut chain = DspChain::from_equalizer(&eq, 48000.0);
        let frame = vec![0.5f32, 0.0, -0.25, 0.0];
        let out = chain.process(&frame);
        assert!((out[0] - 1.0).abs() < 1e-2, "主增益 +6dB 应≈×2，实际 {}", out[0]);
        assert!((out[2] - (-0.5)).abs() < 1e-2);
    }

    // ──────────────────────────────────────────────────────────────
    // QA 补充测试
    // ──────────────────────────────────────────────────────────────

    #[test]
    fn peaking_cut_lowers_center_magnitude() {
        let fs = 48_000.0;
        let f0 = 1_000.0;
        let gain_db = -6.0;
        let coeff = BiquadCoeff::peaking(f0, fs, gain_db, 1.0);
        let w0 = 2.0 * std::f32::consts::PI * f0 / fs;
        let measured = 20.0 * biquad_mag(&coeff, w0).log10();
        assert!(
            (measured - gain_db).abs() < 0.1,
            "中心频率应≈ -6dB，实际 {measured:.3}"
        );
    }

    #[test]
    fn peaking_boost_above_zero_cut_below_zero() {
        let fs = 48_000.0;
        let f0 = 1_000.0;
        let w0 = 2.0 * std::f32::consts::PI * f0 / fs;
        let boost = 20.0 * biquad_mag(&BiquadCoeff::peaking(f0, fs, 6.0, 1.0), w0).log10();
        let cut = 20.0 * biquad_mag(&BiquadCoeff::peaking(f0, fs, -6.0, 1.0), w0).log10();
        assert!(boost > 0.0, "正增益应提升，实际 {boost:.3}dB");
        assert!(cut < 0.0, "负增益应衰减，实际 {cut:.3}dB");
    }

    #[test]
    fn zero_gain_biquad_passes_samples_through() {
        let mut b = Biquad::new(BiquadCoeff::peaking(1_000.0, 48_000.0, 0.0, 1.0));
        for x in [0.3f32, -0.7, 0.1, 0.9, -0.2] {
            let y = b.process(x);
            assert!((y - x).abs() < 1e-6, "0dB 滤波器应恒等传递：in {x} out {y}");
        }
    }

    #[test]
    fn biquad_reset_zeroes_state() {
        let mut b = Biquad::new(BiquadCoeff::peaking(1_000.0, 48_000.0, 6.0, 1.0));
        for _ in 0..16 {
            b.process(0.5);
        }
        b.reset();
        // 重置后以 0 输入，输出应为 0（无历史状态残留）
        assert!(b.process(0.0).abs() < 1e-9);
    }

    #[test]
    fn dsp_chain_all_bands_max_stays_finite() {
        let mut eq = Equalizer::flat();
        eq.bands = [12.0; 10];
        eq.master_gain_db = 6.0;
        let mut chain = DspChain::from_equalizer(&eq, 48_000.0);
        let input: Vec<f32> = (0..4096)
            .map(|i| if i % 2 == 0 { 0.1 } else { -0.1 })
            .collect();
        let out = chain.process(&input);
        assert_eq!(out.len(), input.len());
        assert!(
            out.iter().all(|v| v.is_finite()),
            "10 段全增益串联后不应产生 NaN/Inf"
        );
        assert!(
            out.iter().all(|v| v.abs() < 1e3),
            "输出幅度应保持在合理范围"
        );
    }

    #[test]
    fn dsp_chain_disabled_eq_is_passthrough() {
        let mut eq = Equalizer::flat();
        eq.bands = [12.0; 10];
        eq.master_gain_db = 6.0;
        eq.enabled = false;
        let mut chain = DspChain::from_equalizer(&eq, 48_000.0);
        let frame = vec![0.2f32, -0.4, 0.6, -0.8];
        let out = chain.process(&frame);
        for (a, b) in frame.iter().zip(out.iter()) {
            assert!((a - b).abs() < 1e-5, "EQ 关闭应旁路（含主增益），{a} vs {b}");
        }
    }

    #[test]
    fn dsp_chain_negative_master_gain_scales_down() {
        let mut eq = Equalizer::flat();
        eq.master_gain_db = -6.0; // ≈ ×0.5
        let mut chain = DspChain::from_equalizer(&eq, 48_000.0);
        let out = chain.process(&[1.0f32, 0.0]);
        assert!((out[0] - 0.5).abs() < 1e-2, "-6dB 应≈×0.5，实际 {}", out[0]);
    }

    #[test]
    fn dsp_chain_channels_are_independent() {
        let mut eq = Equalizer::flat();
        eq.bands[0] = 12.0; // 低频提升，验证不会串到右声道
        let mut chain = DspChain::from_equalizer(&eq, 48_000.0);
        // 交错立体声：只喂左声道，右声道恒 0
        let mut frame = Vec::new();
        for _ in 0..256 {
            frame.push(0.5f32); // L
            frame.push(0.0f32); // R
        }
        let out = chain.process(&frame);
        for chunk in out.chunks(2) {
            assert!(
                chunk[1].abs() < 1e-6,
                "右声道无输入时应保持静音，实际 {}",
                chunk[1]
            );
        }
    }

    #[test]
    fn dsp_chain_reset_clears_filter_state() {
        let mut eq = Equalizer::flat();
        eq.bands = [9.0; 10];
        let mut chain = DspChain::from_equalizer(&eq, 48_000.0);
        let loud: Vec<f32> = vec![0.8; 512];
        let _ = chain.process(&loud);
        chain.reset();
        let silence = vec![0.0f32; 512];
        let out = chain.process(&silence);
        assert!(
            out.iter().all(|v| v.abs() < 1e-9),
            "reset 后静音输入不应有残留输出"
        );
    }

    #[test]
    fn dsp_chain_empty_frame_is_empty() {
        let eq = Equalizer::flat();
        let mut chain = DspChain::from_equalizer(&eq, 44_100.0);
        assert!(chain.process(&[]).is_empty());
        assert_eq!(chain.filters.len(), 20, "立体声 2 × 10 段");
    }
}
