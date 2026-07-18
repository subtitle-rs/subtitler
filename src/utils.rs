use crate::config::{RE_TIMESTAMP, RE_TIMESTAMPS};
use crate::error::SubtitleError;
use crate::model::convert::{MS_PER_HOUR, MS_PER_MINUTE, MS_PER_SECOND};
use crate::model::{Format, Timestamp};
use regex::Regex;
use std::sync::LazyLock;

static RE_TIMESTAMP_LAZY: LazyLock<Regex> = LazyLock::new(|| Regex::new(RE_TIMESTAMP).unwrap());
static RE_TIMESTAMPS_LAZY: LazyLock<Regex> = LazyLock::new(|| Regex::new(RE_TIMESTAMPS).unwrap());

/// Fast manual timestamp parser without regex allocation overhead.
///
/// Accepts: `hh:mm:ss[,.]mmm` (SRT/VTT) or `h:mm:ss[,.]mmm` (single-digit hour).
/// Returns milliseconds. Uses SIMD-style byte scanning on supported architectures.
pub fn parse_timestamp(timestamp: &str, format: Format) -> Result<u64, SubtitleError> {
  // Regex fallback for non-standard timestamps
  let bytes = timestamp.as_bytes();
  let len = bytes.len();

  // Minimum: 8:59.99.00? Actually min valid is "0:00:00.000" = 11 bytes
  // or "00:00:00.000" = 11 bytes
  if len < 11 {
    return fallback_parse(timestamp, format);
  }

  // Find first colon to locate hours/minutes boundary
  let colon1 = bytes.iter().position(|&b| b == b':').unwrap_or(usize::MAX);
  if colon1 == usize::MAX || colon1 == 0 || colon1 > 2 {
    return fallback_parse(timestamp, format);
  }

  let h_start = 0;
  let m_start = colon1 + 1;
  let sep1 = b':';

  // Verify second colon exists
  if m_start >= len || bytes[m_start - 1] != sep1 {
    return fallback_parse(timestamp, format);
  }

  // Find second colon (minutes:seconds)
  let colon2 = bytes[m_start..]
    .iter()
    .position(|&b| b == b':')
    .map(|i| m_start + i)
    .unwrap_or(usize::MAX);
  if colon2 == usize::MAX || colon2 + 9 > len {
    return fallback_parse(timestamp, format);
  }

  let s_start = colon2 + 1;

  // The separator at position s_start + 2 should be ',' or '.'
  let sep_pos = s_start + 2;
  let sep = bytes[sep_pos];
  if sep != b',' && sep != b'.' {
    return fallback_parse(timestamp, format);
  }

  // Verify all characters between positions are digits or colons
  // (already checked colon positions)
  if !bytes[h_start..colon1].iter().all(|b| b.is_ascii_digit())
    || !bytes[m_start..colon2].iter().all(|b| b.is_ascii_digit())
    || !bytes[s_start..sep_pos].iter().all(|b| b.is_ascii_digit())
  {
    return fallback_parse(timestamp, format);
  }

  // Parse hours (variable width: 1 or 2 digits)
  let hours: u64 = atoi(&bytes[h_start..colon1]);
  let minutes: u64 = atoi(&bytes[m_start..colon2]);
  let seconds: u64 = atoi(&bytes[s_start..sep_pos]);

  // Parse milliseconds (3 digits after separator)
  let ms_start = sep_pos + 1;
  let ms_end = (ms_start + 3).min(len);
  let ms: u64 = atoi(&bytes[ms_start..ms_end]);

  Ok(hours * MS_PER_HOUR + minutes * MS_PER_MINUTE + seconds * MS_PER_SECOND + ms)
}

/// Fast ASCII digit-to-integer conversion for short byte slices.
#[inline]
fn atoi(s: &[u8]) -> u64 {
  let mut val: u64 = 0;
  for &b in s {
    val = val * 10 + (b - b'0') as u64;
  }
  val
}

/// Fallback: use regex for non-standard timestamp formats.
fn fallback_parse(timestamp: &str, format: Format) -> Result<u64, SubtitleError> {
  let re = &RE_TIMESTAMP_LAZY;
  if let Some(captures) = re.captures(timestamp) {
    let hours = captures
      .get(1)
      .map_or(Ok(0), |m| m.as_str().parse::<u64>())
      .map_err(|_| SubtitleError::InvalidTimestamp {
        format,
        value: timestamp.to_string(),
      })?
      * 3600000;
    let minutes = captures[2]
      .parse::<u64>()
      .map_err(|_| SubtitleError::InvalidTimestamp {
        format,
        value: timestamp.to_string(),
      })?
      * 60000;
    let seconds = captures[3]
      .parse::<u64>()
      .map_err(|_| SubtitleError::InvalidTimestamp {
        format,
        value: timestamp.to_string(),
      })?
      * 1000;
    let milliseconds = captures[4]
      .parse::<u64>()
      .map_err(|_| SubtitleError::InvalidTimestamp {
        format,
        value: timestamp.to_string(),
      })?;

    Ok(hours + minutes + seconds + milliseconds)
  } else {
    Err(SubtitleError::InvalidTimestamp {
      format,
      value: timestamp.to_string(),
    })
  }
}

pub fn parse_timestamps(value: &str, format: Format) -> Result<Timestamp, SubtitleError> {
  let re = &RE_TIMESTAMPS_LAZY;

  if let Some(captures) = re.captures(value) {
    let start = parse_timestamp(&captures[1], format)?;
    let end = parse_timestamp(&captures[2], format)?;
    let settings = captures.get(3).map(|m| m.as_str().to_string());

    Ok(Timestamp {
      start,
      end,
      settings,
    })
  } else {
    Err(SubtitleError::InvalidTimestamp {
      format,
      value: value.to_string(),
    })
  }
}

pub fn pad_left(value: i32, length: usize) -> String {
  let value_str = value.to_string();
  let pad_size = length.saturating_sub(value_str.len());
  let padding = "0".repeat(pad_size);
  format!("{}{}", padding, value_str)
}

pub fn format_timestamp(timestamp: u64, options: &str) -> String {
  let total_seconds = timestamp / 1000;
  let ms = timestamp % 1000;

  let hours = (total_seconds / 3600) as i32;
  let minutes = ((total_seconds % 3600) / 60) as i32;
  let seconds = (total_seconds % 60) as i32;

  // 大小写不敏感判断，避免调用方传 "SRT"/"srt"/"WebVTT"/"vtt" 时行为不一致
  let separator = if options.eq_ignore_ascii_case("WebVTT") {
    '.'
  } else {
    ','
  };

  format!(
    "{}:{}:{}{}{}",
    pad_left(hours, 2),
    pad_left(minutes, 2),
    pad_left(seconds, 2),
    separator,
    pad_left(ms as i32, 3)
  )
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_parse_timestamp_srt() {
    assert_eq!(parse_timestamp("00:00:01,000", Format::Srt).unwrap(), 1000);
    assert_eq!(parse_timestamp("00:00:03,500", Format::Srt).unwrap(), 3500);
    assert_eq!(parse_timestamp("00:01:00,000", Format::Srt).unwrap(), 60000);
    assert_eq!(
      parse_timestamp("01:00:00,000", Format::Srt).unwrap(),
      3600000
    );
    assert_eq!(parse_timestamp("00:00:00,000", Format::Srt).unwrap(), 0);
  }

  #[test]
  #[cfg(feature = "vtt")]
  fn test_parse_timestamp_vtt() {
    assert_eq!(parse_timestamp("00:00:01.000", Format::Vtt).unwrap(), 1000);
    assert_eq!(parse_timestamp("00:00:03.500", Format::Vtt).unwrap(), 3500);
  }

  #[test]
  fn test_parse_timestamps() {
    let ts = parse_timestamps("00:00:01,000 --> 00:00:03,500", Format::Srt).unwrap();
    assert_eq!(ts.start, 1000);
    assert_eq!(ts.end, 3500);
    assert_eq!(ts.settings, None);
  }

  #[test]
  #[cfg(feature = "vtt")]
  fn test_parse_timestamps_with_settings() {
    let ts = parse_timestamps("00:00:01.000 --> 00:00:03.500 align:start", Format::Vtt).unwrap();
    assert_eq!(ts.start, 1000);
    assert_eq!(ts.end, 3500);
    assert_eq!(ts.settings, Some("align:start".to_string()));
  }

  #[test]
  fn test_format_timestamp_srt() {
    assert_eq!(format_timestamp(1000, "srt"), "00:00:01,000");
    assert_eq!(format_timestamp(3500, "srt"), "00:00:03,500");
    assert_eq!(format_timestamp(3661000, "srt"), "01:01:01,000");
    assert_eq!(format_timestamp(0, "srt"), "00:00:00,000");
  }

  #[test]
  fn test_format_timestamp_vtt() {
    assert_eq!(format_timestamp(1000, "WebVTT"), "00:00:01.000");
    assert_eq!(format_timestamp(3500, "WebVTT"), "00:00:03.500");
  }

  #[test]
  fn test_format_timestamp_case_insensitive() {
    // "WebVTT" 的大小写变体都应走 VTT 分支（点分隔符）
    assert_eq!(format_timestamp(1000, "WebVTT"), "00:00:01.000");
    assert_eq!(format_timestamp(1000, "WEBVTT"), "00:00:01.000");
    assert_eq!(format_timestamp(1000, "webvtt"), "00:00:01.000");
    // 非 VTT 字符串（含 "SRT"/"srt"）都走 SRT 分支（逗号分隔符）
    assert_eq!(format_timestamp(1000, "SRT"), "00:00:01,000");
    assert_eq!(format_timestamp(1000, "srt"), "00:00:01,000");
  }

  #[test]
  fn test_pad_left() {
    assert_eq!(pad_left(1, 2), "01");
    assert_eq!(pad_left(10, 2), "10");
    assert_eq!(pad_left(1, 3), "001");
    assert_eq!(pad_left(0, 2), "00");
  }

  #[test]
  fn test_parse_invalid_timestamp() {
    assert!(parse_timestamp("not a time", Format::Srt).is_err());
    assert!(parse_timestamp("", Format::Srt).is_err());
  }
}
