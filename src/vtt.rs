use crate::model::Subtitle;
use crate::types::AnyResult;
use crate::utils::parse_timestamps;
use std::io::Cursor;
use tokio::fs::File;
use tokio::io::{AsyncBufReadExt, BufReader};

#[cfg(feature = "http")]
use reqwest;

async fn parse<R>(reader: R) -> AnyResult<Vec<Subtitle>>
where
  R: AsyncBufReadExt + Unpin, // 添加 Unpin trait bounds
{
  let mut lines = reader.lines();
  let mut subtitles = Vec::new();
  let mut current_subtitle: Option<Subtitle> = None;

  while let Some(line) = lines.next_line().await? {
    // 检查是否为空行以结束当前字幕
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
        let timestamp = parse_timestamps(&line)?;
        sub.start = timestamp.start;
        sub.end = timestamp.end;
        sub.settings = timestamp.settings;
      } else {
        sub.text = line.to_string();
      }
    }
  }

  // 如果最后一个字幕存在，则添加到字幕向量中
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
