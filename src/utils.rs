use crate::config::{RE_TIMESTAMP, RE_TIMESTAMPS};
use crate::model::Timestamp;
use crate::types::AnyResult;
use anyhow::anyhow;
use regex::Regex;
pub fn parse_timestamp(timestamp: &str) -> AnyResult<u64> {
  // 定义正则表达式
  let re = Regex::new(RE_TIMESTAMP)?;

  // 使用正则表达式进行匹配
  if let Some(captures) = re.captures(timestamp) {
    // 提取捕获组并计算时间
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
  let re = Regex::new(RE_TIMESTAMPS)?;

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
    Err(anyhow!("Invalid timestamp format".to_string()))
  }
}

#[allow(dead_code)]
pub fn pad_left(value: i32, length: usize) -> String {
  let value_str = value.to_string();
  let pad_size = length.saturating_sub(value_str.len());
  let padding = "0".repeat(pad_size);
  format!("{}{}", padding, value_str)
}

#[allow(dead_code)]
pub fn format_timestamp(timestamp: i64, options: &str) -> String {
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
