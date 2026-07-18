//! MPL2 (MPlayer) subtitle format parser and generator.
//!
//! Frame-based format popular in Eastern Europe.
//! Format: `[start_frame][end_frame]text`
//!
//! Uses frame numbers instead of timestamps. Frame rate defaults to 23.976 fps.

use crate::error::SubtitleError;
use crate::model::{Format, Subtitle, SubtitleFile};
use crate::types::AnyResult;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::sync::LazyLock;

static RE_MPL2_LINE: LazyLock<Regex> =
  LazyLock::new(|| Regex::new(r"^\[(\d+)\]\[(\d+)\](.+)$").unwrap());

/// Default frame rate for MPL2 format (23.976 fps).
pub const DEFAULT_FPS: f64 = 23.976;

/// MPL2 subtitle data with frame rate information.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Mpl2Data {
  /// Frame rate for converting frames to milliseconds.
  pub fps: f64,
  /// Subtitle entries.
  pub subtitles: Vec<Subtitle>,
}

impl Mpl2Data {
  /// Create a new Mpl2Data with the given frame rate.
  pub fn new(fps: f64) -> Self {
    Mpl2Data {
      fps,
      subtitles: Vec::new(),
    }
  }

  /// Parse MPL2 content into structured data.
  pub fn parse(content: &str, fps: Option<f64>) -> Result<Self, SubtitleError> {
    let fps = fps.unwrap_or(DEFAULT_FPS);
    let mut subtitles: Vec<Subtitle> = Vec::with_capacity((content.len() / 30).max(16));

    for line in content.lines() {
      let trimmed = line.trim();
      if trimmed.is_empty() {
        continue;
      }

      if let Some(caps) = RE_MPL2_LINE.captures(trimmed) {
        let start_frame: u64 = caps[1].parse().unwrap_or(0);
        let end_frame: u64 = caps[2].parse().unwrap_or(0);
        let text = caps[3].trim().to_string();

        if !text.is_empty() {
          // Convert frames to milliseconds
          let start_ms = frame_to_ms(start_frame, fps);
          let end_ms = frame_to_ms(end_frame, fps);

          subtitles.push(Subtitle::new(start_ms, end_ms, &text));
        }
      }
    }

    Ok(Mpl2Data { fps, subtitles })
  }

  /// Convert to Vec<Subtitle> for compatibility.
  pub fn to_subtitles(&self) -> Vec<Subtitle> {
    self.subtitles.clone()
  }

  /// Serialize back to MPL2 format.
  pub fn render(&self) -> String {
    let mut buf = String::new();

    for sub in &self.subtitles {
      let start_frame = ms_to_frame(sub.start, self.fps);
      let end_frame = ms_to_frame(sub.end, self.fps);
      buf.push_str(&format!("[{}][{}]{}\n", start_frame, end_frame, sub.text));
    }

    buf
  }
}

/// Convert frame number to milliseconds.
fn frame_to_ms(frame: u64, fps: f64) -> u64 {
  let seconds = frame as f64 / fps;
  (seconds * 1000.0).round() as u64
}

/// Convert milliseconds to frame number.
fn ms_to_frame(ms: u64, fps: f64) -> u64 {
  let seconds = ms as f64 / 1000.0;
  (seconds * fps).round() as u64
}

/// Parse MPL2 content into a SubtitleFile.
pub fn parse_content(content: &str) -> Result<SubtitleFile, SubtitleError> {
  let data = Mpl2Data::parse(content, None)?;
  Ok(SubtitleFile::Mpl2(data.subtitles))
}

/// Parse MPL2 from a byte slice.
pub fn parse_bytes(data: &[u8]) -> AnyResult<SubtitleFile> {
  let text = crate::encoding::decode_to_string(data)?;
  Ok(parse_content(&text)?)
}

#[cfg(not(target_arch = "wasm32"))]
pub async fn parse_file(path: impl AsRef<std::path::Path>) -> AnyResult<SubtitleFile> {
  let text = tokio::fs::read_to_string(path).await?;
  Ok(parse_content(&text)?)
}

#[cfg(feature = "http")]
pub async fn parse_url(url: &str) -> AnyResult<SubtitleFile> {
  let response = reqwest::get(url).await?;
  let content = response.text().await?;
  Ok(parse_content(&content)?)
}

/// Detect if data looks like MPL2.
pub fn detect_format(data: &[u8]) -> Option<crate::model::Format> {
  let text = crate::encoding::try_decode_for_detection(data)?;
  let has_mpl2 = text.lines().any(|l| {
    let t = l.trim();
    t.starts_with('[') && RE_MPL2_LINE.is_match(t)
  });
  if has_mpl2 {
    return Some(crate::model::Format::Mpl2);
  }
  None
}

/// Write subtitles to a file in MPL2 format.
///
/// `policy` controls overwrite behavior (None = default Overwrite).
/// Uses the default fps (`DEFAULT_FPS`); for a custom fps, call
/// `to_string` directly and write the result with `tokio::fs::write`.
#[cfg(not(target_arch = "wasm32"))]
pub async fn generate(
  subtitles: &[Subtitle],
  file_path: impl AsRef<std::path::Path>,
  policy: Option<crate::model::WritePolicy>,
) -> AnyResult<String> {
  let content = to_string(subtitles, None);
  let path = file_path.as_ref();
  crate::io::write_with_policy(path, content.as_bytes(), policy).await?;
  Ok(path.to_string_lossy().into_owned())
}

/// Serialize subtitles to MPL2 format.
pub fn to_string(subtitles: &[Subtitle], fps: Option<f64>) -> String {
  let data = Mpl2Data {
    fps: fps.unwrap_or(DEFAULT_FPS),
    subtitles: subtitles.to_vec(),
  };
  data.render()
}

/// Streaming parser entry point — yields subtitles one at a time
/// without allocating a full `Vec`. Uses the default fps (`DEFAULT_FPS`);
/// for a custom fps, construct `Mpl2Stream::new(content, Some(fps))` directly.
pub fn parse_stream<'a>(content: &'a str) -> Mpl2Stream<'a> {
  Mpl2Stream::new(content, None)
}

pub struct Mpl2Stream<'a> {
  lines: std::str::Lines<'a>,
  fps: f64,
}

impl<'a> Mpl2Stream<'a> {
  pub fn new(content: &'a str, fps: Option<f64>) -> Self {
    Mpl2Stream {
      lines: content.lines(),
      fps: fps.unwrap_or(DEFAULT_FPS),
    }
  }
}

impl<'a> Iterator for Mpl2Stream<'a> {
  type Item = AnyResult<Subtitle>;

  fn next(&mut self) -> Option<Self::Item> {
    for line in self.lines.by_ref() {
      let trimmed = line.trim();
      if trimmed.is_empty() {
        continue;
      }

      if let Some(caps) = RE_MPL2_LINE.captures(trimmed) {
        let start_frame: u64 = match caps[1].parse() {
          Ok(v) => v,
          Err(e) => {
            return Some(Err(
              SubtitleError::InvalidFrame {
                format: Format::Mpl2,
                role: "start",
                value: e.to_string(),
              }
              .into(),
            ));
          }
        };

        let end_frame: u64 = match caps[2].parse() {
          Ok(v) => v,
          Err(e) => {
            return Some(Err(
              SubtitleError::InvalidFrame {
                format: Format::Mpl2,
                role: "end",
                value: e.to_string(),
              }
              .into(),
            ));
          }
        };

        let text = caps[3].trim().to_string();
        if !text.is_empty() {
          let start_ms = frame_to_ms(start_frame, self.fps);
          let end_ms = frame_to_ms(end_frame, self.fps);
          return Some(Ok(Subtitle::new(start_ms, end_ms, &text)));
        }
      }
    }
    None
  }
}

impl<'a> crate::model::StreamingParser for Mpl2Stream<'a> {}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::model::SubtitleFormat;

  #[test]
  fn test_parse_basic() {
    let content = "[100][200]First subtitle\n[300][400]Second subtitle\n";
    let subs = parse_bytes(content.as_bytes()).unwrap();
    assert_eq!(subs.subtitles().len(), 2);
    assert_eq!(subs.subtitles()[0].text, "First subtitle");
    assert_eq!(subs.subtitles()[1].text, "Second subtitle");
  }

  #[test]
  fn test_frame_conversion() {
    // Test at 23.976 fps
    let fps = 23.976;
    let frame = 240; // 10 seconds
    let ms = frame_to_ms(frame, fps);
    let back = ms_to_frame(ms, fps);
    assert_eq!(frame, back);
  }

  #[test]
  fn test_round_trip() {
    let content = "[100][200]Hello\n";
    let subs = parse_bytes(content.as_bytes()).unwrap();
    let output = to_string(subs.subtitles(), None);
    assert!(output.contains("Hello"));
  }

  #[test]
  fn test_detect() {
    assert!(detect_format(b"[100][200]test").is_some());
    assert!(detect_format(b"WEBVTT").is_none());
  }
}
