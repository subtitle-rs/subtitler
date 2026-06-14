use crate::model::Subtitle;
use crate::types::AnyResult;
use crate::utils::{format_timestamp, parse_timestamps};
use anyhow::anyhow;
#[cfg(feature = "http")]
use reqwest;
use std::io::Cursor;
use tokio::fs::{self, File};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

enum Phase {
  Header,
  Cue,
  Timestamp,
  Text,
}

async fn parse<R>(reader: R) -> AnyResult<Vec<Subtitle>>
where
  R: AsyncBufReadExt + Unpin,
{
  let mut lines = reader.lines();
  let mut subtitles = Vec::new();
  let mut current_subtitle: Option<Subtitle> = None;
  let mut phase = Phase::Header;

  while let Some(line) = lines.next_line().await? {
    let trimmed = line.trim().to_string();
    if trimmed.is_empty() {
      if let Some(sub) = current_subtitle.take() {
        subtitles.push(sub);
      }
      phase = Phase::Cue;
      continue;
    }

    match phase {
      Phase::Header => {
        if trimmed.starts_with("WEBVTT") {
          // header found, but remain in Phase::Header to skip metadata
          // until the blank line that ends the header block
        }
        // non-WEBVTT lines before the first blank line are metadata — ignored
      }
      Phase::Cue => {
        if trimmed.starts_with("WEBVTT") {
          // misplaced WEBVTT header — ignore it
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
            return Err(anyhow!("Expected timestamp, got: \"{}\"", trimmed));
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

  if let Some(sub) = current_subtitle {
    subtitles.push(sub);
  }

  Ok(subtitles)
}

pub async fn parse_file(path: impl AsRef<std::path::Path>) -> AnyResult<Vec<Subtitle>> {
  let file = File::open(path).await?;
  let reader = BufReader::new(file);
  parse(reader).await
}

#[cfg(feature = "http")]
pub async fn parse_url(url: &str) -> AnyResult<Vec<Subtitle>> {
  let response = reqwest::get(url).await?;
  let content = response.text().await?;
  let cursor = Cursor::new(content);
  let reader = BufReader::new(cursor);
  parse(reader).await
}

pub async fn parse_content(content: &str) -> AnyResult<Vec<Subtitle>> {
  let cursor = Cursor::new(content);
  let reader = BufReader::new(cursor);
  parse(reader).await
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
  let mut content = String::new();
  content.push_str("WEBVTT\n\n");
  for (i, subtitle) in subtitles.iter().enumerate() {
    if let Some(index) = subtitle.index {
      content.push_str(&index.to_string());
      content.push('\n');
    }
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
}
