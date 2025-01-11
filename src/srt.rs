use crate::model::Subtitle;
use crate::types::AnyResult;
use crate::utils::{format_timestamp, parse_timestamp};
#[cfg(feature = "http")]
use reqwest;
use std::io::Cursor;
use tokio::fs::File;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::{fs, io::AsyncWriteExt};

async fn parse<R>(reader: R) -> AnyResult<Vec<Subtitle>>
where
  R: AsyncBufReadExt + Unpin, // add Unpin trait bounds
{
  let mut lines = reader.lines();
  let mut subtitles = Vec::new();
  let mut current_subtitle: Option<Subtitle> = None;

  while let Some(line) = lines.next_line().await? {
    if line.trim().is_empty() {
      if let Some(sub) = current_subtitle.take() {
        subtitles.push(sub);
      }
      continue;
    }

    if let Ok(index) = line.parse::<usize>() {
      let mut subtitle = Subtitle::new(0, 0, "");
      subtitle.index = Some(index);
      current_subtitle = Some(subtitle);
    } else if let Some(sub) = &mut current_subtitle {
      // 读取时间戳
      if sub.start == 0 {
        let parts: Vec<&str> = line.split(" --> ").collect();
        if parts.len() == 2 {
          sub.start = parse_timestamp(parts[0])?;
          sub.end = parse_timestamp(parts[1])?;
        }
      } else {
        sub.text = line.to_string();
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
    .append(true)
    .open(&file_path)
    .await?;
  let mut content = String::new();
  for (i, subtitle) in subtitles.into_iter().enumerate() {
    let mut part = String::new();
    if let Some(index) = subtitle.index {
      part.push_str(index.to_string().as_str());
      part.push_str("\n");
    }
    let timestamp = format!(
      "{} --> {}",
      format_timestamp(subtitle.start, "srt"),
      format_timestamp(subtitle.end, "srt")
    );
    part.push_str(&timestamp);
    part.push_str("\n");
    part.push_str(&subtitle.text);
    if i != subtitles.len() - 1 {
      part.push_str("\n");
      part.push_str("\n");
    }
    content.push_str(&part);
  }
  dest.write_all(content.as_bytes()).await?;

  Ok(file_path.to_string())
}
