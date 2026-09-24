//! RFC 4918 WebDAV XML 响应解析器。
//!
//! 解析 `<D:multistatus>` 响应，提取文件/目录名、路径、大小与资源类型。

use quick_xml::events::Event;
use quick_xml::Reader;

/// 远端文件或目录项。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RemoteItem {
    /// 显示名称（缺失时从 href 末尾提取）。
    pub name: String,
    /// 完整资源 href（通常形如 `/dav/music/song.flac`）。
    pub href: String,
    /// 是否为目录。
    pub is_dir: bool,
    /// 文件大小（字节），目录为 0。
    pub size: u64,
    /// 最后修改时间文本。
    pub last_modified: Option<String>,
}

impl RemoteItem {
    /// 是否为支持的音频文件（根据扩展名判断）。
    pub fn is_audio_file(&self) -> bool {
        if self.is_dir {
            return false;
        }
        let ext = self
            .name
            .rsplit('.')
            .next()
            .unwrap_or("")
            .to_ascii_lowercase();
        matches!(
            ext.as_str(),
            "mp3"
                | "flac"
                | "wav"
                | "ogg"
                | "m4a"
                | "aac"
                | "opus"
                | "ape"
                | "wma"
                | "alac"
                | "aiff"
                | "dsf"
                | "dff"
        )
    }

    /// 是否为 LRC 歌词文件。
    pub fn is_lyrics_file(&self) -> bool {
        if self.is_dir {
            return false;
        }
        self.name.to_ascii_lowercase().ends_with(".lrc")
    }
}

/// 解析 WebDAV PROPFIND 返回的 XML 字符串。
///
/// `target_path`: 当前查询的目录路径，解析结果会自动过滤掉代表当前目录自身的首项。
pub fn parse_multistatus(xml: &str, target_path: &str) -> Vec<RemoteItem> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);

    let mut items = Vec::new();
    let mut in_response = false;
    let mut in_href = false;
    let mut in_displayname = false;
    let mut in_contentlength = false;
    let mut in_lastmodified = false;
    let mut in_resourcetype = false;

    let mut current_href = String::new();
    let mut current_displayname = String::new();
    let mut current_size: u64 = 0;
    let mut current_lastmodified: Option<String> = None;
    let mut current_is_dir = false;

    let target_norm = normalize_href(target_path);

    while let Ok(event) = reader.read_event() {
        match event {
            Event::Start(ref e) => {
                let local = e.local_name();
                match local.as_ref() {
                    b"response" => {
                        in_response = true;
                        current_href.clear();
                        current_displayname.clear();
                        current_size = 0;
                        current_lastmodified = None;
                        current_is_dir = false;
                    }
                    b"href" if in_response => in_href = true,
                    b"displayname" if in_response => in_displayname = true,
                    b"getcontentlength" if in_response => in_contentlength = true,
                    b"getlastmodified" if in_response => in_lastmodified = true,
                    b"resourcetype" if in_response => in_resourcetype = true,
                    b"collection" if in_resourcetype => {
                        current_is_dir = true;
                    }
                    _ => {}
                }
            }
            Event::Empty(ref e) => {
                let local = e.local_name();
                if in_resourcetype && local.as_ref() == b"collection" {
                    current_is_dir = true;
                }
            }
            Event::Text(ref e) => {
                if let Ok(text) = e.unescape() {
                    let text = text.trim();
                    if in_href {
                        current_href.push_str(text);
                    } else if in_displayname {
                        current_displayname.push_str(text);
                    } else if in_contentlength {
                        if let Ok(s) = text.parse::<u64>() {
                            current_size = s;
                        }
                    } else if in_lastmodified {
                        current_lastmodified = Some(text.to_string());
                    }
                }
            }
            Event::End(ref e) => {
                let local = e.local_name();
                match local.as_ref() {
                    b"href" => in_href = false,
                    b"displayname" => in_displayname = false,
                    b"getcontentlength" => in_contentlength = false,
                    b"getlastmodified" => in_lastmodified = false,
                    b"resourcetype" => in_resourcetype = false,
                    b"response" => {
                        in_response = false;
                        if !current_href.is_empty() {
                            // URL 解码与处理
                            let decoded_href = urlencoding_decode(&current_href);
                            let norm_href = normalize_href(&decoded_href);

                            // 如果不是自身查询目录，则作为子项加入
                            if !is_same_resource(&norm_href, &target_norm) {
                                let name = if !current_displayname.is_empty() {
                                    current_displayname.clone()
                                } else {
                                    extract_filename_from_href(&norm_href)
                                };

                                if !name.is_empty() {
                                    items.push(RemoteItem {
                                        name,
                                        href: decoded_href,
                                        is_dir: current_is_dir,
                                        size: current_size,
                                        last_modified: current_lastmodified.clone(),
                                    });
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
            Event::Eof => break,
            _ => {}
        }
    }

    // 目录在前，文件在后，按字母序排列
    items.sort_by(|a, b| match (a.is_dir, b.is_dir) {
        (true, false) => std::cmp::Ordering::Less,
        (false, true) => std::cmp::Ordering::Greater,
        _ => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
    });

    items
}

/// 路径规范化（去除多余首尾斜杠）。
fn normalize_href(href: &str) -> String {
    let clean = href.trim();
    let clean = clean.strip_suffix('/').unwrap_or(clean);
    clean.to_string()
}

/// 判断两个 href 是否指向同一个资源（忽略结尾斜杠与前导斜杠）。
fn is_same_resource(a: &str, b: &str) -> bool {
    let a_trim = a.trim_matches('/');
    let b_trim = b.trim_matches('/');
    a_trim == b_trim
}

/// 从 href 路径提取最后的文件/文件夹名。
fn extract_filename_from_href(href: &str) -> String {
    let trimmed = href.trim_matches('/');
    trimmed.rsplit('/').next().unwrap_or(trimmed).to_string()
}

/// 简易安全的 URL 解码（处理 %20, %E4%B8%AD%E6%96%87 等）。
fn urlencoding_decode(s: &str) -> String {
    url::form_urlencoded::parse(s.as_bytes())
        .map(|(k, _)| k.into_owned())
        .collect::<Vec<_>>()
        .join("")
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_XML: &str = r#"<?xml version="1.0" encoding="utf-8"?>
<D:multistatus xmlns:D="DAV:">
  <D:response>
    <D:href>/dav/music/</D:href>
    <D:propstat>
      <D:prop>
        <D:displayname>music</D:displayname>
        <D:resourcetype><D:collection/></D:resourcetype>
      </D:prop>
      <D:status>HTTP/1.1 200 OK</D:status>
    </D:propstat>
  </D:response>
  <D:response>
    <D:href>/dav/music/Jay/</D:href>
    <D:propstat>
      <D:prop>
        <D:displayname>Jay Chou</D:displayname>
        <D:resourcetype><D:collection/></D:resourcetype>
      </D:prop>
      <D:status>HTTP/1.1 200 OK</D:status>
    </D:propstat>
  </D:response>
  <D:response>
    <D:href>/dav/music/%E6%99%B4%E5%A4%A9.flac</D:href>
    <D:propstat>
      <D:prop>
        <D:displayname>晴天.flac</D:displayname>
        <D:getcontentlength>32456789</D:getcontentlength>
        <D:getlastmodified>Sun, 20 Sep 2026 12:00:00 GMT</D:getlastmodified>
        <D:resourcetype/>
      </D:prop>
      <D:status>HTTP/1.1 200 OK</D:status>
    </D:propstat>
  </D:response>
  <D:response>
    <D:href>/dav/music/%E6%99%B4%E5%A4%A9.lrc</D:href>
    <D:propstat>
      <D:prop>
        <D:displayname>晴天.lrc</D:displayname>
        <D:getcontentlength>1234</D:getcontentlength>
        <D:resourcetype/>
      </D:prop>
      <D:status>HTTP/1.1 200 OK</D:status>
    </D:propstat>
  </D:response>
</D:multistatus>"#;

    #[test]
    fn test_parse_multistatus() {
        let items = parse_multistatus(SAMPLE_XML, "/dav/music");
        assert_eq!(items.len(), 3);

        // 目录排在最前
        assert_eq!(items[0].name, "Jay Chou");
        assert!(items[0].is_dir);

        // 文件排在后面
        assert_eq!(items[1].name, "晴天.flac");
        assert!(!items[1].is_dir);
        assert_eq!(items[1].size, 32456789);
        assert!(items[1].is_audio_file());
        assert!(!items[1].is_lyrics_file());

        assert_eq!(items[2].name, "晴天.lrc");
        assert!(!items[2].is_dir);
        assert!(!items[2].is_audio_file());
        assert!(items[2].is_lyrics_file());
    }
}
