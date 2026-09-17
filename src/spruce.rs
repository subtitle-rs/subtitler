//! Spruce STL subtitle parser and generator.
//!
//! The text-based subtitle format of Spruce Technologies authoring tools
//! (DVD Maestro; later Apple DVD Studio Pro). Despite sharing the `.stl`
//! extension it is unrelated to the binary EBU STL — detection relies on
//! content signatures, and the CLI keeps `.stl` mapped to EBU STL
//! (`--from spruce` to force this format).
//!
//! File shape (see the InqScribe / ArchiveTeam format notes):
//! ```text
//! $FontName = Arial
//! $TapeOffset = FALSE
//!
//! // comment
//! 00:00:01:00,00:00:03:12, Hello | line two
//! ```
//! - `$`-prefixed lines are directives, `//` lines are comments.
//! - Data lines are `start , end , text` with `HH:MM:SS:FF` frame
//!   timecodes at the file's frame rate (the file does not declare it —
//!   `DEFAULT_FPS` is PAL 25, override via `parse_content`).
//! - `|` in the text marks a line break.
//!
//! Note: a bare data-line-only file (no directives, no comments) is not
//! auto-detected to keep false positives low; force with `--from spruce`.

use crate::model::convert::{frames_to_ms, ms_to_frames};
use crate::model::{Format, Subtitle, SubtitleFile};
use crate::types::AnyResult;

/// PAL/DVD-Maestro default; the file itself declares no frame rate.
pub const DEFAULT_FPS: f64 = 25.0;

/// Detect Spruce STL: at least one `start , end ,` data line plus a
/// `$`-directive or `//` comment line.
pub fn detect_format(data: &[u8]) -> Option<Format> {
  let text = crate::encoding::try_decode_for_detection(data)?;
  let mut directive_or_comment = false;
  let mut data_line = false;
  for line in text.lines() {
    let t = line.trim_start();
    if t.starts_with("//") || t.starts_with('$') {
      directive_or_comment = true;
    }
    if looks_like_data_line(line) {
      data_line = true;
    }
  }
  (data_line && directive_or_comment).then_some(Format::Spruce)
}

fn looks_like_timecode(s: &str) -> bool {
  let b = s.as_bytes();
  b.len() == 11
    && (b[2] == b':' && b[5] == b':' && (b[8] == b':' || b[8] == b';'))
    && b
      .iter()
      .enumerate()
      .all(|(i, &c)| [2usize, 5, 8].contains(&i) || c.is_ascii_digit())
}

fn looks_like_data_line(line: &str) -> bool {
  let mut parts = line.split(',');
  let a = parts.next().map(str::trim).unwrap_or("");
  let b = parts.next().map(str::trim).unwrap_or("");
  looks_like_timecode(a) && looks_like_timecode(b)
}

fn parse_timecode(tc: &str, fps: f64) -> Option<u64> {
  let mut parts = tc.trim().split([':', ';']);
  let h: u64 = parts.next()?.parse().ok()?;
  let m: u64 = parts.next()?.parse().ok()?;
  let s: u64 = parts.next()?.parse().ok()?;
  let f: u64 = parts.next()?.parse().ok()?;
  if parts.next().is_some() {
    return None;
  }
  let total_frames = ((h * 3600 + m * 60 + s) as f64 * fps).round() as u64 + f;
  Some(frames_to_ms(total_frames, fps))
}

fn parse_data_line(line: &str, fps: f64) -> Option<Subtitle> {
  let (tc_in, rest) = line.split_once(',')?;
  let (tc_out, text) = rest.split_once(',')?;
  let start = parse_timecode(tc_in, fps)?;
  let end = parse_timecode(tc_out, fps)?;
  // `|` marks a line break; spaces around it are layout artifacts.
  let text = text
    .split('|')
    .map(str::trim)
    .collect::<Vec<_>>()
    .join("\n");
  Some(Subtitle::new(start, end.max(start), text.trim()))
}

/// Parse Spruce STL content at `fps` (default [`DEFAULT_FPS`]).
/// `$`-directives and `//` comments are skipped; unknown lines are
/// ignored leniently.
pub fn parse_content(content: &str, fps: Option<f64>) -> AnyResult<SubtitleFile> {
  let fps = fps.unwrap_or(DEFAULT_FPS);
  let mut subtitles = Vec::new();
  for line in content.lines() {
    let line = line.trim();
    if line.is_empty() || line.starts_with("//") || line.starts_with('$') {
      continue;
    }
    if let Some(sub) = parse_data_line(line, fps) {
      subtitles.push(sub.with_index(subtitles.len() + 1));
    }
  }
  Ok(SubtitleFile::Spruce { fps, subtitles })
}

/// Parse Spruce STL bytes — auto-detect encoding then parse.
pub fn parse_bytes(data: &[u8], fps: Option<f64>) -> AnyResult<SubtitleFile> {
  let content = crate::encoding::decode_to_string(data)?;
  parse_content(&content, fps)
}

/// Parse Spruce STL from file.
#[cfg(not(target_arch = "wasm32"))]
pub async fn parse_file(
  path: impl AsRef<std::path::Path>,
  fps: Option<f64>,
) -> AnyResult<SubtitleFile> {
  let text = tokio::fs::read_to_string(path).await?;
  parse_content(&text, fps)
}

/// Serialize subtitles to Spruce STL at `fps` (default [`DEFAULT_FPS`]).
/// Emits a `$TapeOffset = FALSE` header and uses `|` for line breaks.
pub fn to_string(subtitles: &[Subtitle], fps: Option<f64>) -> String {
  let fps = fps.unwrap_or(DEFAULT_FPS);
  let mut out = String::from("$TapeOffset = FALSE\n\n");
  for sub in subtitles {
    let text = sub.text.replace('\n', "|");
    out.push_str(&format!(
      "{},{},{}\n",
      ms_to_timecode(sub.start, fps),
      ms_to_timecode(sub.end, fps),
      text
    ));
  }
  out
}

/// Write subtitles to a file in Spruce STL format.
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

fn ms_to_timecode(ms: u64, fps: f64) -> String {
  let frames = ms_to_frames(ms, fps);
  let nominal = (fps.round() as u64).max(1);
  let h = frames / (3600 * nominal);
  let m = (frames % (3600 * nominal)) / (60 * nominal);
  let s = (frames % (60 * nominal)) / nominal;
  let f = frames % nominal;
  format!("{h:02}:{m:02}:{s:02}:{f:02}")
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::model::SubtitleFormat;

  const SAMPLE: &str = "$FontName = Arial\n$TapeOffset = FALSE\n\n// subtitle text\n00:00:01:00,00:00:03:12, Hello World\n00:00:04:00,00:00:06:00, Second | line here\n";

  #[test]
  fn test_detect_spruce() {
    assert_eq!(detect_format(SAMPLE.as_bytes()), Some(Format::Spruce));
  }

  #[test]
  fn test_detect_rejects_other_formats() {
    assert_eq!(
      detect_format(b"1\n00:00:01,000 --> 00:00:03,500\nHello\n"),
      None
    );
    assert_eq!(detect_format(b"just some text, nothing else\n"), None);
  }

  #[test]
  fn test_parse_default_fps_25() {
    let file = parse_content(SAMPLE, None).unwrap();
    assert_eq!(file.format(), Format::Spruce);
    let subs = file.subtitles();
    assert_eq!(subs.len(), 2);
    // 00:00:01:00 @ 25 fps = frame 25 = 1000 ms.
    assert_eq!(subs[0].start, 1_000);
    // 00:00:03:12 = frame 87 = 3480 ms.
    assert_eq!(subs[0].end, 3_480);
    assert_eq!(subs[0].text, "Hello World");
    // | becomes a line break.
    assert_eq!(subs[1].text, "Second\nline here");
  }

  #[test]
  fn test_parse_custom_fps_2997() {
    let file = parse_content("00:00:01:00,00:00:02:00, Hi\n", Some(29.97)).unwrap();
    let subs = file.subtitles();
    // frame 30 @ 29.97 = 1001 ms.
    assert_eq!(subs[0].start, 1_001);
    assert_eq!(subs[0].end, 2_002);
  }

  #[test]
  fn test_text_with_commas_survives() {
    let file = parse_content("00:00:01:00,00:00:02:00, Hello, World\n", None).unwrap();
    let subs = file.subtitles();
    assert_eq!(subs[0].text, "Hello, World");
  }

  #[test]
  fn test_round_trip() {
    let file = parse_content(SAMPLE, None).unwrap();
    let out = to_string(file.subtitles(), Some(25.0));
    let reparsed = parse_content(&out, None).unwrap();
    let original = file.subtitles();
    let back = reparsed.subtitles();
    assert_eq!(back.len(), original.len());
    for (a, b) in original.iter().zip(back.iter()) {
      assert_eq!(a.start, b.start);
      assert_eq!(a.end, b.end);
      assert_eq!(a.text, b.text);
    }
    assert!(out.starts_with("$TapeOffset = FALSE"));
  }

  #[test]
  fn test_timecode_formatting() {
    assert_eq!(ms_to_timecode(1_000, 25.0), "00:00:01:00");
    assert_eq!(ms_to_timecode(3_480, 25.0), "00:00:03:12");
    assert_eq!(ms_to_timecode(0, 25.0), "00:00:00:00");
  }
}
