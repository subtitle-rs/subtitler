use crate::model::Subtitle;
use crate::types::AnyResult;
use crate::utils::{format_timestamp, parse_timestamp};
#[cfg(feature = "http")]
use reqwest;
use std::io::Cursor;
use tokio::fs::File;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::{fs, io::AsyncWriteExt};

enum Phase {
  Index,
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
  let mut phase = Phase::Index;

  while let Some(line) = lines.next_line().await? {
    let trimmed = line.trim().to_string();
    if trimmed.is_empty() {
      if let Some(sub) = current_subtitle.take() {
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
          });
          phase = Phase::Timestamp;
        }
      }
      Phase::Timestamp => {
        if let Some(sub) = &mut current_subtitle {
          if let Some((start_str, end_str)) = trimmed.split_once(" --> ") {
            sub.start = parse_timestamp(start_str)?;
            sub.end = parse_timestamp(end_str)?;
            phase = Phase::Text;
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

pub async fn parse_file(file_path: &str) -> AnyResult<Vec<Subtitle>> {
  let file = File::open(file_path).await?;
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

pub async fn generate(subtitles: &[Subtitle], file_path: &str) -> AnyResult<String> {
  let mut dest = fs::OpenOptions::new()
    .create(true)
    .write(true)
    .truncate(true)
    .open(file_path)
    .await?;
  let mut content = String::new();
  for (i, subtitle) in subtitles.iter().enumerate() {
    if let Some(index) = subtitle.index {
      content.push_str(&index.to_string());
      content.push('\n');
    }
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
  content.push('\n');
  dest.write_all(content.as_bytes()).await?;

  Ok(file_path.to_string())
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
    }
  }

  #[tokio::test]
  async fn test_parse_basic_srt() {
    let content =
      "1\n00:00:01,000 --> 00:00:03,500\nHello!\n\n2\n00:00:04,000 --> 00:00:06,500\nWorld!\n\n";
    let result = parse_content(content).await.unwrap();
    assert_eq!(result.len(), 2);
    assert_eq!(result[0], make_subtitle(1, 1000, 3500, "Hello!"));
    assert_eq!(result[1], make_subtitle(2, 4000, 6500, "World!"));
  }

  #[tokio::test]
  async fn test_parse_multiline_text() {
    let content = "1\n00:00:01,000 --> 00:00:03,500\nLine one\nLine two\n\n";
    let result = parse_content(content).await.unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].text, "Line one\nLine two");
  }

  #[tokio::test]
  async fn test_parse_start_at_zero() {
    let content = "1\n00:00:00,000 --> 00:00:03,500\nFrom zero\n\n";
    let result = parse_content(content).await.unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].start, 0);
    assert_eq!(result[0].end, 3500);
    assert_eq!(result[0].text, "From zero");
  }

  #[tokio::test]
  async fn test_parse_numeric_text_not_mistaken_for_index() {
    let content = "1\n00:00:01,000 --> 00:00:03,500\n42\n\n";
    let result = parse_content(content).await.unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].text, "42");
  }

  #[tokio::test]
  async fn test_parse_no_trailing_blank_line() {
    let content = "1\n00:00:01,000 --> 00:00:03,500\nHello!";
    let result = parse_content(content).await.unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].text, "Hello!");
  }

  #[tokio::test]
  async fn test_round_trip() {
    let original =
      "1\n00:00:01,000 --> 00:00:03,500\nHello\n\n2\n00:00:04,000 --> 00:00:06,500\nWorld\n\n";
    let subtitles = parse_content(original).await.unwrap();
    let path = "test_round_trip_srt.srt";
    generate(&subtitles, path).await.unwrap();
    let parsed_back = parse_file(path).await.unwrap();
    let _ = std::fs::remove_file(path);
    assert_eq!(subtitles, parsed_back);
  }
}
