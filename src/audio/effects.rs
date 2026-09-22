//! DSP 音效增强引擎：3D 空间立体声拓宽、心理声学低音增强、人声水晶通透与防爆音软限幅。
//!
//! 纯逻辑实现，无 GUI / 外部重型依赖，完全可脱离图形环境独立测试。
//!
//! 设计特点：
//! - **立体声声场拓宽 (Stereo Widener)**：Mid/Side (M/S) 分离 + 耳间梳状微互馈，打破耳机“头中效应”。
//! - **动态低音增强 (Bass Boost)**：低频共振滤波与谐波激励复合，小耳机上也能呈现深沉弹性低音。
//! - **人声水晶通透 (Vocal Crystalizer)**：高频泛音与唇齿细节激励，使人声更加贴耳清澈、乐器更有空气感。
//! - **零延迟软限幅 (Soft Limiter)**：高增益/多音效并发时杜绝数字硬削波破音。

use serde::{Deserialize, Serialize};

use crate::audio::dsp::{Biquad, BiquadCoeff};

/// 音效参数配置（支持序列化与持久化存储）。
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct AudioEffects {
    /// 3D 空间立体声拓宽开关。
    #[serde(default)]
    pub stereo_widener_enabled: bool,
    /// 3D 空间立体声拓宽强度（0.0..=1.0，默认 0.5）。
    #[serde(default = "default_level")]
    pub stereo_widener_level: f32,

    /// 动态低音增强开关。
    #[serde(default)]
    pub bass_boost_enabled: bool,
    /// 动态低音增强强度（0.0..=1.0，默认 0.5）。
    #[serde(default = "default_level")]
    pub bass_boost_level: f32,

    /// 人声水晶通透开关。
    #[serde(default)]
    pub vocal_crystalizer_enabled: bool,
    /// 人声水晶通透强度（0.0..=1.0，默认 0.5）。
    #[serde(default = "default_level")]
    pub vocal_crystalizer_level: f32,
}

fn default_level() -> f32 {
    0.5
}

impl Default for AudioEffects {
    fn default() -> Self {
        Self {
            stereo_widener_enabled: false,
            stereo_widener_level: 0.5,
            bass_boost_enabled: false,
            bass_boost_level: 0.5,
            vocal_crystalizer_enabled: false,
            vocal_crystalizer_level: 0.5,
        }
    }
}

/// 运行时音效信号处理器（持有滤波器状态与延迟环形缓冲）。
#[derive(Clone, Debug)]
pub struct AudioEffectsProcessor {
    sample_rate: f32,

    // ── 立体声拓宽环形延迟线（约 0.4ms~0.8ms Haas 效应）──
    delay_buffer_l: Vec<f32>,
    delay_buffer_r: Vec<f32>,
    delay_idx: usize,

    // ── 低音增强滤波器（左右声道独立，低频 75Hz Peaking 滤波）──
    bass_filter_l: Biquad,
    bass_filter_r: Biquad,

    // ── 人声水晶通透高频激励滤波器（左右声道独立，3.5kHz~5.5kHz Peaking 滤波）──
    crystal_filter_l: Biquad,
    crystal_filter_r: Biquad,
}

impl AudioEffectsProcessor {
    /// 根据采样率构造处理器。
    pub fn new(sample_rate: f32) -> Self {
        // 0.6ms 延迟线长度
        let delay_len = ((sample_rate * 0.0006) as usize).clamp(16, 256);
        let flat_coeff = BiquadCoeff::peaking(1000.0, sample_rate, 0.0, 1.0);

        Self {
            sample_rate,
            delay_buffer_l: vec![0.0; delay_len],
            delay_buffer_r: vec![0.0; delay_len],
            delay_idx: 0,
            bass_filter_l: Biquad::new(flat_coeff),
            bass_filter_r: Biquad::new(flat_coeff),
            crystal_filter_l: Biquad::new(flat_coeff),
            crystal_filter_r: Biquad::new(flat_coeff),
        }
    }

    /// 更新音效滤波器系数（当参数或采样率变动时调用）。
    pub fn update_coefficients(&mut self, effects: &AudioEffects) {
        let fs = self.sample_rate;

        // 1. 低音增强：中心频率 75Hz，根据 level 提供 0 ~ +9dB 提升，Q 约 1.2
        if effects.bass_boost_enabled && effects.bass_boost_level > 0.0 {
            let gain_db = effects.bass_boost_level.clamp(0.0, 1.0) * 9.0;
            let coeff = BiquadCoeff::peaking(75.0, fs, gain_db, 1.2);
            self.bass_filter_l.coeff = coeff;
            self.bass_filter_r.coeff = coeff;
        } else {
            let coeff = BiquadCoeff::peaking(75.0, fs, 0.0, 1.2);
            self.bass_filter_l.coeff = coeff;
            self.bass_filter_r.coeff = coeff;
        }

        // 2. 人声水晶通透：中心频率 4200Hz，根据 level 提供 0 ~ +7.5dB 提升，Q 约 1.0
        if effects.vocal_crystalizer_enabled && effects.vocal_crystalizer_level > 0.0 {
            let gain_db = effects.vocal_crystalizer_level.clamp(0.0, 1.0) * 7.5;
            let coeff = BiquadCoeff::peaking(4200.0, fs, gain_db, 1.0);
            self.crystal_filter_l.coeff = coeff;
            self.crystal_filter_r.coeff = coeff;
        } else {
            let coeff = BiquadCoeff::peaking(4200.0, fs, 0.0, 1.0);
            self.crystal_filter_l.coeff = coeff;
            self.crystal_filter_r.coeff = coeff;
        }
    }

    /// 处理交错立体声 PCM 缓冲区（原位修改）。
    pub fn process_interleaved(&mut self, buffer: &mut [f32], effects: &AudioEffects) {
        let has_widener = effects.stereo_widener_enabled && effects.stereo_widener_level > 0.0;
        let has_bass = effects.bass_boost_enabled && effects.bass_boost_level > 0.0;
        let has_crystal =
            effects.vocal_crystalizer_enabled && effects.vocal_crystalizer_level > 0.0;

        if !has_widener && !has_bass && !has_crystal {
            // 所有音效均未启用，保持 100% 原汁原味直通
            return;
        }

        let delay_len = self.delay_buffer_l.len();
        let widener_amount = effects.stereo_widener_level.clamp(0.0, 1.0);
        // 拓宽倍数：1.0 到 1.85 之间
        let width_factor = 1.0 + widener_amount * 0.85;
        // 增益能量补偿，防止过大幅度
        let width_norm = 1.0 / (1.0 + 0.35 * widener_amount);

        let bass_harmonics_mix = if has_bass {
            effects.bass_boost_level.clamp(0.0, 1.0) * 0.25
        } else {
            0.0
        };

        for chunk in buffer.chunks_exact_mut(2) {
            let mut left = chunk[0];
            let mut right = chunk[1];

            // ── 阶段 1：低音共振滤波与心理声学低频谐波 ──
            if has_bass {
                left = self.bass_filter_l.process(left);
                right = self.bass_filter_r.process(right);

                if bass_harmonics_mix > 0.0 {
                    // 轻微非线性软饱和提取低次偶次与奇次谐波（丰富听觉下潜感知）
                    let harm_l = (left * 1.5).tanh() * 0.6;
                    let harm_r = (right * 1.5).tanh() * 0.6;
                    left += harm_l * bass_harmonics_mix;
                    right += harm_r * bass_harmonics_mix;
                }
            }

            // ── 阶段 2：人声与高频通透水晶激励 ──
            if has_crystal {
                left = self.crystal_filter_l.process(left);
                right = self.crystal_filter_r.process(right);
            }

            // ── 阶段 3：3D 空间立体声拓宽（M/S 分解 + 耳间微互馈）──
            if has_widener {
                let mid = (left + right) * 0.5;
                let side = (left - right) * 0.5;

                // 侧向信号拓宽
                let widened_side = side * width_factor;

                // 取微秒级耳间延迟
                let delayed_l = self.delay_buffer_l[self.delay_idx];
                let delayed_r = self.delay_buffer_r[self.delay_idx];

                self.delay_buffer_l[self.delay_idx] = left;
                self.delay_buffer_r[self.delay_idx] = right;
                self.delay_idx = (self.delay_idx + 1) % delay_len;

                // 交叉微反馈模拟耳廓声学绕射（跨耳微延迟）
                let cross_feed = 0.12 * widener_amount;
                let final_left = (mid + widened_side - delayed_r * cross_feed) * width_norm;
                let final_right = (mid - widened_side - delayed_l * cross_feed) * width_norm;

                left = final_left;
                right = final_right;
            }

            // ── 阶段 4：防破音软限幅 ──
            chunk[0] = soft_limit(left);
            chunk[1] = soft_limit(right);
        }
    }

    /// 重置所有延迟状态与滤波器状态（切歌 / 跳转时调用）。
    pub fn reset(&mut self) {
        self.delay_buffer_l.fill(0.0);
        self.delay_buffer_r.fill(0.0);
        self.delay_idx = 0;
        self.bass_filter_l.reset();
        self.bass_filter_r.reset();
        self.crystal_filter_l.reset();
        self.crystal_filter_r.reset();
    }
}

/// 平滑零延迟抗削波软限幅器。
///
/// 当 |x| <= 0.98 时严格保持 1:1 线性原声；
/// 当 |x| > 0.98 时，采用平滑曲线将其圆滑渐近压缩至 [-1.0, 1.0]，彻底消除刺耳硬剪切爆音。
#[inline(always)]
pub fn soft_limit(x: f32) -> f32 {
    const THRESHOLD: f32 = 0.98;
    const MARGIN: f32 = 1.0 - THRESHOLD;

    if x.abs() <= THRESHOLD {
        x
    } else if x > THRESHOLD {
        let excess = x - THRESHOLD;
        THRESHOLD + MARGIN * (excess / MARGIN).tanh()
    } else {
        let excess = -x - THRESHOLD;
        -(THRESHOLD + MARGIN * (excess / MARGIN).tanh())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_effects_are_disabled() {
        let effects = AudioEffects::default();
        assert!(!effects.stereo_widener_enabled);
        assert!(!effects.bass_boost_enabled);
        assert!(!effects.vocal_crystalizer_enabled);
    }

    #[test]
    fn soft_limit_linear_region() {
        assert_eq!(soft_limit(0.0), 0.0);
        assert_eq!(soft_limit(0.5), 0.5);
        assert_eq!(soft_limit(-0.5), -0.5);
        assert_eq!(soft_limit(0.98), 0.98);
        assert_eq!(soft_limit(-0.98), -0.98);
    }

    #[test]
    fn soft_limit_compresses_extreme_values() {
        let over = soft_limit(2.0);
        assert!(over > 0.98 && over <= 1.0);

        let extreme = soft_limit(10.0);
        assert!(extreme > 0.98 && extreme <= 1.0);

        let negative_extreme = soft_limit(-10.0);
        assert!((-1.0..=-0.98).contains(&negative_extreme));
    }

    #[test]
    fn processor_bypass_preserves_audio() {
        let mut proc = AudioEffectsProcessor::new(44100.0);
        let effects = AudioEffects::default();
        let mut buffer = vec![0.1, -0.2, 0.3, -0.4];
        let original = buffer.clone();

        proc.process_interleaved(&mut buffer, &effects);
        for (a, b) in buffer.iter().zip(original.iter()) {
            assert!((a - b).abs() < 1e-6);
        }
    }

    #[test]
    fn processor_widener_modifies_stereo_channels() {
        let mut proc = AudioEffectsProcessor::new(44100.0);
        let effects = AudioEffects {
            stereo_widener_enabled: true,
            stereo_widener_level: 0.8,
            ..Default::default()
        };
        proc.update_coefficients(&effects);

        let mut buffer = vec![0.5, -0.3, 0.2, 0.4];
        let original = buffer.clone();
        proc.process_interleaved(&mut buffer, &effects);

        // 经立体声拓宽后，左右声道差分能量增加，数值应产生有界合理变化
        assert_ne!(buffer, original);
        for s in &buffer {
            assert!(s.abs() <= 1.0);
        }
    }

    #[test]
    fn processor_bass_boost_alters_low_frequency() {
        let mut proc = AudioEffectsProcessor::new(44100.0);
        let effects = AudioEffects {
            bass_boost_enabled: true,
            bass_boost_level: 1.0,
            ..Default::default()
        };
        proc.update_coefficients(&effects);

        let mut buffer = vec![0.2; 128];
        proc.process_interleaved(&mut buffer, &effects);

        for s in &buffer {
            assert!(s.abs() <= 1.0);
        }
    }
}
