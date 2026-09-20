//! 10 段参数均衡器（EQ）。
//!
//! 对应类图中的 [`Equalizer`] 与 [`EqPreset`]。每段对应一个固定中心频率，
//! 增益以 dB 表示；[`Equalizer::to_biquad_coeffs`] 把当前 10 段增益转换为
//! 10 个 RBJ 峰值 [`BiquadCoeff`]，供 [`DspChain`](crate::audio::dsp::DspChain)
//! 使用。

use std::fmt;

use crate::audio::dsp::BiquadCoeff;

/// 均衡器预设。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EqPreset {
    /// 平直（所有段 0 dB）。
    Flat,
    /// 流行。
    Pop,
    /// 摇滚。
    Rock,
    /// 古典。
    Classical,
    /// 人声。
    Vocal,
    /// 自定义（用户手动调节，保留当前 bands）。
    Custom,
}

impl EqPreset {
    /// 全部预设（供 UI 下拉列表枚举）。
    pub const ALL: [EqPreset; 6] = [
        EqPreset::Flat,
        EqPreset::Pop,
        EqPreset::Rock,
        EqPreset::Classical,
        EqPreset::Vocal,
        EqPreset::Custom,
    ];

    /// 各预设的 10 段增益（dB）。频率顺序见 [`Equalizer::band_freqs`]。
    pub fn bands(self) -> [f32; 10] {
        match self {
            EqPreset::Flat | EqPreset::Custom => [0.0; 10],
            EqPreset::Pop => [-1.5, 1.0, 2.5, 3.0, 1.5, -0.5, -1.0, -1.0, 0.0, 1.5],
            EqPreset::Rock => [3.5, 2.0, 1.0, 0.5, -0.5, 1.0, 2.0, 2.5, 2.5, 2.0],
            EqPreset::Classical => [2.0, 1.5, 0.5, 0.0, 0.0, 0.0, 0.0, 0.5, 1.5, 2.5],
            EqPreset::Vocal => [-2.0, -1.0, 0.5, 2.5, 3.5, 3.0, 1.5, 0.0, -1.0, -2.0],
        }
    }

    /// 返回人类可读名称（中文）。
    pub fn label(self) -> &'static str {
        match self {
            EqPreset::Flat => "平直",
            EqPreset::Pop => "流行",
            EqPreset::Rock => "摇滚",
            EqPreset::Classical => "古典",
            EqPreset::Vocal => "人声",
            EqPreset::Custom => "自定义",
        }
    }

    /// 由名称 / 标签解析预设（用于设置反序列化）。
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "Flat" | "平直" => Some(EqPreset::Flat),
            "Pop" | "流行" => Some(EqPreset::Pop),
            "Rock" | "摇滚" => Some(EqPreset::Rock),
            "Classical" | "古典" => Some(EqPreset::Classical),
            "Vocal" | "人声" => Some(EqPreset::Vocal),
            "Custom" | "自定义" => Some(EqPreset::Custom),
            _ => None,
        }
    }
}

impl fmt::Display for EqPreset {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

/// 10 段参数均衡器。
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Equalizer {
    /// 各段增益（dB），默认 0。
    pub bands: [f32; 10],
    /// 各段中心频率（Hz）：31, 62, 125, 250, 500, 1k, 2k, 4k, 8k, 16k。
    pub band_freqs: [f32; 10],
    /// 主增益（dB）。
    pub master_gain_db: f32,
    /// 是否启用 EQ（关闭时所有段旁路，主增益仍生效）。
    pub enabled: bool,
    /// 当前预设。
    pub preset: EqPreset,
}

impl Default for Equalizer {
    fn default() -> Self {
        Self::flat()
    }
}

impl Equalizer {
    /// 构造平直均衡器（所有段 0 dB，主增益 0 dB，启用，预设 Flat）。
    pub fn flat() -> Self {
        Self {
            bands: [0.0; 10],
            band_freqs: [
                31.0, 62.0, 125.0, 250.0, 500.0, 1000.0, 2000.0, 4000.0, 8000.0, 16000.0,
            ],
            master_gain_db: 0.0,
            enabled: true,
            preset: EqPreset::Flat,
        }
    }

    /// 把当前 10 段增益转换为 Biquad 系数（每段一个 RBJ 峰值滤波器）。
    ///
    /// 仅当 `enabled` 且增益非零时生成有效系数；若某段为 0 dB 仍生成单位增益
    /// 系数（数值恒等），保证链路长度稳定。
    pub fn to_biquad_coeffs(&self, sample_rate: f32) -> Vec<BiquadCoeff> {
        const Q: f32 = 1.0;
        self.band_freqs
            .iter()
            .zip(self.bands.iter())
            .map(|(&f0, &gain)| {
                if !self.enabled || gain == 0.0 {
                    BiquadCoeff::peaking(f0, sample_rate, 0.0, Q)
                } else {
                    BiquadCoeff::peaking(f0, sample_rate, gain, Q)
                }
            })
            .collect()
    }

    /// 应用预设：非 Custom 预设会覆盖 `bands`；Custom 仅设置标记，保留当前 bands。
    pub fn apply_preset(&mut self, preset: EqPreset) {
        self.preset = preset;
        if preset != EqPreset::Custom {
            self.bands = preset.bands();
        }
    }

    /// 设置单段增益（dB），并自动将预设标记为 Custom。
    pub fn set_band(&mut self, index: usize, gain_db: f32) {
        if index < self.bands.len() {
            self.bands[index] = gain_db;
            self.preset = EqPreset::Custom;
        }
    }

    /// 主增益上限钳制：线性值 ≤ 2.0（≈ +6 dB），返回是否触发钳制。
    ///
    /// 超过阈值时 UI 显示「增益过高可能削波」告警（软限幅提示，不直接硬限）。
    pub fn clamp_master_gain(&mut self) -> bool {
        const MAX_DB: f32 = 6.0;
        if self.master_gain_db > MAX_DB {
            self.master_gain_db = MAX_DB;
            true
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preset_bands_are_in_range() {
        for p in [
            EqPreset::Pop,
            EqPreset::Rock,
            EqPreset::Classical,
            EqPreset::Vocal,
        ] {
            for &g in p.bands().iter() {
                assert!(
                    (-12.0..=12.0).contains(&g),
                    "预设 {:?} 段增益应在 ±12dB 内，实际 {}", p, g
                );
            }
        }
    }

    #[test]
    fn apply_preset_overrides_bands() {
        let mut eq = Equalizer::flat();
        eq.set_band(0, 9.0); // 触发 Custom
        assert_eq!(eq.preset, EqPreset::Custom);
        eq.apply_preset(EqPreset::Rock);
        assert_eq!(eq.preset, EqPreset::Rock);
        assert_eq!(eq.bands, EqPreset::Rock.bands());
    }

    #[test]
    fn to_biquad_coeffs_length_matches_bands() {
        let eq = Equalizer::flat();
        let coeffs = eq.to_biquad_coeffs(44100.0);
        assert_eq!(coeffs.len(), 10);
    }

    #[test]
    fn clamp_master_gain() {
        let mut eq = Equalizer::flat();
        eq.master_gain_db = 12.0;
        assert!(eq.clamp_master_gain());
        assert_eq!(eq.master_gain_db, 6.0);
    }

    // ──────────────────────────────────────────────────────────────
    // QA 补充测试
    // ──────────────────────────────────────────────────────────────

    #[test]
    fn flat_defaults_are_zero() {
        let eq = Equalizer::flat();
        assert_eq!(eq.bands, [0.0; 10]);
        assert_eq!(eq.master_gain_db, 0.0);
        assert!(eq.enabled);
        assert_eq!(eq.preset, EqPreset::Flat);
        assert_eq!(eq, Equalizer::default());
    }

    #[test]
    fn band_freqs_are_ascending_31hz_to_16khz() {
        let eq = Equalizer::flat();
        assert_eq!(eq.band_freqs[0], 31.0);
        assert_eq!(eq.band_freqs[9], 16_000.0);
        for w in eq.band_freqs.windows(2) {
            assert!(w[1] > w[0], "频段频率应严格递增");
        }
    }

    #[test]
    fn set_band_out_of_range_is_ignored() {
        let mut eq = Equalizer::flat();
        eq.set_band(99, 9.0);
        assert_eq!(eq.bands, [0.0; 10], "越界下标不应修改任何段");
        assert_eq!(eq.preset, EqPreset::Flat, "越界不应误标为 Custom");
    }

    #[test]
    fn set_band_marks_custom() {
        let mut eq = Equalizer::flat();
        eq.set_band(3, 4.5);
        assert_eq!(eq.bands[3], 4.5);
        assert_eq!(eq.preset, EqPreset::Custom);
    }

    #[test]
    fn apply_custom_preset_keeps_bands() {
        let mut eq = Equalizer::flat();
        eq.set_band(0, 7.0);
        let kept = eq.bands;
        eq.apply_preset(EqPreset::Custom);
        assert_eq!(eq.bands, kept, "Custom 预设不应覆盖现有 bands");
        assert_eq!(eq.preset, EqPreset::Custom);
    }

    #[test]
    fn preset_from_str_accepts_english_and_chinese() {
        assert_eq!(EqPreset::from_str("Flat"), Some(EqPreset::Flat));
        assert_eq!(EqPreset::from_str("平直"), Some(EqPreset::Flat));
        assert_eq!(EqPreset::from_str("Rock"), Some(EqPreset::Rock));
        assert_eq!(EqPreset::from_str("摇滚"), Some(EqPreset::Rock));
        assert_eq!(EqPreset::from_str("Custom"), Some(EqPreset::Custom));
        assert_eq!(EqPreset::from_str("自定义"), Some(EqPreset::Custom));
    }

    #[test]
    fn preset_from_str_unknown_is_none() {
        assert_eq!(EqPreset::from_str("Metal"), None);
        assert_eq!(EqPreset::from_str(""), None);
    }

    #[test]
    fn preset_label_roundtrips() {
        for p in EqPreset::ALL {
            assert_eq!(EqPreset::from_str(p.label()), Some(p));
        }
    }

    #[test]
    fn master_gain_clamp_boundary_is_exclusive() {
        let mut eq = Equalizer::flat();
        eq.master_gain_db = 6.0; // 正好等于上限 → 不触发钳制
        assert!(!eq.clamp_master_gain());
        assert_eq!(eq.master_gain_db, 6.0);

        eq.master_gain_db = 6.01;
        assert!(eq.clamp_master_gain());
        assert_eq!(eq.master_gain_db, 6.0);

        eq.master_gain_db = -6.0; // 负增益不触发
        assert!(!eq.clamp_master_gain());
    }

    #[test]
    fn disabled_eq_yields_identity_coeffs_even_with_nonzero_bands() {
        let mut eq = Equalizer::flat();
        eq.bands = [12.0; 10];
        eq.enabled = false;
        let coeffs = eq.to_biquad_coeffs(48_000.0);
        assert_eq!(coeffs.len(), 10);
        for c in &coeffs {
            // 关闭 EQ → 每段为 0 dB 峰值 = 恒等滤波器
            assert!((c.b0 - 1.0).abs() < 1e-5);
            assert!((c.b1 - c.a1).abs() < 1e-5);
            assert!((c.b2 - c.a2).abs() < 1e-5);
        }
    }

    #[test]
    fn zero_gain_bands_yield_identity_coeffs_when_enabled() {
        let eq = Equalizer::flat(); // enabled=true, 全 0dB
        for c in &eq.to_biquad_coeffs(44_100.0) {
            assert!((c.b0 - 1.0).abs() < 1e-5);
            assert!((c.b1 - c.a1).abs() < 1e-5);
            assert!((c.b2 - c.a2).abs() < 1e-5);
        }
    }
}
