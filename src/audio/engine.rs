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
use crate::audio::{AtomicF32, AudioEffects, EngineCommand, Equalizer};
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

    /// 实时替换 DSP 音效参数 (3D拓宽/低音/人声通透)。
    pub fn set_audio_effects(&self, effects: AudioEffects) {
        let _ = self.cmd_tx.send(EngineCommand::SetAudioEffects(effects));
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

        // 渐入渐出控制原子量：
        // fade_target: true = 期望播放状态, false = 期望暂停/停止状态
        // reset_fade: true = 立即重置渐变进度为 0.0 (用于换歌或 Seek 快速起播)
        // frames_played: 记录声卡实际已播放消费的帧数，精确同步进度与歌词
        let fade_target = Arc::new(AtomicBool::new(false));
        let reset_fade = Arc::new(AtomicBool::new(false));
        let frames_played = Arc::new(std::sync::atomic::AtomicU64::new(0));

        // 设备声道数与采样率在 open 之后才确定，用 Arc<原子量> 让回调延迟读取。
        let device_channels = std::sync::Arc::new(std::sync::atomic::AtomicU16::new(2));
        let device_sample_rate = std::sync::Arc::new(std::sync::atomic::AtomicU32::new(44_100));

        let ring_for_cb = ring.clone();
        let volume_for_cb = volume.clone();
        let muted_for_cb = muted.clone();
        let device_channels_for_cb = device_channels.clone();
        let device_sample_rate_cb = device_sample_rate.clone();
        let spectrum_tap_cb = spectrum_tap.clone();
        let fade_target_cb = fade_target.clone();
        let reset_fade_cb = reset_fade.clone();
        let frames_played_cb = frames_played.clone();

        // 回调闭包内部状态：
        // fade_progress: 0.0 = 完全正常播放 (满音量), 1.0 = 完全静音暂停
        let mut fade_progress: f32 = 1.0;

        let (stream, device_rate, ch) = output::open_default_stream(
            move |data: &mut [f32], _info| {
                if reset_fade_cb.swap(false, Ordering::Relaxed) {
                    fade_progress = 0.0;
                }

                let target_playing = fade_target_cb.load(Ordering::Relaxed);

                // 完全暂停静音状态：直接清零输出，不消耗环形缓冲，零锁开销且保留待播缓冲
                if !target_playing && fade_progress >= 1.0 {
                    data.fill(0.0);
                    return;
                }

                let channels = device_channels_for_cb.load(Ordering::Relaxed) as usize;
                let channels = channels.max(1);
                let frames = data.len() / channels;

                // 渐变时间常数：500ms 舒缓缓冲过渡（约 24,000 帧 @ 48kHz）
                const FADE_SECS: f32 = 0.5;
                let dev_rate = device_sample_rate_cb.load(Ordering::Relaxed).max(8_000) as f32;
                let fade_step = 1.0 / (dev_rate * FADE_SECS);

                let mut guard = ring_for_cb.lock().unwrap();
                let mut popped_frames: u64 = 0;

                for f in 0..frames {
                    // 更新渐变进度
                    if target_playing {
                        fade_progress = (fade_progress - fade_step).max(0.0);
                    } else {
                        fade_progress = (fade_progress + fade_step).min(1.0);
                    }

                    // 升余弦 (Raised Cosine) 平滑包络：两端一阶导数恒为 0，彻底杜绝爆音与突变
                    let fade_gain = raised_cosine_fade_gain(fade_progress);

                    let (l, r) = if fade_progress < 1.0 {
                        if guard.len() >= 2 {
                            popped_frames += 1;
                            (guard.pop_front().unwrap(), guard.pop_front().unwrap())
                        } else {
                            (0.0f32, 0.0f32)
                        }
                    } else {
                        (0.0f32, 0.0f32)
                    };

                    let l = l * fade_gain;
                    let r = r * fade_gain;

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

                if popped_frames > 0 {
                    frames_played_cb.fetch_add(popped_frames, Ordering::Relaxed);
                }

                let v = if muted_for_cb.load(Ordering::Relaxed) {
                    0.0
                } else {
                    volume_for_cb.load(Ordering::Relaxed)
                };
                for s in data.iter_mut() {
                    *s *= v;
                }

                // 频谱实时 Tap：直接捕获经 EQ、渐变包络与音量处理后实际送往声卡的 PCM，实现真正音画同步
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

        // 记录真实设备声道数与采样率，供回调与解码线程使用
        device_channels.store(ch, Ordering::Relaxed);
        device_sample_rate.store(device_rate, Ordering::Relaxed);

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
            let fade_target = fade_target.clone();
            let reset_fade = reset_fade.clone();
            let frames_played = frames_played.clone();
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
                    fade_target,
                    reset_fade,
                    frames_played,
                );
            }
        });

        // 独立 15 FPS 频谱分析线程：经典千千静听 15 FPS 挡位，双窗时间积分，杜绝抽搐狂跳
        let spectrum_thread = thread::spawn({
            let spectrum_tap = spectrum_tap.clone();
            let stop = stop.clone();
            let audio_tx = audio_tx.clone();
            move || {
                let mut analyzer = SpectrumAnalyzer::new(device_rate as f32, 1024);
                while !stop.load(Ordering::Relaxed) {
                    thread::sleep(Duration::from_millis(68)); // 经典 15 FPS (约 68ms) 舒缓律动刷新率
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
    fade_target: Arc<AtomicBool>,
    reset_fade: Arc<AtomicBool>,
    frames_played: Arc<std::sync::atomic::AtomicU64>,
) {
    let mut decoder: Option<SymphoniaDecoder> = None;
    let mut resampler: Option<RubatoResampler> = None;
    let mut src_rate: u32 = 44_100;
    let mut src_channels: u16 = 2;

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
                        src_frames_consumed = 0;
                        frames_played.store(0, Ordering::SeqCst);
                        reset_fade.store(true, Ordering::SeqCst);
                        fade_target.store(true, Ordering::SeqCst);
                        dsp.lock().unwrap().reset();
                        ring.lock().unwrap().clear();
                        if let Ok(mut tap) = spectrum_tap.try_lock() {
                            tap.clear();
                        }
                        playing.store(true, Ordering::Relaxed);
                        let is_hi_res = sr >= 48_000;
                        let _ = audio_tx.send(AudioEvent::Format(
                            crate::audio::events::AudioFormatInfo {
                                sample_rate: sr,
                                channels: ch,
                                device_rate,
                                is_hi_res,
                            },
                        ));
                        let _ = audio_tx.send(AudioEvent::Position(
                            Duration::ZERO,
                            dur.unwrap_or(Duration::ZERO),
                        ));
                    }
                    Err(e) => {
                        let _ = audio_tx.send(AudioEvent::Error(e.to_string()));
                    }
                },
                EngineCommand::Pause => {
                    fade_target.store(false, Ordering::SeqCst);
                    playing.store(false, Ordering::Relaxed);
                }
                EngineCommand::Resume => {
                    fade_target.store(true, Ordering::SeqCst);
                    playing.store(true, Ordering::Relaxed);
                }
                EngineCommand::Seek(d) => {
                    *seek_target.lock().unwrap() = Some(d);
                    reset_fade.store(true, Ordering::SeqCst);
                    ring.lock().unwrap().clear();
                    if let Ok(mut tap) = spectrum_tap.try_lock() {
                        tap.clear();
                    }
                }
                EngineCommand::SetEqualizer(eq) => {
                    let mut guard = dsp.lock().unwrap();
                    let current_effects = guard.effects;
                    let new_chain = DspChain::new(&eq, current_effects, device_rate as f32);
                    *guard = new_chain;
                }
                EngineCommand::SetAudioEffects(effects) => {
                    dsp.lock().unwrap().update_effects(effects);
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
            // 暂停期间若声卡正在进行 500ms 渐出缓冲，根据实际消费帧数汇报真实播放位置
            let actual_frames = frames_played.load(Ordering::Relaxed);
            let pos =
                base_position + Duration::from_secs_f64(actual_frames as f64 / device_rate as f64);
            if last_pos_send.elapsed() >= Duration::from_millis(250) {
                last_pos_send = Instant::now();
                if let Some(dec) = &decoder {
                    let _ = audio_tx.send(AudioEvent::Position(pos, dec.duration().unwrap_or(pos)));
                }
            }
            thread::sleep(Duration::from_millis(15));
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
            fade_target.store(false, Ordering::SeqCst);
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
                frames_played.store(0, Ordering::SeqCst);
                reset_fade.store(true, Ordering::SeqCst);
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
        // 7) 位置（基于声卡硬件真实已消费帧数，实现真正的音画与歌词毫秒级同步）
        let actual_frames = frames_played.load(Ordering::Relaxed);
        let pos =
            base_position + Duration::from_secs_f64(actual_frames as f64 / device_rate as f64);
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

/// 升余弦 (Raised Cosine / Hann) 渐入渐出增益计算：
/// - `progress = 0.0`：满音量输出 (`1.0`)
/// - `progress = 1.0`：完全静音 (`0.0`)
/// - 曲线在两端一阶导数恒为 0，彻底杜绝突变爆音 (click/pop)。
#[inline]
pub fn raised_cosine_fade_gain(progress: f32) -> f32 {
    let p = progress.clamp(0.0, 1.0);
    0.5 * (1.0 + (std::f32::consts::PI * p).cos())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_raised_cosine_fade_gain() {
        // 满音量端点
        assert!((raised_cosine_fade_gain(0.0) - 1.0).abs() < 1e-6);
        // 完全静音端点
        assert!(raised_cosine_fade_gain(1.0).abs() < 1e-6);
        // 中点半能量 0.5
        assert!((raised_cosine_fade_gain(0.5) - 0.5).abs() < 1e-6);

        // 越界值安全截断
        assert!((raised_cosine_fade_gain(-0.5) - 1.0).abs() < 1e-6);
        assert!(raised_cosine_fade_gain(1.5).abs() < 1e-6);

        // 单调递减特性验证
        let mut prev = 1.0;
        for i in 1..=100 {
            let p = i as f32 / 100.0;
            let g = raised_cosine_fade_gain(p);
            assert!(g <= prev, "进度增大时增益必须单调递减");
            prev = g;
        }
    }

    #[test]
    fn test_pcm_ring_basic_and_capacity() {
        let mut ring = PcmRing::new(4);
        assert_eq!(ring.len(), 0);

        ring.push_slice(&[1.0, 2.0]);
        assert_eq!(ring.len(), 2);
        assert_eq!(ring.pop_front(), Some(1.0));
        assert_eq!(ring.pop_front(), Some(2.0));
        assert_eq!(ring.pop_front(), None);

        // 超过容量时应丢弃最旧样本
        ring.push_slice(&[10.0, 20.0, 30.0]);
        ring.push_slice(&[40.0, 50.0]); // 总计 5 样本，容量 4，最旧的 10.0 应被挤出
        assert_eq!(ring.len(), 4);
        assert_eq!(ring.pop_front(), Some(20.0));
        assert_eq!(ring.pop_front(), Some(30.0));
        assert_eq!(ring.pop_front(), Some(40.0));
        assert_eq!(ring.pop_front(), Some(50.0));

        ring.push_slice(&[1.0, 2.0]);
        ring.clear();
        assert_eq!(ring.len(), 0);
    }

    #[test]
    fn test_upmix_to_stereo() {
        // 单声道上混为立体声复制
        let mono = vec![0.5, -0.5];
        let stereo = upmix_to_stereo(&mono, 1);
        assert_eq!(stereo, vec![0.5, 0.5, -0.5, -0.5]);

        // 立体声直通
        let orig_stereo = vec![0.1, 0.2, 0.3, 0.4];
        let pass = upmix_to_stereo(&orig_stereo, 2);
        assert_eq!(pass, orig_stereo);

        // 多声道下混截取前两声道
        let quad = vec![1.0, 2.0, 3.0, 4.0];
        let front = upmix_to_stereo(&quad, 4);
        assert_eq!(front, vec![1.0, 2.0]);
    }
}
