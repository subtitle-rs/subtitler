//! Whisper AI JSON transcript format parser and generator.
//!
//! OpenAI's Whisper speech recognition outputs JSON transcripts in the format:
//! ```json
//! {"text": "Hello world", "segments": [{"start": 0.0, "end": 2.0, "text": "Hello world", ...}]}
//! ```
//!
//! This module parses that JSON into `SubtitleFile`, converting seconds
//! (f64) to milliseconds (u64 via ×1000 round).

use crate::model::{Format, Subtitle, SubtitleFile};
use crate::types::AnyResult;
use serde::Deserialize;

/// A single segment from a Whisper JSON transcript.
#[derive(Debug, Deserialize)]
struct WhisperSegment {
  start: f64,
  end: f64,
  text: String,
}

/// Top-level Whisper JSON transcript.
#[derive(Debug, Deserialize)]
struct WhisperTranscript {
  segments: Vec<WhisperSegment>,
}

/// Detect Whisper JSON format — looks for the `"segments"` key with
/// `"start"`/`"end"` float fields.
pub fn detect_format(data: &[u8]) -> Option<Format> {
  let text = crate::encoding::try_decode_for_detection(data)?;
  if text.contains("\"segments\"") && text.contains("\"start\"") && text.contains("\"end\"") {
    return Some(Format::Whisper);
  }
  None
}

/// Parse Whisper JSON content.
pub fn parse_content(content: &str) -> AnyResult<SubtitleFile> {
  let transcript: WhisperTranscript =
    serde_json::from_str(content).map_err(|e| anyhow::anyhow!("invalid Whisper JSON: {}", e))?;

  let subtitles: Vec<Subtitle> = transcript
    .segments
    .into_iter()
    .enumerate()
    .map(|(i, seg)| {
      let start_ms = (seg.start * 1000.0).round() as u64;
      let end_ms = (seg.end * 1000.0).round() as u64;
      Subtitle::new(start_ms, end_ms, &seg.text).with_index(i + 1)
    })
    .collect();

  Ok(SubtitleFile::Whisper(subtitles))
}

/// Parse Whisper JSON bytes — auto-detect encoding then parse.
pub fn parse_bytes(data: &[u8]) -> AnyResult<SubtitleFile> {
  let content = crate::encoding::decode_to_string(data)?;
  parse_content(&content)
}

/// Parse Whisper JSON from file.
#[cfg(not(target_arch = "wasm32"))]
pub async fn parse_file(path: impl AsRef<std::path::Path>) -> AnyResult<SubtitleFile> {
  let text = tokio::fs::read_to_string(path).await?;
  parse_content(&text)
}

/// Parse Whisper JSON from URL (requires `http` feature).
#[cfg(feature = "http")]
pub async fn parse_url(url: &str) -> AnyResult<SubtitleFile> {
  let response = reqwest::get(url).await?;
  let text = response.text().await?;
  parse_content(&text)
}

/// Serialize subtitles to Whisper JSON format.
pub fn to_string(subtitles: &[Subtitle]) -> String {
  let segments: Vec<serde_json::Value> = subtitles
    .iter()
    .map(|sub| {
      serde_json::json!({
        "id": sub.index.unwrap_or(0),
        "start": sub.start as f64 / 1000.0,
        "end": sub.end as f64 / 1000.0,
        "text": sub.text,
        "seek": 0,
        "tokens": [],
        "temperature": 0.0,
        "avg_logprob": -1.0,
        "compression_ratio": 1.0,
        "no_speech_prob": 0.0,
      })
    })
    .collect();

  let full_text: String = subtitles
    .iter()
    .map(|s| s.text.as_str())
    .collect::<Vec<_>>()
    .join(" ");

  serde_json::to_string_pretty(&serde_json::json!({
    "text": full_text,
    "segments": segments,
  }))
  .unwrap_or_else(|_| "{}".to_string())
}

/// Write subtitles to a file in Whisper JSON format.
#[cfg(not(target_arch = "wasm32"))]
pub async fn generate(
  subtitles: &[Subtitle],
  file_path: impl AsRef<std::path::Path>,
  policy: Option<crate::model::WritePolicy>,
) -> AnyResult<String> {
  let content = to_string(subtitles);
  let path = file_path.as_ref();
  crate::io::write_with_policy(path, content.as_bytes(), policy).await?;
  Ok(path.to_string_lossy().into_owned())
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::model::SubtitleFormat;

  #[test]
  fn test_detect_whisper() {
    let data = br#"{"text": "Hello world", "segments": [{"start": 0.0, "end": 2.0, "text": "Hello world"}]}"#;
    assert_eq!(detect_format(data), Some(Format::Whisper));
  }

  #[test]
  fn test_detect_whisper_not_srt() {
    assert_eq!(
      detect_format(b"1\n00:00:01,000 --> 00:00:03,500\nHello\n"),
      None
    );
  }

  #[test]
  fn test_parse_whisper() {
    let json = r#"{"text": "Hello world", "segments": [{"start": 0.0, "end": 2.0, "text": "Hello world"}, {"start": 3.0, "end": 5.0, "text": "Goodbye"}]}"#;
    let file = parse_content(json).unwrap();
    let subs = file.subtitles();
    assert_eq!(subs.len(), 2);
    assert_eq!(subs[0].start, 0);
    assert_eq!(subs[0].end, 2000);
    assert_eq!(subs[0].text, "Hello world");
    assert_eq!(subs[1].start, 3000);
  }

  #[test]
  fn test_round_trip() {
    let json =
      r#"{"text": "Hello world", "segments": [{"start": 1.5, "end": 3.7, "text": "Hello world"}]}"#;
    let file = parse_content(json).unwrap();
    let out = to_string(file.subtitles());
    let reparsed = parse_content(&out).unwrap();
    assert_eq!(reparsed.subtitles().len(), 1);
    assert_eq!(reparsed.subtitles()[0].text, "Hello world");
  }
}
