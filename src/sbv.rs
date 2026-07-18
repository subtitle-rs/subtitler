//! SBV (YouTube subtitle format) parser and generator.
//!
//! Format: `time1,time2,text`  where time is `[hours:]minutes:seconds.milliseconds`.

use crate::error::SubtitleError;
use crate::model::{Format, Subtitle, SubtitleFile};
use crate::types::AnyResult;
use crate::utils::parse_timestamp;
#[cfg(not(target_arch = "wasm32"))]
use tokio::io::AsyncWriteExt;

/// Parse SBV content into a SubtitleFile.
pub fn parse_content(content: &str) -> AnyResult<SubtitleFile> {
  let mut subtitles: Vec<Subtitle> = Vec::with_capacity((content.len() / 40).max(16));

  for line in content.lines() {
    let trimmed = line.trim();
    if trimmed.is_empty() {
      continue;
    }

    // Format: time1,time2,text  (three comma-separated parts)
    let parts: Vec<&str> = trimmed.splitn(3, ',').collect();
    if parts.len() == 3 {
      let (t1, t2, text) = (parts[0], parts[1], parts[2]);
      // Convert SBV time format (hh:mm:ss.mmm or mm:ss.mmm) to SRT-like
      let t1_fixed = if t1.chars().filter(|&c| c == ':').count() == 1 {
        format!("00:{}", t1)
      } else {
        t1.to_string()
      };
      let t2_fixed = if t2.chars().filter(|&c| c == ':').count() == 1 {
        format!("00:{}", t2)
      } else {
        t2.to_string()
      };

      if let (Ok(start), Ok(end)) = (
        parse_timestamp(&t1_fixed, Format::Sbv),
        parse_timestamp(&t2_fixed, Format::Sbv),
      ) {
        subtitles.push(Subtitle::new(start, end, text.trim()));
      }
    }
  }

  Ok(SubtitleFile::Sbv(subtitles))
}

/// Parse SBV from a byte slice.
pub fn parse_bytes(data: &[u8]) -> AnyResult<SubtitleFile> {
  let text = crate::encoding::decode_to_string(data)?;
  parse_content(&text)
}

/// Parse an SBV file asynchronously.
#[cfg(not(target_arch = "wasm32"))]
pub async fn parse_file(path: impl AsRef<std::path::Path>) -> AnyResult<SubtitleFile> {
  let text = tokio::fs::read_to_string(path).await?;
  parse_content(&text)
}

/// Parse an SBV file from a URL (requires `http` feature).
#[cfg(feature = "http")]
pub async fn parse_url(url: &str) -> AnyResult<SubtitleFile> {
  let response = reqwest::get(url).await?;
  let content = response.text().await?;
  parse_content(&content)
}

/// Detect if data looks like SBV (YouTube subtitle format).
/// SBV lines have the pattern: `H:MM:SS.mmm,H:MM:SS.mmm,text`
pub fn detect_format(data: &[u8]) -> Option<crate::model::Format> {
  let text = crate::encoding::try_decode_for_detection(data)?;
  let has_sbv = text.lines().any(|l| {
    let t = l.trim();
    if t.is_empty() || !t.starts_with(|c: char| c.is_ascii_digit()) {
      return false;
    }
    // Must have at least 2 commas (time1,time2,text)
    let parts: Vec<&str> = t.splitn(3, ',').collect();
    if parts.len() < 3 {
      return false;
    }
    // Both time fields must contain ':' and '.' (SBV time format)
    parts[0].contains(':')
      && parts[0].contains('.')
      && parts[1].contains(':')
      && parts[1].contains('.')
  });
  if has_sbv {
    return Some(crate::model::Format::Sbv);
  }
  None
}

/// Write subtitles to a file in SBV format.
///
/// `policy` controls overwrite behavior (None = default Overwrite).
#[cfg(not(target_arch = "wasm32"))]
pub async fn generate(
  subtitles: &[Subtitle],
  file_path: impl AsRef<std::path::Path>,
  policy: Option<crate::model::WritePolicy>,
) -> AnyResult<String> {
  let content = to_string(subtitles);
  let path = file_path.as_ref();
  crate::io::write_with_policy(path, content.as_bytes(), policy).await?;
  Ok(path.to_string_lossy().into_owned())
}

/// Serialize subtitles to SBV format.
pub fn to_string(subtitles: &[Subtitle]) -> String {
  let mut buf = String::new();
  for sub in subtitles {
    let start = format_sbv_time(sub.start);
    let end = format_sbv_time(sub.end);
    buf.push_str(&format!("{},{},{}\n", start, end, sub.text));
  }
  buf
}

fn format_sbv_time(ms: u64) -> String {
  let total_seconds = ms / 1000;
  let millis = ms % 1000;
  let hours = total_seconds / 3600;
  let minutes = (total_seconds % 3600) / 60;
  let seconds = total_seconds % 60;
  format!("{}:{:02}:{:02}.{:03}", hours, minutes, seconds, millis)
}

/// Streaming parser entry point — yields subtitles one at a time
/// without allocating a full `Vec`.
pub fn parse_stream<'a>(content: &'a str) -> SbVStream<'a> {
  SbVStream::new(content)
}

pub struct SbVStream<'a> {
  lines: std::str::Lines<'a>,
}

impl<'a> SbVStream<'a> {
  pub fn new(content: &'a str) -> Self {
    SbVStream {
      lines: content.lines(),
    }
  }
}

impl<'a> Iterator for SbVStream<'a> {
  type Item = AnyResult<Subtitle>;
  fn next(&mut self) -> Option<Self::Item> {
    for line in self.lines.by_ref() {
      let trimmed = line.trim();
      if trimmed.is_empty() {
        continue;
      }
      return Some(parse_sbv_line(trimmed).map_err(Into::into));
    }
    None
  }
}

impl<'a> crate::model::StreamingParser for SbVStream<'a> {}

fn parse_sbv_line(line: &str) -> Result<Subtitle, SubtitleError> {
  let parts: Vec<&str> = line.splitn(3, ',').collect();
  if parts.len() < 3 {
    return Err(SubtitleError::InvalidLine {
      format: Format::Sbv,
      line: line.to_string(),
    });
  }
  let (t1, t2, text) = (parts[0], parts[1], parts[2]);
  let t1_fixed = if t1.chars().filter(|&c| c == ':').count() == 1 {
    format!("00:{}", t1)
  } else {
    t1.to_string()
  };
  let t2_fixed = if t2.chars().filter(|&c| c == ':').count() == 1 {
    format!("00:{}", t2)
  } else {
    t2.to_string()
  };
  let start = crate::utils::parse_timestamp(&t1_fixed, Format::Sbv)?;
  let end = crate::utils::parse_timestamp(&t2_fixed, Format::Sbv)?;
  Ok(Subtitle::new(start, end, text.trim()))
}

/// Write SBV subtitles to an async writer streamingly.
#[cfg(not(target_arch = "wasm32"))]
pub async fn write_stream<W: tokio::io::AsyncWrite + Unpin>(
  subtitles: &[Subtitle],
  writer: &mut W,
) -> AnyResult<()> {
  for sub in subtitles {
    let start = format_sbv_time(sub.start);
    let end = format_sbv_time(sub.end);
    writer
      .write_all(format!("{},{},{}\n", start, end, sub.text).as_bytes())
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
    let content = "0:00:01.000,0:00:03.500,Hello World\n0:00:04.000,0:00:06.500,Line two\n";
    let file = parse_content(content).unwrap();
    let subs = file.subtitles();
    assert_eq!(subs.len(), 2);
    assert_eq!(subs[0].start, 1000);
    assert_eq!(subs[0].end, 3500);
    assert_eq!(subs[0].text, "Hello World");
  }

  #[test]
  fn test_round_trip() {
    let content = "0:00:01.000,0:00:03.500,Hello\n";
    let file = parse_content(content).unwrap();
    let subs = file.subtitles();
    let output = to_string(subs);
    assert!(output.contains("Hello"));
    let reparsed = parse_content(&output).unwrap();
    assert_eq!(subs.len(), reparsed.subtitles().len());
  }

  #[test]
  fn test_detect() {
    assert!(detect_format(b"0:00:01.000,0:00:03.500,test").is_some());
    assert!(detect_format(b"WEBVTT").is_none());
  }
}
