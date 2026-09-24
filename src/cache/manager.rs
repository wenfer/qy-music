//! 本地持久化缓存管理器与 LRU 自动清理。

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::error::{LingfengError, Result};

/// 缓存条目元数据。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CacheEntry {
    /// URL 的 SHA256 哈希值。
    pub url_hash: String,
    /// 完整源 URL。
    pub original_url: String,
    /// 原始文件名。
    pub file_name: String,
    /// 文件总大小（字节）。
    pub total_size: u64,
    /// 是否已完整下载并落盘。
    pub is_complete: bool,
    /// 最后访问时间戳（Unix 秒数，用于 LRU）。
    pub last_accessed_at: u64,
    /// 文件后缀（如 "flac", "mp3"）。
    pub file_ext: String,
}

/// 缓存索引持久化结构。
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
struct CacheIndex {
    entries: HashMap<String, CacheEntry>,
}

/// 磁盘持久化缓存管理器。
pub struct CacheManager {
    cache_dir: PathBuf,
    index: CacheIndex,
}

impl CacheManager {
    /// 默认系统缓存目录：`dirs::cache_dir()/Lingfeng/webdav_cache`。
    pub fn default_dir() -> PathBuf {
        dirs::cache_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("Lingfeng")
            .join("webdav_cache")
    }

    /// 使用默认缓存路径初始化管理器。
    pub fn new() -> Result<Self> {
        Self::with_dir(Self::default_dir())
    }

    /// 指定缓存路径初始化管理器（便于隔离单测）。
    pub fn with_dir(cache_dir: PathBuf) -> Result<Self> {
        fs::create_dir_all(&cache_dir).map_err(LingfengError::Io)?;
        let mut mgr = Self {
            cache_dir,
            index: CacheIndex::default(),
        };
        mgr.load_index();
        Ok(mgr)
    }

    /// 计算 URL 的 SHA256 哈希字符串。
    pub fn hash_url(url: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(url.as_bytes());
        format!("{:x}", hasher.finalize())
    }

    /// 当前缓存目录。
    pub fn cache_dir(&self) -> &Path {
        &self.cache_dir
    }

    fn index_path(&self) -> PathBuf {
        self.cache_dir.join("cache_index.json")
    }

    /// 加载索引文件。
    fn load_index(&mut self) {
        let path = self.index_path();
        if let Ok(content) = fs::read_to_string(&path) {
            if let Ok(idx) = serde_json::from_str::<CacheIndex>(&content) {
                self.index = idx;
            }
        }
    }

    /// 保存索引文件。
    pub fn save_index(&self) -> Result<()> {
        let path = self.index_path();
        let json = serde_json::to_string_pretty(&self.index)
            .map_err(|e| LingfengError::other(format!("序列化缓存索引失败: {e}")))?;
        fs::write(path, json).map_err(LingfengError::Io)?;
        Ok(())
    }

    /// 检查指定 URL 是否已存在完整缓存。
    pub fn is_cached(&mut self, url: &str) -> Option<PathBuf> {
        let hash = Self::hash_url(url);
        let (is_complete, ext) = match self.index.entries.get(&hash) {
            Some(entry) => (entry.is_complete, entry.file_ext.clone()),
            None => return None,
        };

        if is_complete {
            let target_path = self.get_complete_path(&hash, &ext);
            if target_path.exists() {
                if let Some(entry) = self.index.entries.get_mut(&hash) {
                    entry.last_accessed_at = now_timestamp();
                }
                let _ = self.save_index();
                return Some(target_path);
            } else if let Some(entry) = self.index.entries.get_mut(&hash) {
                entry.is_complete = false;
                let _ = self.save_index();
            }
        }
        None
    }

    /// 获取临时下载分块文件路径（`.part`）。
    pub fn get_part_path(&self, url: &str, ext: &str) -> PathBuf {
        let hash = Self::hash_url(url);
        self.cache_dir.join(format!("{}.{}.part", hash, ext))
    }

    /// 获取完整落盘缓存文件路径（`.cached`）。
    pub fn get_complete_path(&self, hash: &str, ext: &str) -> PathBuf {
        self.cache_dir.join(format!("{}.{}.cached", hash, ext))
    }

    /// 标记曲目下载完毕并持久化落盘。
    pub fn record_complete(
        &mut self,
        url: &str,
        file_name: &str,
        ext: &str,
        total_size: u64,
    ) -> Result<PathBuf> {
        let hash = Self::hash_url(url);
        let part_path = self.get_part_path(url, ext);
        let cached_path = self.get_complete_path(&hash, ext);

        if part_path.exists() {
            let _ = fs::rename(&part_path, &cached_path);
        }

        let entry = CacheEntry {
            url_hash: hash.clone(),
            original_url: url.to_string(),
            file_name: file_name.to_string(),
            total_size,
            is_complete: true,
            last_accessed_at: now_timestamp(),
            file_ext: ext.to_string(),
        };

        self.index.entries.insert(hash, entry);
        self.save_index()?;
        Ok(cached_path)
    }

    /// 刷新最近访问时间。
    pub fn touch(&mut self, url: &str) {
        let hash = Self::hash_url(url);
        if let Some(entry) = self.index.entries.get_mut(&hash) {
            entry.last_accessed_at = now_timestamp();
            let _ = self.save_index();
        }
    }

    /// 计算当前缓存占用的总字节数。
    pub fn total_cache_size(&self) -> u64 {
        let mut total = 0;
        if let Ok(entries) = fs::read_dir(&self.cache_dir) {
            for entry in entries.flatten() {
                if let Ok(meta) = entry.metadata() {
                    if meta.is_file() {
                        total += meta.len();
                    }
                }
            }
        }
        total
    }

    /// 执行 LRU 缓存淘汰算法。
    ///
    /// 当总占用超过 `max_size_mb` 时，按 `last_accessed_at` 从最旧的条目开始清除，
    /// 直至回落到安全阈值（例如最大配额的 85%）。返回清理释放的字节总数。
    pub fn evict_if_needed(&mut self, max_size_mb: u64) -> Result<u64> {
        if max_size_mb == 0 {
            return Ok(0); // 0 代表无限制
        }

        let max_bytes = max_size_mb * 1024 * 1024;
        let mut current_size = self.total_cache_size();
        if current_size <= max_bytes {
            return Ok(0);
        }

        let target_size = (max_bytes as f64 * 0.85) as u64;
        let mut freed_bytes: u64 = 0;

        // 按最后访问时间升序排列（最旧排在最前）
        let mut entries: Vec<CacheEntry> = self.index.entries.values().cloned().collect();
        entries.sort_by_key(|e| e.last_accessed_at);

        for entry in entries {
            if current_size <= target_size {
                break;
            }

            let cached_path = self.get_complete_path(&entry.url_hash, &entry.file_ext);
            let part_path = self
                .cache_dir
                .join(format!("{}.{}.part", entry.url_hash, entry.file_ext));

            let mut removed_for_entry = 0;
            if let Ok(meta) = fs::metadata(&cached_path) {
                removed_for_entry += meta.len();
                let _ = fs::remove_file(&cached_path);
            }
            if let Ok(meta) = fs::metadata(&part_path) {
                removed_for_entry += meta.len();
                let _ = fs::remove_file(&part_path);
            }

            if removed_for_entry > 0 {
                current_size = current_size.saturating_sub(removed_for_entry);
                freed_bytes += removed_for_entry;
                self.index.entries.remove(&entry.url_hash);
            }
        }

        self.save_index()?;
        Ok(freed_bytes)
    }

    /// 清空所有缓存文件与索引。
    pub fn clear_all(&mut self) -> Result<()> {
        if let Ok(entries) = fs::read_dir(&self.cache_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_file() {
                    let _ = fs::remove_file(path);
                }
            }
        }
        self.index.entries.clear();
        self.save_index()?;
        Ok(())
    }
}

fn now_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cache_hashing_and_roundtrip() {
        let temp_dir = std::env::temp_dir().join(format!("lf_cache_test_{}", now_timestamp()));
        let mut mgr = CacheManager::with_dir(temp_dir.clone()).unwrap();

        let url = "http://nas.local/music/晴天.flac";
        assert!(mgr.is_cached(url).is_none());

        let part_path = mgr.get_part_path(url, "flac");
        fs::write(&part_path, b"test audio content 123456").unwrap();

        let complete_path = mgr.record_complete(url, "晴天.flac", "flac", 25).unwrap();
        assert!(complete_path.exists());
        assert!(!part_path.exists());

        let cached = mgr.is_cached(url);
        assert!(cached.is_some());
        assert_eq!(cached.unwrap(), complete_path);

        // 清理临时测试目录
        let _ = mgr.clear_all();
        let _ = fs::remove_dir_all(temp_dir);
    }
}
