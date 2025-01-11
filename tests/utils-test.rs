#[cfg(test)]
mod tests {
  use anyhow::Result;
  use subtitler::utils::{format_timestamp, pad_left, parse_timestamp, parse_timestamps};

  #[test]
  fn test_parse_timestamp_valid() -> Result<()> {
    let timestamp = "01:02:03.456";
    let result = parse_timestamp(timestamp)?;
    assert_eq!(result, 3723456); // 1 hour, 2 minutes, 3 seconds, 456 milliseconds
    Ok(())
  }

  #[test]
  fn test_parse_timestamp_invalid() {
    let timestamp = "invalid_time_format";
    let result = parse_timestamp(timestamp);
    assert!(result.is_err());
  }

  #[test]
  fn test_parse_timestamps_valid() -> Result<()> {
    let value = "00:01:02.000 --> 00:01:05.000";
    let result = parse_timestamps(value)?;
    assert_eq!(result.start, 62000); // 1 minute, 2 seconds
    assert_eq!(result.end, 65000); // 1 minute, 5 seconds
    assert_eq!(result.settings, None); // No settings provided
    Ok(())
  }

  #[test]
  fn test_parse_timestamps_invalid() {
    let value = "invalid_timestamps";
    let result = parse_timestamps(value);
    assert!(result.is_err());
  }

  #[test]
  fn test_pad_left() {
    assert_eq!(pad_left(5, 3), "005");
    assert_eq!(pad_left(123, 5), "00123");
    assert_eq!(pad_left(0, 2), "00");
  }

  #[test]
  fn test_format_timestamp() {
    let timestamp = 3723456; // 1 hour, 2 minutes, 3 seconds, 456 milliseconds
    let result_vtt = format_timestamp(timestamp, "WebVTT");
    assert_eq!(result_vtt, "01:02:03.456");

    let result_srt = format_timestamp(timestamp, "SRT");
    assert_eq!(result_srt, "01:02:03,456");
  }
}
