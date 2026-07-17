use crate::error::SubtitleError;
use crate::model::{Format, Subtitle, SubtitleFile, TextPart};
use crate::types::AnyResult;
use crate::utils::{format_timestamp, parse_timestamps};
use regex::Regex;
#[cfg(feature = "http")]
use reqwest;
use smallvec::SmallVec;
use std::sync::LazyLock;
#[cfg(not(target_arch = "wasm32"))]
use tokio::fs;
#[cfg(not(target_arch = "wasm32"))]
use tokio::io::AsyncWriteExt;

static RE_VTT_TAG: LazyLock<Regex> = LazyLock::new(|| {
  Regex::new(concat!(
    r"<v(?:\s+\w+)?>|</v>|",
    r"</?(?:b|i|u|c)(?:\.[^>]*)?>"
  ))
  .unwrap()
});

#[derive(Debug, PartialEq)]
enum Phase {
  Header,
  Cue,
  Timestamp,
  Text,
  VttComment,
}

fn extract_text_parts(text: &str) -> (String, SmallVec<[TextPart; 4]>) {
  // Pre-allocate capacity to avoid reallocations
  let mut parts = SmallVec::with_capacity(4); // Most subtitles have <4 styled parts
  let mut plain = String::with_capacity(text.len());
  let mut bold = false;
  let mut italic = false;
  let mut underline = false;
  let mut voice: Option<String> = None;
  let mut last_end = 0;

  let re = &RE_VTT_TAG;

  for caps in re.find_iter(text) {
    let tag = caps.as_str();
    let start = caps.start();
    let end = caps.end();

    if start > last_end {
      let segment = &text[last_end..start];
      if !segment.is_empty() {
        plain.push_str(segment);
        if bold || italic || underline || voice.is_some() {
          let mut part = TextPart::new(segment, bold, italic, underline);
          part.voice = voice.clone();
          parts.push(part);
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
        // Extract speaker name: <v Alice> → "Alice", <v> → "unknown"
        let inner = &tag[2..tag.len().saturating_sub(1)];
        let name = inner.split_whitespace().next().unwrap_or("unknown");
        voice = Some(name.to_string());
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
      let mut part = TextPart::new(segment, bold, italic, underline);
      part.voice = voice.clone();
      parts.push(part);
    }
  }

  if parts.is_empty() {
    plain = text.to_string();
  }

  (plain, parts)
}

fn parse(content: &str) -> Result<(Option<String>, Vec<Subtitle>), SubtitleError> {
  let estimated_subs = (content.len() / 200).max(16);
  let mut subtitles: Vec<Subtitle> = Vec::with_capacity(estimated_subs);
  let mut current_subtitle: Option<Subtitle> = None;
  let mut phase = Phase::Header;
  let mut header_lines: Vec<&str> = Vec::new();
  let mut header: Option<String> = None;

  for (row_idx, line) in content.lines().enumerate() {
    let row = row_idx + 1;
    let mut trimmed = line.trim();
    if row == 1 && trimmed.starts_with('\u{FEFF}') {
      trimmed = trimmed.trim_start_matches('\u{FEFF}');
    }

    if trimmed.is_empty() {
      if let Some(mut sub) = current_subtitle.take() {
        let (plain, parts) = extract_text_parts(&sub.text);
        sub.text = plain;
        sub.text_parts = parts;
        subtitles.push(sub);
      }
      if phase == Phase::Header && !header_lines.is_empty() {
        header = Some(header_lines.join("\n"));
        header_lines.clear();
      }
      phase = Phase::Cue;
      continue;
    }

    match phase {
      Phase::Header => {
        header_lines.push(trimmed);
      }
      Phase::VttComment => {
        // Inside a NOTE block — skip all content until blank line (handled above)
      }
      Phase::Cue => {
        if trimmed.starts_with("WEBVTT") {
        } else if trimmed.starts_with("NOTE") {
          phase = Phase::VttComment;
        } else if trimmed.contains("-->") {
          let timestamp = parse_timestamps(trimmed, Format::Vtt)?;
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
            let timestamp = parse_timestamps(trimmed, Format::Vtt)?;
            sub.start = timestamp.start;
            sub.end = timestamp.end;
            sub.settings = timestamp.settings;
            phase = Phase::Text;
          } else {
            return Err(SubtitleError::UnexpectedLine {
              format: Format::Vtt,
              row,
              expected: "timestamp",
              got: trimmed.to_string(),
            });
          }
        }
      }
      Phase::Text => {
        if let Some(sub) = &mut current_subtitle {
          if !sub.text.is_empty() {
            sub.text.push('\n');
          }
          sub.text.push_str(trimmed);
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

  Ok((header, subtitles))
}

#[cfg(not(target_arch = "wasm32"))]
pub async fn parse_file(path: impl AsRef<std::path::Path>) -> AnyResult<SubtitleFile> {
  let text = tokio::fs::read_to_string(path).await?;
  parse_content(&text)
}

pub fn parse_bytes(data: &[u8]) -> AnyResult<SubtitleFile> {
  let text = crate::encoding::decode_to_string(data)?;
  parse_content(&text)
}

pub fn parse_bytes_full(data: &[u8]) -> AnyResult<(Option<String>, Vec<Subtitle>)> {
  let text = crate::encoding::decode_to_string(data)?;
  Ok(parse(&text)?)
}

#[cfg(feature = "http")]
pub async fn parse_url(url: &str) -> AnyResult<SubtitleFile> {
  let response = reqwest::get(url).await?;
  let content = response.text().await?;
  parse_content(&content)
}

pub fn parse_content(content: &str) -> AnyResult<SubtitleFile> {
  let (header, subtitles) = parse(content)?;
  Ok(SubtitleFile::Vtt { header, subtitles })
}

pub fn parse_content_full(content: &str) -> AnyResult<(Option<String>, Vec<Subtitle>)> {
  Ok(parse(content)?)
}

pub fn detect_format(data: &[u8]) -> Option<crate::model::Format> {
  if let Some(text) = crate::encoding::try_decode_for_detection(data)
    && text.trim().starts_with("WEBVTT")
  {
    return Some(crate::model::Format::Vtt);
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

#[cfg(not(target_arch = "wasm32"))]
pub async fn generate(
  subtitles: &[Subtitle],
  file_path: impl AsRef<std::path::Path>,
  policy: Option<crate::model::WritePolicy>,
) -> AnyResult<String> {
  let path = file_path.as_ref();
  let policy = policy.unwrap_or_default();

  if policy == crate::model::WritePolicy::RefuseIfExists && path.exists() {
    return Err(
      SubtitleError::FileExists {
        path: path.to_path_buf(),
      }
      .into(),
    );
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
  let content = to_string(subtitles, None);
  dest.write_all(content.as_bytes()).await?;
  dest.flush().await?;

  Ok(path.to_string_lossy().into_owned())
}

/// Write subtitles to an async writer streamingly (no full-string allocation).
#[cfg(not(target_arch = "wasm32"))]
pub async fn write_stream<W: tokio::io::AsyncWrite + Unpin>(
  subtitles: &[Subtitle],
  header: Option<&str>,
  writer: &mut W,
) -> Result<(), SubtitleError> {
  // Write WEBVTT header
  match header {
    Some(h) => {
      writer.write_all(h.as_bytes()).await?;
      writer.write_all(b"\n\n").await?;
    }
    None => {
      writer.write_all(b"WEBVTT\n\n").await?;
    }
  }

  for (i, sub) in subtitles.iter().enumerate() {
    let index = sub.index.unwrap_or(i + 1);
    let start = format_timestamp(sub.start, "WebVTT");
    let end = format_timestamp(sub.end, "WebVTT");

    writer.write_all(format!("{}\n", index).as_bytes()).await?;
    writer
      .write_all(format!("{} --> {}\n", start, end).as_bytes())
      .await?;
    writer.write_all(sub.text.as_bytes()).await?;
    writer.write_all(b"\n\n").await?;
  }
  writer.flush().await?;
  Ok(())
}

/// Streaming VTT parser. Processes content incrementally, yielding subtitles
/// one at a time without allocating a Vec.
pub fn parse_stream<'a>(content: &'a str) -> VttStream<'a> {
  VttStream::new(content)
}

#[derive(Debug)]
pub struct VttStream<'a> {
  lines: std::str::Lines<'a>,
  phase: Phase,
  current_subtitle: Option<Subtitle>,
  row: usize,
  header_lines: Vec<&'a str>,
  in_note: bool,
}

impl<'a> VttStream<'a> {
  fn new(content: &'a str) -> Self {
    VttStream {
      lines: content.lines(),
      phase: Phase::Header,
      current_subtitle: None,
      row: 0,
      header_lines: Vec::new(),
      in_note: false,
    }
  }

  pub fn header(&self) -> Option<String> {
    if self.header_lines.is_empty() {
      None
    } else {
      Some(self.header_lines.join("\n"))
    }
  }
}

impl<'a> Iterator for VttStream<'a> {
  type Item = AnyResult<Subtitle>;

  fn next(&mut self) -> Option<Self::Item> {
    for line in self.lines.by_ref() {
      self.row += 1;
      let trimmed = line.trim();

      if self.row == 1 && !trimmed.is_empty() && trimmed.starts_with('\u{FEFF}') {
        continue;
      }

      if trimmed.is_empty() {
        if let Some(mut sub) = self.current_subtitle.take() {
          let (plain, parts) = extract_text_parts(&sub.text);
          sub.text = plain;
          sub.text_parts = parts;
          self.phase = Phase::Cue;
          return Some(Ok(sub));
        }
        if self.phase == Phase::Header && !self.header_lines.is_empty() {
          self.phase = Phase::Cue;
        }
        self.phase = Phase::Cue;
        self.in_note = false;
        continue;
      }

      match self.phase {
        Phase::Header => {
          self.header_lines.push(trimmed);
        }
        Phase::VttComment => {}
        Phase::Cue => {
          if trimmed.starts_with("WEBVTT") {
            self.phase = Phase::Cue;
          } else if trimmed.starts_with("NOTE") {
            self.phase = Phase::VttComment;
            self.in_note = true;
          } else if trimmed.contains("-->") {
            match parse_timestamps(trimmed, Format::Vtt) {
              Ok(timestamp) => {
                let mut subtitle = Subtitle::new(timestamp.start, timestamp.end, "");
                subtitle.settings = timestamp.settings;
                self.current_subtitle = Some(subtitle);
                self.phase = Phase::Text;
              }
              Err(e) => return Some(Err(e.into())),
            }
          } else {
            let index = trimmed.parse::<usize>().ok();
            let mut subtitle = Subtitle::new(0, 0, "");
            subtitle.index = index;
            self.current_subtitle = Some(subtitle);
            self.phase = Phase::Timestamp;
          }
        }
        Phase::Timestamp => {
          if let Some(sub) = &mut self.current_subtitle {
            if trimmed.contains("-->") {
              match parse_timestamps(trimmed, Format::Vtt) {
                Ok(timestamp) => {
                  sub.start = timestamp.start;
                  sub.end = timestamp.end;
                  sub.settings = timestamp.settings;
                  self.phase = Phase::Text;
                }
                Err(e) => return Some(Err(e.into())),
              }
            }
          }
        }
        Phase::Text => {
          if let Some(sub) = &mut self.current_subtitle {
            if !sub.text.is_empty() {
              sub.text.push('\n');
            }
            sub.text.push_str(trimmed);
          }
        }
      }
    }

    if let Some(mut sub) = self.current_subtitle.take() {
      let (plain, parts) = extract_text_parts(&sub.text);
      sub.text = plain;
      sub.text_parts = parts;
      return Some(Ok(sub));
    }

    None
  }
}

impl<'a> crate::model::StreamingParser for VttStream<'a> {}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::model::{Subtitle, SubtitleFormat};

  fn make_subtitle(index: Option<usize>, start: u64, end: u64, text: &str) -> Subtitle {
    Subtitle {
      index,
      start,
      end,
      text: text.to_string(),
      settings: None,
      text_parts: SmallVec::new(),
      style: None,
      actor: None,
      is_comment: false,
    }
  }

  #[test]
  fn test_parse_basic_vtt() {
    let content = "WEBVTT\n\n1\n00:00:01.000 --> 00:00:03.500\nHello!\n\n2\n00:00:04.000 --> 00:00:06.500\nWorld!\n\n";
    let result = parse_content(content).unwrap();
    assert_eq!(result.subtitles().len(), 2);
    assert_eq!(
      result.subtitles()[0],
      make_subtitle(Some(1), 1000, 3500, "Hello!")
    );
    assert_eq!(
      result.subtitles()[1],
      make_subtitle(Some(2), 4000, 6500, "World!")
    );
  }

  #[test]
  fn test_parse_multiline_text() {
    let content = "WEBVTT\n\n1\n00:00:01.000 --> 00:00:03.500\nLine one\nLine two\n\n";
    let result = parse_content(content).unwrap();
    assert_eq!(result.subtitles().len(), 1);
    assert_eq!(result.subtitles()[0].text, "Line one\nLine two");
  }

  #[test]
  fn test_parse_with_settings() {
    let content = "WEBVTT\n\n1\n00:00:01.000 --> 00:00:03.500 align:start\nHello!\n\n";
    let result = parse_content(content).unwrap();
    assert_eq!(result.subtitles().len(), 1);
    assert_eq!(
      result.subtitles()[0].settings,
      Some("align:start".to_string())
    );
  }

  #[test]
  fn test_parse_no_cue_id() {
    let content = "WEBVTT\n\n00:00:01.000 --> 00:00:03.500\nNo cue id\n\n";
    let result = parse_content(content).unwrap();
    assert_eq!(result.subtitles().len(), 1);
    assert_eq!(result.subtitles()[0].text, "No cue id");
    assert_eq!(result.subtitles()[0].index, None);
  }

  #[test]
  fn test_parse_start_at_zero() {
    let content = "WEBVTT\n\n1\n00:00:00.000 --> 00:00:03.500\nFrom zero\n\n";
    let result = parse_content(content).unwrap();
    assert_eq!(result.subtitles().len(), 1);
    assert_eq!(result.subtitles()[0].start, 0);
  }

  #[cfg(not(target_arch = "wasm32"))]
  #[tokio::test]
  async fn test_round_trip() {
    let original = "WEBVTT\n\n1\n00:00:01.000 --> 00:00:03.500\nHello\n\n2\n00:00:04.000 --> 00:00:06.500\nWorld\n\n";
    let subtitles = parse_content(original).unwrap();
    let path = "test_round_trip_vtt.vtt";
    generate(subtitles.subtitles(), path, None).await.unwrap();
    let parsed_back = parse_file(path).await.unwrap();
    let _ = std::fs::remove_file(path);
    assert_eq!(subtitles.subtitles(), parsed_back.subtitles());
  }

  #[test]
  fn test_parse_with_metadata_header() {
    let content =
      "WEBVTT\nKind: captions\nLanguage: en\n\n1\n00:00:01.000 --> 00:00:03.500\nHello\n\n";
    let result = parse_content(content).unwrap();
    assert_eq!(result.subtitles().len(), 1);
    assert_eq!(result.subtitles()[0].text, "Hello");
  }

  #[test]
  fn test_parse_missing_timestamp_error() {
    let content = "WEBVTT\n\n1\nnot a timestamp\n\n";
    let result = parse_content(content);
    assert!(result.is_err());
  }

  #[test]
  fn test_parse_bold_tag() {
    let content = "WEBVTT\n\n1\n00:00:01.000 --> 00:00:03.500\n<b>bold</b>\n\n";
    let result = parse_content(content).unwrap();
    assert!(result.subtitles()[0].text_parts[0].bold());
  }

  #[test]
  fn test_parse_voice_tag() {
    let content = "WEBVTT\n\n1\n00:00:01.000 --> 00:00:03.500\n<v Alice>Hello</v>\n\n";
    let result = parse_content(content).unwrap();
    assert_eq!(result.subtitles()[0].text, "Hello");
    assert_eq!(result.subtitles()[0].text_parts.len(), 1);
    assert!(result.subtitles()[0].text_parts[0].voice.is_some());
  }

  #[test]
  fn test_parse_bytes() {
    let data = b"WEBVTT\n\n1\n00:00:01.000 --> 00:00:03.500\nHello\n\n";
    let result = parse_bytes(data.as_ref()).unwrap();
    assert_eq!(result.subtitles().len(), 1);
    assert_eq!(result.subtitles()[0].text, "Hello");
  }

  #[test]
  fn test_detect_format() {
    let data = b"WEBVTT\n\n1\n00:00:01.000 --> 00:00:03.500\nHello\n\n";
    assert_eq!(detect_format(data), Some(crate::model::Format::Vtt));
  }

  #[test]
  fn test_parse_note_block() {
    // NOTE blocks must be skipped, and subtitles after them must still parse
    let content = "WEBVTT\n\nNOTE\nThis is a comment\nspanning multiple lines\n\n1\n00:00:01.000 --> 00:00:03.500\nAfter note\n\n";
    let result = parse_content(content).unwrap();
    assert_eq!(
      result.subtitles().len(),
      1,
      "subtitle after NOTE block was lost"
    );
    assert_eq!(result.subtitles()[0].text, "After note");
  }

  #[test]
  fn test_parse_voice_speaker_name() {
    let content = "WEBVTT\n\n1\n00:00:01.000 --> 00:00:03.500\n<v Alice>Hello</v>\n\n";
    let result = parse_content(content).unwrap();
    assert_eq!(
      result.subtitles()[0].text_parts[0].voice,
      Some("Alice".to_string())
    );
  }

  #[test]
  fn test_parse_bytes_full_preserves_header() {
    let data =
      b"WEBVTT\nKind: captions\nLanguage: en\n\n1\n00:00:01.000 --> 00:00:03.500\nHello\n\n";
    let (header, subs) = parse_bytes_full(data.as_ref()).unwrap();
    assert!(header.as_deref().unwrap().contains("Kind: captions"));
    assert_eq!(subs.len(), 1);
  }
}
