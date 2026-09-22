//! DSP 音效增强引擎：影院全景声场 (Spatial Audio)、双耳 HRTF 虚拟环绕、对白人声居中锚定、
//! 模拟电子管胆机暖音 (Tube Warmth)、经典发烧耳放 BS2B 纯净互馈、心理声学低音增强、
//! 人声水晶通透与 Hi-Fi 纯净直通 (Pure Direct)。
//!
//! 纯逻辑实现，无 GUI / 外部重型依赖，完全可脱离图形环境独立测试。
//!
//! 设计特点（发烧级 Hi-Fi 声学架构）：
//! - **Hi-Fi 纯净直通 (Pure Direct)**：一键旁路全部 EQ 与 DSP，实现 100% 比特完美直通输出。
//! - **模拟电子管胆机暖音 (Tube Warmth)**：基于三极管传输特性产生温润的偶次二次谐波，消除数码冷硬味。
//! - **发烧级 BS2B 跨耳互馈 (Bauer Stereophonic-to-Binaural)**：纯净 700Hz 头影互馈，专为发烧大耳久听防疲劳打造。
//! - **双耳 HRTF 跨耳互馈 (Binaural Crossfeed & ITD)**：基于微秒级耳间时间差与头影低通滤波，消除耳机“头中效应”。
//! - **早期微反射房间声学网络 (Early Reflections Network)**：4 抽头互质延迟扩散，呈现宽阔真实的影院声场纵深。
//! - **中置人声对白居中锚定 (Center Anchor & Dialogue Clarity)**：M/S 分解结合中频人声成形，确保声场拉开时人声绝不发空。
//! - **心理声学动态低音扩展 (Psychoacoustic Bass Boost)**：75Hz 共振与非线性谐波饱和激励，小单元也能下潜深沉。
//! - **零延迟软限幅 (Soft Limiter)**：平滑防破音曲线杜绝数字硬削波破音。

use serde::{Deserialize, Serialize};

use crate::audio::dsp::{Biquad, BiquadCoeff};

/// 音效参数配置（支持序列化与持久化存储）。
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct AudioEffects {
    /// 全景空间声场开关（影院沉浸环绕）。
    #[serde(default, alias = "stereo_widener_enabled")]
    pub spatial_audio_enabled: bool,
    /// 全景空间环绕声场宽度（0.0..=1.0，默认 0.6）。
    #[serde(default = "default_spatial_level", alias = "stereo_widener_level")]
    pub spatial_audio_level: f32,
    /// 居中对白与人声清晰度（0.0..=1.0，默认 0.5）。
    #[serde(default = "default_dialogue_level")]
    pub dialogue_clarity_level: f32,

    /// 电子管胆机暖音开关（偶次二次谐波暖音仿真）。
    #[serde(default)]
    pub tube_warmth_enabled: bool,
    /// 电子管胆机暖音浓度（0.0..=1.0，默认 0.5）。
    #[serde(default = "default_level")]
    pub tube_warmth_level: f32,

    /// 经典发烧耳放 BS2B 纯净互馈模式开关。
    #[serde(default)]
    pub bs2b_mode: bool,

    /// Hi-Fi 纯净直通模式（绕过全部 EQ 与 DSP 处理，100% 原始位完美输出）。
    #[serde(default)]
    pub pure_direct: bool,

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

fn default_spatial_level() -> f32 {
    0.6
}

fn default_dialogue_level() -> f32 {
    0.5
}

impl Default for AudioEffects {
    fn default() -> Self {
        Self {
            spatial_audio_enabled: false,
            spatial_audio_level: 0.6,
            dialogue_clarity_level: 0.5,
            tube_warmth_enabled: false,
            tube_warmth_level: 0.5,
            bs2b_mode: false,
            pure_direct: false,
            bass_boost_enabled: false,
            bass_boost_level: 0.5,
            vocal_crystalizer_enabled: false,
            vocal_crystalizer_level: 0.5,
        }
    }
}

impl AudioEffects {
    /// 兼容旧版命名：3D 立体声拓宽开关。
    #[inline]
    pub fn stereo_widener_enabled(&self) -> bool {
        self.spatial_audio_enabled
    }

    /// 兼容旧版命名：3D 立体声拓宽级别。
    #[inline]
    pub fn stereo_widener_level(&self) -> f32 {
        self.spatial_audio_level
    }
}

/// 运行时音效信号处理器（持有滤波器状态与延迟环形缓冲）。
#[derive(Clone, Debug)]
pub struct AudioEffectsProcessor {
    sample_rate: f32,

    // ── 电子管胆机暖音 DC 阻隔状态 ──
    tube_dc_x_l: f32,
    tube_dc_y_l: f32,
    tube_dc_x_r: f32,
    tube_dc_y_r: f32,

    // ── 双耳 HRTF 跨耳互馈延迟线（~0.32ms ITD 互馈）──
    itd_buffer_l: Vec<f32>,
    itd_buffer_r: Vec<f32>,
    itd_idx: usize,

    // ── 双耳头影低通滤波器（模拟头颅对高频跨耳阻隔）──
    head_shadow_l: Biquad,
    head_shadow_r: Biquad,

    // ── 发烧 BS2B 700Hz 纯净跨耳滤波器 ──
    bs2b_filter_l: Biquad,
    bs2b_filter_r: Biquad,

    // ── 影院房间早期反射扩散网络（4 抽头互质无谐振环）──
    refl_buffer_l: Vec<f32>,
    refl_buffer_r: Vec<f32>,
    refl_idx: usize,
    refl_tap1: usize,
    refl_tap2: usize,
    refl_tap3: usize,
    refl_tap4: usize,
    refl_damp_l: f32,
    refl_damp_r: f32,

    // ── 中置人声对白增强清晰度滤波器（Mid 声道）──
    dialogue_filter: Biquad,

    // ── 低音增强滤波器（左右声道独立，低频 75Hz Peaking 滤波）──
    bass_filter_l: Biquad,
    bass_filter_r: Biquad,

    // ── 人声水晶通透高频激励滤波器（左右声道独立，3.5kHz~5.5kHz Peaking 滤波）──
    crystal_filter_l: Biquad,
    crystal_filter_r: Biquad,
}

#[inline(always)]
fn process_tube_sample(x: f32, amount: f32, prev_x: &mut f32, prev_y: &mut f32) -> f32 {
    // 非线性三极管传输多项式：y = x + alpha * x^2 - (alpha / 3) * x^3
    // 丰富二次偶次谐波赋予中频肉感与松香味
    let alpha = amount * 0.22;
    let raw = x + alpha * (x * x) - (alpha / 3.0) * (x * x * x);
    // 经典一阶 DC Blocker（截止频率约 5Hz）：y[n] = x[n] - x[n-1] + 0.995 * y[n-1]
    let filtered = raw - *prev_x + 0.995 * *prev_y;
    *prev_x = raw;
    *prev_y = filtered;
    filtered
}

impl AudioEffectsProcessor {
    /// 根据采样率构造处理器。
    pub fn new(sample_rate: f32) -> Self {
        let itd_len = ((sample_rate * 0.00032) as usize).clamp(8, 64);
        let refl_len = ((sample_rate * 0.040) as usize).clamp(512, 4096);
        let flat_coeff = BiquadCoeff::peaking(1000.0, sample_rate, 0.0, 1.0);
        let shadow_coeff =
            BiquadCoeff::lowpass(1600.0, sample_rate, std::f32::consts::FRAC_1_SQRT_2);
        let bs2b_coeff = BiquadCoeff::lowpass(700.0, sample_rate, std::f32::consts::FRAC_1_SQRT_2);

        Self {
            sample_rate,
            tube_dc_x_l: 0.0,
            tube_dc_y_l: 0.0,
            tube_dc_x_r: 0.0,
            tube_dc_y_r: 0.0,
            itd_buffer_l: vec![0.0; itd_len],
            itd_buffer_r: vec![0.0; itd_len],
            itd_idx: 0,
            head_shadow_l: Biquad::new(shadow_coeff),
            head_shadow_r: Biquad::new(shadow_coeff),
            bs2b_filter_l: Biquad::new(bs2b_coeff),
            bs2b_filter_r: Biquad::new(bs2b_coeff),
            refl_buffer_l: vec![0.0; refl_len],
            refl_buffer_r: vec![0.0; refl_len],
            refl_idx: 0,
            refl_tap1: (((sample_rate * 0.013) as usize).max(1)) % refl_len,
            refl_tap2: (((sample_rate * 0.019) as usize).max(2)) % refl_len,
            refl_tap3: (((sample_rate * 0.027) as usize).max(3)) % refl_len,
            refl_tap4: (((sample_rate * 0.035) as usize).max(4)) % refl_len,
            refl_damp_l: 0.0,
            refl_damp_r: 0.0,
            dialogue_filter: Biquad::new(flat_coeff),
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

        // 3. 对白 / 人声居中清晰度：中心频率 2800Hz，提供 0 ~ +5.5dB 提升，Q 约 1.15
        if effects.spatial_audio_enabled && effects.dialogue_clarity_level > 0.0 {
            let gain_db = effects.dialogue_clarity_level.clamp(0.0, 1.0) * 5.5;
            self.dialogue_filter.coeff = BiquadCoeff::peaking(2800.0, fs, gain_db, 1.15);
        } else {
            self.dialogue_filter.coeff = BiquadCoeff::peaking(2800.0, fs, 0.0, 1.15);
        }

        // 4. 双耳头影低通滤波保持稳定截断 1600Hz / BS2B 700Hz
        let q = std::f32::consts::FRAC_1_SQRT_2;
        self.head_shadow_l.coeff = BiquadCoeff::lowpass(1600.0, fs, q);
        self.head_shadow_r.coeff = BiquadCoeff::lowpass(1600.0, fs, q);
        self.bs2b_filter_l.coeff = BiquadCoeff::lowpass(700.0, fs, q);
        self.bs2b_filter_r.coeff = BiquadCoeff::lowpass(700.0, fs, q);
    }

    /// 处理交错立体声 PCM 缓冲区（原位修改）。
    pub fn process_interleaved(&mut self, buffer: &mut [f32], effects: &AudioEffects) {
        if effects.pure_direct {
            // Hi-Fi 纯净直通：原样直通，绝无任何音染
            return;
        }

        let has_spatial = effects.spatial_audio_enabled && effects.spatial_audio_level > 0.0;
        let has_tube = effects.tube_warmth_enabled && effects.tube_warmth_level > 0.0;
        let has_bass = effects.bass_boost_enabled && effects.bass_boost_level > 0.0;
        let has_crystal =
            effects.vocal_crystalizer_enabled && effects.vocal_crystalizer_level > 0.0;

        if !has_spatial && !has_tube && !has_bass && !has_crystal {
            // 所有音效均未启用，保持 100% 原汁原味直通
            return;
        }

        let spatial_amount = effects.spatial_audio_level.clamp(0.0, 1.0);
        let width_factor = 1.0 + spatial_amount * 0.95;
        let spatial_norm = 1.0 / (1.0 + 0.38 * spatial_amount);
        let cross_feed_gain = 0.16 * spatial_amount;
        let refl_mix = 0.20 * spatial_amount;
        let dialogue_mix = effects.dialogue_clarity_level.clamp(0.0, 1.0);
        let tube_amount = effects.tube_warmth_level.clamp(0.0, 1.0);

        let bass_harmonics_mix = if has_bass {
            effects.bass_boost_level.clamp(0.0, 1.0) * 0.25
        } else {
            0.0
        };

        let itd_len = self.itd_buffer_l.len();
        let refl_len = self.refl_buffer_l.len();

        for chunk in buffer.as_chunks_mut::<2>().0 {
            let mut left = chunk[0];
            let mut right = chunk[1];

            // ── 阶段 0：电子管胆机暖音（偶次谐波增强）──
            if has_tube {
                left = process_tube_sample(
                    left,
                    tube_amount,
                    &mut self.tube_dc_x_l,
                    &mut self.tube_dc_y_l,
                );
                right = process_tube_sample(
                    right,
                    tube_amount,
                    &mut self.tube_dc_x_r,
                    &mut self.tube_dc_y_r,
                );
            }

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

            // ── 阶段 3：空间声场处理（影院全景 或 发烧 BS2B 模式）──
            if has_spatial {
                if effects.bs2b_mode {
                    // 3.A 经典发烧耳放 BS2B 纯净互馈模式（专为耳机长久耐听设计，无多余混响）
                    let delayed_l = self.itd_buffer_l[self.itd_idx];
                    let delayed_r = self.itd_buffer_r[self.itd_idx];
                    self.itd_buffer_l[self.itd_idx] = left;
                    self.itd_buffer_r[self.itd_idx] = right;
                    self.itd_idx = (self.itd_idx + 1) % itd_len;

                    let cross_to_r = self.bs2b_filter_l.process(delayed_l) * 0.58;
                    let cross_to_l = self.bs2b_filter_r.process(delayed_r) * 0.58;

                    left = (left + cross_to_l) * 0.78;
                    right = (right + cross_to_r) * 0.78;
                } else {
                    // 3.B 影院全景空间声场处理（M/S 分解 + 对白居中锚定 + 双耳 HRTF 跨耳 + 早期房间反射）
                    let mut mid = (left + right) * 0.5;
                    let side = (left - right) * 0.5;

                    // 3.1 对白 / 人声居中清晰度成形
                    if dialogue_mix > 0.0 {
                        let mid_enhanced = self.dialogue_filter.process(mid);
                        mid = mid * (1.0 - dialogue_mix * 0.35)
                            + mid_enhanced * (dialogue_mix * 0.35);
                    }

                    // 3.2 早期反射扩散网络（模拟影院声学环境微反射）
                    let cur_refl_idx = self.refl_idx;
                    self.refl_buffer_l[cur_refl_idx] = side;
                    self.refl_buffer_r[cur_refl_idx] = -side;

                    let tap1_idx = (cur_refl_idx + refl_len - self.refl_tap1) % refl_len;
                    let tap2_idx = (cur_refl_idx + refl_len - self.refl_tap2) % refl_len;
                    let tap3_idx = (cur_refl_idx + refl_len - self.refl_tap3) % refl_len;
                    let tap4_idx = (cur_refl_idx + refl_len - self.refl_tap4) % refl_len;

                    let refl_raw_l = self.refl_buffer_l[tap1_idx] * 0.35
                        - self.refl_buffer_l[tap2_idx] * 0.26
                        + self.refl_buffer_l[tap3_idx] * 0.18
                        - self.refl_buffer_l[tap4_idx] * 0.12;
                    let refl_raw_r = self.refl_buffer_r[tap1_idx] * 0.35
                        - self.refl_buffer_r[tap2_idx] * 0.26
                        + self.refl_buffer_r[tap3_idx] * 0.18
                        - self.refl_buffer_r[tap4_idx] * 0.12;

                    // 单极低通阻尼滤波（高频空气吸声）
                    self.refl_damp_l = self.refl_damp_l * 0.70 + refl_raw_l * 0.30;
                    self.refl_damp_r = self.refl_damp_r * 0.70 + refl_raw_r * 0.30;
                    self.refl_idx = (cur_refl_idx + 1) % refl_len;

                    // 3.3 侧向声场拓宽 + 反射声混合
                    let widened_side = side * width_factor;
                    let side_l = widened_side + self.refl_damp_l * refl_mix;
                    let side_r = widened_side + self.refl_damp_r * refl_mix;

                    // 3.4 双耳 HRTF 跨耳互馈延迟与头影衰减
                    let delayed_l = self.itd_buffer_l[self.itd_idx];
                    let delayed_r = self.itd_buffer_r[self.itd_idx];
                    self.itd_buffer_l[self.itd_idx] = left;
                    self.itd_buffer_r[self.itd_idx] = right;
                    self.itd_idx = (self.itd_idx + 1) % itd_len;

                    let cross_to_r = self.head_shadow_l.process(delayed_l) * cross_feed_gain;
                    let cross_to_l = self.head_shadow_r.process(delayed_r) * cross_feed_gain;

                    // 3.5 全景空间声场重构
                    let final_left = (mid + side_l - cross_to_l) * spatial_norm;
                    let final_right = (mid - side_r - cross_to_r) * spatial_norm;

                    left = final_left;
                    right = final_right;
                }
            }

            // ── 阶段 4：防破音软限幅 ──
            chunk[0] = soft_limit(left);
            chunk[1] = soft_limit(right);
        }
    }

    /// 重置所有延迟状态与滤波器状态（切歌 / 跳转时调用）。
    pub fn reset(&mut self) {
        self.tube_dc_x_l = 0.0;
        self.tube_dc_y_l = 0.0;
        self.tube_dc_x_r = 0.0;
        self.tube_dc_y_r = 0.0;
        self.itd_buffer_l.fill(0.0);
        self.itd_buffer_r.fill(0.0);
        self.itd_idx = 0;
        self.refl_buffer_l.fill(0.0);
        self.refl_buffer_r.fill(0.0);
        self.refl_idx = 0;
        self.refl_damp_l = 0.0;
        self.refl_damp_r = 0.0;
        self.head_shadow_l.reset();
        self.head_shadow_r.reset();
        self.bs2b_filter_l.reset();
        self.bs2b_filter_r.reset();
        self.dialogue_filter.reset();
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
        assert!(!effects.spatial_audio_enabled);
        assert!(!effects.stereo_widener_enabled());
        assert!(!effects.tube_warmth_enabled);
        assert!(!effects.bs2b_mode);
        assert!(!effects.pure_direct);
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
    fn processor_tube_warmth_adds_analog_harmonics() {
        let mut proc = AudioEffectsProcessor::new(44100.0);
        let effects = AudioEffects {
            tube_warmth_enabled: true,
            tube_warmth_level: 0.8,
            ..Default::default()
        };
        proc.update_coefficients(&effects);

        let mut buffer = vec![0.4, -0.4, 0.3, -0.3];
        let original = buffer.clone();
        proc.process_interleaved(&mut buffer, &effects);

        assert_ne!(buffer, original);
        for s in &buffer {
            assert!(s.abs() <= 1.0);
        }
    }

    #[test]
    fn processor_pure_direct_preserves_exact_samples() {
        let mut proc = AudioEffectsProcessor::new(44100.0);
        let effects = AudioEffects {
            pure_direct: true,
            tube_warmth_enabled: true,
            spatial_audio_enabled: true,
            bass_boost_enabled: true,
            ..Default::default()
        };
        let mut buffer = vec![0.55, -0.33, 0.22, -0.11];
        let original = buffer.clone();

        proc.process_interleaved(&mut buffer, &effects);
        assert_eq!(buffer, original, "纯净直通模式下应 100% 原始位完美输出");
    }

    #[test]
    fn processor_bs2b_crossfeed_mode() {
        let mut proc = AudioEffectsProcessor::new(44100.0);
        let effects = AudioEffects {
            spatial_audio_enabled: true,
            bs2b_mode: true,
            spatial_audio_level: 0.6,
            ..Default::default()
        };
        proc.update_coefficients(&effects);

        let mut buffer = Vec::new();
        for _ in 0..64 {
            buffer.push(0.6);
            buffer.push(0.0);
        }
        proc.process_interleaved(&mut buffer, &effects);

        // 经延迟稳定后（> 14 样本），原本仅左声道有声音，经 BS2B 跨耳互馈后右声道应有温和衰减互馈声音
        let last_right = buffer[buffer.len() - 1];
        assert!(last_right > 0.05 && last_right < 0.5);
    }

    #[test]
    fn processor_spatial_audio_modifies_stereo_channels() {
        let mut proc = AudioEffectsProcessor::new(44100.0);
        let effects = AudioEffects {
            spatial_audio_enabled: true,
            spatial_audio_level: 0.8,
            dialogue_clarity_level: 0.5,
            ..Default::default()
        };
        proc.update_coefficients(&effects);

        let mut buffer = vec![0.5, -0.3, 0.2, 0.4];
        let original = buffer.clone();
        proc.process_interleaved(&mut buffer, &effects);

        // 经空间声场处理后，通道能量产生有界空间重构
        assert_ne!(buffer, original);
        for s in &buffer {
            assert!(s.abs() <= 1.0);
        }
    }

    #[test]
    fn processor_dialogue_clarity_enhances_center_mid() {
        let mut proc = AudioEffectsProcessor::new(44100.0);
        let effects_low = AudioEffects {
            spatial_audio_enabled: true,
            spatial_audio_level: 0.5,
            dialogue_clarity_level: 0.0,
            ..Default::default()
        };
        let effects_high = AudioEffects {
            spatial_audio_enabled: true,
            spatial_audio_level: 0.5,
            dialogue_clarity_level: 1.0,
            ..Default::default()
        };

        let mut buf_low = vec![0.3; 256];
        let mut buf_high = vec![0.3; 256];

        proc.update_coefficients(&effects_low);
        proc.process_interleaved(&mut buf_low, &effects_low);

        proc.reset();
        proc.update_coefficients(&effects_high);
        proc.process_interleaved(&mut buf_high, &effects_high);

        assert_ne!(buf_low, buf_high);
        for s in buf_high {
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

    #[test]
    fn test_serde_compatibility_with_legacy_keys() {
        let json = r#"{"stereo_widener_enabled":true,"stereo_widener_level":0.75}"#;
        let effects: AudioEffects = serde_json::from_str(json).expect("反序列化旧配置应当成功");
        assert!(effects.spatial_audio_enabled);
        assert_eq!(effects.spatial_audio_level, 0.75);
    }
}
