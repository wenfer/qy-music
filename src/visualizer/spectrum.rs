//! 频谱数据：频段能量对数分块 + 归一化 + LED 段映射。

/// 频谱数据：已归一化（0..=1）的频段能量数组。
#[derive(Clone, Debug, PartialEq)]
pub struct SpectrumData {
    /// 各频段归一化能量（0..=1）。
    pub bands: Vec<f32>,
    /// 频段数（默认 32）。
    pub band_count: usize,
    /// 每段 LED 数（默认 16）。
    pub led_segments: usize,
}

impl Default for SpectrumData {
    fn default() -> Self {
        Self::silent(32, 16)
    }
}

impl SpectrumData {
    /// 构造全零频谱（暂停 / 静音时归零用）。
    pub fn silent(band_count: usize, led_segments: usize) -> Self {
        Self {
            bands: vec![0.0; band_count],
            band_count,
            led_segments,
        }
    }

    /// 由 FFT 前半幅度谱（长度 = fft_size/2）按对数频率分块映射为 `band_count` 段。
    ///
    /// - 频率范围：`[20Hz, nyquist]`，对数等分。
    /// - 每段取所覆盖 bin 的平均幅度，转 dB 后线性归一化到 0..=1。
    pub fn from_magnitudes(
        mags: &[f32],
        sample_rate: f32,
        band_count: usize,
        led_segments: usize,
    ) -> Self {
        let bins = mags.len().max(1);
        let fft_size = bins * 2;
        let nyquist = sample_rate / 2.0;
        let fmin = 20.0_f32;
        let fmax = nyquist.max(fmin * 2.0);

        let mut bands = Vec::with_capacity(band_count);
        for b in 0..band_count {
            let f0 = fmin * (fmax / fmin).powf(b as f32 / band_count as f32);
            let f1 = fmin * (fmax / fmin).powf((b + 1) as f32 / band_count as f32);
            let i0 = ((f0 / sample_rate) * fft_size as f32).round() as usize;
            let i1 = ((f1 / sample_rate) * fft_size as f32).round() as usize;
            let i0 = i0.clamp(1, bins - 1);
            let i1 = i1.clamp(i0 + 1, bins);
            let sum: f32 = mags[i0..i1].iter().sum();
            let avg = sum / (i1 - i0) as f32;
            if avg <= 1e-6 {
                bands.push(0.0);
                continue;
            }
            // 归一化幅度（FFT 长度归一，N/4 为 Hann 窗满刻度单音基准）
            let ref_amp = (fft_size as f32 / 4.0).max(1.0);
            let norm_amp = (avg / ref_amp).max(1e-6);
            let raw_db = 20.0 * norm_amp.log10();
            // 粉红噪声频响倾斜补偿（真实音乐高频能量自然衰减，适度倾斜使高低频律动活跃均衡）
            let tilt_db = if band_count > 1 {
                (b as f32 / (band_count - 1) as f32) * 10.0
            } else {
                0.0
            };
            let db = raw_db + tilt_db;
            // 经典 -50dBFS ~ 0dBFS 动态视窗，每 3dB 对应约 1 个 LED 段，起伏灵动，绝非死死顶在满格
            let norm = ((db + 50.0) / 50.0).clamp(0.0, 1.0);
            bands.push(norm);
        }
        Self {
            bands,
            band_count,
            led_segments,
        }
    }

    /// 返回每频段点亮的 LED 段数（0..=led_segments）。
    pub fn to_led_levels(&self) -> Vec<u8> {
        let seg = self.led_segments as f32;
        self.bands
            .iter()
            .map(|b| (b.clamp(0.0, 1.0) * seg).round().clamp(0.0, seg) as u8)
            .collect()
    }

    /// 全部归零（暂停 / 静音时调用）。
    pub fn zero(&mut self) {
        self.bands.fill(0.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_magnitudes_length_and_range() {
        let mags = vec![0.5; 512]; // 模拟幅度谱
        let sd = SpectrumData::from_magnitudes(&mags, 44100.0, 32, 16);
        assert_eq!(sd.bands.len(), 32);
        assert_eq!(sd.band_count, 32);
        for &v in &sd.bands {
            assert!((0.0..=1.0).contains(&v), "归一化能量应在 0..1，实际 {v}");
        }
    }

    #[test]
    fn to_led_levels_bounded() {
        let mags = vec![10.0; 512]; // 大信号 → 接近满
        let sd = SpectrumData::from_magnitudes(&mags, 44100.0, 32, 16);
        let levels = sd.to_led_levels();
        assert_eq!(levels.len(), 32);
        for &l in &levels {
            assert!(l <= 16);
        }
        // 强信号应点亮较多段
        assert!(levels.iter().all(|&l| l >= 1));
    }

    #[test]
    fn silent_has_zero_bands() {
        let sd = SpectrumData::silent(32, 16);
        assert!(sd.bands.iter().all(|&b| b == 0.0));
        assert!(sd.to_led_levels().iter().all(|&l| l == 0));
    }

    // ──────────────────────────────────────────────────────────────
    // QA 补充测试
    // ──────────────────────────────────────────────────────────────

    #[test]
    fn default_is_silent_32_bands_16_segments() {
        let sd = SpectrumData::default();
        assert_eq!(sd.band_count, 32);
        assert_eq!(sd.led_segments, 16);
        assert_eq!(sd.bands.len(), 32);
        assert!(sd.bands.iter().all(|&b| b == 0.0));
    }

    #[test]
    fn silent_magnitudes_yield_zero_energy() {
        let mags = vec![0.0f32; 512];
        let sd = SpectrumData::from_magnitudes(&mags, 44_100.0, 32, 16);
        assert!(sd.bands.iter().all(|&b| b == 0.0), "全零幅度谱应得零能量");
        assert!(sd.to_led_levels().iter().all(|&l| l == 0));
    }

    #[test]
    fn zero_clears_previously_nonzero_bands() {
        let mut sd = SpectrumData::from_magnitudes(&vec![1.0f32; 512], 44_100.0, 32, 16);
        assert!(sd.bands.iter().any(|&b| b > 0.0));
        sd.zero();
        assert!(sd.bands.iter().all(|&b| b == 0.0));
        assert!(sd.to_led_levels().iter().all(|&l| l == 0));
    }

    #[test]
    fn to_led_levels_respects_custom_segment_count() {
        let mut sd = SpectrumData::silent(4, 8);
        sd.bands = vec![1.0; 4];
        assert!(sd.to_led_levels().iter().all(|&l| l == 8));
        // 归一化值超出 [0,1] 也应被钳制
        sd.bands = vec![2.0; 4];
        assert!(sd.to_led_levels().iter().all(|&l| l == 8));
        sd.bands = vec![-1.0; 4];
        assert!(sd.to_led_levels().iter().all(|&l| l == 0));
    }

    #[test]
    fn higher_magnitude_does_not_lower_band_energy() {
        // 单调性：整体幅度更大 → 每个频段能量应不减。
        let low = SpectrumData::from_magnitudes(&vec![0.001f32; 512], 44_100.0, 32, 16);
        let high = SpectrumData::from_magnitudes(&vec![0.5f32; 512], 44_100.0, 32, 16);
        for (l, h) in low.bands.iter().zip(high.bands.iter()) {
            assert!(h >= l, "能量应随幅度单调不减：{l} -> {h}");
        }
    }

    #[test]
    fn zero_band_count_yields_empty_bands() {
        let sd = SpectrumData::from_magnitudes(&vec![0.5f32; 512], 44_100.0, 0, 16);
        assert!(sd.bands.is_empty());
        assert!(sd.to_led_levels().is_empty());
    }

    #[test]
    fn from_magnitudes_honors_requested_band_count() {
        for n in [8usize, 16, 32, 64] {
            let sd = SpectrumData::from_magnitudes(&vec![0.3f32; 512], 44_100.0, n, 16);
            assert_eq!(sd.bands.len(), n, "频段映射长度应等于请求值 {n}");
            assert_eq!(sd.band_count, n);
        }
    }
}
