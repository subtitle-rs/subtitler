use crate::model::{Subtitle, TextPart};
use crate::types::AnyResult;
use crate::utils::{format_timestamp, parse_timestamp};
use anyhow::anyhow;
use regex::Regex;
#[cfg(feature = "http")]
use reqwest;
use std::sync::LazyLock;
use tokio::fs;
use tokio::io::AsyncWriteExt;

static RE_SRT_TAG: LazyLock<Regex> =
  LazyLock::new(|| Regex::new(r"</?(?:b|i|u|font)(?:\s[^>]*)?>").unwrap());

static RE_SRT_DETECT: LazyLock<Regex> =
  LazyLock::new(|| Regex::new(r"^\d+\s*\n\d{2}:\d{2}:\d{2}[,.]\d{3}\s*-->").unwrap());

#[derive(Debug)]
enum Phase {
  Index,
  Timestamp,
  Text,
}

fn extract_text_parts(text: &str) -> (String, Vec<TextPart>) {
  // Fast path: if no HTML-like tags exist, return the text as-is
  if !text.contains('<') {
    return (text.to_string(), Vec::new());
  }

  let mut parts = Vec::new();
  let mut plain = String::new();
  let mut bold = false;
  let mut italic = false;
  let mut underline = false;
  let mut color: Option<String> = None;
  let mut last_end = 0;

  for caps in RE_SRT_TAG.find_iter(text) {
    let tag = caps.as_str();
    let start = caps.start();
    let end = caps.end();

    if start > last_end {
      let segment = &text[last_end..start];
      if !segment.is_empty() {
        plain.push_str(segment);
        if bold || italic || underline || color.is_some() {
          parts.push(TextPart {
            text: segment.to_string(),
            bold,
            italic,
            underline,
            color: color.clone(),
            voice: None,
          });
        }
      }
    }

    if tag.starts_with("</") {
      match tag {
        "</b>" => bold = false,
        "</i>" => italic = false,
        "</u>" => underline = false,
        "</font>" => color = None,
        _ => {}
      }
    } else {
      if tag.starts_with("<b") {
        bold = true;
      } else if tag.starts_with("<i") {
        italic = true;
      } else if tag.starts_with("<u") {
        underline = true;
      } else if tag.starts_with("<font")
        && let Some(c) = tag.split("color=").nth(1)
      {
        let color_val = c.trim_matches(|c: char| c == '"' || c == '\'' || c == '>' || c == '/');
        color = Some(color_val.to_string());
      }
    }

    last_end = end;
  }

  if last_end < text.len() {
    let segment = &text[last_end..];
    plain.push_str(segment);
    if bold || italic || underline || color.is_some() {
      parts.push(TextPart {
        text: segment.to_string(),
        bold,
        italic,
        underline,
        color: color.clone(),
        voice: None,
      });
    }
  }

  // If no tags were found, just return the plain text
  if parts.is_empty() {
    plain = text.to_string();
  }

  (plain, parts)
}

fn parse(content: &str) -> AnyResult<Vec<Subtitle>> {
  let mut subtitles = Vec::new();
  let mut current_subtitle: Option<Subtitle> = None;
  let mut phase = Phase::Index;
  let mut row: usize = 0;
  let mut is_first_content_line = true;

  for line in content.lines() {
    row += 1;
    let mut trimmed = line.trim().to_string();

    // Strip BOM from first content line (matching JS strip-bom behavior)
    if is_first_content_line && !trimmed.is_empty() {
      is_first_content_line = false;
      if trimmed.starts_with('\u{FEFF}') {
        trimmed = trimmed.trim_start_matches('\u{FEFF}').to_string();
      }
    }

    if trimmed.is_empty() {
      if let Some(mut sub) = current_subtitle.take() {
        let (plain, parts) = extract_text_parts(&sub.text);
        sub.text = plain;
        sub.text_parts = parts;
        subtitles.push(sub);
      }
      phase = Phase::Index;
      continue;
    }

    match phase {
      Phase::Index => {
        if let Ok(index) = trimmed.parse::<usize>() {
          current_subtitle = Some(Subtitle {
            index: Some(index),
            start: 0,
            end: 0,
            text: String::new(),
            settings: None,
            text_parts: Vec::new(),
            style: None,
            actor: None,
            is_comment: false,
          });
          phase = Phase::Timestamp;
        } else if trimmed.contains("-->") {
          if let Some((start_str, end_str)) = trimmed.split_once(" --> ") {
            let subtitle =
              Subtitle::new(parse_timestamp(start_str)?, parse_timestamp(end_str)?, "");
            phase = Phase::Text;
            current_subtitle = Some(subtitle);
          } else {
            return Err(anyhow!(
              "expected index or timestamp at row {row}, but received: \"{}\"",
              trimmed
            ));
          }
        }
      }
      Phase::Timestamp => {
        if let Some(sub) = &mut current_subtitle {
          if let Some((start_str, end_str)) = trimmed.split_once(" --> ") {
            sub.start = parse_timestamp(start_str)?;
            sub.end = parse_timestamp(end_str)?;
            phase = Phase::Text;
          } else {
            return Err(anyhow!(
              "expected timestamp at row {row}, but received: \"{}\"",
              trimmed
            ));
          }
        }
      }
      Phase::Text => {
        if let Some(sub) = &mut current_subtitle {
          if !sub.text.is_empty() {
            sub.text.push('\n');
          }
          sub.text.push_str(&trimmed);
        }
      }
    }
  }

  if let Some(mut sub) = current_subtitle {
    let (plain, parts) = extract_text_parts(&sub.text);
    sub.text = plain;
    sub.text_parts = parts;
    subtitles.push(sub);
  }

  Ok(subtitles)
}

pub async fn parse_file(path: impl AsRef<std::path::Path>) -> AnyResult<Vec<Subtitle>> {
  let text = tokio::fs::read_to_string(path).await?;
  parse(&text)
}

pub fn parse_bytes(data: &[u8]) -> AnyResult<Vec<Subtitle>> {
  let text = crate::encoding::decode_to_string(data)?;
  parse(&text)
}

#[cfg(feature = "http")]
pub async fn parse_url(url: &str) -> AnyResult<Vec<Subtitle>> {
  let response = reqwest::get(url).await?;
  let content = response.text().await?;
  parse(&content)
}

pub fn parse_content(content: &str) -> AnyResult<Vec<Subtitle>> {
  parse(content)
}

/// Streaming SRT parser. Processes content incrementally, yielding subtitles
/// one at a time without allocating a Vec. Useful for large files or streaming
/// input where you want to process subtitles as they arrive.
///
/// ```ignore
/// use subtitler::srt;
/// let content = "1\n00:00:01,000 --> 00:00:03,500\nHello\n\n";
/// for sub in srt::parse_stream(content) {
///     let sub = sub?;
///     println!("{:?}", sub);
/// }
/// ```
pub fn parse_stream<'a>(content: &'a str) -> SrtStream<'a> {
  SrtStream::new(content)
}

/// An iterator over SRT subtitles. Yields `Result<Subtitle>` for each cue.
#[derive(Debug)]
pub struct SrtStream<'a> {
  lines: std::str::Lines<'a>,
  phase: Phase,
  current_subtitle: Option<Subtitle>,
  row: usize,
}

impl<'a> SrtStream<'a> {
  fn new(content: &'a str) -> Self {
    SrtStream {
      lines: content.lines(),
      phase: Phase::Index,
      current_subtitle: None,
      row: 0,
    }
  }
}

impl<'a> Iterator for SrtStream<'a> {
  type Item = AnyResult<Subtitle>;

  fn next(&mut self) -> Option<Self::Item> {
    for line in self.lines.by_ref() {
      self.row += 1;
      let mut trimmed = line.trim().to_string();
      // Strip BOM (only on first content line)
      if self.row == 1 && !trimmed.is_empty() && trimmed.starts_with('\u{FEFF}') {
        trimmed = trimmed.trim_start_matches('\u{FEFF}').to_string();
      }

      if trimmed.is_empty() {
        if let Some(mut sub) = self.current_subtitle.take() {
          let (plain, parts) = extract_text_parts(&sub.text);
          sub.text = plain;
          sub.text_parts = parts;
          self.phase = Phase::Index;
          return Some(Ok(sub));
        }
        self.phase = Phase::Index;
        continue;
      }

      match self.phase {
        Phase::Index => {
          if let Ok(index) = trimmed.parse::<usize>() {
            self.current_subtitle = Some(Subtitle {
              index: Some(index),
              start: 0,
              end: 0,
              text: String::new(),
              settings: None,
              text_parts: Vec::new(),
              style: None,
              actor: None,
              is_comment: false,
            });
            self.phase = Phase::Timestamp;
          } else if trimmed.contains("-->")
            && let Some((start_str, end_str)) = trimmed.split_once(" --> ")
          {
            match (parse_timestamp(start_str), parse_timestamp(end_str)) {
              (Ok(s), Ok(e)) => {
                self.current_subtitle = Some(Subtitle::new(s, e, ""));
                self.phase = Phase::Text;
              }
              (Err(e), _) | (_, Err(e)) => return Some(Err(e)),
            }
          }
        }
        Phase::Timestamp => {
          if let Some(sub) = &mut self.current_subtitle
            && let Some((start_str, end_str)) = trimmed.split_once(" --> ")
            && let (Ok(s), Ok(e)) = (parse_timestamp(start_str), parse_timestamp(end_str))
          {
            sub.start = s;
            sub.end = e;
            self.phase = Phase::Text;
          }
        }
        Phase::Text => {
          if let Some(sub) = &mut self.current_subtitle {
            if !sub.text.is_empty() {
              sub.text.push('\n');
            }
            sub.text.push_str(&trimmed);
          }
        }
      }
    }

    // Flush trailing subtitle
    if let Some(mut sub) = self.current_subtitle.take() {
      let (plain, parts) = extract_text_parts(&sub.text);
      sub.text = plain;
      sub.text_parts = parts;
      return Some(Ok(sub));
    }

    None
  }
}

pub fn detect_format(data: &[u8]) -> Option<crate::model::Format> {
  if let Some(text) = crate::encoding::try_decode_for_detection(data) {
    let trimmed = text.trim();
    if !trimmed.is_empty() {
      #[cfg(feature = "vtt")]
      if trimmed.starts_with("WEBVTT") {
        return Some(crate::model::Format::Vtt);
      }
      if RE_SRT_DETECT.is_match(trimmed) {
        return Some(crate::model::Format::Srt);
      }
    }
  }
  None
}

pub fn to_string(subtitles: &[Subtitle]) -> String {
  let mut content = String::new();
  for (i, subtitle) in subtitles.iter().enumerate() {
    let position = i + 1;
    content.push_str(&position.to_string());
    content.push('\n');
    let timestamp = format!(
      "{} --> {}",
      format_timestamp(subtitle.start, "srt"),
      format_timestamp(subtitle.end, "srt")
    );
    content.push_str(&timestamp);
    content.push('\n');
    content.push_str(&subtitle.text);
    if i != subtitles.len() - 1 {
      content.push('\n');
      content.push('\n');
    }
  }
  if !subtitles.is_empty() {
    content.push('\n');
  }
  content
}

pub async fn generate(
  subtitles: &[Subtitle],
  file_path: impl AsRef<std::path::Path>,
  policy: Option<crate::model::WritePolicy>,
) -> AnyResult<String> {
  let path = file_path.as_ref();
  let policy = policy.unwrap_or_default();

  if policy == crate::model::WritePolicy::RefuseIfExists && path.exists() {
    anyhow::bail!("Refusing to overwrite existing file: {}", path.display());
  }

  let mut open_opts = fs::OpenOptions::new();
  let mut dest = match policy {
    crate::model::WritePolicy::Append => open_opts.create(true).append(true).open(path).await,
    _ => {
      open_opts
        .create(true)
        .write(true)
        .truncate(true)
        .open(path)
        .await
    }
  }?;
  let content = to_string(subtitles);
  dest.write_all(content.as_bytes()).await?;
  dest.flush().await?;

  Ok(path.to_string_lossy().into_owned())
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::model::Subtitle;

  fn make_subtitle(index: usize, start: u64, end: u64, text: &str) -> Subtitle {
    Subtitle {
      index: Some(index),
      start,
      end,
      text: text.to_string(),
      settings: None,
      text_parts: Vec::new(),
      style: None,
      actor: None,
      is_comment: false,
    }
  }

  #[test]
  fn test_parse_basic_srt() {
    let content =
      "1\n00:00:01,000 --> 00:00:03,500\nHello!\n\n2\n00:00:04,000 --> 00:00:06,500\nWorld!\n\n";
    let result = parse_content(content).unwrap();
    assert_eq!(result.len(), 2);
    assert_eq!(result[0], make_subtitle(1, 1000, 3500, "Hello!"));
    assert_eq!(result[1], make_subtitle(2, 4000, 6500, "World!"));
  }

  #[test]
  fn test_parse_multiline_text() {
    let content = "1\n00:00:01,000 --> 00:00:03,500\nLine one\nLine two\n\n";
    let result = parse_content(content).unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].text, "Line one\nLine two");
  }

  #[test]
  fn test_parse_start_at_zero() {
    let content = "1\n00:00:00,000 --> 00:00:03,500\nFrom zero\n\n";
    let result = parse_content(content).unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].start, 0);
    assert_eq!(result[0].end, 3500);
    assert_eq!(result[0].text, "From zero");
  }

  #[test]
  fn test_parse_numeric_text_not_mistaken_for_index() {
    let content = "1\n00:00:01,000 --> 00:00:03,500\n42\n\n";
    let result = parse_content(content).unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].text, "42");
  }

  #[test]
  fn test_parse_no_trailing_blank_line() {
    let content = "1\n00:00:01,000 --> 00:00:03,500\nHello!";
    let result = parse_content(content).unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].text, "Hello!");
  }

  #[tokio::test]
  async fn test_round_trip() {
    let original =
      "1\n00:00:01,000 --> 00:00:03,500\nHello\n\n2\n00:00:04,000 --> 00:00:06,500\nWorld\n\n";
    let subtitles = parse_content(original).unwrap();
    let path = "test_round_trip_srt.srt";
    generate(&subtitles, path, None).await.unwrap();
    let parsed_back = parse_file(path).await.unwrap();
    let _ = std::fs::remove_file(path);
    assert_eq!(subtitles, parsed_back);
  }

  #[test]
  fn test_parse_missing_index() {
    let content = "00:00:01,000 --> 00:00:03,500\nNo index\n\n";
    let result = parse_content(content).unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].index, None);
    assert_eq!(result[0].start, 1000);
    assert_eq!(result[0].text, "No index");
  }

  #[test]
  fn test_parse_missing_timestamp_error() {
    let content = "1\nnot a timestamp\n";
    let result = parse_content(content);
    assert!(result.is_err());
  }

  #[test]
  fn test_parse_bold_tag() {
    let content = "1\n00:00:01,000 --> 00:00:03,500\n<b>Bold text</b>\n\n";
    let result = parse_content(content).unwrap();
    assert_eq!(result[0].text, "Bold text");
    assert_eq!(result[0].text_parts.len(), 1);
    assert!(result[0].text_parts[0].bold);
    assert_eq!(result[0].text_parts[0].text, "Bold text");
  }

  #[test]
  fn test_parse_italic_tag() {
    let content = "1\n00:00:01,000 --> 00:00:03,500\n<i>Italic</i> plain\n\n";
    let result = parse_content(content).unwrap();
    assert_eq!(result[0].text, "Italic plain");
    assert_eq!(result[0].text_parts.len(), 1);
    assert!(result[0].text_parts[0].italic);
    assert_eq!(result[0].text_parts[0].text, "Italic");
  }

  #[test]
  fn test_parse_underline_tag() {
    let content = "1\n00:00:01,000 --> 00:00:03,500\n<u>underline</u>\n\n";
    let result = parse_content(content).unwrap();
    assert!(result[0].text_parts[0].underline);
  }

  #[test]
  fn test_parse_font_color_tag() {
    let content = "1\n00:00:01,000 --> 00:00:03,500\n<font color=\"#ff0000\">red</font>\n\n";
    let result = parse_content(content).unwrap();
    assert_eq!(result[0].text_parts[0].color, Some("#ff0000".to_string()));
  }

  #[test]
  fn test_parse_bytes() {
    let data = b"1\n00:00:01,000 --> 00:00:03,500\nHello\n\n";
    let result = parse_bytes(data.as_ref()).unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].text, "Hello");
  }

  #[test]
  fn test_detect_format_srt() {
    let data = b"1\n00:00:01,000 --> 00:00:03,500\nHello\n\n";
    assert_eq!(detect_format(data), Some(crate::model::Format::Srt));
  }

  #[test]
  fn test_detect_format_vtt() {
    let data = b"WEBVTT\n\n1\n00:00:01.000 --> 00:00:03.500\nHello\n\n";
    assert_eq!(detect_format(data), Some(crate::model::Format::Vtt));
  }

  #[test]
  fn test_subtitle_shift() {
    let mut sub = Subtitle::new(1000, 5000, "test");
    sub.shift(500);
    assert_eq!(sub.start, 1500);
    assert_eq!(sub.end, 5500);
    sub.shift(-2000);
    assert_eq!(sub.start, 0); // clamped
    assert_eq!(sub.end, 3500);
  }

  #[test]
  fn test_subtitle_duration() {
    let sub = Subtitle::new(1000, 5000, "test");
    assert_eq!(sub.duration_ms(), 4000);
  }

  #[test]
  fn test_stream() {
    let content =
      "1\n00:00:01,000 --> 00:00:03,500\nHello!\n\n2\n00:00:04,000 --> 00:00:06,500\nWorld!\n\n";
    let results: Vec<_> = parse_stream(content)
      .collect::<Result<Vec<_>, _>>()
      .unwrap();
    assert_eq!(results.len(), 2);
    assert_eq!(results[0].start, 1000);
    assert_eq!(results[0].text, "Hello!");
    assert_eq!(results[1].text, "World!");
  }
}
