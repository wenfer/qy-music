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

use cpal::Stream;
use crossbeam_channel::{unbounded, Receiver, Sender};

use crate::audio::decoder::SymphoniaDecoder;
use crate::audio::dsp::DspChain;
use crate::audio::events::AudioEvent;
use crate::audio::output;
use crate::audio::resampler::RubatoResampler;
use crate::audio::{AtomicF32, EngineCommand, Equalizer};
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
        let excess = (self.buf.len() + data.len()).saturating_sub(self.cap);
        if excess > 0 {
            self.buf.drain(..excess.min(self.buf.len()));
        }
        self.buf.extend(data.iter().copied());
    }

    fn pop_front(&mut self) -> Option<f32> {
        self.buf.pop_front()
    }

    fn len(&self) -> usize {
        self.buf.len()
    }

    fn clear(&mut self) {
        self.buf.clear();
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
    _spectrum_thread: Option<JoinHandle<()>>,
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
        let spectrum_tap: Arc<Mutex<VecDeque<f32>>> =
            Arc::new(Mutex::new(VecDeque::with_capacity(4096)));
        let dsp: Arc<Mutex<DspChain>> = Arc::new(Mutex::new(DspChain::from_equalizer(
            &Equalizer::flat(),
            44_100.0,
        )));
        let playing = Arc::new(AtomicBool::new(false));
        let stop = Arc::new(AtomicBool::new(false));
        let seek_target: Arc<Mutex<Option<Duration>>> = Arc::new(Mutex::new(None));

        // 设备声道数在 open 之后才确定，用 Arc<原子量> 让回调延迟读取。
        let device_channels = std::sync::Arc::new(std::sync::atomic::AtomicU16::new(2));

        let ring_for_cb = ring.clone();
        let volume_for_cb = volume.clone();
        let muted_for_cb = muted.clone();
        let device_channels_for_cb = device_channels.clone();
        let spectrum_tap_cb = spectrum_tap.clone();

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

                // 频谱实时 Tap：直接捕获经 EQ 与音量处理后实际送往声卡的 PCM，实现真正音画同步
                if let Ok(mut tap) = spectrum_tap_cb.try_lock() {
                    let tap_len = tap.len();
                    let excess = (tap_len + frames).saturating_sub(2048);
                    if excess > 0 {
                        tap.drain(..excess.min(tap_len));
                    }
                    for f in 0..frames {
                        let mono =
                            (data[f * channels] + data[f * channels + (channels - 1).min(1)]) * 0.5;
                        tap.push_back(mono);
                    }
                }
            },
            |e| {
                let s = e.to_string();
                if s.contains("underrun") || s.contains("overrun") {
                    log::debug!("音频流缓冲瞬态状态: {e}");
                } else {
                    log::warn!("音频流错误: {e}");
                }
            },
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
            let device_channels = ch;
            let audio_tx = audio_tx.clone();
            let spectrum_tap = spectrum_tap.clone();
            move || {
                decode_loop(
                    cmd_rx,
                    audio_tx,
                    ring,
                    dsp,
                    volume,
                    muted,
                    playing,
                    stop,
                    seek_target,
                    device_rate,
                    device_channels,
                    spectrum_tap,
                );
            }
        });

        // 独立 15 FPS 频谱分析线程：经典千千静听 15 FPS 挡位，双窗时间积分，杜绝抽搐狂跳
        let spectrum_thread = thread::spawn({
            let spectrum_tap = spectrum_tap.clone();
            let playing = playing.clone();
            let stop = stop.clone();
            let audio_tx = audio_tx.clone();
            move || {
                let mut analyzer = SpectrumAnalyzer::new(device_rate as f32, 1024);
                while !stop.load(Ordering::Relaxed) {
                    thread::sleep(Duration::from_millis(68)); // 经典 15 FPS (约 68ms) 舒缓律动刷新率
                    if !playing.load(Ordering::Relaxed) {
                        continue;
                    }
                    let drained: Vec<f32> = {
                        if let Ok(mut tap) = spectrum_tap.try_lock() {
                            tap.drain(..).collect()
                        } else {
                            Vec::new()
                        }
                    };
                    if drained.is_empty() {
                        continue;
                    }
                    if drained.len() >= 1536 {
                        // 双窗重叠时间积分：平滑捕获 68ms 内的全部能量分布，彻底消除单窗盲区与突发抽搐
                        let mid = drained.len() / 2;
                        for &s in &drained[..mid] {
                            analyzer.push_mono(s);
                        }
                        let spec1 = analyzer.compute_spectrum(32, 16);
                        for &s in &drained[mid..] {
                            analyzer.push_mono(s);
                        }
                        let spec2 = analyzer.compute_spectrum(32, 16);
                        match (spec1, spec2) {
                            (Some(mut s1), Some(s2)) => {
                                for (b1, b2) in s1.bands.iter_mut().zip(s2.bands.iter()) {
                                    *b1 = (*b1 + *b2) * 0.5;
                                }
                                let _ = audio_tx.send(AudioEvent::Spectrum(s1));
                            }
                            (Some(s), None) | (None, Some(s)) => {
                                let _ = audio_tx.send(AudioEvent::Spectrum(s));
                            }
                            (None, None) => {}
                        }
                    } else {
                        for s in drained {
                            analyzer.push_mono(s);
                        }
                        if let Some(spec) = analyzer.compute_spectrum(32, 16) {
                            let _ = audio_tx.send(AudioEvent::Spectrum(spec));
                        }
                    }
                }
            }
        });

        output::play_stream(&stream);

        Ok(Self {
            _stream: stream,
            _decode_thread: Some(decode_thread),
            _spectrum_thread: Some(spectrum_thread),
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
        if let Some(h) = self._spectrum_thread.take() {
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
    spectrum_tap: Arc<Mutex<VecDeque<f32>>>,
) {
    let mut decoder: Option<SymphoniaDecoder> = None;
    let mut resampler: Option<RubatoResampler> = None;
    let mut src_rate: u32 = 44_100;
    let mut src_channels: u16 = 2;

    let mut produced_frames: u64 = 0;
    let mut base_position: Duration = Duration::ZERO;
    let mut src_frames_consumed: u64 = 0;
    let mut last_pos_send = Instant::now();

    loop {
        // 1) 消费命令
        while let Ok(cmd) = cmd_rx.try_recv() {
            match cmd {
                EngineCommand::Play(track) => match SymphoniaDecoder::open(&track.path) {
                    Ok((dec, sr, ch, dur)) => {
                        src_rate = sr;
                        src_channels = ch;
                        resampler = if sr != device_rate {
                            RubatoResampler::new(sr, device_rate, ch as usize).ok()
                        } else {
                            None
                        };
                        decoder = Some(dec);
                        base_position = Duration::ZERO;
                        produced_frames = 0;
                        src_frames_consumed = 0;
                        dsp.lock().unwrap().reset();
                        ring.lock().unwrap().clear();
                        if let Ok(mut tap) = spectrum_tap.try_lock() {
                            tap.clear();
                        }
                        playing.store(true, Ordering::Relaxed);
                        let _ = audio_tx.send(AudioEvent::Position(
                            Duration::ZERO,
                            dur.unwrap_or(Duration::ZERO),
                        ));
                    }
                    Err(e) => {
                        let _ = audio_tx.send(AudioEvent::Error(e.to_string()));
                    }
                },
                EngineCommand::Pause => playing.store(false, Ordering::Relaxed),
                EngineCommand::Resume => playing.store(true, Ordering::Relaxed),
                EngineCommand::Seek(d) => {
                    *seek_target.lock().unwrap() = Some(d);
                    ring.lock().unwrap().clear();
                    if let Ok(mut tap) = spectrum_tap.try_lock() {
                        tap.clear();
                    }
                }
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

        // 背压控制：当环形缓冲区中已有充足待播放样本（例如 > 1 秒音频）时，
        // 解码线程主动短暂休眠，等待声卡硬件回调消费，避免全速解码造成事件风暴与缓冲溢出。
        let ring_len = ring.lock().unwrap().len();
        if ring_len >= 88_200 {
            thread::sleep(Duration::from_millis(15));
            continue;
        }

        let Some(dec) = decoder.as_mut() else {
            thread::sleep(Duration::from_millis(20));
            continue;
        };

        let Some(packet) = dec.next_packet() else {
            // 曲目结束时刷新重采样器内残留采样
            if let Some(mut r) = resampler.take() {
                if let Ok(flushed) = r.flush() {
                    if !flushed.is_empty() {
                        let stereo = upmix_to_stereo(&flushed, src_channels);
                        let processed = dsp.lock().unwrap().process(&stereo);
                        ring.lock().unwrap().push_slice(&processed);
                    }
                }
            }
            let _ = audio_tx.send(AudioEvent::Ended);
            playing.store(false, Ordering::Relaxed);
            decoder = None;
            resampler = None;
            thread::sleep(Duration::from_millis(20));
            continue;
        };

        let packet_frames = (packet.len() / src_channels as usize) as u64;

        // 2) seek：丢弃直到到达目标时间
        let seeking = *seek_target.lock().unwrap();
        if let Some(target) = seeking {
            let cur_time = Duration::from_secs_f64(src_frames_consumed as f64 / src_rate as f64);
            if cur_time < target {
                src_frames_consumed += packet_frames;
                continue;
            } else {
                *seek_target.lock().unwrap() = None;
                base_position = target;
                produced_frames = 0;
                dsp.lock().unwrap().reset();
                if let Some(r) = resampler.as_mut() {
                    let _ = r.flush();
                }
                ring.lock().unwrap().clear();
                if let Ok(mut tap) = spectrum_tap.try_lock() {
                    tap.clear();
                }
            }
        }
        src_frames_consumed += packet_frames;

        // 3) 重采样（源声道）
        let resampled = match &mut resampler {
            Some(r) => match r.process(&packet) {
                Ok(v) => v,
                Err(e) => {
                    log::warn!("重采样异常: {e}");
                    continue;
                }
            },
            None => packet.clone(),
        };

        if resampled.is_empty() {
            continue;
        }

        // 4) 上混到立体声（DSP 假设 2 声道）
        let stereo = upmix_to_stereo(&resampled, src_channels);
        // 5) DSP（EQ + 主增益）
        let processed = dsp.lock().unwrap().process(&stereo);
        // 6) 推入环形缓冲
        ring.lock().unwrap().push_slice(&processed);
        // 7) 位置
        produced_frames += (processed.len() / 2) as u64;
        let pos =
            base_position + Duration::from_secs_f64(produced_frames as f64 / device_rate as f64);
        if last_pos_send.elapsed() >= Duration::from_millis(250) {
            last_pos_send = Instant::now();
            let _ = audio_tx.send(AudioEvent::Position(pos, dec.duration().unwrap_or(pos)));
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
