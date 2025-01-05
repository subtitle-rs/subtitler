use crate::model::Timestamp;
use crate::types::AnyResult;
use anyhow::anyhow;
use regex::Regex;

pub fn parse_timestamp(timestamp: &str) -> AnyResult<u64> {
  // 定义正则表达式
  let re = Regex::new(r"^(?:(\d{1,}):)?(\d{1,2}):(\d{1,2})[,.](\d{1,3})$")?;

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
  let re = Regex::new(r"^((?:\d{1,}:)?\d{1,2}:\d{1,2}[,.]\d{1,3}) --> ((?:\d{1,}:)?\d{1,2}:\d{1,2}[,.]\d{1,3})(?: (.*))?$").unwrap();

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
