use crate::model::Subtitle;
use crate::types::AnyResult;
use crate::utils::parse_timestamp;
use tokio::fs::File;
use tokio::io::AsyncBufReadExt;
use tokio::io::BufReader;
pub async fn parse_file(file_path: &str) -> AnyResult<Vec<Subtitle>> {
  let file = File::open(file_path).await?;
  let reader = BufReader::new(file);
  let mut lines = reader.lines();

  let mut subtitles = Vec::new();
  let mut current_subtitle: Option<Subtitle> = None;

  while let Some(line) = lines.next_line().await? {
    // println!("{}", line);
    if line.trim().is_empty() {
      // If we encounter an empty line, finalize the current subtitle
      if let Some(sub) = current_subtitle.take() {
        subtitles.push(sub);
      }
      continue;
    }

    if let Ok(index) = line.parse::<usize>() {
      current_subtitle = Some(Subtitle::new(index, 0, 0, ""));
    } else if let Some(sub) = &mut current_subtitle {
      // Read timestamps
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
