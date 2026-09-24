//! WebDAV 服务器连接配置模型与凭据混淆序列化。

use serde::{Deserialize, Serialize};

/// WebDAV 服务器配置项。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WebDavServerConfig {
    /// 唯一标识 UUID / 字符串。
    pub id: String,
    /// 用户自定义服务别名（例如 "群晖 NAS", "Alist 云盘"）。
    pub name: String,
    /// WebDAV 端点 URL（例如 "http://192.168.1.50:5244/dav" 或 "https://nas.local:5006/dav"）。
    pub endpoint: String,
    /// 鉴权用户名。
    pub username: String,
    /// 混淆存储的密码（通过 base64 简单编码，避免在配置文件中以纯明文展示）。
    #[serde(default)]
    pub password_obfuscated: String,
    /// 挂载的根路径目录（默认为 "/"）。
    #[serde(default = "default_root_path")]
    pub root_path: String,
    /// 是否允许自签或无效 TLS/SSL 证书（内网局域网 NAS 常用）。
    #[serde(default)]
    pub allow_insecure_cert: bool,
}

fn default_root_path() -> String {
    "/".to_string()
}

impl WebDavServerConfig {
    /// 构造新的配置项。
    pub fn new(
        id: impl Into<String>,
        name: impl Into<String>,
        endpoint: impl Into<String>,
        username: impl Into<String>,
        password_plain: &str,
    ) -> Self {
        let mut cfg = Self {
            id: id.into(),
            name: name.into(),
            endpoint: endpoint.into(),
            username: username.into(),
            password_obfuscated: String::new(),
            root_path: "/".to_string(),
            allow_insecure_cert: false,
        };
        cfg.set_password(password_plain);
        cfg
    }

    /// 设置明文密码并自动混淆。
    pub fn set_password(&mut self, plain: &str) {
        use base64::Engine;
        self.password_obfuscated =
            base64::engine::general_purpose::STANDARD.encode(plain.as_bytes());
    }

    /// 读取还原后的明文密码。
    pub fn get_password(&self) -> String {
        use base64::Engine;
        if self.password_obfuscated.is_empty() {
            return String::new();
        }
        match base64::engine::general_purpose::STANDARD.decode(&self.password_obfuscated) {
            Ok(bytes) => String::from_utf8_lossy(&bytes).into_owned(),
            Err(_) => self.password_obfuscated.clone(),
        }
    }

    /// 根据相对或绝对远程路径拼接完整 URL（支持协议绝对路径与前缀智能去重）。
    pub fn build_url(&self, remote_path: &str) -> String {
        if remote_path.starts_with("http://") || remote_path.starts_with("https://") {
            return remote_path.to_string();
        }

        let base = self.endpoint.trim_end_matches('/');
        let rel = remote_path.trim_start_matches('/');

        // 解析 endpoint 的 host 与 path 前缀，智能去重
        if let Ok(parsed_endpoint) = url::Url::parse(base) {
            let endpoint_path = parsed_endpoint.path().trim_matches('/');
            if !endpoint_path.is_empty() {
                // 如果 rel 已经包含了 endpoint 的 path（如 "dav/Jay/song.flac" 与 "dav"）
                if rel == endpoint_path {
                    return format!("{}/", base);
                } else if rel.starts_with(&format!("{}/", endpoint_path)) {
                    // 剥离重复的前缀，避免出现 /dav/dav/
                    let host_origin = parsed_endpoint.origin().ascii_serialization();
                    return format!("{}/{}", host_origin.trim_end_matches('/'), rel);
                }
            }
        }

        if rel.is_empty() {
            format!("{}/", base)
        } else {
            format!("{}/{}", base, rel)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_password_obfuscation() {
        let mut cfg =
            WebDavServerConfig::new("1", "NAS", "http://127.0.0.1:5005", "admin", "secret123");
        assert_eq!(cfg.get_password(), "secret123");
        assert_ne!(cfg.password_obfuscated, "secret123");

        cfg.set_password("new_pwd_456");
        assert_eq!(cfg.get_password(), "new_pwd_456");
    }

    #[test]
    fn test_build_url() {
        let cfg = WebDavServerConfig::new("1", "NAS", "http://nas.local:5005/dav", "admin", "123");
        assert_eq!(cfg.build_url("/"), "http://nas.local:5005/dav/");
        assert_eq!(
            cfg.build_url("music/song.flac"),
            "http://nas.local:5005/dav/music/song.flac"
        );
        assert_eq!(
            cfg.build_url("/music/song.flac"),
            "http://nas.local:5005/dav/music/song.flac"
        );
    }
}
