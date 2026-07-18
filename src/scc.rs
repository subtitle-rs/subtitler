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

use crate::error::SubtitleError;
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
  pub fn parse(content: &str) -> Result<Self, SubtitleError> {
    let mut subtitles: Vec<Subtitle> = Vec::with_capacity((content.len() / 100).max(16));
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

        let timecode_ms =
          scc_timecode_to_ms(hours, minutes, seconds, frames, DEFAULT_FPS, drop_frame);

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
  pub fn render(&self) -> String {
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

/// Convert SCC/CEA-608 timecode to milliseconds.
///
/// Implements SMPTE 12M-1-2014 §3.3 drop-frame algorithm for NTSC
/// (29.97 fps). Drop-frame timecodes (separator `;`) drop 2 frames
/// per minute except every 10th minute, so the displayed timecode
/// tracks real elapsed time. Non-drop timecodes (separator `:`) treat
/// each frame as exactly 1/fps seconds and drift ~3.6s/hour vs. real time.
///
/// `fps` is the true frame rate (29.97, not the nominal 30 used
/// internally for the drop-frame bookkeeping).
fn scc_timecode_to_ms(h: u64, m: u64, s: u64, f: u64, fps: f64, drop_frame: bool) -> u64 {
  // Use integer frame counts at the NOMINAL frame rate (30 for NTSC).
  // Going via f64 here would lose precision (29.97 × 3600 ≠ integer).
  let nominal_fps = fps.round() as u64;
  let mut total_frames = h * 3600 * nominal_fps + m * 60 * nominal_fps + s * nominal_fps + f;

  if drop_frame {
    // SMPTE 12M-1-2014 §3.3: drop 2 frames per minute, except every 10th minute.
    //   drop_count = 2 * (total_minutes - total_minutes / 10)
    // Examples:
    //   total_minutes = 0  → drop 0   (origin)
    //   total_minutes = 1  → drop 2
    //   total_minutes = 9  → drop 18
    //   total_minutes = 10 → drop 18  (the 10th minute itself does NOT drop)
    //   total_minutes = 60 → drop 108 (1 hour)
    let total_minutes = h * 60 + m;
    let drop_count = 2 * (total_minutes - total_minutes / 10);
    total_frames -= drop_count;
  }

  // Convert frames → ms using the TRUE fps (29.97), not the nominal 30.
  ((total_frames as f64) / fps * 1000.0).round() as u64
}

/// Convert milliseconds to SCC/CEA-608 timecode string.
///
/// Inverse of `scc_timecode_to_ms`. For drop-frame output, iterates
/// minute-by-minute to invert the "drop 2 per minute except every
/// 10th" rule.
fn ms_to_scc_timecode(ms: u64, fps: f64, drop_frame: bool) -> String {
  let nominal_fps = fps.round() as u64;
  let frames_per_minute = 60 * nominal_fps;

  // Real hours come directly from ms (independent of drop-frame).
  let hours = ms / 3_600_000;
  let ms_within_hour = ms % 3_600_000;

  // Convert ms-within-hour to frames at the nominal rate.
  let mut frames_within_hour = (ms_within_hour as f64 * fps / 1000.0).round() as u64;

  if drop_frame {
    // Walk forward through minutes, subtracting the per-minute drop
    // count, to find the displayed minute value. Each displayed minute
    // is `frames_per_minute - drop_for_this_minute` stored frames apart,
    // where drop_for_this_minute is 0 if the NEXT minute is a 10th
    // (because the transition into a 10th minute doesn't drop).
    let mut minutes = 0u64;
    while minutes < 60 {
      // Determine if transitioning INTO minute (minutes+1) drops frames.
      // Per SMPTE 12M: frames are dropped at the START of each minute
      // except minutes that are exact multiples of 10.
      let drop_for_next = if (minutes + 1) % 10 == 0 { 0 } else { 2 };
      let step = frames_per_minute - drop_for_next;
      if frames_within_hour < step {
        break;
      }
      frames_within_hour -= step;
      minutes += 1;
    }
    let seconds = frames_within_hour / nominal_fps;
    let frames = frames_within_hour % nominal_fps;
    return format!("{:02}:{:02}:{:02};{:02}", hours, minutes, seconds, frames);
  }

  // Non-drop: simple division.
  let minutes = frames_within_hour / frames_per_minute;
  let remaining = frames_within_hour % frames_per_minute;
  let seconds = remaining / nominal_fps;
  let frames = remaining % nominal_fps;
  format!("{:02}:{:02}:{:02}:{:02}", hours, minutes, seconds, frames)
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
pub fn parse_content(content: &str) -> Result<SubtitleFile, SubtitleError> {
  let data = SccData::parse(content)?;
  Ok(SubtitleFile::Scc(data))
}

/// Parse SCC from a byte slice.
pub fn parse_bytes(data: &[u8]) -> AnyResult<SubtitleFile> {
  let text = crate::encoding::decode_to_string(data)?;
  Ok(parse_content(&text)?)
}

/// Parse an SCC file asynchronously.
#[cfg(not(target_arch = "wasm32"))]
pub async fn parse_file(path: impl AsRef<std::path::Path>) -> AnyResult<SubtitleFile> {
  let text = tokio::fs::read_to_string(path).await?;
  Ok(parse_content(&text)?)
}

/// Parse an SCC file from a URL (requires `http` feature).
#[cfg(feature = "http")]
pub async fn parse_url(url: &str) -> AnyResult<SubtitleFile> {
  let response = reqwest::get(url).await?;
  let content = response.text().await?;
  Ok(parse_content(&content)?)
}

/// Detect if data looks like SCC.
pub fn detect_format(data: &[u8]) -> Option<crate::model::Format> {
  let text = crate::encoding::try_decode_for_detection(data)?;
  if text.starts_with("Scenarist_SCC") {
    return Some(crate::model::Format::Scc);
  }
  None
}

/// Write subtitles to a file in SCC format.
///
/// `policy` controls overwrite behavior (None = default Overwrite).
/// Uses non-drop-frame timecodes; for drop-frame output, call
/// `to_string` directly and write the result with `tokio::fs::write`.
#[cfg(not(target_arch = "wasm32"))]
pub async fn generate(
  subtitles: &[Subtitle],
  file_path: impl AsRef<std::path::Path>,
  policy: Option<crate::model::WritePolicy>,
) -> AnyResult<String> {
  let content = to_string(subtitles, false);
  let path = file_path.as_ref();
  crate::io::write_with_policy(path, content.as_bytes(), policy).await?;
  Ok(path.to_string_lossy().into_owned())
}

/// Serialize subtitles to SCC format.
pub fn to_string(subtitles: &[Subtitle], drop_frame: bool) -> String {
  let data = SccData {
    fps: DEFAULT_FPS,
    drop_frame,
    subtitles: subtitles.to_vec(),
  };
  data.render()
}

/// Streaming parser entry point — yields subtitles one at a time
/// without allocating a full `Vec`.
pub fn parse_stream<'a>(content: &'a str) -> SccStream<'a> {
  SccStream::new(content)
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
        // caps[4] is the separator (`:` or `;`); drop-frame uses `;`.
        let drop_frame = &caps[4] == ";";
        let frames: u64 = caps[5].parse().unwrap_or(0);
        let hex_data = &caps[6];

        let timecode_ms =
          scc_timecode_to_ms(hours, minutes, seconds, frames, DEFAULT_FPS, drop_frame);
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
    let ms = scc_timecode_to_ms(1, 2, 3, 15, 29.97, false);
    // Just check that conversion works
    assert!(ms > 0);
    let tc = ms_to_scc_timecode(ms, 29.97, true);
    // Check format is correct (HH:MM:SS;FF or HH:MM:SS:FF)
    assert!(tc.contains(':') || tc.contains(';'));
  }

  #[test]
  fn test_scc_nondrop_accuracy() {
    // Non-drop 01:00:00:00 at 29.97fps = 108000 frames / 29.97 ≈ 3603604ms
    let ms = scc_timecode_to_ms(1, 0, 0, 0, 29.97, false);
    assert_eq!(ms, 3_603_604);
  }

  #[test]
  fn test_scc_dropframe_accuracy() {
    // Drop-frame 01:00:00;00 at 29.97fps = (108000 - 108) / 29.97 = 3600000ms
    // (drop-frame aligns displayed timecode with real elapsed time)
    let ms = scc_timecode_to_ms(1, 0, 0, 0, 29.97, true);
    assert_eq!(ms, 3_600_000);
  }

  #[test]
  fn test_scc_dropframe_edge_zero_minute() {
    // SMPTE 12M edge cases (values hand-verified via Python simulation).
    // Key invariant: drop-frame aligns whole-10-minute and whole-hour
    // boundaries to real elapsed time. Within each decade, individual
    // minutes run short by ~7ms (2 dropped frames at 29.97fps).
    assert_eq!(scc_timecode_to_ms(0, 0, 0, 0, 29.97, true), 0);
    assert_eq!(scc_timecode_to_ms(0, 1, 0, 0, 29.97, true), 59_993);
    assert_eq!(scc_timecode_to_ms(0, 10, 0, 0, 29.97, true), 600_000);
    assert_eq!(scc_timecode_to_ms(0, 11, 0, 0, 29.97, true), 659_993);
    assert_eq!(scc_timecode_to_ms(1, 0, 0, 0, 29.97, true), 3_600_000);

    // The "skip every 10th minute" rule shows up as a 7ms discontinuity:
    // m=9 has the same cumulative drop_count as m=10 (both 18 frames),
    // so the 10th minute itself doesn't add a drop. Compare m=10→11
    // (drops 2 frames) vs the 9→10 transition which carries no new drop.
    let drop_count_at = |m: u64| 2 * (m - m / 10);
    assert_eq!(drop_count_at(9), 18);
    assert_eq!(drop_count_at(10), 18, "m=10 should not add a drop vs m=9");
    assert_eq!(drop_count_at(11), 20, "m=11 should drop 2 more frames");
  }

  #[test]
  fn test_scc_round_trip() {
    // Round-trip a non-trivial drop-frame timecode.
    let ms = scc_timecode_to_ms(1, 30, 45, 12, 29.97, true);
    let tc = ms_to_scc_timecode(ms, 29.97, true);
    // Re-parse the rendered timecode (format is HH:MM:SS;FF).
    let bytes = tc.as_bytes();
    let h = std::str::from_utf8(&bytes[0..2])
      .unwrap()
      .parse::<u64>()
      .unwrap();
    let m = std::str::from_utf8(&bytes[3..5])
      .unwrap()
      .parse::<u64>()
      .unwrap();
    let s = std::str::from_utf8(&bytes[6..8])
      .unwrap()
      .parse::<u64>()
      .unwrap();
    let f = std::str::from_utf8(&bytes[9..11])
      .unwrap()
      .parse::<u64>()
      .unwrap();
    let ms_back = scc_timecode_to_ms(h, m, s, f, 29.97, true);
    // Allow 1-frame tolerance (33ms) for rounding in both directions.
    assert!(
      (ms_back as i64 - ms as i64).abs() <= 34,
      "round-trip drift: original={}, re-parsed={}, tc={}",
      ms,
      ms_back,
      tc
    );
  }

  #[test]
  fn test_detect() {
    assert!(detect_format(b"Scenarist_SCC V1.0\n00:00:01;00 942c").is_some());
    assert!(detect_format(b"WEBVTT").is_none());
  }
}
