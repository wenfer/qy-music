//! 缓存配置模型。

use serde::{Deserialize, Serialize};

/// 缓存配置项。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CacheConfig {
    /// 是否开启磁盘持久化缓存（默认 true）。
    pub enabled: bool,
    /// 最大缓存占用空间（单位 MB，默认 2048 即 2.0 GB；0 表示无限制）。
    pub max_size_mb: u64,
    /// 起播预加载缓冲门限（单位 KB，默认 512 KB）。
    pub prefetch_kb: u64,
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            max_size_mb: 2048, // 2GB
            prefetch_kb: 512,  // 512KB 迅速起播
        }
    }
}
