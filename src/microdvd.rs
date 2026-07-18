use crate::error::SubtitleError;
use crate::model::{Format, Subtitle, SubtitleFile, frames_to_ms, ms_to_frames};
use crate::types::AnyResult;
use regex::Regex;
use std::sync::LazyLock;
#[cfg(not(target_arch = "wasm32"))]
use tokio::io::AsyncWriteExt;

static RE_MICRODVD: LazyLock<Regex> =
  LazyLock::new(|| Regex::new(r"\{(\d+)\}\{(\d+)\}(.*)").unwrap());

static RE_FPS_HEADER: LazyLock<Regex> = LazyLock::new(|| {
  Regex::new(r"\{(\d+)\}\{(\d+)\}(?:\s*\[(\d+(?:\.\d+)?)\])?\s*(\d+(?:\.\d+)?)").unwrap()
});

pub const DEFAULT_FPS: f64 = 23.976;

pub fn detect_format(data: &[u8]) -> Option<crate::model::Format> {
  if let Some(text) = crate::encoding::try_decode_for_detection(data)
    && text.contains('{')
    && text.contains('}')
    && RE_MICRODVD.is_match(&text)
  {
    return Some(crate::model::Format::MicroDvd);
  }
  None
}

pub fn parse_content(content: &str, fps: Option<f64>) -> Result<SubtitleFile, SubtitleError> {
  let fps = fps.unwrap_or(DEFAULT_FPS);
  let mut subtitles: Vec<Subtitle> = Vec::with_capacity((content.len() / 30).max(16));
  let mut saved_fps = fps;

  for line in content.lines() {
    let trimmed = line.trim();
    if trimmed.is_empty() {
      continue;
    }

    // Check for FPS declaration in a comment-like format or first line
    if subtitles.is_empty()
      && let Some(caps) = RE_FPS_HEADER.captures(trimmed)
    {
      let fps_str = caps
        .get(3)
        .map_or_else(|| caps[4].to_string(), |m| m.as_str().to_string());
      if let Ok(f) = fps_str.parse::<f64>() {
        saved_fps = f;
        continue;
      }
    }

    if let Some(caps) = RE_MICRODVD.captures(trimmed) {
      let start_frame: u64 = caps[1].parse().map_err(|_| SubtitleError::InvalidFrame {
        format: Format::MicroDvd,
        role: "start",
        value: caps[1].to_string(),
      })?;
      let end_frame: u64 = caps[2].parse().map_err(|_| SubtitleError::InvalidFrame {
        format: Format::MicroDvd,
        role: "end",
        value: caps[2].to_string(),
      })?;
      let text = caps[3].to_string().replace('|', "\n");

      let subtitle = Subtitle::new(
        frames_to_ms(start_frame, saved_fps),
        frames_to_ms(end_frame, saved_fps),
        &text,
      );
      subtitles.push(subtitle);
    }
  }

  Ok(SubtitleFile::MicroDvd {
    fps: saved_fps,
    subtitles,
  })
}

/// Decode bytes to UTF-8 then parse, returning a `SubtitleFile`.
pub fn parse_bytes(data: &[u8], fps: Option<f64>) -> AnyResult<SubtitleFile> {
  let text = crate::encoding::decode_to_string(data)?;
  Ok(parse_content(&text, fps)?)
}

/// Parse a MicroDVD file asynchronously.
#[cfg(not(target_arch = "wasm32"))]
pub async fn parse_file(
  path: impl AsRef<std::path::Path>,
  fps: Option<f64>,
) -> AnyResult<SubtitleFile> {
  let text = tokio::fs::read_to_string(path).await?;
  Ok(parse_content(&text, fps)?)
}

/// Parse a MicroDVD file from a URL (requires `http` feature).
#[cfg(feature = "http")]
pub async fn parse_url(url: &str, fps: Option<f64>) -> AnyResult<SubtitleFile> {
  let response = reqwest::get(url).await?;
  let content = response.text().await?;
  Ok(parse_content(&content, fps)?)
}

/// Write subtitles to a file in MicroDVD format.
///
/// `policy` controls overwrite behavior (None = default Overwrite).
/// Uses the default fps (`DEFAULT_FPS`); for a custom fps, call
/// `to_string` directly and write the result with `tokio::fs::write`.
#[cfg(not(target_arch = "wasm32"))]
pub async fn generate(
  subtitles: &[Subtitle],
  file_path: impl AsRef<std::path::Path>,
  policy: Option<crate::model::WritePolicy>,
) -> AnyResult<String> {
  let content = to_string(subtitles, None);
  let path = file_path.as_ref();
  crate::io::write_with_policy(path, content.as_bytes(), policy).await?;
  Ok(path.to_string_lossy().into_owned())
}

pub fn to_string(subtitles: &[Subtitle], fps: Option<f64>) -> String {
  let fps = fps.unwrap_or(DEFAULT_FPS);
  let mut buf = String::new();

  for sub in subtitles {
    let start_frame = ms_to_frames(sub.start, fps);
    let end_frame = ms_to_frames(sub.end, fps);
    let text = sub.text.replace('\n', "|");
    buf.push_str(&format!("{{{}}}{{{}}}{}\n", start_frame, end_frame, text));
  }

  buf
}

pub fn to_string_with_fps_header(subtitles: &[Subtitle], fps: f64) -> String {
  let mut buf = format!("{{1}}{{1}}{:.3}\n", fps);
  buf.push_str(&to_string(subtitles, Some(fps)));
  buf
}

/// Streaming parser entry point — yields subtitles one at a time
/// without allocating a full `Vec`. Uses the default fps (`DEFAULT_FPS`);
/// for a custom fps, construct `MicroDvdStream::new(content, Some(fps))` directly.
pub fn parse_stream<'a>(content: &'a str) -> MicroDvdStream<'a> {
  MicroDvdStream::new(content, None)
}

pub struct MicroDvdStream<'a> {
  lines: std::str::Lines<'a>,
  saved_fps: f64,
}
impl<'a> MicroDvdStream<'a> {
  pub fn new(content: &'a str, fps: Option<f64>) -> Self {
    let f = fps.unwrap_or(DEFAULT_FPS);
    MicroDvdStream {
      lines: content.lines(),
      saved_fps: f,
    }
  }
}
impl<'a> Iterator for MicroDvdStream<'a> {
  type Item = AnyResult<Subtitle>;
  fn next(&mut self) -> Option<Self::Item> {
    for line in self.lines.by_ref() {
      let trimmed = line.trim();
      if trimmed.is_empty() {
        continue;
      }
      if let Some(caps) = RE_FPS_HEADER.captures(trimmed) {
        if let Some(fps_str) = caps.get(3).or(caps.get(4)) {
          if let Ok(f) = fps_str.as_str().parse::<f64>() {
            self.saved_fps = f;
            continue;
          }
        }
      }
      if let Some(caps) = RE_MICRODVD.captures(trimmed) {
        if let (Ok(s), Ok(e)) = (caps[1].parse::<u64>(), caps[2].parse::<u64>()) {
          let text = caps[3].to_string().replace(
            '|', "
",
          );
          return Some(Ok(Subtitle::new(
            frames_to_ms(s, self.saved_fps),
            frames_to_ms(e, self.saved_fps),
            &text,
          )));
        }
      }
    }
    None
  }
}

impl<'a> crate::model::StreamingParser for MicroDvdStream<'a> {}

/// Write MicroDVD subtitles to an async writer streamingly.
#[cfg(not(target_arch = "wasm32"))]
pub async fn write_stream<W: tokio::io::AsyncWrite + Unpin>(
  subtitles: &[Subtitle],
  fps: Option<f64>,
  writer: &mut W,
) -> AnyResult<()> {
  let fps = fps.unwrap_or(23.976);

  for sub in subtitles {
    let start_frame = ms_to_frames(sub.start, fps);
    let end_frame = ms_to_frames(sub.end, fps);
    writer
      .write_all(format!("{{{}}}{{{}}}{}\n", start_frame, end_frame, sub.text).as_bytes())
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
    let content = "{25}{50}Hello|World\n{75}{100}Goodbye\n";
    let file = parse_content(content, Some(25.0)).unwrap();
    let subs = file.subtitles();
    assert_eq!(subs.len(), 2);
    assert_eq!(subs[0].start, 1000);
    assert_eq!(subs[0].end, 2000);
    assert_eq!(subs[0].text, "Hello\nWorld");
    assert_eq!(subs[1].text, "Goodbye");
  }

  #[test]
  fn test_parse_with_fps_header() {
    let content = "{1}{1}30.000\n{30}{60}Hello\n";
    let file = parse_content(content, None).unwrap();
    let subs = file.subtitles();
    assert_eq!(subs.len(), 1);
    assert_eq!(subs[0].start, 1000);
  }

  #[test]
  fn test_round_trip() {
    let content = "{25}{50}Hello|World\n{75}{100}Goodbye\n";
    let file = parse_content(content, Some(25.0)).unwrap();
    let output = to_string(file.subtitles(), Some(25.0));
    let reparsed = parse_content(&output, Some(25.0)).unwrap();
    assert_eq!(file.subtitles(), reparsed.subtitles());
  }

  #[test]
  fn test_default_fps() {
    let content = "{24}{48}Hello\n";
    let file = parse_content(content, None).unwrap();
    let subs = file.subtitles();
    // 24 frames @ 23.976 fps ≈ 1001ms
    assert!(subs[0].start >= 990 && subs[0].start <= 1010);
  }

  #[test]
  fn test_detect_format() {
    assert!(detect_format(b"{1}{25}test\n").is_some());
    assert!(detect_format(b"WEBVTT\n").is_none());
  }
}
