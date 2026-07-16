//! LRC (Lyrics) format parser and generator.
//!
//! Lines: `[mm:ss.xx]lyric text`  or  `[ti:Title]` for metadata.
//! Multiple timestamps can share a line: `[00:01.50][00:15.00]text`

use crate::model::Subtitle;
use crate::types::AnyResult;
use anyhow::anyhow;
use regex::Regex;
use std::sync::LazyLock;

static RE_LRC_LINE: LazyLock<Regex> =
  LazyLock::new(|| Regex::new(r"\[(\d{1,3}):(\d{1,2})\.(\d{1,3})\]").unwrap());

fn lrc_time_to_ms(m: &str, s: &str, cs: &str) -> u64 {
  let minutes: u64 = m.parse().unwrap_or(0);
  let seconds: u64 = s.parse().unwrap_or(0);
  let centiseconds: u64 = cs.parse().unwrap_or(0);
  minutes * 60000 + seconds * 1000 + centiseconds * 10
}

/// Parse LRC content into a vector of subtitles.
pub fn parse_content(content: &str) -> AnyResult<Vec<Subtitle>> {
  let mut subtitles = Vec::new();

  for line in content.lines() {
    let trimmed = line.trim();
    if trimmed.is_empty() {
      continue;
    }

    // Collect all timestamps and text
    let mut last_end = 0usize;
    let mut times = Vec::new();
    for caps in RE_LRC_LINE.captures_iter(trimmed) {
      let m = caps.get(0).unwrap();
      times.push((
        lrc_time_to_ms(&caps[1], &caps[2], &caps[3]),
        m.start(),
        m.end(),
      ));
      last_end = m.end();
    }

    if times.is_empty() {
      continue; // metadata line: [ti:Title] etc.
    }

    let text = trimmed[last_end..].trim();
    if text.is_empty() {
      continue;
    }

    for (time, _, _) in times {
      // LRC cues have no explicit end; use a default display duration of 5s
      subtitles.push(Subtitle::new(time, time + 5000, text));
    }
  }

  // Sort by start time
  subtitles.sort_by_key(|s| s.start);
  Ok(subtitles)
}

/// Parse LRC from a byte slice.
pub fn parse_bytes(data: &[u8]) -> AnyResult<Vec<Subtitle>> {
  let text = String::from_utf8(data.to_vec()).map_err(|e| anyhow!("Invalid UTF-8: {}", e))?;
  parse_content(&text)
}

/// Parse an LRC file asynchronously.
pub async fn parse_file(path: impl AsRef<std::path::Path>) -> AnyResult<Vec<Subtitle>> {
  let text = tokio::fs::read_to_string(path).await?;
  parse_content(&text)
}

/// Parse an LRC file from a URL (requires `http` feature).
#[cfg(feature = "http")]
pub async fn parse_url(url: &str) -> AnyResult<Vec<Subtitle>> {
  let response = reqwest::get(url).await?;
  let content = response.text().await?;
  parse_content(&content)
}

/// Detect if data looks like LRC.
pub fn detect_format(data: &[u8]) -> Option<crate::model::Format> {
  let text = std::str::from_utf8(data).ok()?;
  let line_count = text.lines().count();
  // LRC files have bracket timestamps and are typically <500 lines
  let has_lrc = text.lines().any(|l| RE_LRC_LINE.is_match(l.trim()));
  if has_lrc && line_count < 500 {
    return Some(crate::model::Format::Lrc);
  }
  None
}

/// Serialize subtitles to LRC format (using a default 5s duration display).
pub fn to_string(subtitles: &[Subtitle]) -> String {
  let mut buf = String::new();
  for sub in subtitles {
    let total_seconds = sub.start / 1000;
    let minutes = total_seconds / 60;
    let seconds = total_seconds % 60;
    let cs = (sub.start % 1000) / 10;
    buf.push_str(&format!(
      "[{:02}:{:02}.{:02}]{}\n",
      minutes, seconds, cs, sub.text
    ));
  }
  buf
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_parse_basic() {
    let content = "[00:01.50]Hello\n[00:03.20]World\n";
    let subs = parse_content(content).unwrap();
    assert_eq!(subs.len(), 2);
    assert_eq!(subs[0].start, 1500);
    assert_eq!(subs[0].end, 6500); // default 5s display duration
    assert_eq!(subs[0].text, "Hello");
    assert_eq!(subs[1].start, 3200);
  }

  #[test]
  fn test_parse_multi_timestamp() {
    let content = "[00:10.00][00:30.00]Repeated\n";
    let subs = parse_content(content).unwrap();
    // Should produce two subtitles (two timestamps, one text)
    assert_eq!(subs.len(), 2);
    assert_eq!(subs[0].start, 10000);
    assert_eq!(subs[1].start, 30000);
  }

  #[test]
  fn test_round_trip() {
    let content = "[00:01.50]Hello\n[00:03.20]World\n";
    let subs = parse_content(content).unwrap();
    let output = to_string(&subs);
    assert!(output.contains("Hello"));
    let reparsed = parse_content(&output).unwrap();
    assert_eq!(subs.len(), reparsed.len());
  }

  #[test]
  fn test_detect() {
    assert!(detect_format(b"[00:01.50]test").is_some());
    assert!(detect_format(b"WEBVTT").is_none());
  }
}
