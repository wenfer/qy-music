//! 配置持久化：目录管理、首次运行初始化、播放列表落盘。
//!
//! 目录约定（见架构文档 §7）：
//! - 配置：`dirs::config_dir()/Lingfeng`（settings.json、skins/）
//! - 数据/缓存：`dirs::data_dir()/Lingfeng`（后续扩展）

use std::path::PathBuf;

use crate::error::{LingfengError, Result};
use crate::theme::skin::Skin;

/// 配置根目录：`dirs::config_dir()/Lingfeng`。
pub fn config_dir() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("Lingfeng")
}

/// 皮肤目录：`config_dir()/skins`。
pub fn skins_dir() -> PathBuf {
    config_dir().join("skins")
}

/// settings.json 路径。
pub fn settings_path() -> PathBuf {
    config_dir().join("settings.json")
}

/// playlist.json 路径（播放列表路径持久化，跨重启保留）。
pub fn playlist_path() -> PathBuf {
    config_dir().join("playlist.json")
}

/// 确保配置目录与皮肤目录存在。
pub fn ensure_dirs() -> Result<()> {
    let dir = config_dir();
    std::fs::create_dir_all(&dir).map_err(LingfengError::Io)?;
    std::fs::create_dir_all(skins_dir()).map_err(LingfengError::Io)?;
    Ok(())
}

/// 首次运行拷贝内置皮肤到 `skins/`（仅当文件不存在，避免覆盖用户编辑）。
pub fn install_builtin_skins() -> Result<()> {
    ensure_dirs()?;
    for skin in Skin::builtin() {
        let path = skins_dir().join(format!("{}.json", skin.id));
        if !path.exists() {
            let json = skin
                .to_json()
                .map_err(|e| LingfengError::other(format!("序列化内置皮肤失败: {e}")))?;
            std::fs::write(&path, json).map_err(LingfengError::Io)?;
        }
    }
    Ok(())
}

/// 扫描 `skins/` 目录下所有自定义皮肤 JSON（不含内置，供扩展）。
pub fn list_custom_skins() -> Vec<Skin> {
    let dir = skins_dir();
    let mut out = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("json") {
                if let Ok(skin) = Skin::from_json(&path) {
                    out.push(skin);
                }
            }
        }
    }
    out
}

/// 保存播放列表（仅路径，重建时再读元数据）。
pub fn save_playlist_paths(paths: &[PathBuf]) -> Result<()> {
    ensure_dirs()?;
    let serialized: Vec<String> = paths.iter().map(|p| p.to_string_lossy().into_owned()).collect();
    let json = serde_json::to_string_pretty(&serialized)?;
    std::fs::write(playlist_path(), json).map_err(LingfengError::Io)?;
    Ok(())
}

/// 读取播放列表路径（缺失返回空）。
pub fn load_playlist_paths() -> Vec<PathBuf> {
    let path = playlist_path();
    let Ok(text) = std::fs::read_to_string(&path) else {
        return Vec::new();
    };
    let Ok(paths): std::result::Result<Vec<String>, _> = serde_json::from_str(&text) else {
        return Vec::new();
    };
    paths.into_iter().map(PathBuf::from).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_dir_ends_with_lingfeng() {
        assert!(config_dir().ends_with("Lingfeng"));
    }

    #[test]
    fn settings_path_is_under_config_dir() {
        let p = settings_path();
        assert!(p.ends_with("settings.json"));
        assert!(p.starts_with(config_dir()));
    }

    #[test]
    fn playlist_path_is_under_config_dir() {
        let p = playlist_path();
        assert!(p.ends_with("playlist.json"));
        assert!(p.starts_with(config_dir()));
    }

    #[test]
    fn skins_dir_is_under_config_dir() {
        let p = skins_dir();
        assert!(p.ends_with("skins"));
        assert!(p.starts_with(config_dir()));
    }

    #[test]
    fn path_helpers_are_pure_and_do_not_panic() {
        // 仅查询路径不应创建目录、也不应 panic（重复调用幂等）。
        let a = config_dir();
        let b = config_dir();
        assert_eq!(a, b);
    }

    #[test]
    fn corrupt_playlist_json_deserialization_errors() {
        // load_playlist_paths 依赖 Vec<String> 反序列化；损坏内容应返回 Err
        // 从而使函数回退为空列表。
        let bad: std::result::Result<Vec<String>, _> = serde_json::from_str("{bad json");
        assert!(bad.is_err());
    }

    #[test]
    fn list_custom_skins_does_not_panic_on_missing_dir() {
        // 目录不存在时应返回空而非 panic。
        let _ = list_custom_skins();
    }
}
