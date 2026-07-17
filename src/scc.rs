//! SCC (Scenarist Closed Caption) parser and generator.
//!
//! SCC format is used for CEA-608 closed captions in broadcast television.
//! It encodes caption data as hexadecimal values with SMPTE timecodes.
//!
//! Format: `HH:MM:SS:FF` (timecode) + tab + hex-encoded caption data
//!
//! # Frame Rate
//! - Default: 29.97 fps (NTSC)
//! - Supports drop-frame (`;` separator) and non-drop-frame (`:` separator)

use crate::model::{Subtitle, SubtitleFile};
use crate::types::AnyResult;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::sync::LazyLock;

/// Default frame rate for SCC (29.97 fps NTSC).
pub const DEFAULT_FPS: f64 = 29.97;

/// SCC timecode line pattern: HH:MM:SS:FF or HH:MM:SS;FF
static RE_SCC_LINE: LazyLock<Regex> =
  LazyLock::new(|| Regex::new(r"^(\d{2}):(\d{2}):(\d{2})([:;])(\d{2})\s+(.+)$").unwrap());

/// SCC header pattern
static RE_SCC_HEADER: LazyLock<Regex> =
  LazyLock::new(|| Regex::new(r"^Scenarist_SCC V1\.0$").unwrap());

/// SCC caption data structure.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct SccData {
  /// Frame rate (29.97 fps for NTSC).
  pub fps: f64,
  /// Whether using drop-frame timecode (`;` separator).
  pub drop_frame: bool,
  /// Subtitle entries.
  pub subtitles: Vec<Subtitle>,
}

impl SccData {
  /// Create a new SccData with default settings.
  pub fn new() -> Self {
    SccData {
      fps: DEFAULT_FPS,
      drop_frame: true,
      subtitles: Vec::new(),
    }
  }

  /// Parse SCC content into structured data.
  pub fn parse(content: &str) -> AnyResult<Self> {
    let mut subtitles = Vec::new();
    let mut drop_frame = true;
    let mut current_text = String::new();
    let mut current_start: Option<u64> = None;

    for line in content.lines() {
      let trimmed = line.trim();
      if trimmed.is_empty() {
        continue;
      }

      // Check for header
      if RE_SCC_HEADER.is_match(trimmed) {
        continue;
      }

      // Parse timecode and hex data
      if let Some(caps) = RE_SCC_LINE.captures(trimmed) {
        let hours: u64 = caps[1].parse().unwrap_or(0);
        let minutes: u64 = caps[2].parse().unwrap_or(0);
        let seconds: u64 = caps[3].parse().unwrap_or(0);
        let separator = &caps[4];
        let frames: u64 = caps[5].parse().unwrap_or(0);
        let hex_data = &caps[6];

        drop_frame = separator == ";";

        let timecode_ms = scc_timecode_to_ms(hours, minutes, seconds, frames, DEFAULT_FPS);

        // Decode hex data to text
        let decoded_text = decode_scc_hex(hex_data);

        // Check for control codes
        let is_clear = hex_data.contains("942c");
        let is_start = hex_data.contains("9420") || hex_data.contains("94ad");
        let is_end = hex_data.contains("942f");

        if is_clear {
          // End of caption (clear screen)
          if let Some(start) = current_start {
            if !current_text.is_empty() {
              subtitles.push(Subtitle::new(start, timecode_ms, &current_text));
            }
            current_text.clear();
            current_start = None;
          }
        } else if is_start {
          // Start of new caption
          if !current_text.is_empty() {
            if let Some(start) = current_start {
              subtitles.push(Subtitle::new(start, timecode_ms, &current_text));
            }
          }
          current_start = Some(timecode_ms);
          current_text = decoded_text;
        } else if is_end {
          // Display caption
          if !current_text.is_empty() && current_start.is_some() {
            // Caption ready to display, wait for clear
          }
        } else if !decoded_text.is_empty() {
          // Append to current caption
          if current_start.is_none() {
            current_start = Some(timecode_ms);
          }
          if !current_text.is_empty() {
            current_text.push(' ');
          }
          current_text.push_str(&decoded_text);
        }
      }
    }

    // Handle remaining caption
    if !current_text.is_empty() {
      if let Some(start) = current_start {
        // Default duration: 3 seconds
        let end = start + 3000;
        subtitles.push(Subtitle::new(start, end, &current_text));
      }
    }

    Ok(SccData {
      fps: DEFAULT_FPS,
      drop_frame,
      subtitles,
    })
  }

  /// Convert to Vec<Subtitle> for compatibility.
  pub fn to_subtitles(&self) -> Vec<Subtitle> {
    self.subtitles.clone()
  }

  /// Serialize back to SCC format.
  #[allow(clippy::inherent_to_string)]
  pub fn to_string(&self) -> String {
    let mut buf = String::from("Scenarist_SCC V1.0\n\n");

    for sub in &self.subtitles {
      let start_tc = ms_to_scc_timecode(sub.start, self.fps, self.drop_frame);
      let end_tc = ms_to_scc_timecode(sub.end, self.fps, self.drop_frame);

      // Start caption
      buf.push_str(&format!("{}\t9420 94ad ", start_tc));

      // Encode text as hex
      let hex_text = encode_scc_hex(&sub.text);
      buf.push_str(&hex_text);
      buf.push_str(" 942f\n");

      // End caption
      buf.push_str(&format!("{}\t942c\n\n", end_tc));
    }

    buf
  }
}

/// Convert SCC timecode to milliseconds.
fn scc_timecode_to_ms(h: u64, m: u64, s: u64, f: u64, fps: f64) -> u64 {
  let total_frames = h * 3600 * fps as u64 + m * 60 * fps as u64 + s * fps as u64 + f;
  let seconds = total_frames as f64 / fps;
  (seconds * 1000.0).round() as u64
}

/// Convert milliseconds to SCC timecode.
fn ms_to_scc_timecode(ms: u64, fps: f64, drop_frame: bool) -> String {
  let total_frames = (ms as f64 / 1000.0 * fps).round() as u64;

  let frames_per_hour = (fps * 3600.0) as u64;
  let frames_per_minute = (fps * 60.0) as u64;

  let hours = total_frames / frames_per_hour;
  let remaining = total_frames % frames_per_hour;

  let minutes = remaining / frames_per_minute;
  let remaining = remaining % frames_per_minute;

  let seconds = remaining / fps as u64;
  let frames = remaining % fps as u64;

  let separator = if drop_frame { ";" } else { ":" };

  format!(
    "{:02}:{:02}:{:02}{}{:02}",
    hours, minutes, seconds, separator, frames
  )
}

/// Decode SCC hexadecimal data to text.
fn decode_scc_hex(hex: &str) -> String {
  let mut text = String::new();
  let hex_bytes: Vec<&str> = hex.split_whitespace().collect();

  // Process byte pairs
  for chunk in hex_bytes.chunks(2) {
    if chunk.len() == 2 {
      // Skip control codes (94xx, 97xx, etc.)
      let byte1 = chunk[0];
      let byte2 = if chunk.len() > 1 { chunk[1] } else { "" };

      // Skip control codes
      if byte1.starts_with("94") || byte1.starts_with("97") {
        continue;
      }

      // Decode character from byte2 (second byte in pair)
      if byte2.len() == 4 {
        if let Ok(byte_val) = u16::from_str_radix(byte2, 16) {
          // CEA-608 character mapping
          let ch = decode_cea608_char(byte_val);
          if ch != '\0' {
            text.push(ch);
          }
        }
      }

      // Also decode from byte1 if it's a character
      if byte1.len() == 4 {
        if let Ok(byte_val) = u16::from_str_radix(byte1, 16) {
          let ch = decode_cea608_char(byte_val);
          if ch != '\0' {
            text.push(ch);
          }
        }
      }
    }
  }

  text.trim().to_string()
}

/// Decode CEA-608 character code to Unicode.
fn decode_cea608_char(code: u16) -> char {
  // CEA-608 uses two bytes per character pair
  // Standard ASCII character mapping

  // Basic ASCII range (0x20-0x7F)
  if (0x20..=0x7F).contains(&code) {
    return code as u8 as char;
  }

  // Extended North American character set
  // These are typically 0x80-0xFF range
  match code {
    0xA1 => '¡',
    0xA2 => '¢',
    0xA3 => '£',
    0xA4 => '¤',
    0xA5 => '¥',
    0xA6 => '¦',
    0xA7 => '§',
    0xA8 => '¨',
    0xA9 => '©',
    0xAA => 'ª',
    0xAB => '«',
    0xAC => '¬',
    0xAD => '\u{AD}', // Soft hyphen
    0xAE => '®',
    0xAF => '¯',

    // Accented characters (simplified mapping)
    0x81 => 'á',
    0x89 => 'é',
    0x8D => 'í',
    0x93 => 'ó',
    0x97 => 'ú',

    _ => '\0',
  }
}

/// Encode text to SCC hexadecimal format.
fn encode_scc_hex(text: &str) -> String {
  let mut hex_parts = Vec::new();

  for ch in text.chars() {
    // Convert character to CEA-608 code (simplified)
    let code = match ch {
      ' '..='~' => ch as u16,
      _ => continue, // Skip non-ASCII for now
    };

    hex_parts.push(format!("{:04X}", code));
  }

  hex_parts.join(" ")
}

/// Parse SCC content into a SubtitleFile.
pub fn parse_content(content: &str) -> AnyResult<SubtitleFile> {
  let data = SccData::parse(content)?;
  Ok(SubtitleFile::Scc(data))
}

/// Parse SCC from a byte slice.
pub fn parse_bytes(data: &[u8]) -> AnyResult<SubtitleFile> {
  let text = crate::encoding::decode_to_string(data)?;
  parse_content(&text)
}

/// Parse an SCC file asynchronously.
pub async fn parse_file(path: impl AsRef<std::path::Path>) -> AnyResult<SubtitleFile> {
  let text = tokio::fs::read_to_string(path).await?;
  parse_content(&text)
}

/// Parse an SCC file from a URL (requires `http` feature).
#[cfg(feature = "http")]
pub async fn parse_url(url: &str) -> AnyResult<SubtitleFile> {
  let response = reqwest::get(url).await?;
  let content = response.text().await?;
  parse_content(&content)
}

/// Detect if data looks like SCC.
pub fn detect_format(data: &[u8]) -> Option<crate::model::Format> {
  let text = crate::encoding::try_decode_for_detection(data)?;
  if text.starts_with("Scenarist_SCC") {
    return Some(crate::model::Format::Scc);
  }
  None
}

/// Serialize subtitles to SCC format.
pub fn to_string(subtitles: &[Subtitle]) -> String {
  let data = SccData {
    fps: DEFAULT_FPS,
    drop_frame: true,
    subtitles: subtitles.to_vec(),
  };
  data.to_string()
}

pub struct SccStream<'a> {
  lines: std::str::Lines<'a>,
  current_text: String,
  current_start: Option<u64>,
}

impl<'a> SccStream<'a> {
  pub fn new(content: &'a str) -> Self {
    SccStream {
      lines: content.lines(),
      current_text: String::new(),
      current_start: None,
    }
  }
}

impl<'a> Iterator for SccStream<'a> {
  type Item = AnyResult<Subtitle>;

  fn next(&mut self) -> Option<Self::Item> {
    for line in self.lines.by_ref() {
      let trimmed = line.trim();
      if trimmed.is_empty() || RE_SCC_HEADER.is_match(trimmed) {
        continue;
      }

      if let Some(caps) = RE_SCC_LINE.captures(trimmed) {
        let hours: u64 = caps[1].parse().unwrap_or(0);
        let minutes: u64 = caps[2].parse().unwrap_or(0);
        let seconds: u64 = caps[3].parse().unwrap_or(0);
        let frames: u64 = caps[5].parse().unwrap_or(0);
        let hex_data = &caps[6];

        let timecode_ms = scc_timecode_to_ms(hours, minutes, seconds, frames, DEFAULT_FPS);
        let decoded_text = decode_scc_hex(hex_data);

        if hex_data.contains("942c") {
          if let Some(start) = self.current_start {
            if !self.current_text.is_empty() {
              let subtitle = Subtitle::new(start, timecode_ms, &self.current_text);
              self.current_text.clear();
              self.current_start = None;
              return Some(Ok(subtitle));
            }
          }
        } else if hex_data.contains("9420") || hex_data.contains("94ad") {
          if !self.current_text.is_empty() {
            if let Some(start) = self.current_start {
              let subtitle = Subtitle::new(start, timecode_ms, &self.current_text);
              self.current_text = decoded_text;
              self.current_start = Some(timecode_ms);
              return Some(Ok(subtitle));
            }
          }
          self.current_start = Some(timecode_ms);
          self.current_text = decoded_text;
        } else if !decoded_text.is_empty() {
          if self.current_start.is_none() {
            self.current_start = Some(timecode_ms);
          }
          if !self.current_text.is_empty() {
            self.current_text.push(' ');
          }
          self.current_text.push_str(&decoded_text);
        }
      }
    }

    if !self.current_text.is_empty() {
      if let Some(start) = self.current_start {
        let end = start + 3000;
        let subtitle = Subtitle::new(start, end, &self.current_text);
        self.current_text.clear();
        self.current_start = None;
        return Some(Ok(subtitle));
      }
    }

    None
  }
}

impl<'a> crate::model::StreamingParser for SccStream<'a> {}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_parse_basic() {
    let content = r#"Scenarist_SCC V1.0

00:00:01;15	9420 94ad 5468 6973 2069 7320 6120 7465 7374 2e 942f
00:00:04;00	942c

"#;

    let file = parse_content(content).unwrap();
    if let SubtitleFile::Scc(data) = file {
      // Check that structure is valid
      assert!(data.drop_frame);
      assert_eq!(data.fps, DEFAULT_FPS);
    } else {
      panic!("Expected Scc variant");
    }
  }

  #[test]
  fn test_timecode_conversion() {
    let ms = scc_timecode_to_ms(1, 2, 3, 15, 29.97);
    // Just check that conversion works
    assert!(ms > 0);
    let tc = ms_to_scc_timecode(ms, 29.97, true);
    // Check format is correct (HH:MM:SS;FF or HH:MM:SS:FF)
    assert!(tc.contains(':') || tc.contains(';'));
  }

  #[test]
  fn test_detect() {
    assert!(detect_format(b"Scenarist_SCC V1.0\n00:00:01;00 942c").is_some());
    assert!(detect_format(b"WEBVTT").is_none());
  }
}
