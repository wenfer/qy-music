//! 渐进式流媒体读取器（实现 Symphonia MediaSource）。
//!
//! 在后台异步下载的同时，向前提供标准的 `Read + Seek` 接口。
//! 当解码器追赶上网络下载时，通过条件变量优雅进入缓冲等待，绝不阻塞崩溃。

use std::fs::{File, OpenOptions};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::thread;
use std::time::Duration;

use symphonia::core::io::MediaSource;

use crate::cache::manager::CacheManager;
use crate::error::{LingfengError, Result};
use crate::webdav::client::WebDavClient;

/// 跨线程共享的流下载同步状态。
#[derive(Default)]
pub struct DownloadState {
    /// 当前已写入磁盘的字节数。
    pub downloaded_bytes: AtomicU64,
    /// 资源总大小（来自 HTTP Content-Length，未知时为 0）。
    pub total_bytes: AtomicU64,
    /// 下载任务是否已完整结束。
    pub is_finished: AtomicBool,
    /// 是否发生网络/I/O 错误。
    pub has_error: AtomicBool,
    /// 错误详细信息。
    pub error_msg: Mutex<Option<String>>,
    /// 用户是否已取消任务。
    pub cancelled: AtomicBool,
    /// 解码器当前是否因缓冲不足而处于挂起等待状态。
    pub is_buffering: AtomicBool,
    /// 唤醒条件变量锁。
    pub condvar: Condvar,
    pub lock: Mutex<()>,
}

impl DownloadState {
    /// 计算当前缓冲完成百分比（0.0 ~ 1.0）。
    pub fn buffer_ratio(&self) -> f32 {
        let total = self.total_bytes.load(Ordering::Relaxed);
        let downloaded = self.downloaded_bytes.load(Ordering::Relaxed);
        if total == 0 {
            0.0
        } else {
            (downloaded as f32 / total as f32).clamp(0.0, 1.0)
        }
    }
}

/// 渐进式音频媒体源（MediaSource 实现）。
pub struct ProgressiveMediaSource {
    /// 专门用于读取的独立只读文件句柄。
    reader_file: File,
    /// 当前读取游标。
    current_offset: u64,
    /// 资源总大小。
    total_size: Option<u64>,
    /// 共享下载同步状态。
    state: Arc<DownloadState>,
    /// 退出时清理或落盘标识。
    #[allow(dead_code)]
    part_path: PathBuf,
}

impl ProgressiveMediaSource {
    /// 打开本地或远端 WebDAV 音频资源。
    ///
    /// - 若已存在完整落盘缓存，则直接打开本地文件直读（0 流量延迟）。
    /// - 若尚未缓存，则创建 `.part` 文件并拉起后台异步下载线程。
    pub fn open(
        client: &WebDavClient,
        url: &str,
        cache_mgr: &mut CacheManager,
        ext: &str,
    ) -> Result<Self> {
        // 1. 优先命中完整本地缓存
        if let Some(cached_file) = cache_mgr.is_cached(url) {
            let meta = std::fs::metadata(&cached_file).map_err(LingfengError::Io)?;
            let total = meta.len();
            let reader_file = File::open(&cached_file).map_err(LingfengError::Io)?;

            let state = Arc::new(DownloadState::default());
            state.downloaded_bytes.store(total, Ordering::SeqCst);
            state.total_bytes.store(total, Ordering::SeqCst);
            state.is_finished.store(true, Ordering::SeqCst);

            return Ok(Self {
                reader_file,
                current_offset: 0,
                total_size: Some(total),
                state,
                part_path: cached_file,
            });
        }

        // 2. 未命中完整缓存：启动分块边下边播
        let part_path = cache_mgr.get_part_path(url, ext);
        let mut writer_file = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&part_path)
            .map_err(LingfengError::Io)?;

        let reader_file = File::open(&part_path).map_err(LingfengError::Io)?;
        let state = Arc::new(DownloadState::default());

        // 发起网络流拉取
        let resp = client.open_stream(url, None)?;
        let content_len = resp.content_length();
        if let Some(len) = content_len {
            state.total_bytes.store(len, Ordering::SeqCst);
        }

        // 启动后台流式写盘线程
        let thread_state = state.clone();
        let thread_url = url.to_string();
        let thread_ext = ext.to_string();
        let thread_part_path = part_path.clone();
        let cache_dir = cache_mgr.cache_dir().to_path_buf();

        thread::spawn(move || {
            let mut stream = resp;
            let mut buffer = [0u8; 64 * 1024]; // 64KB 稳态下载缓冲块
            let mut total_written: u64 = 0;

            loop {
                if thread_state.cancelled.load(Ordering::Relaxed) {
                    break;
                }

                match stream.read(&mut buffer) {
                    Ok(0) => {
                        // EOF: 下载完毕
                        thread_state.is_finished.store(true, Ordering::SeqCst);
                        let _ = writer_file.flush();

                        // 异步落盘为 .cached 文件
                        if let Ok(mut mgr) = CacheManager::with_dir(cache_dir) {
                            let file_name = thread_part_path
                                .file_name()
                                .and_then(|s| s.to_str())
                                .unwrap_or("audio");
                            let _ = mgr.record_complete(
                                &thread_url,
                                file_name,
                                &thread_ext,
                                total_written,
                            );
                        }

                        thread_state.condvar.notify_all();
                        break;
                    }
                    Ok(n) => {
                        if let Err(e) = writer_file.write_all(&buffer[..n]) {
                            thread_state.has_error.store(true, Ordering::SeqCst);
                            *thread_state.error_msg.lock().unwrap() =
                                Some(format!("写入缓存文件失败: {e}"));
                            thread_state.condvar.notify_all();
                            break;
                        }
                        let _ = writer_file.flush();
                        total_written += n as u64;
                        thread_state
                            .downloaded_bytes
                            .store(total_written, Ordering::SeqCst);

                        // 通知等待中的解码读取线程
                        thread_state.condvar.notify_all();
                    }
                    Err(e) => {
                        thread_state.has_error.store(true, Ordering::SeqCst);
                        *thread_state.error_msg.lock().unwrap() =
                            Some(format!("网络读取流异常中断: {e}"));
                        thread_state.condvar.notify_all();
                        break;
                    }
                }
            }
        });

        Ok(Self {
            reader_file,
            current_offset: 0,
            total_size: content_len,
            state,
            part_path,
        })
    }

    /// 获取底层同步状态引用（用于获取缓冲进度或卡顿标识）。
    pub fn state(&self) -> &Arc<DownloadState> {
        &self.state
    }
}

impl Drop for ProgressiveMediaSource {
    fn drop(&mut self) {
        self.state.cancelled.store(true, Ordering::Relaxed);
        self.state.condvar.notify_all();
    }
}

impl Read for ProgressiveMediaSource {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if buf.is_empty() {
            return Ok(0);
        }

        let total = self.total_size.unwrap_or(u64::MAX);
        if self.current_offset >= total {
            return Ok(0); // 到达文件末尾
        }

        let requested = buf.len() as u64;

        loop {
            let downloaded = self.state.downloaded_bytes.load(Ordering::SeqCst);

            // 1. 如果已有落盘数据覆盖当前游标，直接从本地文件读出
            if self.current_offset < downloaded {
                let available = (downloaded - self.current_offset).min(requested) as usize;
                self.reader_file
                    .seek(SeekFrom::Start(self.current_offset))?;
                let n = self.reader_file.read(&mut buf[..available])?;
                self.current_offset += n as u64;
                self.state.is_buffering.store(false, Ordering::Relaxed);
                return Ok(n);
            }

            // 2. 如果数据已全部下载完毕，且游标已达末尾，返回 EOF
            if self.state.is_finished.load(Ordering::SeqCst) {
                self.state.is_buffering.store(false, Ordering::Relaxed);
                return Ok(0);
            }

            // 3. 如果发生网络或 I/O 错误，向上抛出
            if self.state.has_error.load(Ordering::SeqCst) {
                let msg = self
                    .state
                    .error_msg
                    .lock()
                    .unwrap()
                    .clone()
                    .unwrap_or_else(|| "网络下载中断".to_string());
                return Err(io::Error::new(io::ErrorKind::ConnectionReset, msg));
            }

            // 4. 数据尚未到达：进入缓冲状态，等待条件变量唤醒
            self.state.is_buffering.store(true, Ordering::Relaxed);
            let guard = self.state.lock.lock().unwrap();
            let _ = self
                .state
                .condvar
                .wait_timeout(guard, Duration::from_millis(60));
        }
    }
}

impl Seek for ProgressiveMediaSource {
    fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
        let total = self.total_size.unwrap_or(u64::MAX);

        let new_offset = match pos {
            SeekFrom::Start(off) => off as i64,
            SeekFrom::Current(off) => (self.current_offset as i64) + off,
            SeekFrom::End(off) => (total as i64) + off,
        };

        if new_offset < 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Seek 位置不能小于 0",
            ));
        }

        self.current_offset = new_offset as u64;
        self.reader_file
            .seek(SeekFrom::Start(self.current_offset))?;
        Ok(self.current_offset)
    }
}

impl MediaSource for ProgressiveMediaSource {
    fn is_seekable(&self) -> bool {
        true
    }

    fn byte_len(&self) -> Option<u64> {
        self.total_size
    }
}
