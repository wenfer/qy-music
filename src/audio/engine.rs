//! 播放引擎：持有 cpal 输出流 + 后台解码线程，串联
//! 解码 → 重采样 → Biquad EQ → 频谱 tap → 输出。
//!
//! 设计（见架构文档 §3 链路 A）：
//! - 解码在独立线程进行，产物推入 PCM 环形缓冲（立体声 `f32` 交错）。
//! - cpal `data_callback` 以拉模式从环形缓冲取 PCM 写入声卡，并应用音量 / 静音。
//! - 解码线程把已处理 PCM 喂给频谱分析器，周期性回传 [`AudioEvent::Spectrum`] /
//!   [`AudioEvent::Position`] / [`AudioEvent::Ended`]。
//! - 音频线程绝不直接修改 UI 状态，只经 `crossbeam` 通道回传事件。
//!
//! [`PlaybackEngineHandle`] 是 UI 持有的轻量句柄（命令发送端 + 音量 / 静音原子量）。

use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use crossbeam_channel::{Receiver, Sender, unbounded};
use cpal::Stream;

use crate::audio::decoder::SymphoniaDecoder;
use crate::audio::dsp::DspChain;
use crate::audio::{AtomicF32, Equalizer, EngineCommand};
use crate::audio::events::AudioEvent;
use crate::audio::output;
use crate::audio::resampler::RubatoResampler;
use crate::error::Result;
use crate::playlist::Track;
use crate::visualizer::fft::SpectrumAnalyzer;

/// PCM 环形缓冲（交错的立体声 `f32`）。
///
/// 超出容量时丢弃最旧样本（避免阻塞解码线程；代价是极小概率爆音，原型可接受）。
struct PcmRing {
    buf: VecDeque<f32>,
    cap: usize,
}

impl PcmRing {
    fn new(cap: usize) -> Self {
        Self {
            buf: VecDeque::with_capacity(cap),
            cap,
        }
    }

    fn push_slice(&mut self, data: &[f32]) {
        for &s in data {
            if self.buf.len() >= self.cap {
                self.buf.pop_front();
            }
            self.buf.push_back(s);
        }
    }

    fn pop_front(&mut self) -> Option<f32> {
        self.buf.pop_front()
    }

    fn len(&self) -> usize {
        self.buf.len()
    }
}

/// UI 持有的引擎句柄：命令发送端 + 共享音量 / 静音原子量。
#[derive(Clone)]
pub struct PlaybackEngineHandle {
    cmd_tx: Sender<EngineCommand>,
    volume: Arc<AtomicF32>,
    muted: Arc<AtomicBool>,
}

impl PlaybackEngineHandle {
    /// 构造句柄与对应的命令接收端（交给 [`PlaybackEngine::start`]）。
    pub fn new() -> (Self, Receiver<EngineCommand>) {
        let (cmd_tx, cmd_rx) = unbounded();
        let volume = Arc::new(AtomicF32::new(1.0));
        let muted = Arc::new(AtomicBool::new(false));
        (
            Self {
                cmd_tx,
                volume,
                muted,
            },
            cmd_rx,
        )
    }

    /// 播放指定曲目。
    pub fn play(&self, track: Track) {
        let _ = self.cmd_tx.send(EngineCommand::Play(track));
    }

    /// 暂停。
    pub fn pause(&self) {
        let _ = self.cmd_tx.send(EngineCommand::Pause);
    }

    /// 恢复。
    pub fn resume(&self) {
        let _ = self.cmd_tx.send(EngineCommand::Resume);
    }

    /// 跳转到指定位置。
    pub fn seek(&self, d: Duration) {
        let _ = self.cmd_tx.send(EngineCommand::Seek(d));
    }

    /// 实时替换均衡器参数。
    pub fn set_equalizer(&self, eq: Equalizer) {
        let _ = self.cmd_tx.send(EngineCommand::SetEqualizer(eq));
    }

    /// 设置音量（0.0..=1.0），同步更新共享原子量（回调即时生效）。
    pub fn set_volume(&self, v: f32) {
        self.volume.store(v, Ordering::Relaxed);
        let _ = self.cmd_tx.send(EngineCommand::SetVolume(v));
    }

    /// 设置静音。
    pub fn set_muted(&self, m: bool) {
        self.muted.store(m, Ordering::Relaxed);
        let _ = self.cmd_tx.send(EngineCommand::SetMuted(m));
    }

    /// 当前音量（读取原子量）。
    pub fn volume(&self) -> f32 {
        self.volume.load(Ordering::Relaxed)
    }

    /// 是否静音。
    pub fn muted(&self) -> bool {
        self.muted.load(Ordering::Relaxed)
    }

    /// 共享音量原子量（供输出回调使用）。
    pub fn volume_arc(&self) -> Arc<AtomicF32> {
        self.volume.clone()
    }

    /// 共享静音原子量（供输出回调使用）。
    pub fn muted_arc(&self) -> Arc<AtomicBool> {
        self.muted.clone()
    }
}

impl Default for PlaybackEngineHandle {
    fn default() -> Self {
        let (h, _rx) = Self::new();
        h
    }
}

/// 播放引擎运行时（持有 cpal 流与解码线程）。
pub struct PlaybackEngine {
    _stream: Stream,
    _decode_thread: Option<JoinHandle<()>>,
    stop: Arc<AtomicBool>,
}

impl PlaybackEngine {
    /// 启动引擎：构建 cpal 流并派生解码线程。
    ///
    /// - `cmd_rx`:      命令接收端（来自 [`PlaybackEngineHandle::new`]）
    /// - `audio_tx`:     音频事件发送端（来自全局通道）
    /// - `initial_eq`:   初始均衡器（用于构建 DSP 链）
    /// - `volume`/`muted`: 共享原子量（与句柄同源，回调即时读取）
    pub fn start(
        cmd_rx: Receiver<EngineCommand>,
        audio_tx: Sender<AudioEvent>,
        initial_eq: Equalizer,
        volume: Arc<AtomicF32>,
        muted: Arc<AtomicBool>,
    ) -> Result<Self> {
        let ring: Arc<Mutex<PcmRing>> = Arc::new(Mutex::new(PcmRing::new(176_400))); // ~2s @44.1k 立体声
        let dsp: Arc<Mutex<DspChain>> =
            Arc::new(Mutex::new(DspChain::from_equalizer(&Equalizer::flat(), 44_100.0)));
        let playing = Arc::new(AtomicBool::new(false));
        let stop = Arc::new(AtomicBool::new(false));
        let seek_target: Arc<Mutex<Option<Duration>>> = Arc::new(Mutex::new(None));

        // 设备声道数在 open 之后才确定，用 Arc<原子量> 让回调延迟读取。
        let device_channels = std::sync::Arc::new(std::sync::atomic::AtomicU16::new(2));

        let ring_for_cb = ring.clone();
        let volume_for_cb = volume.clone();
        let muted_for_cb = muted.clone();
        let device_channels_for_cb = device_channels.clone();

        let (stream, device_rate, ch) = output::open_default_stream(
            move |data: &mut [f32], _info| {
                let channels = device_channels_for_cb.load(Ordering::Relaxed) as usize;
                let channels = channels.max(1);
                let frames = data.len() / channels;
                let mut guard = ring_for_cb.lock().unwrap();
                for f in 0..frames {
                    let (l, r) = if guard.len() >= 2 {
                        (guard.pop_front().unwrap(), guard.pop_front().unwrap())
                    } else {
                        (0.0f32, 0.0f32)
                    };
                    for c in 0..channels {
                        let s = match channels {
                            1 => (l + r) * 0.5,
                            2 => {
                                if c == 0 {
                                    l
                                } else {
                                    r
                                }
                            }
                            _ => {
                                if c % 2 == 0 {
                                    l
                                } else {
                                    r
                                }
                            }
                        };
                        data[f * channels + c] = s;
                    }
                }
                drop(guard);
                let v = if muted_for_cb.load(Ordering::Relaxed) {
                    0.0
                } else {
                    volume_for_cb.load(Ordering::Relaxed)
                };
                for s in data.iter_mut() {
                    *s *= v;
                }
            },
            |e| log::warn!("音频流错误: {e}"),
        )?;

        // 记录真实设备声道数，供回调与解码线程使用
        device_channels.store(ch, Ordering::Relaxed);

        // 用真实设备率重建 DSP 链
        {
            let mut d = dsp.lock().unwrap();
            *d = DspChain::from_equalizer(&initial_eq, device_rate as f32);
        }

        let decode_thread = thread::spawn({
            let ring = ring.clone();
            let dsp = dsp.clone();
            let playing = playing.clone();
            let stop = stop.clone();
            let seek_target = seek_target.clone();
            let device_rate = device_rate;
            let device_channels = ch;
            let audio_tx = audio_tx.clone();
            move || {
                decode_loop(
                    cmd_rx, audio_tx, ring, dsp, volume, muted, playing, stop, seek_target,
                    device_rate, device_channels,
                );
            }
        });

        output::play_stream(&stream);

        Ok(Self {
            _stream: stream,
            _decode_thread: Some(decode_thread),
            stop,
        })
    }
}

impl Drop for PlaybackEngine {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(h) = self._decode_thread.take() {
            let _ = h.join();
        }
    }
}

/// 后台解码线程主循环。
#[allow(clippy::too_many_arguments)]
fn decode_loop(
    cmd_rx: Receiver<EngineCommand>,
    audio_tx: Sender<AudioEvent>,
    ring: Arc<Mutex<PcmRing>>,
    dsp: Arc<Mutex<DspChain>>,
    volume: Arc<AtomicF32>,
    muted: Arc<AtomicBool>,
    playing: Arc<AtomicBool>,
    stop: Arc<AtomicBool>,
    seek_target: Arc<Mutex<Option<Duration>>>,
    device_rate: u32,
    _device_channels: u16,
) {
    let mut decoder: Option<SymphoniaDecoder> = None;
    let mut resampler: Option<RubatoResampler> = None;
    let mut src_rate: u32 = 44_100;
    let mut src_channels: u16 = 2;

    let mut analyzer = SpectrumAnalyzer::new(device_rate as f32, 1024);
    let mut produced_frames: u64 = 0;
    let mut base_position: Duration = Duration::ZERO;
    let mut src_frames_consumed: u64 = 0;
    let mut last_pos_send = Instant::now();
    let mut frames_since_spectrum: u64 = 0;
    let spectrum_interval = ((device_rate as f64 / 60.0).max(1.0) as u64).max(1);

    loop {
        // 1) 消费命令
        while let Ok(cmd) = cmd_rx.try_recv() {
            match cmd {
                EngineCommand::Play(track) => {
                    match SymphoniaDecoder::open(&track.path) {
                        Ok((dec, sr, ch, dur)) => {
                            src_rate = sr;
                            src_channels = ch;
                            resampler = RubatoResampler::new(sr, device_rate, ch as usize).ok();
                            decoder = Some(dec);
                            base_position = Duration::ZERO;
                            produced_frames = 0;
                            src_frames_consumed = 0;
                            dsp.lock().unwrap().reset();
                            analyzer = SpectrumAnalyzer::new(device_rate as f32, 1024);
                            playing.store(true, Ordering::Relaxed);
                            let _ = audio_tx.send(AudioEvent::Position(
                                Duration::ZERO,
                                dur.unwrap_or(Duration::ZERO),
                            ));
                        }
                        Err(e) => {
                            let _ = audio_tx.send(AudioEvent::Error(e.to_string()));
                        }
                    }
                }
                EngineCommand::Pause => playing.store(false, Ordering::Relaxed),
                EngineCommand::Resume => playing.store(true, Ordering::Relaxed),
                EngineCommand::Seek(d) => *seek_target.lock().unwrap() = Some(d),
                EngineCommand::SetEqualizer(eq) => {
                    let new_chain = DspChain::from_equalizer(&eq, device_rate as f32);
                    *dsp.lock().unwrap() = new_chain;
                }
                EngineCommand::SetVolume(v) => {
                    volume.store(v, Ordering::Relaxed);
                }
                EngineCommand::SetMuted(m) => {
                    muted.store(m, Ordering::Relaxed);
                }
            }
        }

        if stop.load(Ordering::SeqCst) {
            break;
        }

        if !playing.load(Ordering::Relaxed) {
            thread::sleep(Duration::from_millis(10));
            continue;
        }

        let Some(dec) = decoder.as_mut() else {
            thread::sleep(Duration::from_millis(20));
            continue;
        };

        let Some(packet) = dec.next_packet() else {
            let _ = audio_tx.send(AudioEvent::Ended);
            playing.store(false, Ordering::Relaxed);
            decoder = None;
            resampler = None;
            thread::sleep(Duration::from_millis(20));
            continue;
        };

        // 2) seek：丢弃直到到达目标时间
        let seeking = *seek_target.lock().unwrap();
        if let Some(target) = seeking {
            let cur_time =
                Duration::from_secs_f64(src_frames_consumed as f64 / src_rate as f64);
            if cur_time < target {
                src_frames_consumed += (packet.len() / src_channels as usize) as u64;
                continue;
            } else {
                *seek_target.lock().unwrap() = None;
                base_position = target;
                produced_frames = 0;
                dsp.lock().unwrap().reset();
                analyzer = SpectrumAnalyzer::new(device_rate as f32, 1024);
            }
        }

        // 3) 重采样（源声道）
        let resampled = match &mut resampler {
            Some(r) => match r.process(&packet) {
                Ok(v) => v,
                Err(_) => continue,
            },
            None => packet.clone(),
        };
        // 4) 上混到立体声（DSP 假设 2 声道）
        let stereo = upmix_to_stereo(&resampled, src_channels);
        // 5) DSP（EQ + 主增益）
        let processed = dsp.lock().unwrap().process(&stereo);
        // 6) 推入环形缓冲
        ring.lock().unwrap().push_slice(&processed);
        // 7) 频谱 tap
        analyzer.push_frame(&processed, 2);
        frames_since_spectrum += (processed.len() / 2) as u64;
        if frames_since_spectrum >= spectrum_interval {
            frames_since_spectrum = 0;
            if let Some(spec) = analyzer.compute_spectrum(32, 16) {
                let _ = audio_tx.send(AudioEvent::Spectrum(spec));
            }
        }
        // 8) 位置
        produced_frames += (processed.len() / 2) as u64;
        let pos = base_position
            + Duration::from_secs_f64(produced_frames as f64 / device_rate as f64);
        src_frames_consumed += (packet.len() / src_channels as usize) as u64;
        if last_pos_send.elapsed() >= Duration::from_millis(250) {
            last_pos_send = Instant::now();
            let _ = audio_tx
                .send(AudioEvent::Position(pos, dec.duration().unwrap_or(pos)));
        }
    }
}

/// 把任意声道数的交错 PCM 上混 / 下混到立体声（2 声道）。
fn upmix_to_stereo(frame: &[f32], src_channels: u16) -> Vec<f32> {
    let ch = src_channels as usize;
    if ch == 2 || ch == 0 {
        return frame.to_vec();
    }
    if ch == 1 {
        let mut out = Vec::with_capacity(frame.len() * 2);
        for &s in frame {
            out.push(s);
            out.push(s);
        }
        return out;
    }
    // >2 声道：取前两声道
    let mut out = Vec::with_capacity((frame.len() / ch) * 2);
    for chunk in frame.chunks(ch) {
        out.push(chunk[0]);
        out.push(chunk.get(1).copied().unwrap_or(chunk[0]));
    }
    out
}
