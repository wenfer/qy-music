//! 重采样封装：源采样率 → 设备首选采样率（rubato `FastFixedIn`）。
//!
//! 输入 / 输出均为交错 `f32`。rubato 要求「每声道一个 `Vec`」的分离布局，
//! 这里在 `process` 内完成去交织 / 重交织。

use rubato::{FastFixedIn, PolynomialDegree, Resampler};

use crate::error::{LingfengError, Result};

/// 重采样器（源率 → 设备率）。
pub struct RubatoResampler {
    resampler: FastFixedIn<f32>,
    channels: usize,
}

impl RubatoResampler {
    /// 构造：源率 `src_rate`、目标率 `dst_rate`、声道数 `channels`。
    pub fn new(src_rate: u32, dst_rate: u32, channels: usize) -> Result<Self> {
        let resample_ratio = dst_rate as f64 / src_rate as f64;
        // FastFixedIn：多项式插值快速重采样（原型可接受轻微高频伪影）。
        let resampler = FastFixedIn::<f32>::new(
            resample_ratio,
            10.0, // max_resample_ratio_relative（允许 ±10 倍，覆盖变速需求）
            PolynomialDegree::Linear,
            1024, // chunk size（帧）
            channels,
        )
        .map_err(|e| LingfengError::other(format!("rubato 初始化失败: {e}")))?;
        Ok(Self { resampler, channels })
    }

    /// 重采样一帧交错 PCM，返回设备率下的交错 PCM。
    pub fn process(&mut self, input: &[f32]) -> Result<Vec<f32>> {
        let channels = self.channels.max(1);
        if input.is_empty() {
            return Ok(Vec::new());
        }
        let frames = input.len() / channels;
        if frames == 0 {
            return Ok(Vec::new());
        }
        // 去交织为每声道一个 Vec
        let mut waves: Vec<Vec<f32>> = vec![Vec::with_capacity(frames); channels];
        for f in 0..frames {
            for c in 0..channels {
                waves[c].push(input[f * channels + c]);
            }
        }
        let out = self
            .resampler
            .process(&waves, None)
            .map_err(|e| LingfengError::other(format!("重采样失败: {e}")))?;

        let out_frames = out.first().map(|v| v.len()).unwrap_or(0);
        let mut result = Vec::with_capacity(out_frames * channels);
        for f in 0..out_frames {
            for c in 0..channels {
                result.push(out[c][f]);
            }
        }
        Ok(result)
    }
}
