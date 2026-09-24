//! 流媒体缓存与分块边下边播引擎。
//!
//! 纯逻辑实现（遵循 AGENTS.md 约束 2.3）：
//! - 本地磁盘持久化缓存索引与 LRU 自动清理
//! - Symphonia MediaSource 渐进式流读取器（非阻塞、条件变量唤醒）
//! - 实时网络缓冲进度与卡顿状态监测

pub mod config;
pub mod manager;
pub mod progressive_source;

pub use config::CacheConfig;
pub use manager::{CacheEntry, CacheManager};
pub use progressive_source::ProgressiveMediaSource;
