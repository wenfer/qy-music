//! WebDAV HTTP 客户端实现。
//!
//! 支持 RFC 4918 PROPFIND 目录检索、HTTP Range 流式读取、自签证书放行与连接测试。

use std::time::{Duration, Instant};

use reqwest::blocking::{Client, Response};
use reqwest::header::{HeaderMap, HeaderValue, RANGE};
use reqwest::Method;

use crate::error::{LingfengError, Result};
use crate::webdav::config::WebDavServerConfig;
use crate::webdav::xml_parser::{parse_multistatus, RemoteItem};

/// 导出 RemoteItem 供外层直接引用。
pub use crate::webdav::xml_parser::RemoteItem as WebDavRemoteItem;

/// WebDAV 操作客户端。
#[derive(Clone)]
pub struct WebDavClient {
    client: Client,
    config: WebDavServerConfig,
}

impl WebDavClient {
    /// 根据服务器配置构造客户端。
    pub fn new(config: &WebDavServerConfig) -> Result<Self> {
        let mut builder = Client::builder()
            .timeout(Duration::from_secs(15))
            .connect_timeout(Duration::from_secs(5));

        if config.allow_insecure_cert {
            builder = builder.danger_accept_invalid_certs(true);
        }

        let client = builder
            .build()
            .map_err(|e| LingfengError::other(format!("构建 WebDAV HTTP 客户端失败: {e}")))?;

        Ok(Self {
            client,
            config: config.clone(),
        })
    }

    /// 获取关联的配置引用。
    pub fn config(&self) -> &WebDavServerConfig {
        &self.config
    }

    /// 测试连接连通性与鉴权，返回往返延迟（RTT）。
    pub fn test_connection(&self) -> Result<Duration> {
        let start = Instant::now();
        let target_url = self.config.build_url(&self.config.root_path);

        let propfind_method = Method::from_bytes(b"PROPFIND")
            .map_err(|e| LingfengError::other(format!("无效 HTTP 方法: {e}")))?;

        let mut headers = HeaderMap::new();
        headers.insert("Depth", HeaderValue::from_static("0"));

        let req = self
            .client
            .request(propfind_method, &target_url)
            .headers(headers)
            .basic_auth(&self.config.username, Some(self.config.get_password()));

        let resp = req
            .send()
            .map_err(|e| LingfengError::other(format!("连接 WebDAV 服务器失败: {e}")))?;

        let status = resp.status();
        if status.is_success() || status.as_u16() == 207 {
            Ok(start.elapsed())
        } else if status.as_u16() == 401 {
            Err(LingfengError::other(
                "WebDAV 认证失败：用户名或密码错误 (401 Unauthorized)",
            ))
        } else {
            Err(LingfengError::other(format!(
                "WebDAV 服务器返回错误状态: {}",
                status
            )))
        }
    }

    /// 列出远程目录内容（Depth: 1）。
    pub fn list_dir(&self, remote_path: &str) -> Result<Vec<RemoteItem>> {
        let target_url = self.config.build_url(remote_path);

        let propfind_method = Method::from_bytes(b"PROPFIND")
            .map_err(|e| LingfengError::other(format!("无效 HTTP 方法: {e}")))?;

        let mut headers = HeaderMap::new();
        headers.insert("Depth", HeaderValue::from_static("1"));

        let req = self
            .client
            .request(propfind_method, &target_url)
            .headers(headers)
            .basic_auth(&self.config.username, Some(self.config.get_password()));

        let resp = req
            .send()
            .map_err(|e| LingfengError::other(format!("请求 WebDAV 目录失败: {e}")))?;

        let status = resp.status();
        if !status.is_success() && status.as_u16() != 207 {
            return Err(LingfengError::other(format!(
                "WebDAV 列举目录失败: HTTP 状态码 {}",
                status
            )));
        }

        let xml_text = resp
            .text()
            .map_err(|e| LingfengError::other(format!("读取 WebDAV 响应体失败: {e}")))?;

        let items = parse_multistatus(&xml_text, remote_path);
        Ok(items)
    }

    /// 嗅探指定切片字节范围（用于快速读取 ID3 / FLAC 头标签）。
    pub fn fetch_range_bytes(&self, full_url: &str, start: u64, end: u64) -> Result<Vec<u8>> {
        let range_val = format!("bytes={}-{}", start, end);
        let header_val = HeaderValue::from_str(&range_val)
            .map_err(|e| LingfengError::other(format!("构造 Range 请求头失败: {e}")))?;

        let mut headers = HeaderMap::new();
        headers.insert(RANGE, header_val);

        let req = self
            .client
            .get(full_url)
            .headers(headers)
            .basic_auth(&self.config.username, Some(self.config.get_password()));

        let resp = req
            .send()
            .map_err(|e| LingfengError::other(format!("获取音频 Range 切片失败: {e}")))?;

        let status = resp.status();
        if !status.is_success() && status.as_u16() != 206 {
            return Err(LingfengError::other(format!(
                "服务器不支持 Range 切片或返回错误: {}",
                status
            )));
        }

        let bytes = resp
            .bytes()
            .map_err(|e| LingfengError::other(format!("读取 Range 响应体失败: {e}")))?;

        Ok(bytes.to_vec())
    }

    /// 探测远端资源是否存在（用于同名 .lrc 歌词嗅探）。
    pub fn check_exists(&self, full_url: &str) -> bool {
        let req = self
            .client
            .head(full_url)
            .basic_auth(&self.config.username, Some(self.config.get_password()));

        match req.send() {
            Ok(resp) => resp.status().is_success(),
            Err(_) => false,
        }
    }

    /// 下载远端文本内容（用于同名 .lrc 歌词拉取）。
    pub fn fetch_text(&self, full_url: &str) -> Result<String> {
        let req = self
            .client
            .get(full_url)
            .basic_auth(&self.config.username, Some(self.config.get_password()));

        let resp = req
            .send()
            .map_err(|e| LingfengError::other(format!("拉取文本资源失败: {e}")))?;

        if !resp.status().is_success() {
            return Err(LingfengError::other(format!(
                "拉取文本返回状态码: {}",
                resp.status()
            )));
        }

        let text = resp
            .text()
            .map_err(|e| LingfengError::other(format!("读取文本内容失败: {e}")))?;

        Ok(text)
    }

    /// 获取用于流式边下边播的 Response 对象，支持从指定偏移量继续拉流。
    pub fn open_stream(&self, full_url: &str, range_start: Option<u64>) -> Result<Response> {
        let mut headers = HeaderMap::new();
        if let Some(start) = range_start {
            let range_val = format!("bytes={}-", start);
            if let Ok(hv) = HeaderValue::from_str(&range_val) {
                headers.insert(RANGE, hv);
            }
        }

        let req = self
            .client
            .get(full_url)
            .headers(headers)
            .basic_auth(&self.config.username, Some(self.config.get_password()));

        let resp = req
            .send()
            .map_err(|e| LingfengError::other(format!("发起音频下载流失败: {e}")))?;

        let status = resp.status();
        if !status.is_success() && status.as_u16() != 206 {
            return Err(LingfengError::other(format!(
                "音频下载流返回异常状态: {}",
                status
            )));
        }

        Ok(resp)
    }
}
