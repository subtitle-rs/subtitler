use crate::model::{Subtitle, SubtitleFile};
use crate::types::AnyResult;
use anyhow::anyhow;
use regex::Regex;
use std::sync::LazyLock;

static RE_SUBVIEWER_LINE: LazyLock<Regex> = LazyLock::new(|| {
  Regex::new(r"^(\d{1,2}:\d{2}:\d{2}\.\d{2}),(\d{1,2}:\d{2}:\d{2}\.\d{2})$").unwrap()
});

static RE_SUBVIEWER_BRACKET: LazyLock<Regex> = LazyLock::new(|| {
  Regex::new(r"^\[(?:COLF|STYLE|SIZE|FONT|INFORMATION|TITLE|AUTHOR|SOURCE|FILEPATH|DELAY|COMMENT|END|SUBTITLE)").unwrap()
});

pub fn detect_format(data: &[u8]) -> Option<crate::model::Format> {
  if let Some(text) = crate::encoding::try_decode_for_detection(data) {
    for line in text.lines() {
      let trimmed = line.trim();
      if trimmed.is_empty() || RE_SUBVIEWER_BRACKET.is_match(trimmed) {
        continue;
      }
      if RE_SUBVIEWER_LINE.is_match(trimmed) {
        return Some(crate::model::Format::SubViewer);
      }
      // If we see non-bracket, non-empty, non-timestamp content first, fail
      break;
    }
  }
  None
}

fn parse_subviewer_time(ts: &str) -> AnyResult<u64> {
  let parts: Vec<&str> = ts.split(':').collect();
  if parts.len() != 3 {
    return Err(anyhow!("Invalid SubViewer time: {}", ts));
  }
  let h: u64 = parts[0].parse()?;
  let m: u64 = parts[1].parse()?;
  let s_parts: Vec<&str> = parts[2].split('.').collect();
  let s: u64 = s_parts[0].parse()?;
  let ms: u64 = if s_parts.len() > 1 {
    let centisecs_str = s_parts[1];
    // SubViewer uses centiseconds (0-99), validate
    if centisecs_str.len() > 2 {
      return Err(anyhow!(
        "Invalid SubViewer time: fractional part must be at most 2 digits (centiseconds), got '{}'",
        centisecs_str
      ));
    }
    centisecs_str.parse::<u64>()? * 10 // centiseconds → ms
  } else {
    0
  };
  Ok(h * 3600000 + m * 60000 + s * 1000 + ms)
}

pub fn parse_content(content: &str) -> AnyResult<SubtitleFile> {
  let mut subtitles = Vec::new();
  let mut pending_timestamp: Option<(u64, u64)> = None;
  let mut header_lines: Vec<String> = Vec::new();
  let mut saw_timestamp = false;

  for line in content.lines() {
    let trimmed = line.trim();

    if trimmed.is_empty() {
      continue;
    }

    if RE_SUBVIEWER_BRACKET.is_match(trimmed) {
      if !saw_timestamp {
        header_lines.push(trimmed.to_string());
      }
      continue;
    }

    if let Some(caps) = RE_SUBVIEWER_LINE.captures(trimmed) {
      saw_timestamp = true;
      let start = parse_subviewer_time(&caps[1])?;
      let end = parse_subviewer_time(&caps[2])?;
      pending_timestamp = Some((start, end));
    } else if let Some((start, end)) = pending_timestamp.take() {
      subtitles.push(Subtitle::new(start, end, trimmed));
    }
  }

  let header = if header_lines.is_empty() {
    None
  } else {
    Some(header_lines.join("\n"))
  };

  Ok(SubtitleFile::SubViewer { header, subtitles })
}

/// Decode bytes to UTF-8 then parse, returning a `SubtitleFile`.
pub fn parse_bytes(data: &[u8]) -> AnyResult<SubtitleFile> {
  let text = crate::encoding::decode_to_string(data)?;
  parse_content(&text)
}

/// Parse a SubViewer file asynchronously.
pub async fn parse_file(path: impl AsRef<std::path::Path>) -> AnyResult<SubtitleFile> {
  let text = tokio::fs::read_to_string(path).await?;
  parse_content(&text)
}

/// Parse a SubViewer file from a URL (requires `http` feature).
#[cfg(feature = "http")]
pub async fn parse_url(url: &str) -> AnyResult<SubtitleFile> {
  let response = reqwest::get(url).await?;
  let content = response.text().await?;
  parse_content(&content)
}

pub fn to_string(subtitles: &[Subtitle], header: Option<&str>) -> String {
  let mut buf = match header {
    Some(h) => format!("{h}\n\n"),
    None => String::from(
      "[INFORMATION]\n[TITLE]Subtitles\n[AUTHOR]subtitler\n[SOURCE]\n[FILEPATH]\n[DELAY]0\n[COMMENT]\n[END INFORMATION]\n[SUBTITLE]\n[COLF]&HFFFFFF,[STYLE]bd,[SIZE]18,[FONT]Arial\n\n",
    ),
  };

  for sub in subtitles {
    let start = format_subviewer_time(sub.start);
    let end = format_subviewer_time(sub.end);
    buf.push_str(&format!("{},{}\n{}\n\n", start, end, sub.text));
  }

  buf
}

fn format_subviewer_time(ms: u64) -> String {
  let total_seconds = ms / 1000;
  let centiseconds = (ms % 1000) / 10;
  let hours = total_seconds / 3600;
  let minutes = (total_seconds % 3600) / 60;
  let seconds = total_seconds % 60;
  format!(
    "{:0>2}:{:0>2}:{:0>2}.{:0>2}",
    hours, minutes, seconds, centiseconds
  )
}

pub struct SubViewerStream<'a> {
  lines: std::str::Lines<'a>,
}
impl<'a> SubViewerStream<'a> {
  pub fn new(content: &'a str) -> Self {
    SubViewerStream {
      lines: content.lines(),
    }
  }
}
impl<'a> Iterator for SubViewerStream<'a> {
  type Item = AnyResult<Subtitle>;
  fn next(&mut self) -> Option<Self::Item> {
    for line in self.lines.by_ref() {
      let trimmed = line.trim();
      if trimmed.is_empty() || RE_SUBVIEWER_BRACKET.is_match(trimmed) {
        continue;
      }
      if let Some(caps) = RE_SUBVIEWER_LINE.captures(trimmed) {
        if let (Ok(s), Ok(e)) = (
          parse_subviewer_time(&caps[1]),
          parse_subviewer_time(&caps[2]),
        ) {
          let text = self
            .lines
            .by_ref()
            .find(|l| !l.trim().is_empty() && !RE_SUBVIEWER_BRACKET.is_match(l.trim()))
            .unwrap_or("");
          return Some(Ok(Subtitle::new(s, e, text.trim())));
        }
      }
    }
    None
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::model::SubtitleFormat;

  #[test]
  fn test_parse_basic() {
    let content = "[SUBTITLE]\n[COLF]&HFFFFFF,[STYLE]bd,[SIZE]18,[FONT]Arial\n\n00:00:01.00,00:00:03.50\nHello World\n\n00:00:04.00,00:00:06.50\nGoodbye\n\n";
    let file = parse_content(content).unwrap();
    let result = file.subtitles();
    assert_eq!(result.len(), 2);
    assert_eq!(result[0].start, 1000);
    assert_eq!(result[0].end, 3500);
    assert_eq!(result[0].text, "Hello World");
  }

  #[test]
  fn test_parse_simple() {
    let content = "00:00:01.00,00:00:03.50\nHello\n\n00:00:04.00,00:00:06.50\nWorld\n";
    let file = parse_content(content).unwrap();
    assert_eq!(file.subtitles().len(), 2);
  }

  #[test]
  fn test_round_trip() {
    let content = "00:00:01.00,00:00:03.50\nHello\n\n00:00:04.00,00:00:06.50\nWorld\n";
    let file = parse_content(content).unwrap();
    let subs = file.subtitles().to_vec();
    let output = to_string(&subs, None);
    let reparsed_file = parse_content(&output).unwrap();
    let reparsed = reparsed_file.subtitles();
    assert_eq!(subs.len(), reparsed.len());
    assert_eq!(subs[0].text, reparsed[0].text);
    assert_eq!(subs[0].start, reparsed[0].start);
    assert_eq!(subs[1].text, reparsed[1].text);
  }

  #[test]
  fn test_detect_format() {
    assert!(detect_format(b"00:00:01.00,00:00:03.50\ntest").is_some());
    assert!(detect_format(b"[SUBTITLE]\n00:00:01.00,00:00:03.50\ntest").is_some());
    assert!(detect_format(b"WEBVTT\n").is_none());
  }

  #[test]
  fn test_parse_empty() {
    let file = parse_content("").unwrap();
    assert!(file.subtitles().is_empty());
  }
}
