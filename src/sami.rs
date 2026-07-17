//! SAMI (Synchronized Accessible Media Interchange) parser and generator.
//!
//! Microsoft-developed subtitle format used in Windows Media Player and
//! widely adopted in Asian markets. Supports styling and multi-language subtitles.
//!
//! Format: HTML-like structure with `<Sync>` and `<P>` tags.

use crate::error::SubtitleError;
use crate::model::{Format, Subtitle, SubtitleFile};
use crate::types::AnyResult;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::LazyLock;

static RE_SYNC_TAG: LazyLock<Regex> =
  LazyLock::new(|| Regex::new(r"(?i)<Sync[^>]*Start\s*=\s*(\d+)[^>]*>").unwrap());

static RE_P_TAG: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)<P[^>]*>(.*?)</P>").unwrap());

static RE_STRIP_TAGS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"<[^>]+>").unwrap());

/// Case-insensitive search for a literal needle, returns the byte index of the
/// match start. Used for HTML tag lookups where `<Head>`, `<HEAD>`, `<head>`
/// must all be recognized.
fn find_ci(haystack: &str, needle: &str) -> Option<usize> {
  haystack.to_lowercase().find(&needle.to_lowercase())
}

/// Case-insensitive search returning both start and end offset of the match,
/// so callers can slice the original (case-preserved) text.
fn find_ci_range(haystack: &str, needle: &str) -> Option<(usize, usize)> {
  let lower = haystack.to_lowercase();
  let needle = needle.to_lowercase();
  let start = lower.find(&needle)?;
  Some((start, start + needle.len()))
}

/// SAMI subtitle data including header and styles.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct SamiData {
  /// Optional header content (Head section)
  pub header: Option<String>,
  /// Style definitions (CSS)
  pub styles: HashMap<String, String>,
  /// Subtitle entries
  pub subtitles: Vec<Subtitle>,
}

impl SamiData {
  /// Parse SAMI content into structured data.
  pub fn parse(content: &str) -> Result<Self, SubtitleError> {
    let mut header = None;
    let mut styles = HashMap::new();

    // Extract header (Head section) — 大小写不敏感，兼容 <HEAD>/<head>
    if let Some((head_start, _)) = find_ci_range(content, "<Head>") {
      if let Some(head_end) = find_ci(content, "</Head>") {
        header = Some(content[head_start..head_end + 7].to_string());
      }
    }

    // Extract styles (Style section) — 大小写不敏感
    if let Some(style_start) = find_ci(content, "<Style") {
      if let Some(style_end) = find_ci(content, "</Style>") {
        let style_content = &content[style_start..style_end + 8];
        // Simple CSS parsing - extract class definitions
        if let Some(css_start) = style_content.find('>') {
          let css = &style_content[css_start + 1..style_content.len().saturating_sub(8)];
          for line in css.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with('.') {
              let parts: Vec<&str> = trimmed.splitn(2, '{').collect();
              if parts.len() == 2 {
                let class_name = parts[0].trim()[1..].to_string();
                let style_def = parts[1].trim();
                styles.insert(class_name, style_def.to_string());
              }
            }
          }
        }
      }
    }

    // 两遍扫描：第一遍收集所有 Sync 的起始时间，用于第二遍推导结束时间。
    // 旧的实现把每条字幕的 end 硬编码为 start + 3000ms，丢失了真实时长信息。
    let mut sync_starts: Vec<u64> = Vec::new();
    for caps in RE_SYNC_TAG.captures_iter(content) {
      let start_ms: u64 = caps[1].parse().unwrap_or(0);
      sync_starts.push(start_ms);
    }
    sync_starts.sort_unstable();
    sync_starts.dedup();

    // 第二遍：解析每个 Sync 块，用「下一个更晚的 Sync start」作为 end。
    // 默认兜底 3 秒，保证最后一条字幕仍有合理时长。
    const DEFAULT_TAIL_MS: u64 = 3000;
    let mut subtitles: Vec<Subtitle> = Vec::with_capacity(64);
    let mut pos = 0;
    while let Some(sync_match) = RE_SYNC_TAG.captures(&content[pos..]) {
      let full_match = sync_match.get(0).unwrap();
      let start_ms: u64 = sync_match[1].parse().unwrap_or(0);

      // Find the end of this Sync block — 大小写不敏感
      let sync_end = match find_ci(&content[pos..], "</Sync>") {
        Some(offset) => offset,
        None => break,
      };

      let sync_block = &content[pos + full_match.start()..pos + sync_end + 7];

      // 推导 end：取首个严格大于当前 start 的 Sync 时间；找不到则加默认尾段。
      let end_ms = sync_starts
        .iter()
        .copied()
        .find(|&t| t > start_ms)
        .unwrap_or(start_ms + DEFAULT_TAIL_MS);

      // Extract P tag content
      for p_match in RE_P_TAG.captures_iter(sync_block) {
        let p_content = &p_match[1];
        let text = RE_STRIP_TAGS.replace_all(p_content, "").to_string();
        let text = text.trim().to_string();

        if !text.is_empty() {
          subtitles.push(Subtitle::new(start_ms, end_ms, &text));
          break; // Only take first P tag per Sync
        }
      }

      pos += sync_end + 7;
    }

    Ok(SamiData {
      header,
      styles,
      subtitles,
    })
  }

  /// Convert to Vec<Subtitle> for compatibility.
  pub fn to_subtitles(&self) -> Vec<Subtitle> {
    self.subtitles.clone()
  }

  /// Serialize back to SAMI format.
  pub fn render(&self) -> String {
    let mut buf = String::from("<SAMI>\n");

    // Header
    if let Some(ref head) = self.header {
      buf.push_str(head);
      buf.push('\n');
    } else {
      buf.push_str("<Head>\n");
      buf.push_str("<Title>Subtitles</Title>\n");
      buf.push_str("<Style Type=\"text/css\">\n");
      buf.push_str("<!--\n");
      buf.push_str("  .ENCC {Name: English; lang: en-US;}\n");
      buf.push_str("-->\n");
      buf.push_str("</Style>\n");
      buf.push_str("</Head>\n\n");
    }

    // Body
    buf.push_str("<Body>\n");

    for sub in &self.subtitles {
      buf.push_str(&format!(
        "<Sync Start={}><P>{}</P></Sync>\n",
        sub.start, sub.text
      ));
    }

    buf.push_str("</Body>\n");
    buf.push_str("</SAMI>\n");

    buf
  }
}

/// Parse SAMI content into a SubtitleFile.
pub fn parse_content(content: &str) -> Result<SubtitleFile, SubtitleError> {
  let data = SamiData::parse(content)?;
  Ok(SubtitleFile::Sami(data))
}

/// Parse SAMI from a byte slice.
pub fn parse_bytes(data: &[u8]) -> AnyResult<SubtitleFile> {
  let text = crate::encoding::decode_to_string(data)?;
  Ok(parse_content(&text)?)
}

/// Parse a SAMI file asynchronously.
pub async fn parse_file(path: impl AsRef<std::path::Path>) -> AnyResult<SubtitleFile> {
  let text = tokio::fs::read_to_string(path).await?;
  Ok(parse_content(&text)?)
}

/// Parse a SAMI file from a URL (requires `http` feature).
#[cfg(feature = "http")]
pub async fn parse_url(url: &str) -> AnyResult<SubtitleFile> {
  let response = reqwest::get(url).await?;
  let content = response.text().await?;
  Ok(parse_content(&content)?)
}

/// Detect if data looks like SAMI.
pub fn detect_format(data: &[u8]) -> Option<crate::model::Format> {
  let text = crate::encoding::try_decode_for_detection(data)?;
  if text.contains("<SAMI>") || text.contains("<Sync") {
    return Some(crate::model::Format::Sami);
  }
  None
}

/// Serialize subtitles to SAMI format.
pub fn to_string(subtitles: &[Subtitle], header: Option<&str>) -> String {
  let data = SamiData {
    header: header.map(|s| s.to_string()),
    styles: HashMap::new(),
    subtitles: subtitles.to_vec(),
  };
  data.render()
}

pub struct SamiStream<'a> {
  content: &'a str,
  pos: usize,
}

impl<'a> SamiStream<'a> {
  pub fn new(content: &'a str) -> Self {
    SamiStream { content, pos: 0 }
  }
}

impl<'a> Iterator for SamiStream<'a> {
  type Item = AnyResult<Subtitle>;

  fn next(&mut self) -> Option<Self::Item> {
    // 流式场景下无法预知后续 Sync 的 start，故 end 取默认尾段。
    // 语义上与 SamiData::parse 的「最后一条/无后续」兜底一致。
    const DEFAULT_TAIL_MS: u64 = 3000;
    while self.pos < self.content.len() {
      let sync_match = match RE_SYNC_TAG.captures(&self.content[self.pos..]) {
        Some(m) => m,
        None => break,
      };
      let full_match = sync_match.get(0).unwrap();
      let start_ms: u64 = match sync_match[1].parse() {
        Ok(v) => v,
        Err(e) => {
          // 解析失败时跳过本 Sync 标签，避免死循环
          self.pos += full_match.end();
          return Some(Err(
            SubtitleError::InvalidTimestamp {
              format: Format::Sami,
              value: e.to_string(),
            }
            .into(),
          ));
        }
      };

      // 找到本 Sync 块的闭合标签
      let sync_end_rel = match find_ci(&self.content[self.pos..], "</Sync>") {
        Some(off) => off,
        None => break,
      };
      let sync_block = &self.content[self.pos + full_match.start()..self.pos + sync_end_rel + 7];

      // 无论是否产出字幕，都推进到闭合标签之后，避免重复处理
      self.pos += sync_end_rel + 7;

      // 提取 P 内容
      if let Some(p_match) = RE_P_TAG.captures(sync_block) {
        let p_content = &p_match[1];
        let text = RE_STRIP_TAGS.replace_all(p_content, "").to_string();
        let text = text.trim().to_string();

        if !text.is_empty() {
          return Some(Ok(Subtitle::new(
            start_ms,
            start_ms + DEFAULT_TAIL_MS,
            &text,
          )));
        }
      }
    }
    None
  }
}

impl<'a> crate::model::StreamingParser for SamiStream<'a> {}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_parse_basic() {
    let content = r#"<SAMI>
<Head><Title>Test</Title></Head>
<Body>
<Sync Start=1000><P>First subtitle</P></Sync>
<Sync Start=4000><P>Second subtitle</P></Sync>
</Body>
</SAMI>"#;

    let file = parse_content(content).unwrap();
    if let SubtitleFile::Sami(data) = file {
      assert_eq!(data.subtitles.len(), 2);
      assert_eq!(data.subtitles[0].start, 1000);
      assert_eq!(data.subtitles[0].text, "First subtitle");
      assert_eq!(data.subtitles[1].start, 4000);
    } else {
      panic!("Expected Sami variant");
    }
  }

  #[test]
  fn test_round_trip() {
    let content = r#"<SAMI>
<Body>
<Sync Start=1000><P>Hello</P></Sync>
</Body>
</SAMI>"#;

    let file = parse_content(content).unwrap();
    let subs = match &file {
      SubtitleFile::Sami(data) => &data.subtitles,
      _ => panic!("Expected Sami variant"),
    };
    let output = to_string(subs, None);
    assert!(output.contains("Hello"));
  }

  #[test]
  fn test_detect() {
    assert!(detect_format(b"<SAMI><Body></Body></SAMI>").is_some());
    assert!(detect_format(b"WEBVTT").is_none());
  }

  #[test]
  fn test_end_time_uses_next_sync() {
    // end_ms 应取下一个 Sync 的 start，而非硬编码 start + 3000
    let content = r#"<SAMI>
<Body>
<Sync Start=1000><P>First</P></Sync>
<Sync Start=5000><P>Second</P></Sync>
<Sync Start=20000><P>Third</P></Sync>
</Body>
</SAMI>"#;
    let file = parse_content(content).unwrap();
    if let SubtitleFile::Sami(data) = file {
      assert_eq!(data.subtitles[0].end, 5000, "第一条 end 应为下一条的 start");
      assert_eq!(
        data.subtitles[1].end, 20000,
        "第二条 end 应为第三条的 start"
      );
      // 最后一条无后续，应使用默认尾段 3000ms
      assert_eq!(data.subtitles[2].end, 23000, "最后一条 end = start + 3000");
    } else {
      panic!("Expected Sami variant");
    }
  }

  #[test]
  fn test_parse_case_insensitive_tags() {
    // 真实 SAMI 文件常见大写/混合大小写标签
    let content = r#"<SAMI>
<HEAD><Title>Test</Title></HEAD>
<BODY>
<SYNC START=1000><P>First</P></SYNC>
<SYNC START=4000><P>Second</P></SYNC>
</BODY>
</SAMI>"#;
    let file = parse_content(content).unwrap();
    if let SubtitleFile::Sami(data) = file {
      assert_eq!(
        data.subtitles.len(),
        2,
        "大小写变体的 Sync/P 标签必须被识别"
      );
      assert_eq!(data.subtitles[0].start, 1000);
      assert_eq!(data.subtitles[0].text, "First");
      assert_eq!(data.subtitles[1].start, 4000);
      assert!(data.header.is_some(), "大小写变体的 <HEAD> 必须被识别");
    } else {
      panic!("Expected Sami variant");
    }
  }
}
