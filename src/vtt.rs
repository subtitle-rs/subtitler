use crate::model::{Subtitle, TextPart};
use crate::types::AnyResult;
use crate::utils::{format_timestamp, parse_timestamps};
use anyhow::anyhow;
use regex::Regex;
#[cfg(feature = "http")]
use reqwest;
use std::io::Cursor;
use tokio::fs::{self, File};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

#[derive(PartialEq)]
enum Phase {
  Header,
  Cue,
  Timestamp,
  Text,
  VttComment,
}

fn extract_text_parts(text: &str) -> (String, Vec<TextPart>) {
  let mut parts = Vec::new();
  let mut plain = String::new();
  let mut bold = false;
  let mut italic = false;
  let mut underline = false;
  let mut voice: Option<String> = None;
  let mut last_end = 0;

  let combined = format!(
    "{}{}",
    r"<v(?:\s+\w+)?>|</v>|", r"</?(?:b|i|u|c)(?:\.[^>]*)?>"
  );
  let re = Regex::new(&combined).unwrap();

  for caps in re.find_iter(text) {
    let tag = caps.as_str();
    let start = caps.start();
    let end = caps.end();

    if start > last_end {
      let segment = &text[last_end..start];
      if !segment.is_empty() {
        plain.push_str(segment);
        if bold || italic || underline || voice.is_some() {
          parts.push(TextPart {
            text: segment.to_string(),
            bold,
            italic,
            underline,
            color: None,
            voice: voice.clone(),
          });
        }
      }
    }

    match tag {
      "</v>" => voice = None,
      "</b>" | "</b.c1>" | "</b.c2>" => bold = false,
      "</i>" | "</i.c1>" | "</i.c2>" => italic = false,
      "</u>" | "</u.c1>" | "</u.c2>" => underline = false,
      "</c>" | "</c.c>" | "</c.c1>" | "</c.c2>" => {}
      _ if tag.starts_with("<v") => {
        voice = Some("v".to_string());
      }
      _ if tag.starts_with("<b") => bold = true,
      _ if tag.starts_with("<i") => italic = true,
      _ if tag.starts_with("<u") => underline = true,
      _ => {}
    }

    last_end = end;
  }

  if last_end < text.len() {
    let segment = &text[last_end..];
    plain.push_str(segment);
    if bold || italic || underline || voice.is_some() {
      parts.push(TextPart {
        text: segment.to_string(),
        bold,
        italic,
        underline,
        color: None,
        voice: voice.clone(),
      });
    }
  }

  if parts.is_empty() {
    plain = text.to_string();
  }

  (plain, parts)
}

async fn parse<R>(reader: R) -> AnyResult<(Option<String>, Vec<Subtitle>)>
where
  R: AsyncBufReadExt + Unpin,
{
  let mut lines = reader.lines();
  let mut subtitles = Vec::new();
  let mut current_subtitle: Option<Subtitle> = None;
  let mut phase = Phase::Header;
  let mut row: usize = 0;
  let mut is_first_content_line = true;
  let mut header_lines: Vec<String> = Vec::new();
  let mut header: Option<String> = None;

  while let Some(line) = lines.next_line().await? {
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
      if let Some(sub) = current_subtitle.take() {
        subtitles.push(sub);
      }
      if phase == Phase::Header && !header_lines.is_empty() {
        header = Some(header_lines.join("\n"));
        header_lines.clear();
      }
      phase = match phase {
        Phase::VttComment => Phase::VttComment,
        _ => Phase::Cue,
      };
      continue;
    }

    match phase {
      Phase::Header => {
        if trimmed.starts_with("WEBVTT") {
          header_lines.push(trimmed);
        } else {
          header_lines.push(trimmed);
        }
      }
      Phase::VttComment => {
        if trimmed.starts_with("NOTE") {
        } else if trimmed.contains("-->") {
          let timestamp = parse_timestamps(&trimmed)?;
          let mut subtitle = Subtitle::new(timestamp.start, timestamp.end, "");
          subtitle.settings = timestamp.settings;
          current_subtitle = Some(subtitle);
          phase = Phase::Text;
        } else {
          let index = trimmed.parse::<usize>().ok();
          let mut subtitle = Subtitle::new(0, 0, "");
          subtitle.index = index;
          current_subtitle = Some(subtitle);
          phase = Phase::Timestamp;
        }
      }
      Phase::Cue => {
        if trimmed.starts_with("WEBVTT") {
        } else if trimmed.starts_with("NOTE") {
          phase = Phase::VttComment;
        } else if trimmed.contains("-->") {
          let timestamp = parse_timestamps(&trimmed)?;
          let mut subtitle = Subtitle::new(timestamp.start, timestamp.end, "");
          subtitle.settings = timestamp.settings;
          current_subtitle = Some(subtitle);
          phase = Phase::Text;
        } else {
          let index = trimmed.parse::<usize>().ok();
          let mut subtitle = Subtitle::new(0, 0, "");
          subtitle.index = index;
          current_subtitle = Some(subtitle);
          phase = Phase::Timestamp;
        }
      }
      Phase::Timestamp => {
        if let Some(sub) = &mut current_subtitle {
          if trimmed.contains("-->") {
            let timestamp = parse_timestamps(&trimmed)?;
            sub.start = timestamp.start;
            sub.end = timestamp.end;
            sub.settings = timestamp.settings;
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

  // Finalize header if still in header phase and has content
  if header.is_none() && !header_lines.is_empty() {
    header = Some(header_lines.join("\n"));
  }

  for sub in &mut subtitles {
    let (plain, parts) = extract_text_parts(&sub.text);
    sub.text = plain;
    sub.text_parts = parts;
  }

  Ok((header, subtitles))
}

pub async fn parse_file(path: impl AsRef<std::path::Path>) -> AnyResult<Vec<Subtitle>> {
  let file = File::open(path).await?;
  let reader = BufReader::new(file);
  let (_, subtitles) = parse(reader).await?;
  Ok(subtitles)
}

pub async fn parse_bytes(data: &[u8]) -> AnyResult<Vec<Subtitle>> {
  let text = String::from_utf8(data.to_vec()).map_err(|e| anyhow!("Invalid UTF-8: {}", e))?;
  let cursor = Cursor::new(text);
  let reader = BufReader::new(cursor);
  let (_, subtitles) = parse(reader).await?;
  Ok(subtitles)
}

#[cfg(feature = "http")]
pub async fn parse_url(url: &str) -> AnyResult<Vec<Subtitle>> {
  let response = reqwest::get(url).await?;
  let content = response.text().await?;
  let cursor = Cursor::new(content);
  let reader = BufReader::new(cursor);
  let (_, subtitles) = parse(reader).await?;
  Ok(subtitles)
}

pub async fn parse_content(content: &str) -> AnyResult<Vec<Subtitle>> {
  let cursor = Cursor::new(content);
  let reader = BufReader::new(cursor);
  let (_, subtitles) = parse(reader).await?;
  Ok(subtitles)
}

pub async fn parse_content_full(content: &str) -> AnyResult<(Option<String>, Vec<Subtitle>)> {
  let cursor = Cursor::new(content);
  let reader = BufReader::new(cursor);
  parse(reader).await
}

pub fn detect_format(data: &[u8]) -> Option<crate::model::SubtitleFormat> {
  if let Ok(text) = String::from_utf8(data.to_vec())
    && text.trim().starts_with("WEBVTT")
  {
    return Some(crate::model::SubtitleFormat::Vtt);
  }
  None
}

pub fn to_string(subtitles: &[Subtitle], header: Option<&str>) -> String {
  let mut content = if let Some(h) = header {
    format!("{}\n\n", h)
  } else {
    String::from("WEBVTT\n\n")
  };
  for (i, subtitle) in subtitles.iter().enumerate() {
    let position = i + 1;
    content.push_str(&position.to_string());
    content.push('\n');
    let mut timestamp = format!(
      "{} --> {}",
      format_timestamp(subtitle.start, "WebVTT"),
      format_timestamp(subtitle.end, "WebVTT")
    );
    if let Some(ref settings) = subtitle.settings {
      timestamp = format!("{} {}", timestamp, settings);
    }
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
) -> AnyResult<String> {
  let path = file_path.as_ref();
  let mut dest = fs::OpenOptions::new()
    .create(true)
    .write(true)
    .truncate(true)
    .open(path)
    .await?;
  let content = to_string(subtitles, None);
  dest.write_all(content.as_bytes()).await?;
  dest.flush().await?;

  Ok(path.to_string_lossy().into_owned())
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::model::Subtitle;

  fn make_subtitle(index: Option<usize>, start: u64, end: u64, text: &str) -> Subtitle {
    Subtitle {
      index,
      start,
      end,
      text: text.to_string(),
      settings: None,
      text_parts: Vec::new(),
      style: None,
      actor: None,
      layer: None,
      margin_l: None,
      margin_r: None,
      margin_v: None,
      effect: None,
      is_comment: false,
    }
  }

  #[tokio::test]
  async fn test_parse_basic_vtt() {
    let content = "WEBVTT\n\n1\n00:00:01.000 --> 00:00:03.500\nHello!\n\n2\n00:00:04.000 --> 00:00:06.500\nWorld!\n\n";
    let result = parse_content(content).await.unwrap();
    assert_eq!(result.len(), 2);
    assert_eq!(result[0], make_subtitle(Some(1), 1000, 3500, "Hello!"));
    assert_eq!(result[1], make_subtitle(Some(2), 4000, 6500, "World!"));
  }

  #[tokio::test]
  async fn test_parse_multiline_text() {
    let content = "WEBVTT\n\n1\n00:00:01.000 --> 00:00:03.500\nLine one\nLine two\n\n";
    let result = parse_content(content).await.unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].text, "Line one\nLine two");
  }

  #[tokio::test]
  async fn test_parse_with_settings() {
    let content = "WEBVTT\n\n1\n00:00:01.000 --> 00:00:03.500 align:start\nHello!\n\n";
    let result = parse_content(content).await.unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].settings, Some("align:start".to_string()));
  }

  #[tokio::test]
  async fn test_parse_no_cue_id() {
    let content = "WEBVTT\n\n00:00:01.000 --> 00:00:03.500\nNo cue id\n\n";
    let result = parse_content(content).await.unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].text, "No cue id");
    assert_eq!(result[0].index, None);
  }

  #[tokio::test]
  async fn test_parse_start_at_zero() {
    let content = "WEBVTT\n\n1\n00:00:00.000 --> 00:00:03.500\nFrom zero\n\n";
    let result = parse_content(content).await.unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].start, 0);
  }

  #[tokio::test]
  async fn test_round_trip() {
    let original = "WEBVTT\n\n1\n00:00:01.000 --> 00:00:03.500\nHello\n\n2\n00:00:04.000 --> 00:00:06.500\nWorld\n\n";
    let subtitles = parse_content(original).await.unwrap();
    let path = "test_round_trip_vtt.vtt";
    generate(&subtitles, path).await.unwrap();
    let parsed_back = parse_file(path).await.unwrap();
    let _ = std::fs::remove_file(path);
    assert_eq!(subtitles, parsed_back);
  }

  #[tokio::test]
  async fn test_parse_with_metadata_header() {
    let content =
      "WEBVTT\nKind: captions\nLanguage: en\n\n1\n00:00:01.000 --> 00:00:03.500\nHello\n\n";
    let result = parse_content(content).await.unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].text, "Hello");
  }

  #[tokio::test]
  async fn test_parse_missing_timestamp_error() {
    let content = "WEBVTT\n\n1\nnot a timestamp\n\n";
    let result = parse_content(content).await;
    assert!(result.is_err());
  }

  #[tokio::test]
  async fn test_parse_bold_tag() {
    let content = "WEBVTT\n\n1\n00:00:01.000 --> 00:00:03.500\n<b>bold</b>\n\n";
    let result = parse_content(content).await.unwrap();
    assert!(result[0].text_parts[0].bold);
  }

  #[tokio::test]
  async fn test_parse_voice_tag() {
    let content = "WEBVTT\n\n1\n00:00:01.000 --> 00:00:03.500\n<v Alice>Hello</v>\n\n";
    let result = parse_content(content).await.unwrap();
    assert_eq!(result[0].text, "Hello");
    assert_eq!(result[0].text_parts.len(), 1);
    assert!(result[0].text_parts[0].voice.is_some());
  }

  #[tokio::test]
  async fn test_parse_bytes() {
    let data = b"WEBVTT\n\n1\n00:00:01.000 --> 00:00:03.500\nHello\n\n";
    let result = parse_bytes(data.as_ref()).await.unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].text, "Hello");
  }

  #[test]
  fn test_detect_format() {
    let data = b"WEBVTT\n\n1\n00:00:01.000 --> 00:00:03.500\nHello\n\n";
    assert_eq!(detect_format(data), Some(crate::model::SubtitleFormat::Vtt));
  }
}
