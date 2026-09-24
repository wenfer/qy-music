//! WebDAV 远程存储与流媒体模块。
//!
//! 纯逻辑实现（遵循 AGENTS.md 约束 2.3）：
//! - 服务器配置管理与凭据安全混淆
//! - RFC 4918 PROPFIND 目录枚举与 XML 响应解析
//! - HTTP Range 分块嗅探（快速标签与同级歌词匹配）

pub mod client;
pub mod config;
pub mod xml_parser;

pub use client::WebDavClient;
pub use config::WebDavServerConfig;
pub use xml_parser::RemoteItem;

/// 根据音频 URL 自动匹配已保存的 WebDAV 服务器配置并构造 Client。
pub fn resolve_client_for_url(url: &str) -> WebDavClient {
    let settings = crate::config::Settings::load();
    for server in &settings.webdav_servers {
        let endpoint = server.endpoint.trim_end_matches('/');
        let server_prefix = format!("webdav://{}/", server.id);
        if url.starts_with(endpoint) || url.starts_with(&server_prefix) {
            if let Ok(client) = WebDavClient::new(server) {
                return client;
            }
        }
    }
    // 回退到匿名直连客户端
    let default_cfg = WebDavServerConfig::new("anon", "Anon", url, "", "");
    WebDavClient::new(&default_cfg).unwrap_or_else(|_| {
        let dummy = WebDavServerConfig::new("dummy", "", "http://127.0.0.1", "", "");
        WebDavClient::new(&dummy).unwrap()
    })
}
