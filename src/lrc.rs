//! LRC (Lyrics) format parser and generator.
//!
//! Lines: `[mm:ss.xx]lyric text`  or  `[ti:Title]` for metadata.
//! Multiple timestamps can share a line: `[00:01.50][00:15.00]text`

use crate::error::SubtitleError;
use crate::model::{Subtitle, SubtitleFile};
use crate::types::AnyResult;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::sync::LazyLock;
use tokio::io::AsyncWriteExt;

static RE_LRC_LINE: LazyLock<Regex> =
  LazyLock::new(|| Regex::new(r"\[(\d{1,3}):(\d{1,2})\.(\d{1,3})\]").unwrap());

fn lrc_time_to_ms(m: &str, s: &str, cs: &str) -> u64 {
  let minutes: u64 = m.parse().unwrap_or(0);
  let seconds: u64 = s.parse().unwrap_or(0);
  let centiseconds: u64 = cs.parse().unwrap_or(0);
  minutes * 60000 + seconds * 1000 + centiseconds * 10
}

/// LRC lyrics data, preserving multi-timestamp lines.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct LrcData {
  /// One entry per lyric line; each line may have multiple timestamps.
  pub lines: Vec<LrcLine>,
}

/// A single LRC lyric line with all its timestamps and text.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct LrcLine {
  /// All timestamps (ms) at which this text is sung.
  pub times_ms: Vec<u64>,
  /// The lyric text.
  pub text: String,
}

impl LrcData {
  /// Parse LRC content into structured data, preserving multi-timestamp lines.
  pub fn parse(content: &str) -> Result<Self, SubtitleError> {
    let mut lines: Vec<LrcLine> = Vec::new();

    for line in content.lines() {
      let trimmed = line.trim();
      if trimmed.is_empty() {
        continue;
      }

      let mut times = Vec::new();
      let mut last_end = 0usize;
      for caps in RE_LRC_LINE.captures_iter(trimmed) {
        let m = caps.get(0).unwrap();
        times.push(lrc_time_to_ms(&caps[1], &caps[2], &caps[3]));
        last_end = m.end();
      }

      if times.is_empty() {
        continue; // metadata line
      }

      let text = trimmed[last_end..].trim();
      if text.is_empty() {
        continue;
      }

      lines.push(LrcLine {
        times_ms: times,
        text: text.to_string(),
      });
    }

    Ok(LrcData { lines })
  }

  /// Serialize back to LRC string format.
  #[allow(clippy::inherent_to_string)]
  pub fn to_string(&self) -> String {
    let mut buf = String::new();
    for line in &self.lines {
      for &t in &line.times_ms {
        let total_seconds = t / 1000;
        let minutes = total_seconds / 60;
        let seconds = total_seconds % 60;
        let cs = (t % 1000) / 10;
        buf.push_str(&format!("[{:02}:{:02}.{:02}]", minutes, seconds, cs));
      }
      buf.push_str(&line.text);
      buf.push('\n');
    }
    buf
  }

  /// Convert to `Vec<Subtitle>` (compatibility with the deprecated `parse_content`).
  /// Each timestamp becomes a separate subtitle with a 5-second default display duration.
  pub fn to_subtitles(&self) -> Vec<Subtitle> {
    let mut subs = Vec::new();
    for line in &self.lines {
      for &t in &line.times_ms {
        subs.push(Subtitle::new(t, t + 5000, &line.text));
      }
    }
    subs.sort_by_key(|s| s.start);
    subs
  }
}

/// Parse LRC content into a `SubtitleFile`.
pub fn parse_content(content: &str) -> Result<SubtitleFile, SubtitleError> {
  let data = LrcData::parse(content)?;
  let subtitles = data.to_subtitles();
  Ok(SubtitleFile::Lrc { data, subtitles })
}

/// Parse LRC from a byte slice.
pub fn parse_bytes(data: &[u8]) -> AnyResult<SubtitleFile> {
  let text = crate::encoding::decode_to_string(data)?;
  Ok(parse_content(&text)?)
}

/// Parse an LRC file asynchronously.
pub async fn parse_file(path: impl AsRef<std::path::Path>) -> AnyResult<SubtitleFile> {
  let text = tokio::fs::read_to_string(path).await?;
  Ok(parse_content(&text)?)
}

/// Parse an LRC file from a URL (requires `http` feature).
#[cfg(feature = "http")]
pub async fn parse_url(url: &str) -> AnyResult<SubtitleFile> {
  let response = reqwest::get(url).await?;
  let content = response.text().await?;
  Ok(parse_content(&content)?)
}

/// Detect if data looks like LRC.
pub fn detect_format(data: &[u8]) -> Option<crate::model::Format> {
  let text = crate::encoding::try_decode_for_detection(data)?;
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

pub struct LrcStream<'a> {
  lines: std::str::Lines<'a>,
  pending_subs: VecDeque<AnyResult<Subtitle>>,
}
impl<'a> LrcStream<'a> {
  pub fn new(content: &'a str) -> Self {
    LrcStream {
      lines: content.lines(),
      pending_subs: VecDeque::new(),
    }
  }
}
impl<'a> Iterator for LrcStream<'a> {
  type Item = AnyResult<Subtitle>;
  fn next(&mut self) -> Option<Self::Item> {
    // Drain pending subs from multi-timestamp lines first
    if let Some(sub) = self.pending_subs.pop_front() {
      return Some(sub);
    }
    for line in self.lines.by_ref() {
      let trimmed = line.trim();
      if trimmed.is_empty() {
        continue;
      }
      let mut times = Vec::new();
      let mut last_end = 0usize;
      for caps in RE_LRC_LINE.captures_iter(trimmed) {
        let m = caps.get(0).unwrap();
        times.push(lrc_time_to_ms(&caps[1], &caps[2], &caps[3]));
        last_end = m.end();
      }
      if times.is_empty() || last_end >= trimmed.len() {
        continue;
      }
      let text = trimmed[last_end..].trim().to_string();
      if text.is_empty() {
        continue;
      }
      // Queue all timestamps as separate subtitles
      for &t in &times {
        self
          .pending_subs
          .push_back(Ok(Subtitle::new(t, t + 5000, &text)));
      }
      return self.pending_subs.pop_front();
    }
    None
  }
}

impl<'a> crate::model::StreamingParser for LrcStream<'a> {}

/// Write LRC lyrics to an async writer streamingly.
pub async fn write_stream<W: tokio::io::AsyncWrite + Unpin>(
  subtitles: &[Subtitle],
  writer: &mut W,
) -> AnyResult<()> {
  for sub in subtitles {
    let total_seconds = sub.start / 1000;
    let minutes = total_seconds / 60;
    let seconds = total_seconds % 60;
    let cs = (sub.start % 1000) / 10;
    writer
      .write_all(format!("[{:02}:{:02}.{:02}]{}\n", minutes, seconds, cs, sub.text).as_bytes())
      .await?;
  }
  writer.flush().await?;
  Ok(())
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::model::SubtitleFormat;

  #[test]
  fn test_parse_basic() {
    let content = "[00:01.50]Hello\n[00:03.20]World\n";
    let file = parse_content(content).unwrap();
    let subs = file.subtitles();
    assert_eq!(subs.len(), 2);
    assert_eq!(subs[0].start, 1500);
    assert_eq!(subs[0].text, "Hello");
    assert_eq!(subs[1].start, 3200);
  }

  #[test]
  fn test_parse_multi_timestamp() {
    let content = "[00:10.00][00:30.00]Repeated\n";
    let file = parse_content(content).unwrap();
    let subs = file.subtitles();
    assert_eq!(subs.len(), 2);
    assert_eq!(subs[0].start, 10000);
    assert_eq!(subs[1].start, 30000);
  }

  #[test]
  fn test_round_trip() {
    let content = "[00:01.50]Hello\n[00:03.20]World\n";
    let file = parse_content(content).unwrap();
    let subs = file.subtitles();
    let output = to_string(&subs);
    assert!(output.contains("Hello"));
    let reparsed = parse_content(&output).unwrap();
    assert_eq!(subs.len(), reparsed.subtitles().len());
  }

  #[test]
  fn test_lrc_data_preserves_multi_timestamp() {
    let content = "[00:10.00][00:30.00]Repeated line\n";
    let data = LrcData::parse(content).unwrap();
    assert_eq!(data.lines.len(), 1);
    assert_eq!(data.lines[0].times_ms.len(), 2);
    assert_eq!(data.lines[0].times_ms[0], 10000);
    assert_eq!(data.lines[0].times_ms[1], 30000);
    // Round-trip
    let output = data.to_string();
    assert!(
      output.contains("[00:10.00]"),
      "missing first timestamp in:\n{output}"
    );
    assert!(
      output.contains("[00:30.00]"),
      "missing second timestamp:\n{output}"
    );
  }

  #[test]
  fn test_detect() {
    assert!(detect_format(b"[00:01.50]test").is_some());
    assert!(detect_format(b"WEBVTT").is_none());
  }
}
