//! 重采样封装：源采样率 → 设备首选采样率（rubato 5.0 `Async`）。
//!
//! 输入 / 输出均为交错 `f32`。通过 rubato 的 `audioadapter_buffers`
//! 完成高效零拷贝的交错处理。

use rubato::audioadapter_buffers::direct::InterleavedSlice;
use rubato::{Async, FixedAsync, PolynomialDegree, Resampler};

use crate::error::{LingfengError, Result};

/// 重采样器（源率 → 设备率）。
pub struct RubatoResampler {
    resampler: Async<f32>,
    channels: usize,
    input_buffer: Vec<f32>,
}

impl RubatoResampler {
    /// 构造：源率 `src_rate`、目标率 `dst_rate`、声道数 `channels`。
    pub fn new(src_rate: u32, dst_rate: u32, channels: usize) -> Result<Self> {
        let resample_ratio = dst_rate as f64 / src_rate as f64;
        let resampler = Async::<f32>::new_poly(
            resample_ratio,
            10.0,
            PolynomialDegree::Cubic,
            1024,
            channels,
            FixedAsync::Input,
        )
        .map_err(|e| LingfengError::other(format!("rubato 初始化失败: {e}")))?;
        Ok(Self {
            resampler,
            channels,
            input_buffer: Vec::with_capacity(4096 * channels),
        })
    }

    /// 重采样一帧交错 PCM，返回设备率下的交错 PCM。
    ///
    /// 内部维护待重采样缓存，完美支持解码器吐出任意大小的数据包（如 MP3 的 1152 帧，
    /// FLAC 的 4096 帧等），杜绝任何样本截断与时基断裂。
    pub fn process(&mut self, input: &[f32]) -> Result<Vec<f32>> {
        let channels = self.channels.max(1);
        if !input.is_empty() {
            self.input_buffer.extend_from_slice(input);
        }

        let mut output = Vec::new();
        loop {
            let needed_frames = self.resampler.input_frames_next();
            let needed_samples = needed_frames * channels;
            if self.input_buffer.len() < needed_samples {
                break;
            }

            let input_adapter = InterleavedSlice::new(
                &self.input_buffer[..needed_samples],
                channels,
                needed_frames,
            )
            .map_err(|e| LingfengError::other(format!("构建输入适配器失败: {e:?}")))?;

            let out = self
                .resampler
                .process(&input_adapter, None)
                .map_err(|e| LingfengError::other(format!("重采样失败: {e}")))?;

            output.extend(out.take_data());
            self.input_buffer.drain(..needed_samples);
        }

        Ok(output)
    }

    /// 刷新重采样器内残留的样本（在音轨结束时调用）。
    pub fn flush(&mut self) -> Result<Vec<f32>> {
        let channels = self.channels.max(1);
        let remaining_frames = self.input_buffer.len() / channels;
        if remaining_frames == 0 {
            self.input_buffer.clear();
            return Ok(Vec::new());
        }

        let needed_frames = self.resampler.input_frames_next();
        let needed_samples = needed_frames * channels;

        // 补齐零采样以满足 rubato 块要求
        if self.input_buffer.len() < needed_samples {
            self.input_buffer.resize(needed_samples, 0.0);
        }

        let input_adapter = InterleavedSlice::new(
            &self.input_buffer[..needed_samples],
            channels,
            needed_frames,
        )
        .map_err(|e| LingfengError::other(format!("构建刷新输入适配器失败: {e:?}")))?;

        let out = self
            .resampler
            .process(&input_adapter, None)
            .map_err(|e| LingfengError::other(format!("重采样刷新失败: {e}")))?;

        let mut data = out.take_data();
        let valid_out_frames =
            ((remaining_frames as f64) * self.resampler.resample_ratio()).round() as usize;
        data.truncate(valid_out_frames * channels);
        self.input_buffer.clear();
        Ok(data)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resampler_mp3_frame_size() {
        let mut resampler = RubatoResampler::new(44100, 48000, 2).unwrap();
        // MP3 frame is 1152 frames, stereo = 2304 samples
        let input = vec![0.1f32; 1152 * 2];
        let mut total_output_samples = 0;
        for _ in 0..10 {
            let result = resampler.process(&input).unwrap();
            total_output_samples += result.len();
        }
        let flushed = resampler.flush().unwrap();
        total_output_samples += flushed.len();

        let expected_frames = (1152.0f64 * 10.0 * 48000.0 / 44100.0).round() as usize;
        let actual_frames = total_output_samples / 2;
        println!("Expected frames: {expected_frames}, Actual frames: {actual_frames}");
        // Within filter delay margin (~4 frames for cubic interpolation)
        assert!((expected_frames as isize - actual_frames as isize).abs() <= 10);
    }

    #[test]
    fn test_resampler_various_sizes() {
        let mut resampler = RubatoResampler::new(44100, 48000, 2).unwrap();
        for size in [512, 1024, 1152, 2048, 4096, 576] {
            let input = vec![0.2f32; size * 2];
            let res = resampler.process(&input);
            assert!(res.is_ok(), "Failed on size {size}: {:?}", res.err());
        }
    }
}
