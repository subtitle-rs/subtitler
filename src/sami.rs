//! SAMI (Synchronized Accessible Media Interchange) parser and generator.
//!
//! Microsoft-developed subtitle format used in Windows Media Player and
//! widely adopted in Asian markets. Supports styling and multi-language subtitles.
//!
//! Format: HTML-like structure with `<Sync>` and `<P>` tags.

use crate::model::{Subtitle, SubtitleFile};
use crate::types::AnyResult;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::LazyLock;

static RE_SYNC_TAG: LazyLock<Regex> =
  LazyLock::new(|| Regex::new(r"<Sync[^>]*Start\s*=\s*(\d+)[^>]*>").unwrap());

static RE_P_TAG: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"<P[^>]*>(.*?)</P>").unwrap());

static RE_STRIP_TAGS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"<[^>]+>").unwrap());

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
  pub fn parse(content: &str) -> AnyResult<Self> {
    let mut header = None;
    let mut styles = HashMap::new();
    let mut subtitles = Vec::new();

    // Extract header (Head section)
    if let Some(head_start) = content.find("<Head>") {
      if let Some(head_end) = content.find("</Head>") {
        header = Some(content[head_start..head_end + 7].to_string());
      }
    }

    // Extract styles (Style section)
    if let Some(style_start) = content.find("<Style") {
      if let Some(style_end) = content.find("</Style>") {
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

    // Parse Sync tags
    let mut pos = 0;
    while let Some(sync_match) = RE_SYNC_TAG.captures(&content[pos..]) {
      let full_match = sync_match.get(0).unwrap();
      let start_ms: u64 = sync_match[1].parse().unwrap_or(0);

      // Find the end of this Sync block
      let sync_end = content[pos..].find("</Sync>");
      if sync_end.is_none() {
        break;
      }

      let sync_block = &content[pos + full_match.start()..pos + sync_end.unwrap() + 7];

      // Extract P tag content
      for p_match in RE_P_TAG.captures_iter(sync_block) {
        let p_content = &p_match[1];
        let text = RE_STRIP_TAGS.replace_all(p_content, "").to_string();
        let text = text.trim().to_string();

        if !text.is_empty() {
          // Calculate end time (use next Sync's start time or add 3s default)
          let end_ms = start_ms + 3000; // Default 3s duration

          subtitles.push(Subtitle::new(start_ms, end_ms, &text));
          break; // Only take first P tag per Sync
        }
      }

      pos += sync_end.unwrap() + 7;
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
  #[allow(clippy::inherent_to_string)]
  pub fn to_string(&self) -> String {
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
pub fn parse_content(content: &str) -> AnyResult<SubtitleFile> {
  let data = SamiData::parse(content)?;
  Ok(SubtitleFile::Sami(data))
}

/// Parse SAMI from a byte slice.
pub fn parse_bytes(data: &[u8]) -> AnyResult<SubtitleFile> {
  let text = crate::encoding::decode_to_string(data)?;
  parse_content(&text)
}

/// Parse a SAMI file asynchronously.
pub async fn parse_file(path: impl AsRef<std::path::Path>) -> AnyResult<SubtitleFile> {
  let text = tokio::fs::read_to_string(path).await?;
  parse_content(&text)
}

/// Parse a SAMI file from a URL (requires `http` feature).
#[cfg(feature = "http")]
pub async fn parse_url(url: &str) -> AnyResult<SubtitleFile> {
  let response = reqwest::get(url).await?;
  let content = response.text().await?;
  parse_content(&content)
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
  data.to_string()
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
    while self.pos < self.content.len() {
      if let Some(sync_match) = RE_SYNC_TAG.captures(&self.content[self.pos..]) {
        let full_match = sync_match.get(0).unwrap();
        let start_ms: u64 = match sync_match[1].parse() {
          Ok(v) => v,
          Err(e) => {
            self.pos += full_match.end();
            return Some(Err(anyhow::anyhow!("Invalid start time: {}", e)));
          }
        };

        // Find Sync block end
        if let Some(sync_end_rel) = self.content[self.pos..].find("</Sync>") {
          let sync_block =
            &self.content[self.pos + full_match.start()..self.pos + sync_end_rel + 7];

          // Extract P content
          if let Some(p_match) = RE_P_TAG.captures(sync_block) {
            let p_content = &p_match[1];
            let text = RE_STRIP_TAGS.replace_all(p_content, "").to_string();
            let text = text.trim().to_string();

            if !text.is_empty() {
              self.pos += sync_end_rel + 7;
              let end_ms = start_ms + 3000;
              return Some(Ok(Subtitle::new(start_ms, end_ms, &text)));
            }
          }

          self.pos += sync_end_rel + 7;
        } else {
          break;
        }
      } else {
        break;
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
}
