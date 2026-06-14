use crate::config::{RE_TIMESTAMP, RE_TIMESTAMPS};
use crate::model::Timestamp;
use crate::types::AnyResult;
use anyhow::anyhow;
use regex::Regex;
use std::sync::LazyLock;

static RE_TIMESTAMP_LAZY: LazyLock<Regex> = LazyLock::new(|| Regex::new(RE_TIMESTAMP).unwrap());
static RE_TIMESTAMPS_LAZY: LazyLock<Regex> = LazyLock::new(|| Regex::new(RE_TIMESTAMPS).unwrap());

pub fn parse_timestamp(timestamp: &str) -> AnyResult<u64> {
  let re = &RE_TIMESTAMP_LAZY;

  if let Some(captures) = re.captures(timestamp) {
    let hours = captures
      .get(1)
      .map_or(0, |m| m.as_str().parse::<u64>().unwrap())
      * 3600000;
    let minutes = captures[2].parse::<u64>()? * 60000;
    let seconds = captures[3].parse::<u64>()? * 1000;
    let milliseconds = captures[4].parse::<u64>()?;

    Ok(hours + minutes + seconds + milliseconds)
  } else {
    Err(anyhow!("Invalid SRT or VTT time format: \"{}\"", timestamp))
  }
}

pub fn parse_timestamps(value: &str) -> AnyResult<Timestamp> {
  let re = &RE_TIMESTAMPS_LAZY;

  if let Some(captures) = re.captures(value) {
    let start = parse_timestamp(&captures[1])?;
    let end = parse_timestamp(&captures[2])?;
    let settings = captures.get(3).map(|m| m.as_str().to_string());

    Ok(Timestamp {
      start,
      end,
      settings,
    })
  } else {
    Err(anyhow!("Invalid timestamp format"))
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

  let separator = if options == "WebVTT" { '.' } else { ',' };

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
    assert_eq!(parse_timestamp("00:00:01,000").unwrap(), 1000);
    assert_eq!(parse_timestamp("00:00:03,500").unwrap(), 3500);
    assert_eq!(parse_timestamp("00:01:00,000").unwrap(), 60000);
    assert_eq!(parse_timestamp("01:00:00,000").unwrap(), 3600000);
    assert_eq!(parse_timestamp("00:00:00,000").unwrap(), 0);
  }

  #[test]
  fn test_parse_timestamp_vtt() {
    assert_eq!(parse_timestamp("00:00:01.000").unwrap(), 1000);
    assert_eq!(parse_timestamp("00:00:03.500").unwrap(), 3500);
  }

  #[test]
  fn test_parse_timestamps() {
    let ts = parse_timestamps("00:00:01,000 --> 00:00:03,500").unwrap();
    assert_eq!(ts.start, 1000);
    assert_eq!(ts.end, 3500);
    assert_eq!(ts.settings, None);
  }

  #[test]
  fn test_parse_timestamps_with_settings() {
    let ts = parse_timestamps("00:00:01.000 --> 00:00:03.500 align:start").unwrap();
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
  fn test_pad_left() {
    assert_eq!(pad_left(1, 2), "01");
    assert_eq!(pad_left(10, 2), "10");
    assert_eq!(pad_left(1, 3), "001");
    assert_eq!(pad_left(0, 2), "00");
  }

  #[test]
  fn test_parse_invalid_timestamp() {
    assert!(parse_timestamp("not a time").is_err());
    assert!(parse_timestamp("").is_err());
  }
}
